use crate::grpc::collector_grpc::collector_client::CollectorClient;
use crate::grpc::collector_grpc::{FPingReportReq, PingReportReq, TcpPingReportReq};
use crate::structures::{FPingResult, PingResult, TcpPingResult};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task;
use tokio::time;
use tokio::time::MissedTickBehavior;
use tonic::codegen::http::uri::InvalidUri;
use tonic::transport::{Channel, Uri};
use tracing::{info, warn};

const RETRY_INTERVAL: u64 = 10;
const BATCH_SIZE: usize = 1024;
const BATCH_INTERVAL: Duration = Duration::from_secs(1);

type PingResultRx = mpsc::Receiver<PingResult>;
type TcpPingResultRx = mpsc::Receiver<TcpPingResult>;
type FpingResultRx = mpsc::Receiver<Vec<FPingResult>>;
type FlushSignalTx = mpsc::Sender<()>;

#[derive(Clone)]
pub struct Reporter {
    channel: Channel,
    agent_id: u32,
}

impl Reporter {
    pub fn new(server_add: &str, agent_id: u32) -> Result<Self, InvalidUri> {
        let uri = Uri::from_str(server_add)?;
        let channel = Channel::builder(uri).connect_lazy();
        Ok(Self { channel, agent_id })
    }

    fn build_ping_request(&self, results: Vec<PingResult>) -> PingReportReq {
        let r = results.into_iter().map(|x| x.into()).collect();
        PingReportReq {
            agent_id: self.agent_id,
            results: r,
        }
    }

    fn build_tcp_ping_request(&self, results: Vec<TcpPingResult>) -> TcpPingReportReq {
        let r = results.into_iter().map(|x| x.into()).collect();
        TcpPingReportReq {
            agent_id: self.agent_id,
            results: r,
        }
    }

    fn build_fping_request(&self, results: Vec<FPingResult>) -> FPingReportReq {
        let r = results.into_iter().map(|x| x.into()).collect();
        FPingReportReq {
            results: r,
            agent_id: self.agent_id,
        }
    }

    async fn backoff() {
        let rand_num = SmallRng::from_entropy().gen_range(0..=5);
        let wait_sec = RETRY_INTERVAL + rand_num;
        info!("Wait {} sec retry", wait_sec);

        let wait = Duration::from_secs(wait_sec);
        time::sleep(wait).await;
    }

    fn start_timer(period: Duration, tx: FlushSignalTx) {
        let mut timer = time::interval(period);
        timer.set_missed_tick_behavior(MissedTickBehavior::Delay);

        task::spawn(async move {
            loop {
                timer.tick().await;
                tx.send(()).await.expect("Send flush buff signal fail");
            }
        });
    }

    pub(crate) async fn report_ping_result(self, mut rx: PingResultRx) {
        let mut client = CollectorClient::new(self.channel.clone());
        let (failed_tx, mut failed_rx) = mpsc::channel::<PingReportReq>(1);
        let (flush_buff_tx, mut flush_buff_rx) = mpsc::channel(1);
        let mut buff = Vec::with_capacity(BATCH_SIZE);

        Self::start_timer(BATCH_INTERVAL, flush_buff_tx.clone());

        loop {
            tokio::select! {
                biased;

                req = failed_rx.recv() => {
                    let req = req.expect("Recv failed ping req fail");
                    let result = client.ping_report(req.clone()).await;
                    if let Err(e) = result {
                        warn!("Send ping result fail, err:{}", e.message());
                        failed_tx.send(req).await.expect("Secv failed ping req fail");
                        Self::backoff().await;
                    }
                }
                s = flush_buff_rx.recv() => {
                    s.expect("Recv flush buff signal fail");
                    if buff.is_empty() {
                        continue
                    }

                    let req = self.build_ping_request(buff);
                    let result = client.ping_report(req.clone()).await;
                    if let Err(e) = result {
                        warn!("Send ping result fail, err:{}", e.message());
                        failed_tx.send(req).await.expect("Secv failed req fail");
                    }

                    buff = Vec::with_capacity(BATCH_SIZE);
                }
                r = rx.recv() => {
                    let r = r.expect("Recv ping result fail");
                    buff.push(r);
                    if buff.len() == BATCH_SIZE {
                        flush_buff_tx.send(()).await.expect("Send flush buff signal fail")
                    }
                }
            }
        }
    }

    pub(crate) async fn report_tcp_ping_result(self, mut rx: TcpPingResultRx) {
        let mut client = CollectorClient::new(self.channel.clone());
        let (flush_buff_tx, mut flush_buff_rx) = mpsc::channel(1);
        let (failed_tx, mut failed_rx) = mpsc::channel::<TcpPingReportReq>(1);

        Self::start_timer(BATCH_INTERVAL, flush_buff_tx.clone());
        let mut buff = Vec::with_capacity(BATCH_SIZE);

        loop {
            tokio::select! {
                biased;

                req = failed_rx.recv() => {
                    let req = req.expect("Recv failed tcp ping req fail");
                    let result = client.tcp_ping_report(req.clone()).await;
                    if let Err(e) = result {
                        warn!("Send ping result fail, err:{}", e.message());
                        failed_tx.send(req).await.expect("Secv failed tcp ping req fail");
                        Self::backoff().await;
                    }
                }
                s = flush_buff_rx.recv() => {
                    s.expect("Recv flush buff signal fail");
                    if buff.is_empty() {
                        continue
                    }
                    let req = self.build_tcp_ping_request(buff);
                    let result = client.tcp_ping_report(req.clone()).await;
                    if let Err(e) = result {
                        warn!("Send tcp ping result fail, err:{}", e.message());
                        failed_tx.send(req).await.expect("Secv failed req fail");
                    }

                    buff = Vec::with_capacity(BATCH_SIZE);
                }
                r = rx.recv() => {
                    let r = r.expect("Recv tcp ping result fail");
                    buff.push(r);
                    if buff.len() == BATCH_SIZE {
                        flush_buff_tx.send(()).await.expect("Send flush buff signal fail")
                    }
                }
            }
        }
    }

    pub(crate) async fn report_fping_result(self, mut rx: FpingResultRx) {
        let mut client = CollectorClient::new(self.channel.clone());
        let (failed_tx, mut failed_rx) = mpsc::channel::<FPingReportReq>(1);
        loop {
            tokio::select! {
                biased;

                req = failed_rx.recv() => {
                    let req = req.unwrap();
                    let result = client.fping_report(req.clone()).await;
                    if let Err(e) = result {
                        warn!("Send fping result fail, err:{}", e.message());
                        failed_tx.send(req).await.unwrap();
                        Self::backoff().await;
                    }
                }
                r = rx.recv() => {
                    let r = r.unwrap();
                    let req = self.build_fping_request(r);
                    let result = client.fping_report(req.clone()).await;
                    if let Err(e) = result {
                        warn!("Send fping result fail, err:{}", e.message());
                        failed_tx.send(req).await.unwrap();
                    }
                }
            }
        }
    }
}
