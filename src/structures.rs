use crate::grpc::collector_grpc::{
    GrpcFPingResult, GrpcMtrResult, GrpcPingResult, GrpcTcpPingResult,
};
use crate::grpc::controller_grpc::{
    FpingCommandResp, GrpcPingCommand, GrpcTcpPingCommand, MtrCommandResp,
};
use chrono::{DateTime, Utc};
use std::convert::TryFrom;
use std::net::{AddrParseError, IpAddr};
use std::option::Option::Some;
use std::time::Duration;

pub struct PingCommand {
    pub ip: IpAddr,
    pub address: String,
    pub interval: Duration,
    pub timeout: Duration,
}

impl TryFrom<GrpcPingCommand> for PingCommand {
    type Error = AddrParseError;

    fn try_from(c: GrpcPingCommand) -> Result<Self, Self::Error> {
        let ip = c.ip.parse::<IpAddr>()?;
        Ok(Self {
            ip,
            address: c.ip,
            interval: Duration::from_millis(u64::from(c.interval_ms)),
            timeout: Duration::from_millis(u64::from(c.timeout_ms)),
        })
    }
}

pub struct PingResult {
    pub address: String,
    pub is_timeout: bool,
    pub send_at: chrono::DateTime<Utc>,
    pub rtt: Option<Duration>,
}

impl From<PingResult> for GrpcPingResult {
    fn from(v: PingResult) -> Self {
        let mut rtt_micros = 0;
        if let Some(rtt) = v.rtt {
            rtt_micros = rtt.as_micros() as u32;
        }
        GrpcPingResult {
            ip: v.address,
            is_timeout: v.is_timeout,
            rtt_micros,
            utc_send_at: v.send_at.timestamp(),
        }
    }
}

pub struct TcpPingCommand {
    pub target: String,
    pub interval: Duration,
    pub timeout: Duration,
}

impl From<GrpcTcpPingCommand> for TcpPingCommand {
    fn from(c: GrpcTcpPingCommand) -> Self {
        Self {
            target: c.target,
            interval: Duration::from_millis(u64::from(c.interval_ms)),
            timeout: Duration::from_millis(u64::from(c.timeout_ms)),
        }
    }
}

pub struct TcpPingResult {
    pub target: String,
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
            target: v.target,
            is_timeout: v.is_timeout,
            rtt_micros,
            utc_send_at: v.send_at.timestamp(),
        }
    }
}

pub struct FPingCommand {
    pub version: String,
    pub ips: Vec<IpAddr>,
    pub addrs: Vec<String>,
    pub timeout: Duration,
}

impl TryFrom<FpingCommandResp> for FPingCommand {
    type Error = AddrParseError;

    fn try_from(value: FpingCommandResp) -> Result<Self, Self::Error> {
        let mut ips = Vec::with_capacity(value.ip_addrs.len());
        for ip in value.ip_addrs.iter() {
            let ip = ip.parse::<IpAddr>()?;
            ips.push(ip);
        }

        Ok(Self {
            version: value.version,
            ips,
            addrs: value.ip_addrs,
            timeout: Duration::from_millis(u64::from(value.timeout_ms)),
        })
    }
}

pub struct FPingResult {
    pub ip: String,
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
            ip: v.ip,
            is_timeout: v.is_timeout,
            rtt_micros: rtt,
        }
    }
}

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
