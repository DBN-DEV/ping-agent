use bytes::{Buf, BufMut, Bytes, BytesMut};
use socket2::{Protocol, SockAddr, Socket, Type};
use std::io::Result;
use tokio::io::unix::AsyncFd;
use tracing::info;

#[allow(dead_code)]
pub(crate) enum Domain {
    V4,
    V6,
}

pub(crate) struct PingSocket {
    inner: AsyncFd<Socket>,
}

impl PingSocket {
    pub(crate) fn new(domain: &Domain) -> Result<Self> {
        let domain = match domain {
            Domain::V4 => socket2::Domain::ipv4(),
            Domain::V6 => socket2::Domain::ipv6(),
        };
        let dgram = Type::dgram();
        let icmp4 = Some(Protocol::icmpv4());
        let inner = Socket::new(domain, dgram, icmp4)?;
        inner.set_nonblocking(true)?;
        let inner = AsyncFd::new(inner)?;
        Ok(Self { inner })
    }

    pub(crate) async fn send_to(&self, buf: &[u8], addr: &SockAddr) -> Result<usize> {
        loop {
            let mut guard = self.inner.writable().await?;

            match guard.try_io(|inner| inner.get_ref().send_to(buf, addr)) {
                Ok(s) => return s,
                Err(_) => continue,
            }
        }
    }

    pub(crate) async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SockAddr)> {
        loop {
            let mut guard = self.inner.readable().await?;
            match guard.try_io(|inner| inner.get_ref().recv_from(buf)) {
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

    pub(crate) async fn send_request(&self, seq: u16, len: usize, addr: &SockAddr) -> Result<()> {
        let buf = Self::build_request(seq, len);
        let result = self.send_to(&buf, addr).await?;
        if result != buf.len() {
            info!("Send packet len:{} less than buf len:{}", result, buf.len());
        }
        Ok(())
    }

    pub(crate) async fn recv_reply(&self, expect_seq: u16, len: usize) -> Result<()> {
        loop {
            let mut buf = BytesMut::with_capacity(len);
            buf.resize(len, 0);
            let (reply_len, _) = self.recv_from(&mut buf).await?;
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
