use super::pinger::Pinger;
use crate::structures::{PingCommand, PingResult};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::task;
use tracing::info;

type CommandRx = mpsc::Receiver<Vec<PingCommand>>;
type ResultTx = mpsc::Sender<PingResult>;
type ExitSignalTx = broadcast::Sender<()>;
type ExitedTx = mpsc::Sender<()>;
type ExitedRx = mpsc::Receiver<()>;

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

        info!("Stop ping tasks. total {}", total);
        self.exit_signal_tx
            .send(())
            .expect("Broadcast stop ping task signal fail");
        let mut completed_num = 0;
        loop {
            self.exited_rx.recv().await.expect("Recv ping exited fail");
            completed_num += 1;
            if completed_num == total {
                info!("All ping tasks have been stopped");
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
                    let pinger = Pinger::from_ping_command(&command);
                    pinger
                        .loop_ping(command.interval, result_tx, exit_signal_rx, exited_tx)
                        .await;
                });
            }

            info!("All ping tasks was started, total {}", total)
        }
    }
}
