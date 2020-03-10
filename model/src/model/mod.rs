///! SOCKS5 message types exchanged between client and proxy.
///!
///! client            proxy            service
///!   |                 |                 |
///!   .                 .                 .
///!   .                 .                 .
///!   |                 |                 |
///!   |---------------->|                 |
///!   |MethodCandidates |                 |
///!   |                 |                 |
///!   |<----------------|                 |
///!   |  MethodSelection|                 |
///!   |                 |                 |
///!   |---------------->|                 |
///!   |ConnectRequest   |                 |
///!   |                 |                 |
///!   |<----------------|                 |
///!   |     ConnectReply|                 |
///!   |                 |                 |
///!   |                 |                 |
///!   .                 .                 .
///!   .                 .                 .
///!   | - - - - - - - ->| - - - - - - - ->|
///!   |            [[ Relay ]]            |
///!   |<- - - - - - - - |< - - - - - - - -|
///!   .                 .                 .
///!   .                 .                 .
///!   |                 |                 |
///!
use derive_more::{Display, From, Into};
use std::net::ToSocketAddrs;
pub use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Into, From, Display)]
pub struct ProtocolVersion(u8);

/// Authentication Methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Display)]
pub enum Method {
    /// No Authentication
    NoAuth,
    /// GSSAPI
    GssApi,
    /// Authenticate with a username / password
    UserPass,
    /// IANA assigned method
    IANAMethod(u8),
    /// Reserved for private method
    Private(u8),
    /// No acceptable method
    NoMethods,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MethodCandidates {
    pub version: ProtocolVersion,
    pub method: Vec<Method>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MethodSelection {
    pub version: ProtocolVersion,
    pub method: Method,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Command {
    Connect,
    Bind,
    UdpAssociate,
}

/// ip address and port
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Address {
    IpAddr(IpAddr, u16),
    Domain(String, u16),
}

impl From<SocketAddr> for Address {
    fn from(addr: SocketAddr) -> Self {
        Address::IpAddr(addr.ip().clone(), addr.port())
    }
}

impl ToSocketAddrs for Address {
    type Iter = std::vec::IntoIter<SocketAddr>;

    /// Convert an address and AddrType to a SocketAddr
    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        use Address::*;
        match self {
            IpAddr(ipaddr, port) => Ok(vec![SocketAddr::new(*ipaddr, *port)].into_iter()),
            Domain(domain, port) => Ok((domain.as_str(), *port).to_socket_addrs()?),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnectRequest {
    pub version: ProtocolVersion,
    pub command: Command,
    pub connect_to: Address,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Display)]
pub enum ConnectError {
    /// general server failure
    ServerFailure,
    ConnectionNotAllowed,
    NetworkUnreachable,
    HostUnreachable,
    ConnectionRefused,
    TtlExpired,
    CommandNotSupported,
    AddrTypeNotSupported,
}

impl From<crate::error::ErrorKind> for ConnectError {
    fn from(err: crate::error::ErrorKind) -> Self {
        use crate::error::ErrorKind as K;
        use ConnectError as CErr;
        match err {
            K::Io => CErr::ServerFailure,
            K::MessageFormat { .. } => CErr::ServerFailure,
            K::Authentication => CErr::ConnectionNotAllowed,
            K::NoAcceptableMethod => CErr::ConnectionNotAllowed,
            K::UnrecognizedUsernamePassword => CErr::ConnectionNotAllowed,
            K::CommandNotSupported { .. } => CErr::CommandNotSupported,
            K::HostUnreachable { .. } => CErr::HostUnreachable,
            K::DomainNotResolved { .. } => CErr::NetworkUnreachable,
            K::PacketSizeLimitExceeded { .. } => CErr::ServerFailure,
        }
    }
}

impl std::error::Error for ConnectError {
    fn description(&self) -> &str {
        "ConnectError"
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

pub type ConnectResult = std::result::Result<(), ConnectError>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnectReply {
    pub version: ProtocolVersion,
    pub connect_result: ConnectResult,
    pub server_addr: Address,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum L4Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UdpDatagram<'a> {
    pub frag: u8,
    pub dst_addr: Address,
    pub data: &'a [u8],
}
