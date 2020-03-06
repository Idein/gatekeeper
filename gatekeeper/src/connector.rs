use std::net::{TcpStream, UdpSocket};

use log::*;

use crate::byte_stream::ByteStream;
use crate::pkt_stream::{PktStream, UdpPktStream};

use model::error::Error;
use model::model::*;

pub trait Connector: Send {
    type B: ByteStream;
    type P: PktStream;
    fn connect_byte_stream(&self, addr: Address) -> Result<Self::B, Error>;
    fn connect_pkt_stream(&self, addr: Address) -> Result<Self::P, Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpUdpConnector {
    local_addr: SocketAddr,
    udp_pkt_size: usize,
}

impl TcpUdpConnector {
    pub fn new(local_addr: SocketAddr, udp_pkt_size: usize) -> Self {
        Self {
            local_addr,
            udp_pkt_size,
        }
    }
}

impl Connector for TcpUdpConnector {
    type B = TcpStream;
    type P = UdpPktStream;
    fn connect_byte_stream(&self, addr: Address) -> Result<Self::B, Error> {
        trace!("TcpUdpConnector::connect_byte_stream");
        TcpStream::connect(addr).map_err(Into::into)
    }
    fn connect_pkt_stream(&self, addr: Address) -> Result<Self::P, Error> {
        trace!("TcpUdpConnector::connect_pkt_stream");
        let socket = UdpSocket::bind(self.local_addr)?;
        socket.connect(addr)?;
        Ok(UdpPktStream::new(self.udp_pkt_size, socket))
    }
}
