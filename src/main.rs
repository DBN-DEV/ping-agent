use futures::future;
use ping_agent::commander::SuperCommander;
use ping_agent::conf;
use ping_agent::detectors::FpingDetector;
use ping_agent::detectors::{PingDetector, TcpPingDetector};
use ping_agent::reporter::Reporter;
use std::process;
use tokio::sync::mpsc;
use tokio::task;
use tracing::error;

const LVL: tracing::Level = tracing::Level::INFO;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_max_level(LVL).init();

    let conf = conf::read_conf().await.unwrap_or_else(|e| {
        error!("{}", e);
        process::exit(exitcode::CONFIG);
    });

    let super_commander = SuperCommander::new(&conf.controller.url, conf.agent.id);
    let super_commander = match super_commander {
        Ok(commander) => commander,
        Err(e) => {
            error!("Parse controller url: {}", e);
            process::exit(exitcode::CONFIG);
        }
    };

    let reporter = Reporter::new(&conf.collector.url, conf.agent.id);
    let reporter = match reporter {
        Ok(reporter) => reporter,
        Err(e) => {
            error!("Parse collector url: {}", e);
            process::exit(exitcode::CONFIG);
        }
    };

    let mut handlers = vec![];

    // icmp ping pipe
    let (ping_command_tx, ping_command_rx) = mpsc::channel(16);
    let (ping_result_tx, ping_result_rx) = mpsc::channel(1024);
    let c = super_commander.build_commander();
    let ping_detector = PingDetector::new();
    handlers.push(task::spawn(c.forward_ping_command(ping_command_tx)));
    handlers.push(task::spawn(
        ping_detector.detect(ping_command_rx, ping_result_tx),
    ));
    let r = reporter.clone();
    handlers.push(task::spawn(r.report_ping_result(ping_result_rx)));

    // tcp ping pipe
    let (tcp_ping_command_tx, tcp_ping_command_rx) = mpsc::channel(16);
    let (tcp_ping_result_tx, tcp_ping_result_rx) = mpsc::channel(1024);
    let tcp_ping_detector = TcpPingDetector::new();
    let c = super_commander.build_commander();
    handlers.push(task::spawn(c.forward_tcp_ping_command(tcp_ping_command_tx)));
    handlers.push(task::spawn(
        tcp_ping_detector.detect(tcp_ping_command_rx, tcp_ping_result_tx),
    ));
    let r = reporter.clone();
    handlers.push(task::spawn(r.report_tcp_ping_result(tcp_ping_result_rx)));

    // fping pipe
    let (fping_command_tx, fping_command_rx) = mpsc::channel(16);
    let (fping_result_tx, fping_result_rx) = mpsc::channel(1024);
    let c = super_commander.build_commander();
    handlers.push(task::spawn(c.forward_fping_command(fping_command_tx)));
    handlers.push(task::spawn(FpingDetector::detect(
        fping_command_rx,
        fping_result_tx,
    )));
    let r = reporter.clone();
    handlers.push(task::spawn(r.report_fping_result(fping_result_rx)));

    handlers.push(task::spawn(super_commander.register()));

    future::join_all(handlers).await;
}
