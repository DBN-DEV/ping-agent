use crate::structures::{TcpPingCommand, TcpPingResult};
use chrono::Utc;
use std::io;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::mpsc;
use tokio::task;
use tokio::time;

use tokio::time::MissedTickBehavior;
use tracing::{info, warn};

const SMOOTH_MICROS: u64 = 1_000_000;

type ExitedTx = mpsc::Sender<()>;
type ExitedRx = mpsc::Receiver<()>;
type ExitSignalTx = broadcast::Sender<()>;
type ExitSignalRx = broadcast::Receiver<()>;
type ResultTx = mpsc::Sender<TcpPingResult>;
type CommandRx = mpsc::Receiver<Vec<TcpPingCommand>>;

struct TcpPinger {
    target: String,
    timeout: Duration,
    interval: Duration,
}

impl TcpPinger {
    fn from_command(comm: TcpPingCommand) -> Self {
        Self {
            target: comm.target,
            timeout: comm.timeout,
            interval: comm.interval,
        }
    }

    async fn ping(&self) -> io::Result<TcpPingResult> {
        let conn = TcpStream::connect(self.target.clone());

        let utc_send_at = Utc::now();

        let result = time::timeout(self.timeout, conn).await;
        match result {
            Ok(Ok(_)) => {
                let utc_recv_at = Utc::now();
                let rtt = (utc_recv_at - utc_send_at).to_std().unwrap();
                Ok(TcpPingResult {
                    target: self.target.clone(),
                    is_timeout: false,
                    send_at: utc_send_at,
                    rtt: Some(rtt),
                })
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(TcpPingResult {
                target: self.target.clone(),
                is_timeout: true,
                send_at: utc_send_at,
                rtt: None,
            }),
        }
    }

    async fn loop_ping(&self, result_tx: ResultTx, mut rx: ExitSignalRx, tx: ExitedTx) {
        let mut interval = time::interval(self.interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;

            let result = self.ping().await;

            match result {
                Ok(r) => result_tx.send(r).await.expect("Send tcp ping result fail"),
                Err(e) => warn!("Tcp ping fail target:{}, err:{}", self.target, e),
            }

            match rx.try_recv() {
                Ok(_) => {
                    tx.send(()).await.expect("Exited tx send fail");
                    return;
                }
                Err(TryRecvError::Closed | TryRecvError::Lagged(_)) => {
                    panic!("Exit signal recv fail");
                }
                Err(broadcast::error::TryRecvError::Empty) => (),
            }
        }
    }
}

pub struct TcpPingDetector {
    exited_tx: ExitedTx,
    exited_rx: ExitedRx,
    exit_signal_tx: ExitSignalTx,
}

impl TcpPingDetector {
    pub fn new() -> Self {
        let (exited_tx, exited_rx) = mpsc::channel(10);
        let (exit_signal_tx, _) = broadcast::channel(1);
        Self {
            exited_tx,
            exited_rx,
            exit_signal_tx,
        }
    }

    async fn stop_all_tcp_ping_task(&mut self) {
        let total = self.exit_signal_tx.receiver_count();

        if total == 0 {
            info!("No tcp ping task need to be stop");
            return;
        }

        info!("Stop tcp ping task total:{}", total);
        self.exit_signal_tx
            .send(())
            .expect("Broadcast stop tcp ping fail");

        let mut completed_num = 0;

        loop {
            self.exited_rx
                .recv()
                .await
                .expect("Recv tcp ping exited fail");
            completed_num += 1;
            if completed_num == total {
                info!("All tcp ping tasks was stop.");
                return;
            }
        }
    }

    pub(crate) async fn detect(mut self, mut command_rx: CommandRx, result_tx: ResultTx) {
        loop {
            let commands = command_rx.recv().await.expect("Command rx fail");
            info!("Recv tcp ping commands");

            self.stop_all_tcp_ping_task().await;

            if commands.is_empty() {
                info!("Commands is empty, noting to do");
                continue;
            }

            let total = commands.len();

            info!("Start tcp ping tasks, total {}", commands.len());

            let smooth_task_time = time::Duration::from_micros(SMOOTH_MICROS / total as u64);
            let mut smooth_task_ticker = time::interval(smooth_task_time);
            for command in commands {
                smooth_task_ticker.tick().await;

                let result_tx = result_tx.clone();
                let exit_signal_rx = self.exit_signal_tx.subscribe();
                let exited_tx = self.exited_tx.clone();
                task::spawn(async move {
                    let pinger = TcpPinger::from_command(command);
                    pinger.loop_ping(result_tx, exit_signal_rx, exited_tx).await;
                });
            }

            info!("All ping tasks was started, total {}", total)
        }
    }
}
