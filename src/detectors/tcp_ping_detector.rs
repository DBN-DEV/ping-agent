use tokio::time;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{Sender, Receiver, channel};
use crate::util::{TcpPingResult, TcpPingCommand, Result};

use tracing::{error, info};

const SMOOTH_MICROS: u64 = 1_000_000;


pub struct TcpPingDetector {
    exited_tx: Sender<()>,
    exited_rx: Receiver<()>,
    exit_signal_tx: broadcast::Sender<()>,
}

impl TcpPingDetector {
    pub fn new() -> Self {
        let (exited_tx, exited_rx) = channel(10);
        let (exit_signal_tx, _) = broadcast::channel(1);
        Self {
            exited_tx,
            exited_rx,
            exit_signal_tx,
        }
    }

    async fn tcp_ping(
        target: String,
        timeout: time::Duration,
        interval: time::Duration,
        mut exit_signal_rx: broadcast::Receiver<()>,
        result_tx: Sender<Result>,
        exited_tx: Sender<()>,
    ) {
        loop {
            time::sleep(interval).await;

            let conn = TcpStream::connect(target.clone());
            let send_at = time::Instant::now();
            let utc_send_at = chrono::Utc::now();
            let result = time::timeout(timeout, conn).await;
            let result = match result  {
                Ok(_) => TcpPingResult {
                    target: target.clone(),
                    is_timeout: false,
                    send_at: utc_send_at,
                    rtt: Some(send_at.elapsed())
                },
                Err(_) => TcpPingResult {
                    target: target.clone(),
                    is_timeout: true,
                    send_at: utc_send_at,
                    rtt: None
                }
            };
            let result = Result::TcpPingResult(result);

            result_tx.send(result).await.unwrap_or_else(|err| {
                error!("Send tcp ping result fail, {}", err);
                std::process::exit(1);
            });

            match exit_signal_rx.try_recv() {
                Ok(_) => {
                    exited_tx.send(()).await.unwrap_or_else(|err| {
                        error!("Exited tx send fail, {}", err);
                        std::process::exit(1)
                    });
                    return;
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    error!("Exit signal recv fail, closed");
                    std::process::exit(1)
                }
                Err(broadcast::error::TryRecvError::Lagged(_)) => {
                    error!("Exit signal recv fail, lagged");
                    std::process::exit(1)
                }
                Err(broadcast::error::TryRecvError::Empty) => (),
            }
        }
    }

    pub(crate) async fn start_loop(mut self, mut command_rx: Receiver<Vec<TcpPingCommand>>,
                                   result_tx: Sender<Result>) {
        let mut first_loop = true;
        let mut total = 0;
        loop {
            let commands = match command_rx.recv().await {
                None => {
                    error!("Command rx fail");
                    std::process::exit(1);
                }
                Some(c) => c,
            };
            if commands.is_empty() {
                info!("Command is empty, have noting to do");
                continue;
            }

            if first_loop {
                first_loop = false;
            } else {
                info!(
                    "Command change start to stop all tcp ping tasks. total {}",
                    total
                );
                if let Err(e) = self.exit_signal_tx.send(()) {
                    error!("Exit signal tx fail. error: {}", e);
                    std::process::exit(1);
                }
                let mut completed_num = 0;
                'inner: loop {
                    if self.exited_rx.recv().await.is_some() {
                        completed_num += 1;
                        if completed_num == total {
                            info!("All tasks was stop.");
                            break 'inner;
                        }
                    } else {
                        // tx 不可能失效，如果 tx 失效则直接退出进程
                        error!("Completed rx fail.");
                        std::process::exit(1);
                    }
                }
            }

            total = commands.len();
            info!("Start all ping tasks, total {}", total);
            let total_u64 = total as u64;
            let smooth_task_time = time::Duration::from_micros(SMOOTH_MICROS / total_u64);
            let mut smooth_task_ticker = time::interval(smooth_task_time);
            for command in &commands {
                smooth_task_ticker.tick().await;
                tokio::task::spawn(Self::tcp_ping(
                    command.target.clone(),
                    command.timeout,
                    command.interval,
                    self.exit_signal_tx.subscribe(),
                    result_tx.clone(),
                    self.exited_tx.clone(),
                ));
            }
            info!("All ping tasks was started, total {}", total)
        }
    }
}

