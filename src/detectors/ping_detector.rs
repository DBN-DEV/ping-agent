use super::ping_socket::{Domain, PingSocket};
use crate::structures::{DetectionResult, PingCommand, PingResult};
use std::net::{IpAddr, Ipv4Addr, SocketAddrV4};
use std::num::Wrapping;
use tokio::sync::broadcast;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time;
use tracing::{error, info};

const SMOOTH_MICROS: u64 = 1_000_000;
const PING_PACKET_LEN: usize = 64;

pub struct PingDetector {
    exited_tx: Sender<()>,
    exited_rx: Receiver<()>,
    exit_signal_tx: broadcast::Sender<()>,
}

impl PingDetector {
    pub fn new() -> Self {
        let (exited_tx, exited_rx) = channel(10);
        let (exit_signal_tx, _) = broadcast::channel(1);
        Self {
            exited_tx,
            exited_rx,
            exit_signal_tx,
        }
    }

    async fn ping_v4(
        address: String,
        ip: Ipv4Addr,
        timeout: time::Duration,
        interval: time::Duration,
        mut exit_signal_rx: broadcast::Receiver<()>,
        result_tx: Sender<DetectionResult>,
        exited_tx: Sender<()>,
    ) {
        let dst = SocketAddrV4::new(ip, 0).into();
        let mut seq = Wrapping(0_u16);
        let sock = PingSocket::new(&Domain::V4);
        let sock = match sock {
            Ok(s) => s,
            Err(e) => {
                error!("Get socket fail! {}", e);
                std::process::exit(1);
            }
        };
        loop {
            let sock = &sock;
            time::sleep(interval).await;
            seq += Wrapping(1);
            if seq == Wrapping(0) {
                seq += Wrapping(1);
            }
            match sock.send_request(seq.0, PING_PACKET_LEN, &dst).await {
                Ok(_) => (),
                Err(e) => {
                    error!("Socket send fail! {}", e);
                    exited_tx.send(()).await.unwrap_or_else(|err| {
                        error!("Exited tx send fail, {}", err);
                        std::process::exit(1)
                    });
                    return;
                }
            };

            let send_at = time::Instant::now();
            let utc_send_at = chrono::Utc::now();
            let result = time::timeout(timeout, sock.recv_reply(seq.0, PING_PACKET_LEN)).await;
            let address = address.clone();
            let result = match result {
                Ok(_) => PingResult {
                    address,
                    is_timeout: false,
                    send_at: utc_send_at,
                    rtt: Some(send_at.elapsed()),
                },
                Err(_) => PingResult {
                    address,
                    is_timeout: true,
                    send_at: utc_send_at,
                    rtt: None,
                },
            };
            let result = DetectionResult::PingResult(result);
            result_tx.send(result).await.unwrap_or_else(|err| {
                error!("Exited tx send fail, {}", err);
                std::process::exit(1)
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

    pub async fn start_loop(
        mut self,
        mut command_rx: Receiver<Vec<PingCommand>>,
        result_tx: Sender<DetectionResult>,
    ) {
        let mut total = 0;
        loop {
            let commands = match command_rx.recv().await {
                None => {
                    error!("Command rx fail");
                    std::process::exit(1);
                }
                Some(c) => c,
            };

            if total == 0{
                info!("No task need to stop");
            } else {
                info!(
                    "Command change start to stop all ping tasks. total {}",
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

            if commands.is_empty() {
                info!("Command is empty, have noting to do");
                continue;
            }

            total = commands.len();
            info!("Start all ping tasks, total {}", total);
            let total_u64 = total as u64;
            let smooth_task_time = time::Duration::from_micros(SMOOTH_MICROS / total_u64);
            let mut smooth_task_ticker = time::interval(smooth_task_time);
            for command in &commands {
                smooth_task_ticker.tick().await;
                let timeout = command.timeout;
                let interval = command.interval;
                match command.ip {
                    IpAddr::V4(ip) => {
                        tokio::task::spawn(Self::ping_v4(
                            command.address.clone(),
                            ip,
                            timeout,
                            interval,
                            self.exit_signal_tx.subscribe(),
                            result_tx.clone(),
                            self.exited_tx.clone(),
                        ));
                    }
                    IpAddr::V6(_) => unimplemented!(),
                }
            }
            info!("All ping tasks was started, total {}", total)
        }
    }
}
