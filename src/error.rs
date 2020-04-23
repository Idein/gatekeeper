use std::fmt;
use std::fmt::Display;

use failure::{Backtrace, Context, Fail};

use crate::model;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Fail, Debug)]
pub enum ErrorKind {
    #[fail(display = "io error")]
    Io,
    #[fail(display = "config error")]
    Config,
    #[fail(display = "auth error")]
    Auth,
    #[fail(display = "permission error")]
    Permission,
    #[fail(display = "not supported error")]
    NotSupported,
    #[fail(display = "not allowed error")]
    NotAllowed,
    #[fail(display = "unknown error")]
    Unknown,
}

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

impl Fail for Error {
    fn cause(&self) -> Option<&dyn Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Error {
    pub fn new(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }

    pub fn kind(&self) -> &ErrorKind {
        self.inner.get_context()
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(inner: Context<ErrorKind>) -> Error {
        Error { inner }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error {
            inner: error.context(ErrorKind::Io),
        }
    }
}

impl From<model::Error> for Error {
    fn from(err: model::Error) -> Self {
        use model::ErrorKind as K;
        let ctx = match err.kind() {
            K::Io => err.context(ErrorKind::Io),
            K::Poisoned(_) => err.context(ErrorKind::Io),
            K::Disconnected { .. } => err.context(ErrorKind::Io),
            K::MessageFormat { .. } => err.context(ErrorKind::Unknown),
            K::Authentication => err.context(ErrorKind::Auth),
            K::NoAcceptableMethod => err.context(ErrorKind::NotSupported),
            K::UnrecognizedUsernamePassword => err.context(ErrorKind::Auth),
            K::CommandNotSupported { .. } => err.context(ErrorKind::NotSupported),
            K::HostUnreachable { .. } => err.context(ErrorKind::Io),
            K::DomainNotResolved { .. } => err.context(ErrorKind::Io),
            K::PacketSizeLimitExceeded { .. } => err.context(ErrorKind::Io),
            K::AddressAlreadInUse { .. } => err.context(ErrorKind::Io),
            K::AddressNotAvailable { .. } => err.context(ErrorKind::Io),
            K::ConnectionNotAllowed { .. } => err.context(ErrorKind::NotAllowed),
            K::ConnectionRefused { .. } => err.context(ErrorKind::Io),
        };
        Error { inner: ctx }
    }
}
