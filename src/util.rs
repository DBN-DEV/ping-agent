use chrono::Utc;
use std::net::IpAddr;
use tokio::time;

pub struct PingCommand {
    pub ip: IpAddr,
    pub address: String,
    pub interval: time::Duration,
    pub timeout: time::Duration,
}

pub struct PingResult {
    pub address: String,
    pub is_timeout: bool,
    pub send_at: chrono::DateTime<Utc>,
    pub rtt: Option<time::Duration>,
}
