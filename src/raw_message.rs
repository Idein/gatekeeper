///! RFC1928 SOCKS Protocol Version 5 Raw Message Types
///! For each type structures correspond to SOCKS5 packet layout.
///!
use std::convert::{TryFrom, TryInto};
use std::fmt;
pub use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use crate::model;

pub const RESERVED: u8 = 0x00;

/// Version of socks
pub use model::ProtocolVersion;

/// Section 6. Replies > Reply field value
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResponseCode {
    Success = 0x00,
    Failure = 0x01,
    RuleFailure = 0x02,
    NetworkUnreachable = 0x03,
    HostUnreachable = 0x04,
    ConnectionRefused = 0x05,
    TtlExpired = 0x06,
    CommandNotSupported = 0x07,
    AddrTypeNotSupported = 0x08,
}

impl From<model::ConnectResult> for ResponseCode {
    fn from(res: model::ConnectResult) -> Self {
        use model::ConnectError::*;
        match res {
            Ok(()) => ResponseCode::Success,
            Err(ServerFailure) => ResponseCode::Failure,
            Err(ConnectionNotAllowed) => ResponseCode::RuleFailure,
            Err(NetworkUnreachable) => ResponseCode::NetworkUnreachable,
            Err(HostUnreachable) => ResponseCode::HostUnreachable,
            Err(ConnectionRefused) => ResponseCode::ConnectionRefused,
            Err(TtlExpired) => ResponseCode::TtlExpired,
            Err(CommandNotSupported) => ResponseCode::CommandNotSupported,
            Err(AddrTypeNotSupported) => ResponseCode::AddrTypeNotSupported,
        }
    }
}

impl From<ResponseCode> for model::ConnectResult {
    fn from(res: ResponseCode) -> Self {
        use model::ConnectError as CErr;
        use ResponseCode::*;
        match res {
            Success => Ok(()),
            Failure => Err(CErr::ServerFailure),
            RuleFailure => Err(CErr::ConnectionNotAllowed),
            NetworkUnreachable => Err(CErr::NetworkUnreachable),
            HostUnreachable => Err(CErr::HostUnreachable),
            ConnectionRefused => Err(CErr::ConnectionRefused),
            TtlExpired => Err(CErr::TtlExpired),
            CommandNotSupported => Err(CErr::CommandNotSupported),
            AddrTypeNotSupported => Err(CErr::AddrTypeNotSupported),
        }
    }
}

impl ResponseCode {
    pub fn code(&self) -> u8 {
        *self as u8
    }

    pub fn from_u8(code: u8) -> Result<Self, TryFromU8Error> {
        match code {
            0 => Ok(ResponseCode::Success),
            1 => Ok(ResponseCode::Failure),
            2 => Ok(ResponseCode::RuleFailure),
            3 => Ok(ResponseCode::NetworkUnreachable),
            4 => Ok(ResponseCode::HostUnreachable),
            5 => Ok(ResponseCode::ConnectionRefused),
            6 => Ok(ResponseCode::TtlExpired),
            7 => Ok(ResponseCode::CommandNotSupported),
            8 => Ok(ResponseCode::AddrTypeNotSupported),
            c => Err(TryFromU8Error {
                value: c,
                to: "ResponseCode".to_owned(),
            }),
        }
    }
}

impl fmt::Display for ResponseCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ResponseCode::*;
        match self {
            Success => write!(f, "succeeded"),
            Failure => write!(f, "general SOCKS server failure"),
            RuleFailure => write!(f, "connection now allowed by ruleset"),
            NetworkUnreachable => write!(f, "Network unreachable"),
            HostUnreachable => write!(f, "Host unreachable"),
            ConnectionRefused => write!(f, "Connection refused"),
            TtlExpired => write!(f, "TTL expired"),
            CommandNotSupported => write!(f, "Command not supported"),
            AddrTypeNotSupported => write!(f, "Address type not supported"),
        }
    }
}

/// Client Authentication Methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthMethods {
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

impl AuthMethods {
    pub fn code(&self) -> u8 {
        use AuthMethods::*;
        match self {
            NoAuth => 0x00,
            GssApi => 0x01,
            UserPass => 0x02,
            IANAMethod(c) => *c,
            Private(c) => *c,
            NoMethods => 0xff,
        }
    }
}

impl From<AuthMethods> for model::Method {
    fn from(methods: AuthMethods) -> Self {
        use model::Method::*;
        match methods {
            AuthMethods::NoAuth => NoAuth,
            AuthMethods::GssApi => GssApi,
            AuthMethods::UserPass => UserPass,
            AuthMethods::IANAMethod(c) => IANAMethod(c),
            AuthMethods::Private(c) => Private(c),
            AuthMethods::NoMethods => NoMethods,
        }
    }
}

impl From<model::Method> for AuthMethods {
    fn from(method: model::Method) -> Self {
        use AuthMethods::*;
        match method {
            model::Method::NoAuth => NoAuth,
            model::Method::GssApi => GssApi,
            model::Method::UserPass => UserPass,
            model::Method::IANAMethod(c) => IANAMethod(c),
            model::Method::Private(c) => Private(c),
            model::Method::NoMethods => NoMethods,
        }
    }
}

impl From<u8> for AuthMethods {
    fn from(code: u8) -> Self {
        use AuthMethods::*;
        match code {
            0x00 => NoAuth,
            0x01 => GssApi,
            0x02 => UserPass,
            0x03..=0x7F => IANAMethod(code),
            0x80..=0xFE => Private(code),
            0xFF => NoMethods,
        }
    }
}

impl fmt::Display for AuthMethods {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use AuthMethods::*;
        match self {
            NoAuth => write!(f, "No Authentication Required"),
            GssApi => write!(f, "GSSAPI"),
            UserPass => write!(f, "Username/Password"),
            IANAMethod(c) => write!(f, "IANA Assigned: {:#X}", c),
            Private(c) => write!(f, "Private Methods: {:#X}", c),
            NoMethods => write!(f, "No Acceptable Methods"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TryFromU8Error {
    /// source value
    value: u8,
    /// target type
    to: String,
}

impl fmt::Display for TryFromU8Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "try from u8({:#X}) error to {}", self.value, self.to)
    }
}

impl std::error::Error for TryFromU8Error {
    fn description(&self) -> &str {
        "TryFromU8Error"
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

/// ATYP
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AddrType {
    V4 = 0x01,
    Domain = 0x03,
    V6 = 0x04,
}

impl TryFrom<u8> for AddrType {
    type Error = TryFromU8Error;
    /// Parse Byte to Command
    fn try_from(n: u8) -> Result<AddrType, Self::Error> {
        match n {
            1 => Ok(AddrType::V4),
            3 => Ok(AddrType::Domain),
            4 => Ok(AddrType::V6),
            _ => Err(TryFromU8Error {
                value: n,
                to: "protocol::AddrType".to_owned(),
            }),
        }
    }
}

impl fmt::Display for AddrType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use AddrType::*;
        match self {
            V4 => write!(f, "Version4 IP Address"),
            Domain => write!(f, "Fully Qualified Domain Name"),
            V6 => write!(f, "Version6 IP Address"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Addr {
    IpAddr(IpAddr),
    Domain(Vec<u8>),
}

impl From<IpAddr> for Addr {
    fn from(addr: IpAddr) -> Self {
        Addr::IpAddr(addr)
    }
}

impl From<model::Address> for Addr {
    fn from(addr: model::Address) -> Self {
        match addr {
            model::Address::IpAddr(addr, _) => Addr::IpAddr(addr),
            model::Address::Domain(domain, _) => Addr::Domain(domain.as_bytes().to_vec()),
        }
    }
}

/// SOCK5 CMD Type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SockCommand {
    Connect = 0x01,
    Bind = 0x02,
    UdpAssociate = 0x3,
}

impl From<SockCommand> for model::Command {
    fn from(cmd: SockCommand) -> Self {
        use SockCommand::*;
        match cmd {
            Connect => model::Command::Connect,
            Bind => model::Command::Bind,
            UdpAssociate => model::Command::UdpAssociate,
        }
    }
}

impl From<model::Command> for SockCommand {
    fn from(cmd: model::Command) -> Self {
        use SockCommand::*;
        match cmd {
            model::Command::Connect => Connect,
            model::Command::Bind => Bind,
            model::Command::UdpAssociate => UdpAssociate,
        }
    }
}

impl TryFrom<u8> for SockCommand {
    type Error = TryFromU8Error;
    /// Parse Byte to Command
    fn try_from(n: u8) -> Result<SockCommand, Self::Error> {
        match n {
            1 => Ok(SockCommand::Connect),
            2 => Ok(SockCommand::Bind),
            3 => Ok(SockCommand::UdpAssociate),
            _ => Err(TryFromU8Error {
                value: n,
                to: "protocol::SockCommand".to_owned(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MethodCandidates {
    pub ver: ProtocolVersion,
    pub methods: Vec<AuthMethods>,
}

impl From<MethodCandidates> for model::MethodCandidates {
    fn from(candidates: MethodCandidates) -> Self {
        model::MethodCandidates {
            version: candidates.ver,
            method: candidates.methods.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<model::MethodCandidates> for MethodCandidates {
    fn from(candidates: model::MethodCandidates) -> Self {
        MethodCandidates {
            ver: candidates.version,
            methods: candidates.method.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MethodSelection {
    pub ver: ProtocolVersion,
    pub method: AuthMethods,
}

impl From<model::MethodSelection> for MethodSelection {
    fn from(select: model::MethodSelection) -> Self {
        MethodSelection {
            ver: select.version,
            method: select.method.into(),
        }
    }
}

impl From<MethodSelection> for model::MethodSelection {
    fn from(select: MethodSelection) -> Self {
        model::MethodSelection {
            version: select.ver,
            method: select.method.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnectRequest {
    pub ver: ProtocolVersion,
    pub cmd: SockCommand,
    pub rsv: u8,
    pub atyp: AddrType,
    pub dst_addr: Addr,
    pub dst_port: u16,
}

/// aux for impl TryFrom to model::Address
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddrTriple {
    atyp: AddrType,
    addr: Addr,
    port: u16,
}

impl AddrTriple {
    pub fn new(atyp: AddrType, addr: Addr, port: u16) -> Self {
        Self { atyp, addr, port }
    }
}

impl TryFrom<AddrTriple> for model::Address {
    type Error = TryFromAddress;

    fn try_from(addr: AddrTriple) -> Result<Self, Self::Error> {
        use AddrType::*;
        let AddrTriple { atyp, addr, port } = addr;
        match (atyp, addr) {
            (V4, Addr::IpAddr(addr @ IpAddr::V4(_))) => Ok(model::Address::IpAddr(addr, port)),
            (V6, Addr::IpAddr(addr @ IpAddr::V6(_))) => Ok(model::Address::IpAddr(addr, port)),
            (Domain, Addr::Domain(domain)) => Ok(model::Address::Domain(
                String::from_utf8_lossy(&domain).to_string(),
                port,
            )),
            (atyp, addr) => Err(TryFromAddress { atyp, addr, port }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TryFromAddress {
    atyp: AddrType,
    addr: Addr,
    port: u16,
}

impl fmt::Display for TryFromAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "try_from address({}, {:?}, {})",
            self.atyp, self.addr, self.port
        )
    }
}

impl std::error::Error for TryFromAddress {
    fn description(&self) -> &str {
        "TryFromAddress"
    }

    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<TryFromAddress> for model::Error {
    fn from(err: TryFromAddress) -> Self {
        model::ErrorKind::message_fmt(format_args!("{}", err)).into()
    }
}

impl TryFrom<ConnectRequest> for model::ConnectRequest {
    type Error = TryFromAddress;
    fn try_from(req: ConnectRequest) -> Result<Self, Self::Error> {
        let dst = AddrTriple::new(req.atyp, req.dst_addr, req.dst_port).try_into()?;
        Ok(model::ConnectRequest {
            version: req.ver,
            command: req.cmd.into(),
            connect_to: dst,
        })
    }
}

impl From<model::ConnectRequest> for ConnectRequest {
    fn from(req: model::ConnectRequest) -> Self {
        use model::Address as A;
        let (atyp, dst_addr, dst_port) = match req.connect_to {
            A::IpAddr(addr @ IpAddr::V4(_), port) => (AddrType::V4, Addr::IpAddr(addr), port),
            A::IpAddr(addr @ IpAddr::V6(_), port) => (AddrType::V6, Addr::IpAddr(addr), port),
            A::Domain(addr, port) => (
                AddrType::Domain,
                Addr::Domain(addr.as_bytes().to_vec()),
                port,
            ),
        };
        ConnectRequest {
            ver: req.version,
            cmd: req.command.into(),
            rsv: 0,
            atyp,
            dst_addr,
            dst_port,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnectReply {
    pub ver: ProtocolVersion,
    pub rep: ResponseCode,
    pub rsv: u8,
    pub atyp: AddrType,
    pub bnd_addr: Addr,
    pub bnd_port: u16,
}

impl TryFrom<ConnectReply> for model::ConnectReply {
    type Error = TryFromAddress;
    fn try_from(rep: ConnectReply) -> Result<Self, Self::Error> {
        Ok(model::ConnectReply {
            version: rep.ver,
            connect_result: rep.rep.into(),
            server_addr: AddrTriple::new(rep.atyp, rep.bnd_addr, rep.bnd_port).try_into()?,
        })
    }
}

impl From<model::ConnectReply> for ConnectReply {
    fn from(rep: model::ConnectReply) -> Self {
        use model::Address as A;
        let (atyp, addr, port) = match rep.server_addr {
            A::IpAddr(addr @ IpAddr::V4(_), port) => (AddrType::V4, Addr::IpAddr(addr), port),
            A::IpAddr(addr @ IpAddr::V6(_), port) => (AddrType::V6, Addr::IpAddr(addr), port),
            A::Domain(addr, port) => (
                AddrType::Domain,
                Addr::Domain(addr.as_bytes().to_vec()),
                port,
            ),
        };
        ConnectReply {
            ver: rep.version,
            rep: rep.connect_result.into(),
            rsv: 0u8,
            atyp,
            bnd_addr: addr,
            bnd_port: port,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct UdpHeader {
    pub rsv: u16,
    /// fragment number
    pub frag: u8,
    pub atyp: AddrType,
    pub dst_addr: Addr,
    pub dst_port: u16,
}
