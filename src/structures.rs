use crate::grpc::collector_grpc::{
    GrpcFPingResult, GrpcMtrResult, GrpcPingResult, GrpcTcpPingResult,
};
use crate::grpc::controller_grpc::{
    GrpcFpingCommand, GrpcPingCommand, GrpcTcpPingCommand, MtrCommandResp,
};
use chrono::{DateTime, Utc};
use std::convert::TryFrom;
use std::net::{AddrParseError, IpAddr};
use std::option::Option::Some;
use std::time::Duration;

#[derive(Debug)]
pub struct PingCommand {
    pub id: u64,
    pub ip: IpAddr,
    pub interval: Duration,
    pub timeout: Duration,
    pub dscp: u32,
}

impl TryFrom<GrpcPingCommand> for PingCommand {
    type Error = AddrParseError;

    fn try_from(c: GrpcPingCommand) -> Result<Self, Self::Error> {
        let ip = c.ip.parse::<IpAddr>()?;
        Ok(Self {
            id: c.id,
            ip,
            interval: Duration::from_millis(u64::from(c.interval_ms)),
            timeout: Duration::from_millis(u64::from(c.timeout_ms)),
            dscp: c.dscp,
        })
    }
}

#[derive(Debug)]
pub struct PingResult {
    pub id: u64,
    pub is_timeout: bool,
    pub send_at: DateTime<Utc>,
    pub rtt: Option<Duration>,
}

impl From<PingResult> for GrpcPingResult {
    fn from(v: PingResult) -> Self {
        let mut rtt_micros = 0;
        if let Some(rtt) = v.rtt {
            rtt_micros = rtt.as_micros() as u32;
        }
        GrpcPingResult {
            id: v.id,
            is_timeout: v.is_timeout,
            rtt_micros,
            send_at: v.send_at.timestamp(),
        }
    }
}

#[derive(Debug)]
pub struct TcpPingCommand {
    pub id: u64,
    pub target: String,
    pub interval: Duration,
    pub timeout: Duration,
}

impl From<GrpcTcpPingCommand> for TcpPingCommand {
    fn from(c: GrpcTcpPingCommand) -> Self {
        Self {
            id: c.id,
            target: c.target,
            interval: Duration::from_millis(u64::from(c.interval_ms)),
            timeout: Duration::from_millis(u64::from(c.timeout_ms)),
        }
    }
}

#[derive(Debug)]
pub struct TcpPingResult {
    pub id: u64,
    pub is_timeout: bool,
    pub send_at: DateTime<Utc>,
    pub rtt: Option<Duration>,
}

impl From<TcpPingResult> for GrpcTcpPingResult {
    fn from(v: TcpPingResult) -> Self {
        let mut rtt_micros = 0;
        if let Some(rtt) = v.rtt {
            rtt_micros = rtt.as_micros() as u32;
        }
        GrpcTcpPingResult {
            id: v.id,
            is_timeout: v.is_timeout,
            rtt_micros,
            send_at: v.send_at.timestamp(),
        }
    }
}

#[derive(Debug)]
pub struct FPingCommand {
    pub id: u64,
    pub ip: IpAddr,
    pub timeout: Duration,
    pub dscp: u32,
}

impl TryFrom<GrpcFpingCommand> for FPingCommand {
    type Error = AddrParseError;

    fn try_from(value: GrpcFpingCommand) -> Result<Self, Self::Error> {
        let ip = value.ip.parse::<IpAddr>()?;

        Ok(Self {
            id: value.id,
            ip,
            timeout: Duration::from_millis(u64::from(value.timeout_ms)),
            dscp: value.dscp,
        })
    }
}

#[derive(Debug)]
pub struct FPingResult {
    pub id: u64,
    pub is_timeout: bool,
    pub rtt: Option<Duration>,
}

impl From<FPingResult> for GrpcFPingResult {
    fn from(v: FPingResult) -> Self {
        let mut rtt = 0;
        if let Some(r) = v.rtt {
            rtt = r.as_micros() as u32;
        }

        GrpcFPingResult {
            id: v.id,
            is_timeout: v.is_timeout,
            rtt_micros: rtt,
        }
    }
}

impl From<&PingResult> for FPingResult {
    fn from(v: &PingResult) -> Self {
        Self {
            id: v.id,
            is_timeout: v.is_timeout,
            rtt: v.rtt,
        }
    }
}

#[derive(Debug)]
pub struct MtrCommand {
    pub version: String,
    pub ip: IpAddr,
    pub times: u32,
    pub hop_limit: u32,
    pub timeout: Duration,
}

impl TryFrom<MtrCommandResp> for MtrCommand {
    type Error = AddrParseError;

    fn try_from(value: MtrCommandResp) -> Result<Self, Self::Error> {
        let ip = value.ip.parse::<IpAddr>()?;
        Ok(Self {
            version: value.version,
            ip,
            times: value.times,
            hop_limit: value.hop_limit,
            timeout: Duration::from_millis(u64::from(value.timeout_ms)),
        })
    }
}

#[derive(Debug)]
pub struct MtrResult {
    pub hop: u32,
    pub ip: String,
    pub is_timeout: bool,
    pub rtt: Option<Duration>,
}

impl From<MtrResult> for GrpcMtrResult {
    fn from(v: MtrResult) -> Self {
        let mut rtt_micros = 0;
        if let Some(r) = v.rtt {
            rtt_micros = r.as_micros() as u32;
        }

        GrpcMtrResult {
            hop: v.hop,
            ip: v.ip,
            is_timeout: v.is_timeout,
            rtt_micros,
        }
    }
}
