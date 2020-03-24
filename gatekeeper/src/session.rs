use std::ops::Deref;

use log::*;

use crate::auth_service::AuthService;
use crate::byte_stream::{BoxedStream, ByteStream};
use crate::connector::Connector;
use crate::error::Error;
use crate::relay;
use crate::rw_socks_stream::ReadWriteStream;

use model::dao::*;
use model::error::ErrorKind;
use model::model::*;

pub struct Session<D, S> {
    pub version: ProtocolVersion,
    pub dst_connector: D,
    pub authorizer: S,
    pub server_addr: SocketAddr,
    pub conn_rule: ConnectRule,
}

impl<D, S> Session<D, S>
where
    D: Connector,
    S: AuthService,
{
    pub fn new(
        version: ProtocolVersion,
        dst_connector: D,
        authorizer: S,
        server_addr: SocketAddr,
        conn_rule: ConnectRule,
    ) -> Self {
        Self {
            version,
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

    pub fn start<'a>(
        &mut self,
        _addr: SocketAddr,
        src_conn: impl ByteStream + 'a,
    ) -> std::result::Result<(), Error> {
        let relay = authorize(self.version, &self.authorizer, src_conn)?;

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
                    // filter out request not sufficies the connection rule
                    strm.send_connect_reply(self.connect_reply(Err(err.kind().clone())))?;
                    return Ok(());
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

fn authorize<'a>(
    version: ProtocolVersion,
    auth: impl Deref<Target = impl AuthService>,
    mut src_conn: impl ByteStream + 'a,
) -> Result<BoxedStream<'a>, model::Error> {
    let mut strm = ReadWriteStream::new(&mut src_conn);
    let candidates = strm.recv_method_candidates()?;
    trace!("candidates: {:?}", candidates);

    let selection = auth.select(&candidates.method)?;
    trace!("selection: {:?}", selection);

    match selection {
        Some(method) => {
            strm.send_method_selection(MethodSelection { version, method })?;
            auth.authorize(method, src_conn)
        }
        None => {
            // no acceptable method
            strm.send_method_selection(MethodSelection {
                version,
                method: Method::NoMethods,
            })?;
            Err(ErrorKind::NoAcceptableMethod.into())
        }
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
