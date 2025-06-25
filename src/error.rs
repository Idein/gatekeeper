#![allow(non_local_definitions)]

use thiserror::Error;

use crate::model;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error")]
    Io,
    #[error("config error")]
    Config,
    #[error("auth error")]
    Auth,
    #[error("permission error")]
    Permission,
    #[error("not supported error")]
    NotSupported,
    #[error("not allowed error")]
    NotAllowed,
    #[error("unknown error")]
    Unknown,
}

impl From<std::io::Error> for Error {
    fn from(_error: std::io::Error) -> Self {
        Error::Io
    }
}

impl From<model::Error> for Error {
    fn from(err: model::Error) -> Self {
        match err {
            model::Error::Io => Error::Io,
            model::Error::Poisoned(_) => Error::Io,
            model::Error::Disconnected { .. } => Error::Io,
            model::Error::MessageFormat { .. } => Error::Unknown,
            model::Error::Authentication => Error::Auth,
            model::Error::NoAcceptableMethod => Error::NotSupported,
            model::Error::UnrecognizedUsernamePassword => Error::Auth,
            model::Error::CommandNotSupported { .. } => Error::NotSupported,
            model::Error::HostUnreachable { .. } => Error::Io,
            model::Error::DomainNotResolved { .. } => Error::Io,
            model::Error::PacketSizeLimitExceeded { .. } => Error::Io,
            model::Error::AddressAlreadInUse { .. } => Error::Io,
            model::Error::AddressNotAvailable { .. } => Error::Io,
            model::Error::ConnectionNotAllowed { .. } => Error::NotAllowed,
            model::Error::ConnectionRefused { .. } => Error::Io,
        }
    }
}
