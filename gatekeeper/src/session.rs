use log::*;

use crate::auth_service::*;
use crate::byte_stream::ByteStream;
use crate::connector::Connector;
use crate::error::Error;
use crate::method_selector::MethodSelector;
use crate::relay_connector::RelayConnector;
use crate::rw_socks_stream::ReadWriteStream;

use model::dao::*;
use model::model::*;

pub struct Session<C, D, S> {
    pub version: ProtocolVersion,
    /// connection from client for auth method negotiation
    pub src_conn: Option<C>,
    pub src_addr: SocketAddr,
    pub dst_connector: D,
    pub method_selector: S,
}

impl<C, D, S> Session<C, D, S>
where
    C: ByteStream + 'static,
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
            src_conn: Some(src_conn),
            src_addr,
            dst_connector,
            method_selector,
        }
    }

    pub fn start(&mut self) -> std::result::Result<(), Error> {
        let (src_conn, selection) = {
            let mut src_conn = self.src_conn.take().unwrap();
            let mut strm = ReadWriteStream::new(&mut src_conn);
            let candidates = strm.recv_method_candidates()?;
            trace!("candidates: {:?}", candidates);

            let selection = self.method_selector.select(&candidates.method)?;
            trace!("selection: {:?}", selection);

            let method = selection
                .as_ref()
                .map(|(m, _)| m)
                .unwrap_or(&Method::NoMethods);
            strm.send_method_selection(MethodSelection {
                version: self.version,
                method: *method,
            })?;

            (src_conn, selection)
        };

        let relay_conn = if let Some((m, s)) = selection {
            s.auth(src_conn)?
        } else {
            RelayConnector::new(src_conn)
        };
        trace!("relay_conn: {:?}", relay_conn);

        unimplemented!("Session::start");
    }
}
