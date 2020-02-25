use std::net::SocketAddr;

use crate::byte_stream::ByteStream;
use crate::connector::Connector;
use crate::error::Error;
use crate::method_selector::MethodSelector;

pub struct Session<C, D, S> {
    pub src_conn: C,
    pub src_addr: SocketAddr,
    pub dst_connector: D,
    pub method_selector: S,
}

impl<C, D, S> Session<C, D, S>
where
    C: ByteStream,
    D: Connector,
    S: MethodSelector,
{
    pub fn new(src_conn: C, src_addr: SocketAddr, dst_connector: D, method_selector: S) -> Self {
        Self {
            src_conn,
            src_addr,
            dst_connector,
            method_selector,
        }
    }

    pub fn start(&self) -> Result<(), Error> {
        unimplemented!("Session::start");
    }
}
