use crate::grpc::controller_grpc::controller_client::ControllerClient;
use crate::grpc::controller_grpc::{CommandCheckSumResponse, CommandRequest, CommandsResponse};
use crate::structures::{PingCommand, TcpPingCommand};
use rand::{Rng, SeedableRng};
use std::convert::TryFrom;
use std::process;
use std::result::Result::Err;
use tokio::sync::mpsc::Sender;
use tokio::time;
use tokio::time::MissedTickBehavior;
use tracing::{debug, error, info, warn};

const BASE_CONNECT_TO_CONTROLLER_RETRY_INTERVAL: u64 = 10;

type Client = ControllerClient<tonic::transport::Channel>;
type Result<T> = std::result::Result<T, tonic::Status>;

#[derive(Debug)]
pub struct Commander {
    polling_update_interval: time::Duration,
    agent_id: u32,
    controller_add: String,
}

impl Commander {
    pub fn new(
        controller_add: String,
        agent_id: u32,
        polling_update_interval: time::Duration,
    ) -> Self {
        Self {
            controller_add,
            agent_id,
            polling_update_interval,
        }
    }

    async fn connect(controller_add: String) -> Client {
        let mut first_loop = true;
        loop {
            if first_loop {
                first_loop = false;
            } else {
                let mut rng = rand::rngs::SmallRng::from_entropy();
                let rand_num = rng.gen_range(0..=5);
                let wait_sec = BASE_CONNECT_TO_CONTROLLER_RETRY_INTERVAL + rand_num;
                let wait = time::Duration::from_secs(wait_sec);
                warn!("Wail {}s retry to connect to controller.", wait_sec);
                time::sleep(wait).await;
            }
            let client = ControllerClient::connect(controller_add.clone()).await;
            if let Ok(client) = client {
                info!("Connect to controller success.");
                return client;
            } else {
                info!("Connect to controller fail, wail next try.");
            }
        }
    }

    pub(crate) async fn start_loop(
        &self,
        ping_command_tx: Sender<Vec<PingCommand>>,
        tcp_ping_command_tx: Sender<Vec<TcpPingCommand>>,
    ) {
        let mut now_check_sum = String::new();
        let mut client = Self::connect(self.controller_add.clone()).await;
        let mut loss_connect = false;
        let mut interval = time::interval(self.polling_update_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            if loss_connect {
                client = Self::connect(self.controller_add.clone()).await;
                loss_connect = false;
            }

            debug!(
                "Start to poll command update, now check sum {}",
                now_check_sum
            );
            let check_sum_response = Self::pull_command_check_sum(&mut client, self.agent_id).await;
            let check_sum = match check_sum_response {
                Err(_) => {
                    warn!("Get command check sum fail.");
                    loss_connect = true;
                    continue;
                }
                Ok(resp) => resp,
            };
            if check_sum.check_sum == now_check_sum {
                debug!("Command check sum is same as now check sum.");
                continue;
            } else {
                info!("Check sum change.");
            }

            now_check_sum = check_sum.check_sum;

            info!("Start to get ping commands.");
            let commands_resp = Self::pull_ping_commands(&mut client, self.agent_id).await;
            let commands = match commands_resp {
                Err(_) => {
                    warn!("Get command fail, wait next poll.");
                    loss_connect = true;
                    now_check_sum = String::new();
                    continue;
                }
                Ok(c) => {
                    info!("Get command success.");
                    c
                }
            };
            Self::build_and_send_commands(
                ping_command_tx.clone(),
                tcp_ping_command_tx.clone(),
                commands,
            )
            .await;
        }
    }

    async fn pull_command_check_sum(
        client: &mut Client,
        agent_id: u32,
    ) -> Result<CommandCheckSumResponse> {
        let request = CommandRequest { agent_id };
        let command_check_sum_response = client.get_command_check_sum(request).await?;
        Ok(command_check_sum_response.into_inner())
    }

    async fn pull_ping_commands(client: &mut Client, agent_id: u32) -> Result<CommandsResponse> {
        let request = CommandRequest { agent_id };
        let ping_command_from_controller = client.get_ping_command(request).await?;
        Ok(ping_command_from_controller.into_inner())
    }

    async fn build_and_send_commands(
        ping_command_tx: Sender<Vec<PingCommand>>,
        tcp_ping_command_tx: Sender<Vec<TcpPingCommand>>,
        commands_resp: CommandsResponse,
    ) {
        let mut ping_commands = Vec::with_capacity(commands_resp.ping_commands.len());
        for command in commands_resp.ping_commands {
            let ip = command.ip.clone();
            let ping_command = PingCommand::try_from(command);
            if let Ok(ping_command) = ping_command {
                ping_commands.push(ping_command);
            } else {
                warn!("Parse ip:{} error, jump this add.", ip);
            }
        }
        let r = ping_command_tx.send(ping_commands).await;
        if let Err(e) = r {
            error!("Send ping_commands to ping detector fail!, {}", e);
            process::abort();
        }

        let mut tcp_ping_commands = Vec::with_capacity(commands_resp.tcp_ping_commands.len());
        for command in commands_resp.tcp_ping_commands {
            let tcp_ping_command = TcpPingCommand::from(command);
            tcp_ping_commands.push(tcp_ping_command);
        }
        let r = tcp_ping_command_tx.send(tcp_ping_commands).await;
        if let Err(e) = r {
            error!("Send tcp_ping_commands to tcp ping detector fail!, {}", e);
            process::abort();
        }
    }
}
