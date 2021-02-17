//! SOCKS5 message types exchanged between client and proxy.
//!
//! ```text
//! client            proxy            service
//!   |                 |                 |
//!   .                 .                 .
//!   .                 .                 .
//!   |                 |                 |
//!   |---------------->|                 |
//!   |MethodCandidates |                 |
//!   |                 |                 |
//!   |<----------------|                 |
//!   |  MethodSelection|                 |
//!   |                 |                 |
//!   |---------------->|                 |
//!   |ConnectRequest   |                 |
//!   |                 |                 |
//!   |<----------------|                 |
//!   |     ConnectReply|                 |
//!   |                 |                 |
//!   |                 |                 |
//!   .                 .                 .
//!   .                 .                 .
//!   | - - - - - - - ->| - - - - - - - ->|
//!   |            [[ Relay ]]            |
//!   |<- - - - - - - - |< - - - - - - - -|
//!   .                 .                 .
//!   .                 .                 .
//!   |                 |                 |
//! ```
//!
use std::fmt;
use std::net::ToSocketAddrs;
pub use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::str::FromStr;

use derive_more::{Display, From, Into};
use failure::Fail;
use log::*;
use regex::{escape, Regex};
use serde::*;

pub const DEFAULT_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion(5);

// Domain labels can include letter, digit and hyphen
// See https://tools.ietf.org/html/rfc1035#section-2.3.1
static AVAILABLE_STRINGS_FOR_DOMAIN_LABEL: &str = r"[A-Za-z0-9-]{1,63}";

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

impl MethodCandidates {
    pub fn new(method: &[Method]) -> Self {
        Self {
            version: DEFAULT_PROTOCOL_VERSION,
            method: method.to_vec(),
        }
    }
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

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use Address::*;
        match self {
            IpAddr(addr, port) => write!(f, "{}:{}", addr, port),
            Domain(host, port) => write!(f, "{}:{}", host, port),
        }
    }
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
        Address::IpAddr(addr.ip(), addr.port())
    }
}

impl From<SocketAddrV4> for Address {
    fn from(addr: SocketAddrV4) -> Self {
        Address::IpAddr(addr.ip().clone().into(), addr.port())
    }
}

impl From<SocketAddrV6> for Address {
    fn from(addr: SocketAddrV6) -> Self {
        Address::IpAddr(addr.ip().clone().into(), addr.port())
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

impl ConnectRequest {
    pub fn connect_to<A>(addr: A) -> Self
    where
        Address: From<A>,
    {
        Self {
            version: DEFAULT_PROTOCOL_VERSION,
            command: Command::Connect,
            connect_to: addr.into(),
        }
    }

    pub fn bind<A>(addr: A) -> Self
    where
        Address: From<A>,
    {
        Self {
            version: DEFAULT_PROTOCOL_VERSION,
            command: Command::Bind,
            connect_to: addr.into(),
        }
    }

    pub fn udp_associate<A>(addr: A) -> Self
    where
        Address: From<A>,
    {
        Self {
            version: DEFAULT_PROTOCOL_VERSION,
            command: Command::UdpAssociate,
            connect_to: addr.into(),
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum L4Protocol {
    Tcp,
    Udp,
}

impl fmt::Display for L4Protocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            L4Protocol::Tcp => write!(f, "Tcp"),
            L4Protocol::Udp => write!(f, "Udp"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UdpDatagram<'a> {
    pub frag: u8,
    pub dst_addr: Address,
    pub data: &'a [u8],
}

#[derive(Debug, Clone, Serialize)]
pub enum AddressPattern {
    /// e.g. 127.0.0.1/16
    IpAddr {
        addr: IpAddr,
        prefix: u8,
    },
    Domain(DomainPattern),
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum DomainPattern {
    Regex {
        #[serde(with = "serde_regex")]
        pattern: Regex,
    },
    Wildcard {
        wildcard: String,
    },
}

#[derive(Fail, Debug)]
pub enum InvalidPrefix {
    V4 { addr: Ipv4Addr, prefix: u8 },
    V6 { addr: Ipv6Addr, prefix: u8 },
}

impl fmt::Display for InvalidPrefix {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use InvalidPrefix::*;
        match self {
            V4 { addr, prefix } => write!(f, "{} is too large for {}", prefix, addr),
            V6 { addr, prefix } => write!(f, "{} is too large for {}", prefix, addr),
        }
    }
}

impl de::Expected for InvalidPrefix {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use InvalidPrefix::*;
        match self {
            V4 { addr, prefix } => write!(f, "less than or equals to 32: {}/{}", addr, prefix),
            V6 { addr, prefix } => write!(f, "less than or equals to 128: {}/{}", addr, prefix),
        }
    }
}

impl InvalidPrefix {
    fn v4(addr: Ipv4Addr, prefix: u8) -> Self {
        InvalidPrefix::V4 { addr, prefix }
    }
    fn v6(addr: Ipv6Addr, prefix: u8) -> Self {
        InvalidPrefix::V6 { addr, prefix }
    }
}

impl AddressPattern {
    pub fn addr(addr: IpAddr, prefix: u8) -> Result<Self, InvalidPrefix> {
        match addr {
            IpAddr::V4(addr) => {
                if prefix <= 32 {
                    Ok(AddressPattern::IpAddr {
                        addr: addr.into(),
                        prefix,
                    })
                } else {
                    Err(InvalidPrefix::v4(addr, prefix))
                }
            }
            IpAddr::V6(addr) => {
                if prefix <= 128 {
                    Ok(AddressPattern::IpAddr {
                        addr: addr.into(),
                        prefix,
                    })
                } else {
                    Err(InvalidPrefix::v6(addr, prefix))
                }
            }
        }
    }
}

impl From<Regex> for AddressPattern {
    fn from(reg: Regex) -> Self {
        AddressPattern::Domain(DomainPattern::Regex { pattern: reg })
    }
}

impl Matcher for AddressPattern {
    type Item = Address;

    fn r#match(&self, addr: &Self::Item) -> bool {
        use AddressPattern as P;
        use DomainPattern as DP;
        match (self, addr) {
            (
                P::IpAddr {
                    addr: IpAddr::V4(addrp),
                    prefix,
                },
                Address::IpAddr(IpAddr::V4(addr), _),
            ) => {
                let bmask = !0u32 << (32 - prefix);
                u32::from(*addrp) & bmask == u32::from(*addr) & bmask
            }
            (
                P::IpAddr {
                    addr: IpAddr::V6(addrp),
                    prefix,
                },
                Address::IpAddr(IpAddr::V6(addr), _),
            ) => {
                let bmask = !0u128 << (128 - prefix);
                u128::from(*addrp) & bmask == u128::from(*addr) & bmask
            }
            (P::Domain(DP::Regex { pattern }), Address::Domain(domain, _)) => {
                pattern.is_match(domain)
            }
            (P::Domain(DP::Wildcard { wildcard }), Address::Domain(domain, _)) => {
                let pattern = format!(
                    r"\A{}\z",
                    &escape(wildcard).replace(r"\*", AVAILABLE_STRINGS_FOR_DOMAIN_LABEL)
                );
                let reg = Regex::new(&pattern).unwrap();
                reg.is_match(domain)
            }

            _ => false,
        }
    }
}

pub trait Matcher {
    type Item;
    fn r#match(&self, t: &Self::Item) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RulePattern<P> {
    Any,
    Specif(P),
}

impl<P> RulePattern<P> {
    pub fn is_any(&self) -> bool {
        matches!(self, RulePattern::Any)
    }

    pub fn is_specif(&self) -> bool {
        matches!(self, RulePattern::Specif(_))
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn is_any(&self) -> bool {
        let Self {
            ref address,
            ref port,
            ref protocol,
        } = self;
        address.is_any() && port.is_any() && protocol.is_any()
    }

    pub fn r#match(&self, addr: &Address, protocol: L4Protocol) -> bool {
        self.address.r#match(addr)
            && self.port.any_or(addr.port())
            && self.protocol.any_or(protocol)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectRuleEntry {
    Allow(ConnectRulePattern),
    Deny(ConnectRulePattern),
}

impl ConnectRuleEntry {
    pub fn sum<R>(&self, f: impl FnOnce(&ConnectRulePattern) -> R) -> R {
        use ConnectRuleEntry::*;
        match self {
            Allow(pat) => f(pat),
            Deny(pat) => f(pat),
        }
    }
}

/// Connection rules
///
/// All instances of this type are constructed by `any` or `none` method.
///
/// # Example
/// This example only allow connecting to local networks.
///
/// ```
/// # use gatekeeper::model::L4Protocol::*;
/// # use gatekeeper::{AddressPattern, ConnectRule, RulePattern::*};
/// use AddressPattern as Pat;
/// # fn main() -> Result<(), std::net::AddrParseError> {
/// let mut rule = ConnectRule::none();
/// rule.allow(
///     Specif(Pat::IpAddr {
///         addr: "192.168.0.1".parse()?,
///         prefix: 16,
///     }),
///     Specif(80),
///     Any,
/// );
/// assert!(rule.check("192.168.0.2:80".parse()?, Tcp));
/// assert!(!rule.check("192.167.0.2:80".parse()?, Udp));
/// # Ok(())
/// # }
/// ```
///
#[derive(Debug, Clone)]
pub struct ConnectRule {
    // rules.len() >= 1
    rules: Vec<ConnectRuleEntry>,
}

mod format {
    use super::*;
    use de::Unexpected;

    // dummy type for acquires derived deserializer
    #[derive(Debug, Clone, Deserialize)]
    enum AddressPatternDef {
        IpAddr { addr: IpAddr, prefix: u8 },
        Domain(DomainPatternDef),
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(untagged)]
    enum DomainPatternDef {
        Regex {
            #[serde(with = "serde_regex")]
            pattern: Regex,
        },
        Wildcard {
            wildcard: String,
        },
    }

    impl<'de> Deserialize<'de> for AddressPattern {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            // impl Deserialize for AddressPattern using the Deserialize for AddressPatternDef
            use AddressPatternDef::*;
            use DomainPatternDef::*;
            match AddressPatternDef::deserialize(deserializer)? {
                IpAddr { addr, prefix } => AddressPattern::addr(addr, prefix).map_err(|err| {
                    de::Error::invalid_value(Unexpected::Unsigned(prefix as u64), &err)
                }),
                Domain(Regex { pattern }) => {
                    Ok(AddressPattern::Domain(DomainPattern::Regex { pattern }))
                }
                Domain(Wildcard { wildcard }) => {
                    Ok(AddressPattern::Domain(DomainPattern::Wildcard { wildcard }))
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn deserialize_addr_pat() {
            let ipv4 = r#"
IpAddr:
  addr: 192.168.0.1
  prefix: 24
"#;
            assert!(
                match serde_yaml::from_str::<AddressPattern>(&ipv4).unwrap() {
                    AddressPattern::IpAddr {
                        addr: IpAddr::V4(addr),
                        prefix,
                    } =>
                        u32::from(addr) == u32::from(Ipv4Addr::new(192, 168, 0, 1)) && prefix == 24,
                    _ => false,
                }
            );
        }

        #[test]
        fn deserialize_addr_large_prefix() {
            let ipv4_invalid = r#"
IpAddr:
  addr: 192.168.0.1
  prefix: 33
"#;
            let res = serde_yaml::from_str::<AddressPattern>(&ipv4_invalid).unwrap_err();
            println!("invalid: {}", res);
        }
    }

    impl Serialize for ConnectRule {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            use serde::ser::SerializeSeq;
            let mut seq = serializer.serialize_seq(Some(self.rules.len()))?;
            for elm in &self.rules {
                seq.serialize_element(elm)?;
            }
            seq.end()
        }
    }

    fn deserialize_connect_rule<'de, D>(deserializer: D) -> Result<ConnectRule, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{SeqAccess, Visitor};
        struct ConnectRuleVisitor;

        impl<'de> Visitor<'de> for ConnectRuleVisitor {
            type Value = Vec<ConnectRuleEntry>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a nonempty sequence of ConnectRules")
            }

            fn visit_seq<S>(self, mut seq: S) -> Result<Self::Value, S::Error>
            where
                S: SeqAccess<'de>,
            {
                let mut rules = vec![];
                // read 1st element as base(=default) rule
                let base_rule: ConnectRuleEntry = seq.next_element()?.ok_or_else(|| {
                    de::Error::custom("no values in seq when looking for ConnectRule")
                })?;
                // base rule should be equal to any or none
                if !base_rule.sum(|entry| entry.is_any()) {
                    return Err(de::Error::custom("base rule is not any or none"));
                }
                rules.push(base_rule);
                // read remaining elements
                while let Some(elm) = seq.next_element()? {
                    rules.push(elm);
                }
                Ok(rules)
            }
        }

        deserializer
            .deserialize_seq(ConnectRuleVisitor)
            .map(|rules| ConnectRule { rules })
    }

    impl<'de> Deserialize<'de> for ConnectRule {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserialize_connect_rule(deserializer)
        }
    }
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

    pub fn is_any(&self) -> bool {
        if let Some(entry) = self.rules.get(0) {
            use ConnectRuleEntry::*;
            match entry {
                Allow(pat) => pat.is_any(),
                Deny(_) => false,
            }
        } else {
            false
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
                        trace!("match(allow): {:?}: {}/{}", pat, addr, protocol);
                        return true;
                    }
                }
                Deny(pat) => {
                    if pat.r#match(&addr, protocol) {
                        trace!("match(deny): {:?}: {}/{}", pat, addr, protocol);
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
    fn none_match() {
        use Address::Domain;
        let rule = ConnectRule::none();
        assert!(!rule.check("0.0.0.0:80".parse().unwrap(), Tcp));
        assert!(!rule.check(Domain("example.com".to_owned(), 443), Tcp));
        assert!(!rule.check("1.2.3.4:5000".parse().unwrap(), Udp));
        assert!(!rule.check(Domain("example.com".to_owned(), 60000), Udp),);
    }

    #[test]
    fn domain_pattern() {
        use Address::Domain;
        use RulePattern::*;
        let mut rule = ConnectRule::none();
        rule.allow(
            Specif(Regex::new(r"(.*\.)?actcast\.io").unwrap().into()),
            Any,
            Specif(L4Protocol::Tcp),
        );
        assert!(!rule.check("0.0.0.0:80".parse().unwrap(), Tcp));
        assert!(!rule.check(Domain("example.com".to_owned(), 443), Tcp));
        assert!(rule.check(Domain("actcast.io".to_owned(), 60000), Tcp));
        assert!(!rule.check(Domain("actcast.io".to_owned(), 60000), Udp));
        assert!(rule.check(Domain("www.actcast.io".to_owned(), 65535), Tcp));
        assert!(!rule.check(Domain("www.actcast.io".to_owned(), 32768), Udp));
    }

    #[test]
    fn domain_wildcard() {
        use Address::Domain;
        use RulePattern::*;
        struct Case {
            wildcard: String,
            match_domains: Vec<String>,
            unmatch_domains: Vec<String>,
        }
        impl Case {
            fn new(wildcard: &str, match_domains: Vec<&str>, unmatch_domains: Vec<&str>) -> Self {
                Case {
                    wildcard: wildcard.to_owned(),
                    match_domains: match_domains.into_iter().map(|s| s.to_owned()).collect(),
                    unmatch_domains: unmatch_domains.into_iter().map(|s| s.to_owned()).collect(),
                }
            }
        }
        let cases = vec![
            Case::new(
                "*.*.example.com",
                vec!["b.a.example.com"],
                vec!["example.com", "a.example.com", "c.b.a.example.com"],
            ),
            Case::new(
                "fo*.b*r.*az.example.com",
                vec!["foo.bar.baz.example.com"],
                vec![
                    "fuu.bar.baz.example.com",
                    "foo.var.baz.example.com",
                    "foo.bar.buz.example.com",
                ],
            ),
            Case::new(
                "*.execute-api.*-east-*.amazonaws.com",
                vec![
                    "foo.execute-api.us-east-1.amazonaws.com",
                    "foo.execute-api.us-east-2.amazonaws.com",
                ],
                vec![
                    "foo.execute-api.us-west-1.amazonaws.com",
                    "foo.execute-api.ap-northeast-1.amazonaws.com",
                ],
            ),
        ];
        for case in cases {
            let mut rule = ConnectRule::none();
            rule.allow(
                Specif(AddressPattern::Domain(DomainPattern::Wildcard {
                    wildcard: case.wildcard,
                })),
                Specif(443),
                Specif(Tcp),
            );
            for domain in case.match_domains {
                assert!(rule.check(Domain(domain, 443), Tcp))
            }
            for domain in case.unmatch_domains {
                assert!(!rule.check(Domain(domain, 443), Tcp))
            }
        }
    }

    #[test]
    fn address_pattern() {
        use Address::Domain;
        use AddressPattern as Pat;
        use RulePattern::*;
        let mut rule = ConnectRule::none();
        rule.allow(
            Specif(Pat::addr("192.168.0.1".parse().unwrap(), 24).unwrap()),
            Specif(80),
            Any,
        );
        rule.allow(
            Specif(Pat::addr("192.168.0.1".parse().unwrap(), 24).unwrap()),
            Specif(443),
            Any,
        );
        assert!(!rule.check("0.0.0.0:80".parse().unwrap(), Tcp));
        assert!(rule.check("192.168.0.0:80".parse().unwrap(), Tcp),);
        assert!(rule.check("192.168.0.0:443".parse().unwrap(), Tcp),);
        assert!(!rule.check("192.168.1.2:443".parse().unwrap(), Udp));
        assert!(!rule.check("192.167.0.3:443".parse().unwrap(), Tcp));
        assert!(rule.check("192.168.0.255:80".parse().unwrap(), Tcp),);
        assert!(rule.check("192.168.0.42:80".parse().unwrap(), Tcp),);
        assert!(!rule.check(Domain("example.com".to_owned(), 443), Tcp));
        assert!(!rule.check(Domain("actcast.io".to_owned(), 60000), Udp));
    }

    #[test]
    fn ipv6_pattern() {
        use AddressPattern as Pat;
        use RulePattern::*;
        let mut rule = ConnectRule::none();
        rule.allow(
            Specif(Pat::addr("ff01::0".parse().unwrap(), 32).unwrap()),
            Any,
            Any,
        );
        assert!(rule.check(
            SocketAddrV6::new(Ipv6Addr::new(0xff01, 0, 0, 0, 0, 0, 0, 0x1), 80, 0, 0).into(),
            Tcp
        ));
        assert!(!rule.check(
            SocketAddrV6::new(Ipv6Addr::new(0xffff, 0, 0, 0, 0, 0, 0, 0x1), 80, 0, 0).into(),
            Tcp
        ));
    }

    #[test]
    fn serde_rules() {
        use AddressPattern as Pat;
        use RulePattern::*;
        let mut rule = ConnectRule::none();
        rule.allow(
            Specif(Pat::addr("192.168.0.1".parse().unwrap(), 16).unwrap()),
            Specif(80),
            Any,
        );
        rule.allow(
            Specif(Pat::addr("192.168.0.1".parse().unwrap(), 16).unwrap()),
            Specif(443),
            Any,
        );
        rule.allow(
            Specif(Regex::new(r"\A(.+\.)?actcast\.io\z").unwrap().into()),
            Any,
            Specif(L4Protocol::Tcp),
        );
        rule.allow(
            Specif(Pat::Domain(DomainPattern::Wildcard {
                wildcard: "*.actcast.io".to_owned(),
            })),
            Any,
            Specif(L4Protocol::Tcp),
        );

        // compares on yaml::Value
        // rule -> yaml
        let value = serde_yaml::to_value(&rule).unwrap();
        // rule -> str -> rule' -> yaml
        let value2 = {
            let str = serde_yaml::to_string(&rule).unwrap();
            let rule: ConnectRule = serde_yaml::from_str(&str).unwrap();
            serde_yaml::to_value(&rule).unwrap()
        };
        println!("rule(as yaml):\n{}", serde_yaml::to_string(&rule).unwrap());
        assert_eq!(&value, &value2);
    }

    #[test]
    fn example_rule() {
        use std::fs::File;
        use std::path::Path;
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("example.yml");
        let example = File::open(path).unwrap();
        let value: serde_yaml::Value = serde_yaml::from_reader(&example).unwrap();
        println!("value: {}", serde_yaml::to_string(&value).unwrap());
        let value2 = {
            let rule: ConnectRule = serde_yaml::from_value(value.clone()).unwrap();
            serde_yaml::to_value(&rule).unwrap()
        };
        println!("value2: {}", serde_yaml::to_string(&value).unwrap());
        assert_eq!(&value, &value2);
    }
}
