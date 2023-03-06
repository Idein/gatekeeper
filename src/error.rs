use crate::model;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error")]
    Io(#[source] anyhow::Error),
    #[error("config error")]
    Config(#[source] anyhow::Error),
    #[error("auth error")]
    Auth(#[source] anyhow::Error),
    #[error("permission error")]
    Permission(#[source] anyhow::Error),
    #[error("not supported error")]
    NotSupported(#[source] anyhow::Error),
    #[error("not allowed error")]
    NotAllowed(#[source] anyhow::Error),
    #[error("unknown error")]
    Unknown(#[source] anyhow::Error),
}

impl From<model::Error> for Error {
    fn from(err: model::Error) -> Self {
        use model::Error as K;
        match err {
            K::Io(..) => Error::Io(err.into()),
            K::Poisoned(..) => Error::Io(err.into()),
            K::Disconnected { .. } => Error::Io(err.into()),
            K::MessageFormat { .. } => Error::Unknown(err.into()),
            K::Authentication => Error::Auth(err.into()),
            K::NoAcceptableMethod => Error::NotSupported(err.into()),
            K::UnrecognizedUsernamePassword => Error::Auth(err.into()),
            K::CommandNotSupported { .. } => Error::NotSupported(err.into()),
            K::HostUnreachable { .. } => Error::Io(err.into()),
            K::DomainNotResolved { .. } => Error::Io(err.into()),
            K::PacketSizeLimitExceeded { .. } => Error::Io(err.into()),
            K::AddressAlreadInUse { .. } => Error::Io(err.into()),
            K::AddressNotAvailable { .. } => Error::Io(err.into()),
            K::ConnectionNotAllowed { .. } => Error::NotAllowed(err.into()),
            K::ConnectionRefused { .. } => Error::Io(err.into()),
        }
    }
}
