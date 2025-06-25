#![allow(non_local_definitions)]
use std::fmt;
use std::sync;

use thiserror::Error;

use crate::model::*;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("io error")]
    Io,
    #[error("poisoned error: {0}")]
    Poisoned(String),
    #[error("disconnected channel error: {name}")]
    Disconnected { name: String },
    #[error("message format error: {message}")]
    MessageFormat { message: String },
    #[error("authentication error: general")]
    Authentication,
    #[error("authentication error: no acceptable method")]
    NoAcceptableMethod,
    #[error("authentication error: unrecognized username/password")]
    UnrecognizedUsernamePassword,
    #[error("command not supported: {cmd:?}")]
    CommandNotSupported { cmd: Command },
    #[error("host unreachable: {host}:{port}")]
    HostUnreachable { host: String, port: u16 },
    #[error("name not resolved: {domain}:{port}")]
    DomainNotResolved { domain: String, port: u16 },
    #[error("packet size limit exceeded: {size} > {limit}")]
    PacketSizeLimitExceeded { size: usize, limit: usize },
    #[error("address already in use: {addr}")]
    AddressAlreadInUse { addr: SocketAddr },
    #[error("address not available: {addr}")]
    AddressNotAvailable { addr: SocketAddr },
    /// rejected by gatekeeper
    #[error("connection not allowed: {addr}: {protocol}")]
    ConnectionNotAllowed { addr: Address, protocol: L4Protocol },
    /// rejected by external server
    #[error("connection refused: {addr}: {protocol}")]
    ConnectionRefused { addr: Address, protocol: L4Protocol },
}

impl Error {
    pub fn disconnected<S: Into<String>>(name: S) -> Self {
        Error::Disconnected { name: name.into() }
    }

    pub fn message_fmt(message: fmt::Arguments) -> Self {
        Error::MessageFormat {
            message: message.to_string(),
        }
    }

    pub fn command_not_supported(cmd: Command) -> Self {
        Error::CommandNotSupported { cmd }
    }

    pub fn connection_not_allowed(addr: Address, protocol: L4Protocol) -> Self {
        Error::ConnectionNotAllowed { addr, protocol }
    }

    pub fn connection_refused(addr: Address, protocol: L4Protocol) -> Self {
        Error::ConnectionRefused { addr, protocol }
    }
}

impl Error {
    pub fn cerr(&self) -> ConnectError {
        use ConnectError as CErr;
        use Error as E;
        match self {
            E::Io => CErr::ServerFailure,
            E::Poisoned(_) => CErr::ServerFailure,
            E::Disconnected { .. } => CErr::ServerFailure,
            E::MessageFormat { .. } => CErr::ServerFailure,
            E::Authentication => CErr::ConnectionNotAllowed,
            E::NoAcceptableMethod => CErr::ConnectionNotAllowed,
            E::UnrecognizedUsernamePassword => CErr::ConnectionNotAllowed,
            E::CommandNotSupported { .. } => CErr::CommandNotSupported,
            E::HostUnreachable { .. } => CErr::HostUnreachable,
            E::DomainNotResolved { .. } => CErr::NetworkUnreachable,
            E::PacketSizeLimitExceeded { .. } => CErr::ServerFailure,
            E::AddressAlreadInUse { .. } => CErr::ServerFailure,
            E::AddressNotAvailable { .. } => CErr::ServerFailure,
            E::ConnectionNotAllowed { .. } => CErr::ConnectionNotAllowed,
            E::ConnectionRefused { .. } => CErr::ConnectionRefused,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(_error: std::io::Error) -> Self {
        Error::Io
    }
}

impl<T: fmt::Debug> From<sync::PoisonError<T>> for Error {
    fn from(error: sync::PoisonError<T>) -> Self {
        Error::Poisoned(format!("{:?}", error))
    }
}
