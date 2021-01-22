use crate::reporter::collector_grpc::collector_client::CollectorClient;
use crate::reporter::collector_grpc::PingResult as GrpcPingResult;
use crate::reporter::collector_grpc::SingleReportRequest;
use crate::util::PingResult;
use rand::{Rng, SeedableRng};
use std::time;
use tokio::sync::mpsc::Receiver;
use tonic::transport::Channel;
use tracing::{error, info, warn};

const BASE_CONNECT_RETRY_INTERVAL: u64 = 10;

pub struct Reporter {
    server_addr: String,
    agent_id: u32,
}

impl Reporter {
    pub fn new(server_addr: String, agent_id: u32) -> Self {
        Self {
            server_addr,
            agent_id,
        }
    }

    fn build_request(&self, result: PingResult) -> SingleReportRequest {
        let mut rtt_sec = 0f32;
        if let Some(rtt) = result.rtt {
            rtt_sec = rtt.as_secs_f32();
        }
        let grpc_result = GrpcPingResult {
            ip: result.address,
            is_timeout: result.is_timeout,
            rtt: rtt_sec,
            time: result.send_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        };

        SingleReportRequest {
            agent_id: self.agent_id,
            result: Some(grpc_result),
        }
    }

    async fn keep_trying_connect_to_server(
        server_addr: &str,
    ) -> CollectorClient<Channel> {
        loop {
            let client = CollectorClient::connect(String::from(server_addr)).await;
            match client {
                Ok(c) => {
                    info!("Connect to collector success.");
                    return c;
                }
                Err(_) => {
                    let mut rng = rand::rngs::SmallRng::from_entropy();
                    let rand_num = rng.gen_range(0..=5);
                    let wait = BASE_CONNECT_RETRY_INTERVAL + rand_num;
                    warn!("Connect to report error, wait {} secs retry", wait);
                    tokio::time::sleep(time::Duration::from_secs(wait)).await;
                }
            }
        }
    }

    pub async fn start_loop(&self, mut ping_result_rx: Receiver<PingResult>) {
        let mut client: Option<CollectorClient<Channel>> = None;
        loop {
            let result = ping_result_rx.recv().await;
            let result = match result {
                Some(r) => r,
                None => {
                    // 不可能发生tx被回收事件 如果发生了那直接退出进程
                    error!("All result tx was drop!");
                    std::process::exit(1);
                }
            };

            match client {
                Some(ref mut inner_client) => {
                    let request = self.build_request(result);
                    let result = inner_client.ping_single_report(request).await;
                    match result {
                        Ok(_) => continue,
                        Err(_) => client = None,
                    }
                }
                None => client = Some(Self::keep_trying_connect_to_server(&self.server_addr).await),
            };
        }
    }
}
