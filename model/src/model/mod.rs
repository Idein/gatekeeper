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
use std::str::FromStr;

use log::*;
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

impl FromStr for Address {
    type Err = std::net::AddrParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let addr: SocketAddr = s.parse()?;
        Ok(addr.into())
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

impl From<Regex> for AddressPattern {
    fn from(reg: Regex) -> Self {
        AddressPattern::Domain { pattern: reg }
    }
}

impl Matcher for AddressPattern {
    type Item = Address;

    fn r#match(&self, addr: &Self::Item) -> bool {
        use AddressPattern as P;
        match (self, addr) {
            (
                P::IpAddr {
                    addr: IpAddr::V4(addrp),
                    mask,
                },
                Address::IpAddr(IpAddr::V4(addr), _),
            ) => {
                let bmask = !0u32 << mask;
                u32::from_be_bytes(addrp.octets()) & bmask
                    == u32::from_be_bytes(addr.octets()) & bmask
            }
            (
                P::IpAddr {
                    addr: IpAddr::V6(addrp),
                    mask,
                },
                Address::IpAddr(IpAddr::V6(addr), _),
            ) => {
                let bmask = !0u128 << mask;
                u128::from_be_bytes(addrp.octets()) & bmask
                    == u128::from_be_bytes(addr.octets()) & bmask
            }
            (P::Domain { pattern }, Address::Domain(domain, _)) => pattern.is_match(domain),
            _ => false,
        }
    }
}

pub trait Matcher {
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
    pub fn new(
        address: RulePattern<AddressPattern>,
        port: RulePattern<u16>,
        protocol: RulePattern<L4Protocol>,
    ) -> Self {
        ConnectRulePattern {
            address,
            port,
            protocol,
        }
    }

    pub fn any() -> Self {
        Self {
            address: RulePattern::Any,
            port: RulePattern::Any,
            protocol: RulePattern::Any,
        }
    }

    pub fn r#match(&self, addr: &Address, protocol: L4Protocol) -> bool {
        self.address.r#match(addr)
            && self.port.any_or(addr.port())
            && self.protocol.any_or(protocol)
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

    pub fn allow(
        &mut self,
        addr: RulePattern<AddressPattern>,
        port: RulePattern<u16>,
        protocol: RulePattern<L4Protocol>,
    ) {
        self.rules
            .push(ConnectRuleEntry::Allow(ConnectRulePattern::new(
                addr, port, protocol,
            )));
    }

    pub fn deny(
        &mut self,
        addr: RulePattern<AddressPattern>,
        port: RulePattern<u16>,
        protocol: RulePattern<L4Protocol>,
    ) {
        self.rules
            .push(ConnectRuleEntry::Deny(ConnectRulePattern::new(
                addr, port, protocol,
            )));
    }

    pub fn check(&self, addr: Address, protocol: L4Protocol) -> bool {
        use ConnectRuleEntry::*;
        for rule in self.rules.iter().rev() {
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
        use Address::Domain;
        let rule = ConnectRule::any();
        assert!(rule.check("0.0.0.0:80".parse().unwrap(), Tcp));
        assert!(rule.check(Domain("example.com".to_owned(), 443), Tcp));
        assert!(rule.check("1.2.3.4:5000".parse().unwrap(), Udp));
        assert!(rule.check(Domain("example.com".to_owned(), 60000), Udp),);
    }

    #[test]
    fn domain_pattern() {
        use Address::Domain;
        use AddressPattern as Pat;
        use RulePattern::*;
        let mut rule = ConnectRule::none();
        rule.allow(
            Specif(Regex::new(r"(.*\.)?actcast\.io").unwrap().into()),
            Any,
            Any,
        );
        assert!(!rule.check("0.0.0.0:80".parse().unwrap(), Tcp));
        assert!(!rule.check(Domain("example.com".to_owned(), 443), Tcp));
        assert!(rule.check(Domain("actcast.io".to_owned(), 60000), Udp));
        assert!(rule.check(Domain("www.actcast.io".to_owned(), 60000), Udp));
    }
}
