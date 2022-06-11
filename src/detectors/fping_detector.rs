use crate::detectors::pinger::Pinger;
use crate::structures::{FPingCommand, FPingResult};
use futures::future::join_all;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

type CommandRx = Receiver<Vec<FPingCommand>>;
type ResultTx = Sender<Vec<FPingResult>>;

pub struct FpingDetector {}

impl FpingDetector {
    pub async fn detect(mut command_rx: CommandRx, result_tx: ResultTx) {
        loop {
            let comm = command_rx.recv().await.unwrap();
            tokio::spawn(Self::detect_once(comm, result_tx.clone()));
        }
    }

    async fn detect_once(commands: Vec<FPingCommand>, result_tx: ResultTx) {
        let results = Vec::with_capacity(commands.len());
        let results = Arc::new(Mutex::new(results));
        let mut handlers = Vec::with_capacity(commands.len());

        for comm in commands {
            let results = results.clone();
            let h = tokio::spawn(async move {
                let pinger = Pinger::from_fping_command(&comm);
                let result = pinger.ping(1).await.unwrap();
                let mut results = results.lock().await;
                results.push(result);
            });
            handlers.push(h)
        }

        join_all(handlers).await;

        let results = results.lock().await;
        let results = results.iter().map(|x| x.into()).collect();
        result_tx.send(results).await.unwrap();
    }
}
