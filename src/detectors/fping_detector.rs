use crate::detectors::pinger::Pinger;
use crate::structures::{FPingCommand, FPingResults};
use futures::future::join_all;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

type CommandRx = Receiver<FPingCommand>;
type ResultTx = Sender<FPingResults>;

const PING_PACKET_LEN: usize = 64;

pub struct FpingDetector {}

impl FpingDetector {
    pub(crate) async fn detect(mut command_rx: CommandRx, result_tx: ResultTx) {
        loop {
            let comm = command_rx.recv().await.unwrap();
            tokio::spawn(Self::detect_once(comm, result_tx.clone()));
        }
    }

    async fn detect_once(command: FPingCommand, result_tx: ResultTx) {
        let results = Vec::with_capacity(command.ips.len());
        let results = Arc::new(Mutex::new(results));
        let mut handlers = Vec::with_capacity(command.ips.len());

        for ip in command.ips {
            let results = results.clone();
            let h = tokio::spawn(async move {
                let pinger = Pinger::new(ip, command.timeout, PING_PACKET_LEN);
                let result = pinger.ping(1).await.unwrap();
                let mut results = results.lock().await;
                results.push(result);
            });
            handlers.push(h)
        }

        join_all(handlers).await;

        let results = results.lock().await;
        let results = results.iter().map(|x| x.into()).collect();
        let results = FPingResults {
            results,
            version: command.version,
        };
        result_tx.send(results).await.unwrap();
    }
}
