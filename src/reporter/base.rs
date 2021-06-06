use crate::grpc::collector_grpc::collector_client::CollectorClient;
use crate::grpc::collector_grpc::{PingReportRequest, TcpPingReportRequest};
use crate::structures::{DetectionResult, PingResult, TcpPingResult};
use rand::{Rng, SeedableRng};
use std::time;
use tokio::sync::mpsc::Receiver;
use tracing::{error, info, warn};

type Client = CollectorClient<tonic::transport::Channel>;

const BASE_CONNECT_RETRY_INTERVAL: u64 = 10;

pub struct Reporter {
    server_add: String,
    agent_id: u32,
}

impl Reporter {
    pub fn new(server_add: String, agent_id: u32) -> Self {
        Self {
            server_add,
            agent_id,
        }
    }

    fn build_ping_request(&self, result: PingResult) -> PingReportRequest {
        let result = result.into();
        PingReportRequest {
            agent_id: self.agent_id,
            result: Some(result),
        }
    }

    fn build_tcp_ping_request(&self, result: TcpPingResult) -> TcpPingReportRequest {
        let result = result.into();
        TcpPingReportRequest {
            agent_id: self.agent_id,
            result: Some(result),
        }
    }

    async fn connect(server_add: &str) -> Client {
        loop {
            let client = CollectorClient::connect(String::from(server_add)).await;
            if let Ok(c) = client {
                info!("Connect to collector success.");
                return c;
            } else {
                let mut rng = rand::rngs::SmallRng::from_entropy();
                let rand_num = rng.gen_range(0..=5);
                let wait = BASE_CONNECT_RETRY_INTERVAL + rand_num;
                warn!("Connect to report error, wait {} secs retry", wait);
                tokio::time::sleep(time::Duration::from_secs(wait)).await;
            }
        }
    }

    pub async fn start_loop(&self, mut ping_result_rx: Receiver<DetectionResult>) {
        let mut client: Option<Client> = None;
        loop {
            let result = ping_result_rx.recv().await;
            let result = if let Some(r) = result {
                r
            } else {
                // 不可能发生tx被回收事件 如果发生了那直接退出进程
                error!("All result tx was drop!");
                std::process::abort();
            };

            if let Some(ref mut inner_client) = client {
                match result {
                    DetectionResult::PingResult(r) => {
                        let request = self.build_ping_request(r);
                        let result = inner_client.ping_report(request).await;
                        if let Err(e) = result {
                            warn!("Send ping result fail, {:?}", e);
                            client = None
                        } else {
                            continue;
                        }
                    }
                    DetectionResult::TcpPingResult(r) => {
                        let request = self.build_tcp_ping_request(r);
                        let result = inner_client.tcp_ping_report(request).await;
                        if let Err(e) = result {
                            warn!("Send tcp ping result fail, {:?}", e);
                            client = None
                        } else {
                            continue;
                        }
                    }
                }
            } else {
                client = Some(Self::connect(&self.server_add).await);
            };
        }
    }
}
