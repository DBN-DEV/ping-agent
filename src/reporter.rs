use crate::grpc::collector_grpc::collector_client::CollectorClient;
use crate::grpc::collector_grpc::{PingReportReq, TcpPingReportReq};
use crate::structures::{PingResult, TcpPingResult};
use std::str::FromStr;
use tokio::sync::mpsc;
use tonic::codegen::http::uri::InvalidUri;
use tonic::transport::{Channel, Uri};
use tracing::warn;

type Client = CollectorClient<Channel>;
type PingResultRx = mpsc::Receiver<PingResult>;
type TcpPingResultRx = mpsc::Receiver<TcpPingResult>;

const BASE_CONNECT_RETRY_INTERVAL: u64 = 10;

pub struct Reporter {
    server_add: String,
    channel: Channel,
    agent_id: u32,
}

impl Reporter {
    pub fn new(server_add: &str, agent_id: u32) -> Result<Self, InvalidUri> {
        let uri = Uri::from_str(server_add)?;
        let channel = Channel::builder(uri).connect_lazy();
        Ok(Self {
            server_add: server_add.to_string(),
            channel,
            agent_id,
        })
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

    pub(crate) async fn report_ping_result(&self, rx: &mut PingResultRx) {
        let mut client = CollectorClient::new(self.channel.clone());
        loop {
            let r = rx.recv().await.expect("Recv Ping result fail");
            let req = self.build_ping_request(r);

            let result = client.ping_report(req).await;
            if let Err(e) = result {
                warn!("Send ping result fail, err:{}", e.message());
            }
        }
    }

    pub(crate) async fn report_tcp_ping_result(&self, rx: &mut TcpPingResultRx) {
        let mut client = CollectorClient::new(self.channel.clone());
        loop {
            let r = rx.recv().await.expect("Recv tcp ping result fail");
            let req = self.build_tcp_ping_request(r);

            let result = client.tcp_ping_report(req).await;
            if let Err(e) = result {
                warn!("Send tcp ping result fail, err:{}", e.message());
            }
        }
    }
}
