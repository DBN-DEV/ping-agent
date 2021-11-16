use crate::grpc::controller_grpc::controller_client::ControllerClient;
use crate::grpc::controller_grpc::{
    CommandReq, CommandType, PingCommandsResp, RegisterReq, TcpPingCommandResp, UpdateCommandResp,
};
use crate::structures::{PingCommand, TcpPingCommand};
use rand::{rngs::SmallRng, Rng, SeedableRng};
use std::convert::TryFrom;
use std::result::Result::Err;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::Sender;
use tokio::time;
use tonic::codec::Streaming;
use tonic::codegen::http::uri::InvalidUri;
use tonic::transport::{Channel, Uri};
use tonic::{Code, Status};
use tracing::{info, warn};

const RETRY_INTERVAL: u64 = 10;
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(10);

type Client = ControllerClient<Channel>;

#[derive(Debug)]
pub struct Commander {
    agent_id: u32,
    channel: Channel,
    update_tx: broadcast::Sender<UpdateCommandResp>,
}

impl Commander {
    pub fn new(controller_add: &str, agent_id: u32) -> Result<Self, InvalidUri> {
        let uri = Uri::from_str(controller_add)?;
        let endpoint = Channel::builder(uri);
        let channel = endpoint
            .http2_keep_alive_interval(KEEP_ALIVE_INTERVAL)
            .connect_lazy();
        let (update_tx, _) = broadcast::channel::<UpdateCommandResp>(16);

        Ok(Self {
            agent_id,
            channel,
            update_tx,
        })
    }

    async fn backoff() {
        let rand_num = SmallRng::from_entropy().gen_range(0..=5);
        let wait_sec = RETRY_INTERVAL + rand_num;
        info!("Wait {} sec retry", wait_sec);

        let wait = Duration::from_secs(wait_sec);
        time::sleep(wait).await;
    }

    fn build_command_req(&self, version: String) -> CommandReq {
        return CommandReq {
            agent_id: self.agent_id,
            version,
        };
    }

    pub(crate) async fn register(&self) {
        loop {
            info!("Start register");
            let mut client = Client::new(self.channel.clone());
            let req = RegisterReq {
                agent_id: self.agent_id,
            };
            let resp = client.register(req).await;

            match resp {
                Ok(s) => {
                    info!("Register success");
                    let stream = s.into_inner();

                    info!("Start forward update command");
                    if let Err(r) = self.forward_update_command(stream).await {
                        warn!("Forward task stop, err:{}", r.message());
                        warn!("Start re-registration process");
                    }
                }
                Err(e) => {
                    // must be err
                    warn!("Agent register fail err:{}", e.message());
                    Self::backoff().await;
                }
            }
        }
    }

    async fn forward_update_command(
        &self,
        mut stream: Streaming<UpdateCommandResp>,
    ) -> Result<(), Status> {
        loop {
            let update = stream.message().await?;
            if let Some(comm) = update {
                info!("Recv update command");
                self.update_tx.send(comm).expect("Send update command fail");
                info!("Forward update command success");
            } else {
                return Err(Status::new(Code::Internal, "Recv None update command"));
            }
        }
    }

    pub(crate) async fn forward_ping_command(&self, tx: Sender<Vec<PingCommand>>) {
        let mut rx = self.update_tx.subscribe();
        let mut client = Client::new(self.channel.clone());
        loop {
            let comm = match rx.recv().await {
                Ok(c) => c,
                Err(RecvError::Lagged(v)) => {
                    warn!("Recv ping command lagged skipped:{}", v);
                    continue;
                }
                Err(RecvError::Closed) => panic!("Recv ping command on closed channel"),
            };

            if comm.command_type != CommandType::Ping as i32 {
                continue;
            }
            info!("Recv ping command update");

            let req = self.build_command_req(comm.version);

            info!("Send get ping command req version:{}", req.version.clone());
            let resp = client.get_ping_command(req).await;
            match resp {
                Ok(resp) => {
                    let resp = resp.into_inner();
                    info!("Recv ping commands len:{}", resp.ping_commands.len());
                    let commands = Self::build_ping_commands(resp);
                    tx.send(commands)
                        .await
                        .expect_err("Send ping commands fail");
                }
                Err(e) => warn!("Get ping command fail, err:{}", e.message()),
            }
        }
    }

    fn build_ping_commands(resp: PingCommandsResp) -> Vec<PingCommand> {
        let mut v = Vec::with_capacity(resp.ping_commands.len());
        for comm in resp.ping_commands {
            let command = PingCommand::try_from(comm);
            if let Ok(command) = command {
                v.push(command);
            } else {
                warn!("Parse ip:{} fail skip this addr", comm.ip);
            }
        }

        v
    }

    pub(crate) async fn forward_tcp_ping_command(&self, tx: Sender<Vec<TcpPingCommand>>) {
        let mut rx = self.update_tx.subscribe();
        let mut client = Client::new(self.channel.clone());
        loop {
            let comm = match rx.recv().await {
                Ok(c) => c,
                Err(RecvError::Lagged(v)) => {
                    warn!("Recv tcp ping command lagged skipped:{}", v);
                    continue;
                }
                Err(RecvError::Closed) => panic!("Recv tcp ping command on closed channel"),
            };

            if comm.command_type != CommandType::TcpPing as i32 {
                continue;
            }
            info!("Recv tcp ping command update");

            let req = self.build_command_req(comm.version);

            info!("Send get tcp ping command req version:{}", req.version);
            let resp = client.get_tcp_ping_command(req).await;
            match resp {
                Ok(resp) => {
                    let resp = resp.into_inner();
                    info!(
                        "Recv tcp ping commands len:{}",
                        resp.tcp_ping_commands.len()
                    );
                    let commands = Self::build_tcp_ping_commands(resp);
                    tx.send(commands)
                        .await
                        .expect_err("Send tcp ping commands fail");
                }
                Err(e) => warn!("Get ping command fail, err:{}", e.message()),
            }
        }
    }

    fn build_tcp_ping_commands(resp: TcpPingCommandResp) -> Vec<TcpPingCommand> {
        let mut v = Vec::with_capacity(resp.tcp_ping_commands.len());
        for comm in resp.tcp_ping_commands {
            let command = TcpPingCommand::from(comm);
            v.push(command);
        }

        v
    }
}
