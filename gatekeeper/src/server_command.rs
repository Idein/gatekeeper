///! Server control command
///!
use std::fmt;
use std::net::SocketAddr;

use crate::byte_stream::BoxedStream;

pub enum ServerCommand {
    /// terminate
    Terminate,
    /// connected stream and client address
    Connect(BoxedStream, SocketAddr),
}

impl fmt::Debug for ServerCommand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ServerCommand::*;
        match self {
            Terminate => write!(f, "Terminate"),
            Connect(_, addr) => write!(f, "Connect(_, {})", addr),
        }
    }
}
