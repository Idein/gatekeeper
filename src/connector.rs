use std::io;
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use crate::byte_stream::ByteStream;
use crate::model;
use crate::model::error::Error;
use crate::model::model::*;
use crate::pkt_stream::{PktStream, UdpPktStream};

use failure::Fail;

pub trait Connector: Send {
    type B: ByteStream;
    type P: PktStream;
    fn connect_byte_stream(&self, addr: Address) -> Result<(Self::B, SocketAddr), Error>;
    fn connect_pkt_stream(&self, addr: Address) -> Result<(Self::P, SocketAddr), Error>;
}

#[derive(Debug, Clone)]
pub struct TcpUdpConnector {
    rw_timeout: Option<Duration>,
}
impl TcpUdpConnector {
    pub fn new(rw_timeout: Option<Duration>) -> Self {
        Self { rw_timeout }
    }
}

impl Connector for TcpUdpConnector {
    type B = TcpStream;
    type P = UdpPktStream;
    fn connect_byte_stream(&self, addr: Address) -> Result<(Self::B, SocketAddr), Error> {
        let strm = match &addr {
            Address::IpAddr(addr, port) => TcpStream::connect(SocketAddr::new(*addr, *port)),
            Address::Domain(host, port) => TcpStream::connect((host.as_str(), *port)),
        }
        .map_err(|err| conn_error(err, addr, L4Protocol::Tcp))?;
        strm.set_read_timeout(self.rw_timeout)?;
        strm.set_write_timeout(self.rw_timeout)?;

        let peer = strm.peer_addr()?;
        Ok((strm, peer))
    }
    fn connect_pkt_stream(&self, _addr: Address) -> Result<(Self::P, SocketAddr), Error> {
        unimplemented!("connect_pkt_stream")
        /*
        let sock_addr = self.resolve(addr)?;
        UdpSocket::connect(sock_addr).map_err(Into::into)
        */
    }
}

fn conn_error(io_err: io::Error, addr: Address, prot: L4Protocol) -> model::Error {
    use model::ErrorKind;
    match io_err.kind() {
        io::ErrorKind::ConnectionRefused => ErrorKind::connection_refused(addr, prot).into(),
        _ => io_err.context(ErrorKind::Io),
    }
    .into()
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::byte_stream::ByteStream;
    use model::ErrorKind;
    use std::collections::BTreeMap;
    use std::iter::FromIterator;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct BufferConnector<S> {
        pub strms: BTreeMap<Address, Result<S, ConnectError>>,
    }

    impl<S> FromIterator<(Address, Result<S, ConnectError>)> for BufferConnector<S> {
        fn from_iter<T>(iter: T) -> Self
        where
            T: IntoIterator<Item = (Address, Result<S, ConnectError>)>,
        {
            Self {
                strms: iter.into_iter().collect(),
            }
        }
    }

    impl<S> BufferConnector<S> {
        pub fn stream(&self, addr: &Address) -> &S {
            &self.strms[addr].as_ref().unwrap()
        }
    }

    impl<S> Connector for BufferConnector<S>
    where
        S: ByteStream + Clone,
    {
        type B = S;
        type P = UdpPktStream;
        fn connect_byte_stream(&self, addr: Address) -> Result<(Self::B, SocketAddr), Error> {
            println!("connect_byte_stream: {:?}", &addr);
            match &self.strms[&addr] {
                Ok(strm) => {
                    use Address::*;
                    let peer = match addr {
                        IpAddr(peer, port) => SocketAddr::new(peer, port),
                        Domain(_, port) => format!("192.168.1.1:{}", port).parse().unwrap(),
                    };
                    Ok((strm.clone(), peer))
                }
                Err(err) => {
                    use Address::*;
                    use ConnectError::*;
                    use L4Protocol::*;
                    let kind = match err {
                        NetworkUnreachable => match addr {
                            Domain(domain, port) => ErrorKind::DomainNotResolved { domain, port },
                            IpAddr(ipaddr, port) => ErrorKind::HostUnreachable {
                                host: ipaddr.to_string(),
                                port,
                            },
                        },
                        HostUnreachable => {
                            let port = addr.port();
                            let host = match addr {
                                Domain(domain, _) => domain,
                                IpAddr(ipaddr, _) => ipaddr.to_string(),
                            };
                            ErrorKind::HostUnreachable { host, port }
                        }
                        ConnectionNotAllowed => ErrorKind::connection_not_allowed(addr, Tcp),
                        ConnectionRefused => ErrorKind::connection_refused(addr, Tcp),
                        _ => ErrorKind::Io,
                    };
                    Err(kind.into())
                }
            }
        }
        fn connect_pkt_stream(&self, _addr: Address) -> Result<(Self::P, SocketAddr), Error> {
            unimplemented!("BufferConnector::connect_pkt_stream")
        }
    }
}
