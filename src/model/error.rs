use std::fmt;
use std::sync;

use crate::model::*;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error")]
    Io(#[source] anyhow::Error),
    #[error("poisoned error: {}", _0)]
    Poisoned(String),
    #[error("disconnected channel error: {}", name)]
    Disconnected { name: String },
    #[error("message format error: {}", message)]
    MessageFormat { message: String },
    #[error("authentication error: general")]
    Authentication,
    #[error("authentication error: no acceptable method")]
    NoAcceptableMethod,
    #[error("authentication error: unrecognized username/password")]
    UnrecognizedUsernamePassword,
    #[error("command not supported: {:?}", cmd)]
    CommandNotSupported { cmd: Command },
    #[error("host unreachable: {}:{}", host, port)]
    HostUnreachable { host: String, port: u16 },
    #[error("name not resolved: {}:{}", domain, port)]
    DomainNotResolved { domain: String, port: u16 },
    #[error("packet size limit exceeded: {} > {}", size, limit)]
    PacketSizeLimitExceeded { size: usize, limit: usize },
    #[error("address already in use: {}", addr)]
    AddressAlreadInUse { addr: SocketAddr },
    #[error("address not available: {}", addr)]
    AddressNotAvailable { addr: SocketAddr },
    /// rejected by gatekeeper
    #[error("connection not allowed: {}: {}", addr, protocol)]
    ConnectionNotAllowed { addr: Address, protocol: L4Protocol },
    /// rejected by external server
    #[error("connection refused: {}: {}", addr, protocol)]
    ConnectionRefused { addr: Address, protocol: L4Protocol },
}

impl Error {
    pub fn disconnected<S: Into<String>>(name: S) -> Self {
        Self::Disconnected { name: name.into() }
    }

    pub fn message_fmt(message: fmt::Arguments) -> Self {
        Self::MessageFormat {
            message: message.to_string(),
        }
    }

    pub fn command_not_supported(cmd: Command) -> Self {
        Self::CommandNotSupported { cmd }
    }

    pub fn connection_not_allowed(addr: Address, protocol: L4Protocol) -> Self {
        Self::ConnectionNotAllowed { addr, protocol }
    }

    pub fn connection_refused(addr: Address, protocol: L4Protocol) -> Self {
        Self::ConnectionRefused { addr, protocol }
    }

    pub fn cerr(&self) -> ConnectError {
        use Error::*;
        use ConnectError as CErr;
        match self {
            Io(_) => CErr::ServerFailure,
            Poisoned(_) => CErr::ServerFailure,
            Disconnected { .. } => CErr::ServerFailure,
            MessageFormat { .. } => CErr::ServerFailure,
            Authentication => CErr::ConnectionNotAllowed,
            NoAcceptableMethod => CErr::ConnectionNotAllowed,
            UnrecognizedUsernamePassword => CErr::ConnectionNotAllowed,
            CommandNotSupported { .. } => CErr::CommandNotSupported,
            HostUnreachable { .. } => CErr::HostUnreachable,
            DomainNotResolved { .. } => CErr::NetworkUnreachable,
            PacketSizeLimitExceeded { .. } => CErr::ServerFailure,
            AddressAlreadInUse { .. } => CErr::ServerFailure,
            AddressNotAvailable { .. } => CErr::ServerFailure,
            ConnectionNotAllowed { .. } => CErr::ConnectionNotAllowed,
            ConnectionRefused { .. } => CErr::ConnectionRefused,
        }
    }
}

impl<T: fmt::Debug> From<sync::PoisonError<T>> for Error {
    fn from(error: sync::PoisonError<T>) -> Self {
        Error::Poisoned(format!("{:?}", error)).into()
    }
}
