use std::net::{TcpStream, ToSocketAddrs};

use failure::ResultExt;
use log::*;

use crate::byte_stream::ByteStream;
use crate::pkt_stream::{PktStream, UdpPktStream};

use model::error::{Error, ErrorKind};
use model::model::*;

pub trait Connector: Send {
    type B: ByteStream;
    type P: PktStream;
    fn connect_byte_stream(&self, addr: Address) -> Result<Self::B, Error>;
    fn connect_pkt_stream(&self, addr: Address) -> Result<Self::P, Error>;
}

#[derive(Debug, Clone)]
pub struct TcpUdpConnector;

impl TcpUdpConnector {
    fn resolve(&self, addr: Address) -> Result<SocketAddr, Error> {
        let sock_addr = match addr {
            Address::IpAddr(addr, port) => SocketAddr::new(addr, port),
            Address::Domain(host, port) => {
                let mut iter = (host.as_str(), port)
                    .to_socket_addrs()
                    .context(ErrorKind::DomainNotResolved { domain: host, port }.into())?;
                iter.next().unwrap()
            }
        };
        trace!("resolved: {}", sock_addr);
        Ok(sock_addr)
    }
}

impl Connector for TcpUdpConnector {
    type B = TcpStream;
    type P = UdpPktStream;
    fn connect_byte_stream(&self, addr: Address) -> Result<Self::B, Error> {
        let sock_addr = self.resolve(addr)?;
        TcpStream::connect(sock_addr).map_err(Into::into)
    }
    fn connect_pkt_stream(&self, _addr: Address) -> Result<Self::P, Error> {
        unimplemented!("connect_pkt_stream")
        /*
        let sock_addr = self.resolve(addr)?;
        UdpSocket::connect(sock_addr).map_err(Into::into)
        */
    }
}
