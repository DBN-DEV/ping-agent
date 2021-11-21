mod commander;
mod detectors;
mod grpc;
mod reporter;
mod structures;

use crate::commander::SuperCommander;
use detectors::{PingDetector, TcpPingDetector};
use futures::future;
use reporter::Reporter;
use std::env;
use std::process;
use tokio::sync::mpsc;
use tokio::task;
use tracing::error;

const LOG_LEVEL: tracing::Level = tracing::Level::INFO;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(LOG_LEVEL).init();

    let args: Vec<String> = env::args().collect();
    let controller_addr = format!("http://{}", &args[1]);
    let reporter_addr = format!("http://{}", &args[2]);
    let agent_id: u32 = args[3].parse().unwrap();

    let (ping_command_tx, ping_command_rx) = mpsc::channel(16);
    let (tcp_ping_command_tx, tcp_ping_command_rx) = mpsc::channel(16);
    let (ping_result_tx, ping_result_rx) = mpsc::channel(1024);
    let (tcp_ping_result_tx, tcp_ping_result_rx) = mpsc::channel(1024);

    let super_commander = SuperCommander::new(&controller_addr, agent_id);
    let super_commander = match super_commander {
        Ok(commander) => commander,
        Err(_) => {
            error!("Invalid controller url.");
            process::abort();
        }
    };
    let ping_detector = PingDetector::new();
    let tcp_ping_detector = TcpPingDetector::new();
    let reporter = Reporter::new(&reporter_addr, agent_id);
    let reporter = match reporter {
        Ok(reporter) => reporter,
        Err(_) => {
            error!("Invalid collector url");
            process::abort();
        }
    };

    let mut handlers = vec![];

    let c = super_commander.build_commander();
    handlers.push(task::spawn(c.forward_ping_command(ping_command_tx)));
    handlers.push(task::spawn(
        ping_detector.detect(ping_command_rx, ping_result_tx),
    ));
    let r = reporter.clone();
    handlers.push(task::spawn(r.report_ping_result(ping_result_rx)));

    let c = super_commander.build_commander();
    handlers.push(task::spawn(c.forward_tcp_ping_command(tcp_ping_command_tx)));
    handlers.push(task::spawn(
        tcp_ping_detector.detect(tcp_ping_command_rx, tcp_ping_result_tx),
    ));
    let r = reporter.clone();
    handlers.push(task::spawn(r.report_tcp_ping_result(tcp_ping_result_rx)));

    handlers.push(task::spawn(super_commander.register()));

    future::join_all(handlers).await;
}
