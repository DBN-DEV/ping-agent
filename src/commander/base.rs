use super::controller_grpc::controller_client::ControllerClient;
use super::controller_grpc::CommandRequest;
use crate::commander::controller_grpc::{CommandCheckSumResponse, CommandsResponse};
use crate::util::{PingCommand, TcpPingCommand};
use rand::{Rng, SeedableRng};
use std::net::IpAddr;
use std::process;
use std::result::Result::Err;
use tokio::sync::mpsc::Sender;
use tokio::time;
use tracing::{debug, error, info, warn};

const BASE_CONNECT_TO_CONTROLLER_RETRY_INTERVAL: u64 = 10;

type Client = ControllerClient<tonic::transport::Channel>;
type Result<T> = std::result::Result<T, tonic::Status>;

#[derive(Debug)]
pub struct Commander {
    polling_update_interval: time::Duration,
    agent_id: u32,
    controller_addr: String,
}

impl Commander {
    pub async fn new(
        controller_addr: String,
        agent_id: u32,
        polling_update_interval: time::Duration,
    ) -> Self {
        Self {
            controller_addr,
            agent_id,
            polling_update_interval,
        }
    }

    async fn keep_trying_connect_to_controller(controller_addr: String) -> Client {
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
            let client = ControllerClient::connect(controller_addr.clone()).await;
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
        let mut client =
            Self::keep_trying_connect_to_controller(self.controller_addr.clone()).await;
        let mut loss_connect = false;
        loop {
            time::sleep(self.polling_update_interval).await;
            if loss_connect {
                client =
                    Self::keep_trying_connect_to_controller(self.controller_addr.clone()).await;
                loss_connect = false;
            }

            debug!(
                "Start to poll command update, now check sum {}",
                now_check_sum
            );
            let check_sum_response = Self::get_command_check_sum(&mut client, self.agent_id).await;
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
            let commands_resp = Self::get_ping_commands(&mut client, self.agent_id).await;
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

    async fn get_command_check_sum(
        client: &mut Client,
        agent_id: u32,
    ) -> Result<CommandCheckSumResponse> {
        let request = CommandRequest { agent_id };
        let command_check_sum_response = client.get_command_check_sum(request).await?;
        Ok(command_check_sum_response.into_inner())
    }

    async fn get_ping_commands(client: &mut Client, agent_id: u32) -> Result<CommandsResponse> {
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
            let ip = command.ip.parse::<IpAddr>();
            let ip = if let Ok(ip) = ip {
                ip
            } else {
                warn!("parse ip:{} fail", command.ip);
                continue;
            };
            let timeout = time::Duration::from_millis(u64::from(command.timeout_ms));
            let interval = time::Duration::from_millis(u64::from(command.timeout_ms));
            let ping_command = PingCommand {
                ip,
                address: command.ip,
                interval,
                timeout,
            };
            ping_commands.push(ping_command)
        }
        let r = ping_command_tx.send(ping_commands).await;
        match r {
            Ok(_) => (),
            Err(e) => {
                error!("Send ping_commands to ping detector fail!, {}", e);
                process::exit(1);
            }
        }

        let mut tcp_ping_commands = Vec::with_capacity(commands_resp.tcp_ping_commands.len());
        for command in commands_resp.tcp_ping_commands {
            let timeout = time::Duration::from_millis(u64::from(command.timeout_ms));
            let interval = time::Duration::from_millis(u64::from(command.interval_ms));
            let tcp_ping_command = TcpPingCommand {
                target: command.target,
                timeout,
                interval,
            };
            tcp_ping_commands.push(tcp_ping_command);
        }
        let r = tcp_ping_command_tx.send(tcp_ping_commands).await;
        if let Err(e) = r {
            error!("Send tcp_ping_commands to tcp ping detector fail!, {}", e);
            process::abort();
        }
    }
}
