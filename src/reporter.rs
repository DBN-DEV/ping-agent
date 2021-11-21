use crate::grpc::collector_grpc::collector_client::CollectorClient;
use crate::grpc::collector_grpc::{PingReportReq, TcpPingReportReq};
use crate::structures::{PingResult, TcpPingResult};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;
use tonic::codegen::http::uri::InvalidUri;
use tonic::transport::{Channel, Uri};
use tracing::{info, warn};

const RETRY_INTERVAL: u64 = 10;

type PingResultRx = mpsc::Receiver<PingResult>;
type TcpPingResultRx = mpsc::Receiver<TcpPingResult>;

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

    fn build_ping_request(&self, result: PingResult) -> PingReportReq {
        let result = result.into();
        PingReportReq {
            agent_id: self.agent_id,
            result: Some(result),
        }
    }

    fn build_tcp_ping_request(&self, result: TcpPingResult) -> TcpPingReportReq {
        let result = result.into();
        TcpPingReportReq {
            agent_id: self.agent_id,
            result: Some(result),
        }
    }

    async fn backoff() {
        let rand_num = SmallRng::from_entropy().gen_range(0..=5);
        let wait_sec = RETRY_INTERVAL + rand_num;
        info!("Wait {} sec retry", wait_sec);

        let wait = Duration::from_secs(wait_sec);
        time::sleep(wait).await;
    }

    pub(crate) async fn report_ping_result(self, mut rx: PingResultRx) {
        let mut client = CollectorClient::new(self.channel.clone());
        let (failed_tx, mut failed_rx) = mpsc::channel::<PingReportReq>(1);

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
                r = rx.recv() => {
                    let r = r.expect("Recv ping result fail");
                    let req = self.build_ping_request(r);
                    let result = client.ping_report(req.clone()).await;
                    if let Err(e) = result {
                        warn!("Send ping result fail, err:{}", e.message());
                        failed_tx.send(req).await.expect("Secv failed req fail");
                    }
                }
            }
        }
    }

    pub(crate) async fn report_tcp_ping_result(self, mut rx: TcpPingResultRx) {
        let mut client = CollectorClient::new(self.channel.clone());
        let (failed_tx, mut failed_rx) = mpsc::channel::<TcpPingReportReq>(1);
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
                r = rx.recv() => {
                    let r = r.expect("Recv tcp ping result fail");
                    let req = self.build_tcp_ping_request(r);
                    let result = client.tcp_ping_report(req.clone()).await;
                    if let Err(e) = result {
                        warn!("Send tcp ping result fail, err:{}", e.message());
                        failed_tx.send(req).await.expect("Secv failed tcp ping req fail");
                    }
                }
            }
        }
    }
}
