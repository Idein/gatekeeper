use std::net::TcpStream;

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

#[derive(Debug, Clone)]
pub struct TcpUdpConnector;

impl Connector for TcpUdpConnector {
    type B = TcpStream;
    type P = UdpPktStream;
    fn connect_byte_stream(&self, addr: Address) -> Result<Self::B, Error> {
        match addr {
            Address::IpAddr(addr, port) => TcpStream::connect(SocketAddr::new(addr, port)),
            Address::Domain(host, port) => TcpStream::connect((host.as_str(), port)),
        }
        .map_err(Into::into)
    }
    fn connect_pkt_stream(&self, _addr: Address) -> Result<Self::P, Error> {
        unimplemented!("connect_pkt_stream")
        /*
        let sock_addr = self.resolve(addr)?;
        UdpSocket::connect(sock_addr).map_err(Into::into)
        */
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::byte_stream::ByteStream;
    use model::ErrorKind;
    use std::collections::BTreeMap;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct BufferConnector<S> {
        pub strms: BTreeMap<Address, S>,
    }

    impl<S> BufferConnector<S> {
        pub fn stream(&self, addr: &Address) -> &S {
            &self.strms[addr]
        }
    }

    impl<S> Connector for BufferConnector<S>
    where
        S: ByteStream + Clone,
    {
        type B = S;
        type P = UdpPktStream;
        fn connect_byte_stream(&self, addr: Address) -> Result<Self::B, Error> {
            println!("connect_byte_stream: {:?}", &addr);
            if self.strms.contains_key(&addr) {
                println!("collect buffer: {:?}", self.strms[&addr]);
                Ok(self.strms[&addr].clone())
            } else {
                use Address::*;
                match addr {
                    Domain(domain, port) => Err(ErrorKind::DomainNotResolved {
                        domain: domain.into(),
                        port,
                    }
                    .into()),
                    IpAddr(ipaddr, port) => Err(ErrorKind::HostUnreachable {
                        host: ipaddr.to_string(),
                        port,
                    }
                    .into()),
                }
            }
        }
        fn connect_pkt_stream(&self, _addr: Address) -> Result<Self::P, Error> {
            unimplemented!("BufferConnector::connect_pkt_stream")
        }
    }
}
