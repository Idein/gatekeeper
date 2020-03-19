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

use regex::Regex;

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

impl Address {
    pub fn port(&self) -> u16 {
        match self {
            Address::IpAddr(_, port) => *port,
            Address::Domain(_, port) => *port,
        }
    }
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
            K::AddressAlreadInUse { .. } => CErr::ServerFailure,
            K::AddressNotAvailable { .. } => CErr::ServerFailure,
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

#[derive(Debug, Clone)]
pub enum AddressPattern {
    IpAddr { addr: IpAddr, mask: u8 },
    Domain { pattern: Regex },
}

impl Matcher for AddressPattern {
    type Item = Address;

    fn r#match(&self, addr: &Self::Item) -> bool {
        use AddressPattern as P;
        match (self, addr) {
            (P::IpAddr { addr: addrp, mask }, Address::IpAddr(addr, _)) => {
                unimplemented!("AddressPattern::match")
            }
            (P::Domain { pattern }, Address::Domain(domain, _)) => {
                unimplemented!("AddressPattern::match")
            }
            _ => false,
        }
    }
}

trait Matcher {
    type Item;
    fn r#match(&self, t: &Self::Item) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RulePattern<P> {
    Any,
    Specif(P),
}

impl<P: Eq> RulePattern<P> {
    fn any_or(&self, pat: P) -> bool {
        use RulePattern::*;
        match self {
            Any => true,
            Specif(spat) => spat == &pat,
        }
    }
}

impl<T, P> RulePattern<P>
where
    P: Matcher<Item = T>,
{
    fn r#match(&self, t: &T) -> bool {
        match self {
            RulePattern::Any => true,
            RulePattern::Specif(pat) => pat.r#match(t),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectRulePattern {
    pub address: RulePattern<AddressPattern>,
    pub port: RulePattern<u16>,
    pub protocol: RulePattern<L4Protocol>,
}

impl ConnectRulePattern {
    pub fn any() -> Self {
        Self {
            address: RulePattern::Any,
            port: RulePattern::Any,
            protocol: RulePattern::Any,
        }
    }

    pub fn r#match(&self, addr: &Address, protocol: L4Protocol) -> bool {
        if self.address.r#match(addr) {
            return true;
        }
        if self.port.any_or(addr.port()) {
            return true;
        }
        if self.protocol.any_or(protocol) {
            return true;
        }
        return false;
    }
}

#[derive(Debug, Clone)]
pub enum ConnectRuleEntry {
    Allow(ConnectRulePattern),
    Deny(ConnectRulePattern),
}

#[derive(Debug, Clone)]
pub struct ConnectRule {
    rules: Vec<ConnectRuleEntry>,
}

impl ConnectRule {
    /// allow all patterns
    pub fn any() -> Self {
        ConnectRule {
            rules: vec![ConnectRuleEntry::Allow(ConnectRulePattern::any())],
        }
    }

    /// deny all patterns
    pub fn none() -> Self {
        ConnectRule {
            rules: vec![ConnectRuleEntry::Deny(ConnectRulePattern::any())],
        }
    }

    pub fn allow(&self, addr: Address, protocol: L4Protocol) -> bool {
        use ConnectRuleEntry::*;
        for rule in &self.rules {
            match rule {
                Allow(pat) => {
                    if pat.r#match(&addr, protocol) {
                        return true;
                    }
                }
                Deny(pat) => {
                    if pat.r#match(&addr, protocol) {
                        return false;
                    }
                }
            }
        }
        unreachable!("ConnectRule::allow")
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use L4Protocol::*;

    #[test]
    fn any_match() {
        let rule = ConnectRule::any();
        assert_eq!(
            rule.allow(Address::IpAddr("0.0.0.0".parse().unwrap(), 80), Tcp),
            true
        );
        assert_eq!(
            rule.allow(Address::Domain("example.com".to_owned(), 443), Tcp),
            true
        );
        assert_eq!(
            rule.allow(Address::IpAddr("1.2.3.4".parse().unwrap(), 5000), Udp),
            true
        );
        assert_eq!(
            rule.allow(Address::Domain("example.com".to_owned(), 60000), Udp),
            true
        );
    }
}
