use super::ping_socket::{Domain, PingSocket};
use crate::structures::{PingCommand, PingResult};
use chrono::Utc;
use socket2::SockAddr;
use std::io;
use std::net::{IpAddr, SocketAddrV4, SocketAddrV6};
use std::num::Wrapping;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::mpsc;
use tokio::task;
use tokio::time;
use tokio::time::MissedTickBehavior;
use tracing::info;

const PING_PACKET_LEN: usize = 64;

type CommandRx = mpsc::Receiver<Vec<PingCommand>>;
type ResultTx = mpsc::Sender<PingResult>;
type ExitSignalRx = broadcast::Receiver<()>;
type ExitSignalTx = broadcast::Sender<()>;
type ExitedTx = mpsc::Sender<()>;
type ExitedRx = mpsc::Receiver<()>;

struct Pinger {
    sock: PingSocket,
    timeout: Duration,
    dst_addr: String,
    interval: Duration,
    dst: SockAddr,
    len: usize,
}

impl Pinger {
    fn from_command(comm: PingCommand) -> Self {
        let (dst, sock) = match comm.ip {
            IpAddr::V4(ip) => {
                let dst = SocketAddrV4::new(ip, 0);
                let sock = PingSocket::new(Domain::V4).expect("Get socket fail");

                (SockAddr::from(dst), sock)
            }
            IpAddr::V6(ip) => {
                let dst = SocketAddrV6::new(ip, 0, 0, 0);
                let sock = PingSocket::new(Domain::V6).expect("Get socket fail");

                (SockAddr::from(dst), sock)
            }
        };

        Self {
            sock,
            timeout: comm.timeout,
            dst_addr: comm.address,
            interval: comm.interval,
            dst,
            len: PING_PACKET_LEN,
        }
    }

    async fn loop_ping(&self, result_tx: ResultTx, mut rx: ExitSignalRx, tx: ExitedTx) {
        let mut seq = Wrapping(0_u16);

        let mut interval = time::interval(self.interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            seq += Wrapping(1);
            if seq == Wrapping(0) {
                seq += Wrapping(1);
            }

            let result = self.ping(seq.0).await.expect("Send/Recv ping fail");

            result_tx.send(result).await.expect("Send result fail");

            match rx.try_recv() {
                Ok(_) => {
                    tx.send(()).await.expect("Send exited signal fail");
                    return;
                }
                Err(TryRecvError::Closed) | Err(TryRecvError::Lagged(_)) => {
                    panic!("Recv exit signal fail");
                }
                Err(broadcast::error::TryRecvError::Empty) => (),
            }
        }
    }

    async fn ping(&self, seq: u16) -> io::Result<PingResult> {
        self.sock.send_request(seq, self.len, &self.dst).await?;

        let utc_send_at = Utc::now();
        let result = time::timeout(self.timeout, self.sock.recv_reply(seq, self.len)).await;

        match result {
            Ok(Ok(())) => {
                let utc_recv_at = Utc::now();
                let rtt = (utc_recv_at - utc_send_at).to_std().unwrap();
                Ok(PingResult {
                    address: self.dst_addr.clone(),
                    is_timeout: false,
                    send_at: utc_send_at,
                    rtt: Some(rtt),
                })
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(PingResult {
                address: self.dst_addr.clone(),
                is_timeout: true,
                send_at: utc_send_at,
                rtt: None,
            }),
        }
    }
}

pub struct PingDetector {
    exited_tx: ExitedTx,
    exited_rx: ExitedRx,
    exit_signal_tx: ExitSignalTx,
}

impl PingDetector {
    pub(crate) fn new() -> Self {
        let (exited_tx, exited_rx) = mpsc::channel(10);
        let (exit_signal_tx, _) = broadcast::channel(1);
        Self {
            exited_tx,
            exited_rx,
            exit_signal_tx,
        }
    }

    async fn stop_all_ping_task(&mut self) {
        let total = self.exit_signal_tx.receiver_count();
        if total == 0 {
            info!("No ping task need to be stop");
            return;
        }

        info!("Stop all ping tasks. total {}", total);
        self.exit_signal_tx
            .send(())
            .expect("Broadcast stop ping task fail");
        let mut completed_num = 0;
        loop {
            self.exited_rx.recv().await.expect("Recv ping exited fail");
            completed_num += 1;
            if completed_num == total {
                info!("All ping tasks was stop.");
                return;
            }
        }
    }

    pub(crate) async fn detect(mut self, mut command_rx: CommandRx, result_tx: ResultTx) {
        loop {
            let commands = command_rx.recv().await.expect("Command rx fail");
            info!("Recv ping commands");

            self.stop_all_ping_task().await;

            if commands.is_empty() {
                info!("Commands is empty, noting to do");
                continue;
            }

            let total = commands.len();

            info!("Start ping tasks, total {}", commands.len());

            for command in commands {
                let result_tx = result_tx.clone();
                let exit_signal_rx = self.exit_signal_tx.subscribe();
                let exited_tx = self.exited_tx.clone();
                task::spawn(async move {
                    let pinger = Pinger::from_command(command);
                    pinger.loop_ping(result_tx, exit_signal_rx, exited_tx).await;
                });
            }

            info!("All ping tasks was started, total {}", total)
        }
    }
}
