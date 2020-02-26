use log::*;

use crate::auth_service::*;
use crate::byte_stream::ByteStream;
use crate::connector::Connector;
use crate::error::Error;
use crate::method_selector::MethodSelector;
use crate::relay_connector::RelayConnector;
use crate::rw_socks_stream::ReadWriteStream;

use model::dao::*;
use model::error::ErrorKind;
use model::model::*;

pub struct Session<C, D, S> {
    pub version: ProtocolVersion,
    /// connection from client for auth method negotiation
    pub src_conn: Option<C>,
    pub src_addr: SocketAddr,
    pub dst_connector: D,
    pub method_selector: S,
    pub server_addr: SocketAddr,
}

impl<C, D, S> Session<C, D, S>
where
    C: ByteStream + 'static,
    D: Connector,
    S: MethodSelector<C>,
{
    pub fn new(
        version: ProtocolVersion,
        src_conn: C,
        src_addr: SocketAddr,
        dst_connector: D,
        method_selector: S,
        server_addr: SocketAddr,
    ) -> Self {
        Self {
            version,
            src_conn: Some(src_conn),
            src_addr,
            dst_connector,
            method_selector,
            server_addr,
        }
    }

    fn connect_reply<R>(&self, connect_result: Result<(), R>) -> ConnectReply
    where
        ConnectError: From<R>,
    {
        ConnectReply {
            version: self.version,
            connect_result: connect_result.map_err(Into::into),
            server_addr: self.server_addr.clone().into(),
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

        let mut relay = auth_with_selection(src_conn, selection)?;

        let mut strm = ReadWriteStream::new(relay.tcp_stream());
        let conn_req = strm.recv_connect_request()?;
        debug!("connect request: {:?}", conn_req);
        match &conn_req.command {
            Command::Connect => {}
            cmd @ Command::Bind | cmd @ Command::UdpAssociate => {
                debug!("command not supported: {:?}", cmd);
                let cmd_not_supported: model::Error = ErrorKind::command_not_supported(*cmd).into();
                // reply error
                let rep = self.connect_reply(Err(cmd_not_supported.kind().clone()));
                strm.send_connect_reply(rep)?;
                return Err(cmd_not_supported.into());
            }
        }

        match self.dst_connector.connect(conn_req.connect_to.clone()) {
            Ok(_conn) => {}
            Err(err) => {
                error!("connect error: {:?}", err);
                strm.send_connect_reply(self.connect_reply(Err(err.kind().clone())))?;
                return Err(err.into());
            }
        }

        strm.send_connect_reply(self.connect_reply::<model::ErrorKind>(Ok(())))?;

        unimplemented!("Session::start");
    }
}
