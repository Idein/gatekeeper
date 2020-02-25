use std::net::{SocketAddr, TcpStream};

use crate::byte_stream::ByteStream;
use crate::error::Error;

pub trait Connector: Send {
    type Stream: ByteStream;
    fn connect(&self, addr: SocketAddr) -> Result<Self::Stream, Error>;
}

#[derive(Debug, Clone)]
pub struct TcpConnector {}

impl Connector for TcpConnector {
    type Stream = TcpStream;
    fn connect(&self, addr: SocketAddr) -> Result<Self::Stream, Error> {
        TcpStream::connect(addr).map_err(Into::into)
    }
}
