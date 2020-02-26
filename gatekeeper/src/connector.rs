use std::net::{TcpStream, ToSocketAddrs};

use failure::ResultExt;

use crate::byte_stream::ByteStream;

use model::error::{Error, ErrorKind};
use model::model::*;

pub trait Connector: Send {
    type Stream: ByteStream;
    fn connect(&self, addr: Address) -> Result<Self::Stream, Error>;
}

#[derive(Debug, Clone)]
pub struct TcpConnector {}

impl Connector for TcpConnector {
    type Stream = TcpStream;
    fn connect(&self, addr: Address) -> Result<Self::Stream, Error> {
        let sock_addr = match addr {
            Address::IpAddr(addr, port) => SocketAddr::new(addr, port),
            Address::Domain(host, port) => {
                let mut iter = (host.as_str(), port)
                    .to_socket_addrs()
                    .context(ErrorKind::DomainNotResolved { domain: host, port }.into())?;
                iter.next().unwrap()
            }
        };
        TcpStream::connect(sock_addr).map_err(Into::into)
    }
}
