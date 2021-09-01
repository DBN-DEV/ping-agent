mod commander;
mod detectors;
mod grpc;
mod reporter;
mod structures;

use commander::Commander;
use detectors::{PingDetector, TcpPingDetector};
use futures::future;
use reporter::Reporter;
use std::env;
use std::time;
use tokio::sync::mpsc::channel;

const COMMAND_POLL_INTERVAL: time::Duration = time::Duration::from_secs(10);
const LOG_LEVEL: tracing::Level = tracing::Level::INFO;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(LOG_LEVEL).init();

    let args: Vec<String> = env::args().collect();
    let controller_addr = format!("http://{}", &args[1]);
    let reporter_addr = format!("http://{}", &args[2]);
    let agent_id: u32 = args[3].parse().unwrap();

    let (ping_command_tx, ping_command_rx) = channel(10);
    let (tcp_ping_command_tx, tcp_ping_command_rx) = channel(10);
    let (ping_result_tx, ping_result_rx) = channel(1024);

    let commander = Commander::new(controller_addr, agent_id, COMMAND_POLL_INTERVAL);
    let ping_detector = PingDetector::new();
    let tcp_ping_detector = TcpPingDetector::new();
    let reporter = Reporter::new(reporter_addr, agent_id);

    let mut handlers = vec![];
    handlers.push(tokio::task::spawn(async move {
        commander
            .start_loop(ping_command_tx, tcp_ping_command_tx)
            .await;
    }));

    let result_tx = ping_result_tx.clone();
    handlers.push(tokio::task::spawn(async move {
        ping_detector.start_loop(ping_command_rx, result_tx).await;
    }));
    handlers.push(tokio::task::spawn(async move {
        tcp_ping_detector
            .start_loop(tcp_ping_command_rx, ping_result_tx)
            .await;
    }));
    handlers.push(tokio::task::spawn(async move {
        reporter.start_loop(ping_result_rx).await;
    }));

    future::join_all(handlers).await;
}
