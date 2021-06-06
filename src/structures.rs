use crate::grpc::collector_grpc::{GrpcPingResult, GrpcTcpPingResult};
use crate::grpc::controller_grpc::{GrpcPingCommand, GrpcTcpPingCommand};
use chrono::Utc;
use std::convert::TryFrom;
use std::net::{AddrParseError, IpAddr};
use std::option::Option::Some;
use tokio::time;

pub struct PingCommand {
    pub ip: IpAddr,
    pub address: String,
    pub interval: time::Duration,
    pub timeout: time::Duration,
}

impl TryFrom<GrpcPingCommand> for PingCommand {
    type Error = AddrParseError;

    fn try_from(c: GrpcPingCommand) -> Result<Self, Self::Error> {
        let ip = c.ip.parse::<IpAddr>();
        match ip {
            Ok(ip) => Ok(Self {
                ip,
                address: c.ip,
                interval: time::Duration::from_millis(u64::from(c.interval_ms)),
                timeout: time::Duration::from_millis(u64::from(c.timeout_ms)),
            }),
            Err(e) => Err(e),
        }
    }
}

pub struct PingResult {
    pub address: String,
    pub is_timeout: bool,
    pub send_at: chrono::DateTime<Utc>,
    pub rtt: Option<time::Duration>,
}

impl Into<GrpcPingResult> for PingResult {
    fn into(self) -> GrpcPingResult {
        let mut rtt_sec = 0_f32;
        if let Some(rtt) = self.rtt {
            rtt_sec = rtt.as_secs_f32();
        }
        GrpcPingResult {
            ip: self.address,
            is_timeout: self.is_timeout,
            rtt: rtt_sec,
            time: self.send_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

pub struct TcpPingCommand {
    pub target: String,
    pub interval: time::Duration,
    pub timeout: time::Duration,
}

impl From<GrpcTcpPingCommand> for TcpPingCommand {
    fn from(c: GrpcTcpPingCommand) -> Self {
        Self {
            target: c.target,
            interval: time::Duration::from_millis(u64::from(c.interval_ms)),
            timeout: time::Duration::from_millis(u64::from(c.timeout_ms)),
        }
    }
}

pub struct TcpPingResult {
    pub target: String,
    pub is_timeout: bool,
    pub send_at: chrono::DateTime<Utc>,
    pub rtt: Option<time::Duration>,
}

impl Into<GrpcTcpPingResult> for TcpPingResult {
    fn into(self) -> GrpcTcpPingResult {
        let mut rtt_sec = 0_f32;
        if let Some(rtt) = self.rtt {
            rtt_sec = rtt.as_secs_f32();
        }
        GrpcTcpPingResult {
            target: self.target,
            is_timeout: self.is_timeout,
            rtt: rtt_sec,
            send_at: self.send_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

pub enum DetectionResult {
    PingResult(PingResult),
    TcpPingResult(TcpPingResult),
}
