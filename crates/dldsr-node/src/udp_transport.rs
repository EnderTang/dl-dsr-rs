use anyhow::Result;
use dldsr_core::Packet;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub struct UdpTransport {
    socket: UdpSocket,
}

impl UdpTransport {
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(addr).await?,
        })
    }

    pub async fn send(&self, packet: &Packet, addr: SocketAddr) -> Result<usize> {
        Ok(self.socket.send_to(&packet.encode()?, addr).await?)
    }

    pub async fn recv(&self, buf: &mut [u8]) -> Result<(Packet, SocketAddr, usize)> {
        let (len, src) = self.socket.recv_from(buf).await?;
        let packet = Packet::decode(&buf[..len])?;
        Ok((packet, src, len))
    }
}
