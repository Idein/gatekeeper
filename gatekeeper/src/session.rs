use std::net::SocketAddr;

use crate::byte_stream::ByteStream;
use crate::connector::Connector;
use crate::error::Error;

pub struct Session<C, D> {
    pub src_conn: C,
    pub src_addr: SocketAddr,
    pub dst_connector: D,
}

impl<C, D> Session<C, D>
where
    C: ByteStream,
    D: Connector,
{
    pub fn new(src_conn: C, src_addr: SocketAddr, dst_connector: D) -> Self {
        Self {
            src_conn,
            src_addr,
            dst_connector,
        }
    }

    pub fn start(&self) -> Result<(), Error> {
        unimplemented!("Session::start")
    }
}
