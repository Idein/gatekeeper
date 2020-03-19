use std::fmt;
use std::fmt::Display;

use failure::{Backtrace, Context, Fail};

use crate::model::*;

pub type Result<T> = ::std::result::Result<T, Error>;

#[derive(Fail, Debug, Clone)]
pub enum ErrorKind {
    #[fail(display = "io error")]
    Io,
    #[fail(display = "message format error: {}", message)]
    MessageFormat { message: String },
    #[fail(display = "authentication error: general")]
    Authentication,
    #[fail(display = "authentication error: no acceptable method")]
    NoAcceptableMethod,
    #[fail(display = "authentication error: unrecognized username/password")]
    UnrecognizedUsernamePassword,
    #[fail(display = "command not supported: {:?}", cmd)]
    CommandNotSupported { cmd: Command },
    #[fail(display = "host unreachable: {}:{}", host, port)]
    HostUnreachable { host: String, port: u16 },
    #[fail(display = "name not resolved: {}:{}", domain, port)]
    DomainNotResolved { domain: String, port: u16 },
    #[fail(display = "packet size limit exceeded: {} > {}", size, limit)]
    PacketSizeLimitExceeded { size: usize, limit: usize },
    #[fail(display = "address already in use: {}", addr)]
    AddressAlreadInUse { addr: SocketAddr },
    #[fail(display = "address not available: {}", addr)]
    AddressNotAvailable { addr: SocketAddr },
    #[fail(display = "connection not allowed: {}: {}", addr, protocol)]
    ConnectionNotAllowed { addr: Address, protocol: L4Protocol },
}

impl ErrorKind {
    pub fn message_fmt(message: fmt::Arguments) -> Self {
        ErrorKind::MessageFormat {
            message: message.to_string(),
        }
    }

    pub fn command_not_supported(cmd: Command) -> Self {
        ErrorKind::CommandNotSupported { cmd }
    }

    pub fn connection_not_allowed(addr: Address, protocol: L4Protocol) -> Self {
        ErrorKind::ConnectionNotAllowed { addr, protocol }
    }
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
