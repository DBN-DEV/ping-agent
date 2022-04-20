use crate::structures::{PingCommand, PingResult};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use chrono::Utc;
use socket2::{Protocol, SockAddr, Socket, Type};
use std::{
    io::{Read, Result},
    net::{IpAddr, SocketAddrV4, SocketAddrV6},
    num::Wrapping,
};
use tokio::{
    io::unix::AsyncFd,
    sync::broadcast,
    sync::broadcast::error::TryRecvError,
    time,
    time::{Duration, MissedTickBehavior},
};
use tracing::info;

const PING_PACKET_LEN: usize = 64;

type ResultTx = tokio::sync::mpsc::Sender<PingResult>;
type ExitSignalRx = broadcast::Receiver<()>;
type ExitedTx = tokio::sync::mpsc::Sender<()>;

pub(super) enum Domain {
    V4,
    V6,
}

pub(super) struct Pinger {
    sock: PingSocket,
    timeout: Duration,
    dst: (SockAddr, String),
    len: usize,
}

impl Pinger {
    pub(super) fn from_ping_command(comm: &PingCommand) -> Self {
        Self::new(comm.ip, comm.timeout, PING_PACKET_LEN)
    }

    pub(super) fn new(ip: IpAddr, timeout: Duration, len: usize) -> Self {
        let (dst, sock) = match ip {
            IpAddr::V4(ip) => {
                let dst = SocketAddrV4::new(ip, 0);
                let sock = PingSocket::new(Domain::V4).expect("Get socket fail");

                (SockAddr::from(dst), sock)
            }
            IpAddr::V6(ip) => {
                let dst = SocketAddrV6::new(ip, 0, 0, 0);
                let sock = PingSocket::new(Domain::V6).expect("Get socket fail");

                (SockAddr::from(dst), sock)
            }
        };

        let dst = (dst, ip.to_string());

        Self {
            sock,
            timeout,
            dst,
            len,
        }
    }

    pub(super) async fn loop_ping(
        &self,
        interval: Duration,
        result_tx: ResultTx,
        mut rx: ExitSignalRx,
        tx: ExitedTx,
    ) {
        let mut seq = Wrapping(0_u16);

        let mut interval = time::interval(interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            seq += Wrapping(1);

            let result = self.ping(seq.0).await.expect("Send/Recv ping fail");

            result_tx.send(result).await.expect("Send result fail");

            match rx.try_recv() {
                Ok(_) => {
                    tx.send(()).await.expect("Send exited signal fail");
                    return;
                }
                Err(TryRecvError::Closed) | Err(TryRecvError::Lagged(_)) => {
                    panic!("Recv exit signal fail");
                }
                Err(broadcast::error::TryRecvError::Empty) => (),
            }
        }
    }

    pub(super) async fn ping(&self, seq: u16) -> Result<PingResult> {
        self.sock.send_request(seq, self.len, &self.dst.0).await?;

        let utc_send_at = Utc::now();
        let result = time::timeout(self.timeout, self.sock.recv_reply(seq, self.len)).await;

        match result {
            Ok(Ok(())) => {
                let utc_recv_at = Utc::now();
                let rtt = (utc_recv_at - utc_send_at).to_std().unwrap();
                Ok(PingResult {
                    address: self.dst.1.clone(),
                    is_timeout: false,
                    send_at: utc_send_at,
                    rtt: Some(rtt),
                })
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(PingResult {
                address: self.dst.1.clone(),
                is_timeout: true,
                send_at: utc_send_at,
                rtt: None,
            }),
        }
    }
}

pub(crate) struct PingSocket {
    inner: AsyncFd<Socket>,
}

impl PingSocket {
    pub(super) fn new(domain: Domain) -> Result<Self> {
        let (domain, protocol) = match domain {
            Domain::V4 => (socket2::Domain::IPV4, Some(Protocol::ICMPV4)),
            Domain::V6 => (socket2::Domain::IPV6, Some(Protocol::ICMPV6)),
        };
        let dgram = Type::DGRAM;
        let inner = Socket::new(domain, dgram, protocol)?;
        inner.set_nonblocking(true)?;
        let inner = AsyncFd::new(inner)?;
        Ok(Self { inner })
    }

    pub(super) async fn send_to(&self, buf: &[u8], addr: &SockAddr) -> Result<usize> {
        loop {
            let mut guard = self.inner.writable().await?;

            match guard.try_io(|inner| inner.get_ref().send_to(buf, addr)) {
                Ok(s) => return s,
                Err(_) => continue,
            }
        }
    }

    pub(super) async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        loop {
            let mut guard = self.inner.readable().await?;
            match guard.try_io(|inner| inner.get_ref().read(buf)) {
                Ok(s) => return s,
                Err(_) => continue,
            }
        }
    }

    fn build_request(seq: u16, len: usize) -> Bytes {
        let mut buf = BytesMut::with_capacity(len);
        // set icmp type and code
        buf.put_u8(8);
        buf.put_u8(0);
        // set icmp check sum and id. linux kernel will handle it, so just put 0.
        buf.put_u16(0);
        buf.put_u16(0);
        // set seq
        buf.put_u16(seq);
        // fill
        buf.resize(len, 1);
        buf.freeze()
    }

    pub(super) async fn send_request(&self, seq: u16, len: usize, addr: &SockAddr) -> Result<()> {
        let buf = Self::build_request(seq, len);
        let result = self.send_to(&buf, addr).await?;
        if result != buf.len() {
            info!("Send packet len:{} less than buf len:{}", result, buf.len());
        }
        Ok(())
    }

    pub(super) async fn recv_reply(&self, expect_seq: u16, len: usize) -> Result<()> {
        loop {
            let mut buf = BytesMut::with_capacity(len);
            buf.resize(len, 0);
            let reply_len = self.read(&mut buf).await?;
            if reply_len != len {
                info!("Recv packet len:{} less than expect len:{}", reply_len, len);
                continue;
            }
            let buf = buf.freeze();
            let seq = buf.slice(6..9).get_u16();
            if seq == expect_seq {
                return Ok(());
            } else {
                info!("Recv packet seq:{} != expect seq:{}", seq, expect_seq);
                continue;
            }
        }
    }
}
