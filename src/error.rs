#![allow(non_local_definitions)]

use thiserror::Error;

use crate::model;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
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

impl From<model::Error> for Error {
    fn from(err: model::Error) -> Self {
        match err {
            model::Error::Io(io_err) => Error::Io(io_err),
            model::Error::Poisoned(_) => Error::Unknown,
            model::Error::Disconnected { .. } => Error::Unknown,
            model::Error::MessageFormat { .. } => Error::Unknown,
            model::Error::Authentication => Error::Auth,
            model::Error::NoAcceptableMethod => Error::NotSupported,
            model::Error::UnrecognizedUsernamePassword => Error::Auth,
            model::Error::CommandNotSupported { .. } => Error::NotSupported,
            model::Error::HostUnreachable { .. } => Error::Unknown,
            model::Error::DomainNotResolved { .. } => Error::Unknown,
            model::Error::PacketSizeLimitExceeded { .. } => Error::Unknown,
            model::Error::AddressAlreadInUse { .. } => Error::Unknown,
            model::Error::AddressNotAvailable { .. } => Error::Unknown,
            model::Error::ConnectionNotAllowed { .. } => Error::NotAllowed,
            model::Error::ConnectionRefused { .. } => Error::Unknown,
            model::Error::Unknown(_) => Error::Unknown,
        }
    }
}
