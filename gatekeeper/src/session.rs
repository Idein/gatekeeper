use log::*;

use crate::auth_service::AuthService;
use crate::byte_stream::ByteStream;
use crate::connector::Connector;
use crate::error::Error;
use crate::relay;
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
    pub authorizer: S,
    pub server_addr: SocketAddr,
    pub conn_rule: ConnectRule,
}

impl<C, D, S> Session<C, D, S>
where
    C: ByteStream + 'static,
    D: Connector,
    S: AuthService,
{
    pub fn new(
        version: ProtocolVersion,
        src_conn: C,
        src_addr: SocketAddr,
        dst_connector: D,
        authorizer: S,
        server_addr: SocketAddr,
        conn_rule: ConnectRule,
    ) -> Self {
        Self {
            version,
            src_conn: Some(src_conn),
            src_addr,
            dst_connector,
            authorizer,
            server_addr,
            conn_rule,
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
        let (src_conn, method) = {
            let mut src_conn = self.src_conn.take().unwrap();
            let mut strm = ReadWriteStream::new(&mut src_conn);
            let candidates = strm.recv_method_candidates()?;
            trace!("candidates: {:?}", candidates);

            let selection = self.authorizer.select(&candidates.method)?;
            trace!("selection: {:?}", selection);

            match selection {
                Some(method) => {
                    strm.send_method_selection(MethodSelection {
                        version: self.version,
                        method: method.clone(),
                    })?;
                    (src_conn, method)
                }
                None => {
                    // no acceptable method
                    strm.send_method_selection(MethodSelection {
                        version: self.version,
                        method: Method::NoMethods,
                    })?;
                    return Err(model::Error::from(ErrorKind::NoAcceptableMethod).into());
                }
            }
        };

        let relay = self.authorizer.authorize(method, src_conn)?;

        let mut strm = ReadWriteStream::new(relay);
        let conn_req = strm.recv_connect_request()?;
        debug!("connect request: {:?}", conn_req);
        let dst_conn = match &conn_req.command {
            Command::Connect => {
                if let Err(err) = check_rule(
                    &self.conn_rule,
                    conn_req.connect_to.clone(),
                    L4Protocol::Tcp,
                ) {
                    strm.send_connect_reply(self.connect_reply(Err(err.kind().clone())))?;
                }
                match self
                    .dst_connector
                    .connect_byte_stream(conn_req.connect_to.clone())
                {
                    Ok(conn) => {
                        strm.send_connect_reply(self.connect_reply::<model::ErrorKind>(Ok(())))?;
                        conn
                    }
                    Err(err) => {
                        error!("connect error: {:?}", err);
                        strm.send_connect_reply(self.connect_reply(Err(err.kind().clone())))?;
                        return Err(err.into());
                    }
                }
            }
            cmd @ Command::Bind | cmd @ Command::UdpAssociate => {
                debug!("command not supported: {:?}", cmd);
                let not_supported: model::Error = ErrorKind::command_not_supported(*cmd).into();
                // reply error
                let rep = self.connect_reply(Err(not_supported.kind().clone()));
                strm.send_connect_reply(rep)?;
                return Err(not_supported.into());
            }
        };

        relay::spawn_relay(strm.into_inner(), dst_conn)?;
        Ok(())
    }
}

fn check_rule(rule: &ConnectRule, addr: Address, proto: L4Protocol) -> Result<(), model::Error> {
    if rule.check(addr.clone(), proto) {
        Ok(())
    } else {
        let err: model::Error = model::ErrorKind::connection_not_allowed(addr, proto).into();
        error!("connection rule: {}", err);
        Err(err)
    }
}
