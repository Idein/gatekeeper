///! Server control command
///!
use std::fmt;
use std::net::SocketAddr;

pub enum ServerCommand<T> {
    /// terminate
    Terminate,
    /// connected stream and client address
    Connect(T, SocketAddr),
}

impl<T> fmt::Debug for ServerCommand<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ServerCommand::*;
        match self {
            Terminate => write!(f, "Terminate"),
            Connect(_, addr) => write!(f, "Connect(_, {})", addr),
        }
    }
}
