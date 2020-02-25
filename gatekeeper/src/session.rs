use log::*;

use crate::byte_stream::ByteStream;
use crate::connector::Connector;
use crate::error::Error;
use crate::method_selector::MethodSelector;
use crate::rw_socks_stream::ReadWriteStream;

use model::dao::*;
use model::model::*;

pub struct Session<C, D, S> {
    pub version: ProtocolVersion,
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
    pub fn new(
        version: ProtocolVersion,
        src_conn: C,
        src_addr: SocketAddr,
        dst_connector: D,
        method_selector: S,
    ) -> Self {
        Self {
            version,
            src_conn,
            src_addr,
            dst_connector,
            method_selector,
        }
    }

    pub fn start(&mut self) -> std::result::Result<(), Error> {
        let mut strm = ReadWriteStream::new(&mut self.src_conn);
        let candidates = strm.recv_method_candidates()?;
        trace!("candidates: {:?}", candidates);

        let selection = self.method_selector.select(&candidates.method)?;
        trace!("selection: {:?}", selection);

        let method = selection.map(|(m, _)| m).unwrap_or(Method::NoMethods);
        strm.send_method_selection(MethodSelection {
            version: self.version,
            method,
        })?;

        unimplemented!("Session::start");
    }
}
