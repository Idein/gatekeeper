use std::ops::{Deref, DerefMut};
use std::thread::JoinHandle;

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

#[derive(Debug)]
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

    fn connect_reply(&self, connect_result: Result<(), ConnectError>) -> ConnectReply {
        ConnectReply {
            version: self.version,
            connect_result,
            server_addr: self.server_addr.clone().into(),
        }
    }

    fn make_session<'a>(
        &mut self,
        mut src_conn: impl ByteStream + 'a,
    ) -> Result<(JoinHandle<()>, JoinHandle<()>), model::Error> {
        let mut socks = ReadWriteStream::new(&mut src_conn);

        let select = negotiate_auth_method(self.version, &self.authorizer, &mut socks)?;
        debug!("auth method: {:?}", select);
        let mut socks = ReadWriteStream::new(self.authorizer.authorize(select.method, src_conn)?);

        let req = socks.recv_connect_request()?;
        debug!("connect request: {:?}", req);

        let conn = match perform_command(
            req.command,
            &self.dst_connector,
            &self.conn_rule,
            req.connect_to.clone(),
        ) {
            Ok(conn) => {
                info!("connected: {}", req.connect_to);
                socks.send_connect_reply(self.connect_reply(Ok(())))?;
                conn
            }
            Err(err) => {
                error!("command error: {}", err);
                trace!("command error: {:?}", err);
                // reply error
                socks.send_connect_reply(self.connect_reply(Err(err.cerr())))?;
                Err(err)?
            }
        };

        relay::spawn_relay(socks.into_inner(), conn)
    }

    pub fn start<'a>(
        &mut self,
        _addr: SocketAddr,
        src_conn: impl ByteStream + 'a,
    ) -> Result<(), Error> {
        if let Err(err) = self.make_session(src_conn) {
            error!("session error: {}", err);
            trace!("session error: {:?}", err);
            return Err(err.into());
        }
        Ok(())
    }
}

fn perform_command(
    cmd: Command,
    connector: impl Deref<Target = impl Connector>,
    rule: &ConnectRule,
    connect_to: Address,
) -> Result<impl ByteStream, model::Error> {
    match cmd {
        Command::Connect => {}
        cmd @ Command::Bind | cmd @ Command::UdpAssociate => {
            Err(ErrorKind::command_not_supported(cmd))?
        }
    };
    // filter out request not sufficies the connection rule
    check_rule(rule, connect_to.clone(), L4Protocol::Tcp)?;
    connector.connect_byte_stream(connect_to)
}

fn negotiate_auth_method<'a>(
    version: ProtocolVersion,
    auth: impl Deref<Target = impl AuthService>,
    mut socks: impl DerefMut<Target = impl SocksStream>,
) -> Result<MethodSelection, model::Error> {
    let candidates = socks.recv_method_candidates()?;
    trace!("candidates: {:?}", candidates);

    let selection = auth.select(&candidates.method)?;
    trace!("selection: {:?}", selection);

    let method_sel = MethodSelection {
        version,
        method: selection.unwrap_or(Method::NoMethods),
    };
    socks.send_method_selection(method_sel)?;
    match method_sel.method {
        Method::NoMethods => Err(ErrorKind::NoAcceptableMethod.into()),
        _ => Ok(method_sel),
    }
}

fn check_rule(rule: &ConnectRule, addr: Address, proto: L4Protocol) -> Result<(), model::Error> {
    if rule.check(addr.clone(), proto) {
        Ok(())
    } else {
        Err(model::ErrorKind::connection_not_allowed(addr, proto).into())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::auth_service::test::RejectService;
    use crate::byte_stream::test::BufferStream;
    use crate::connector::test::BufferConnector;
    use crate::rw_socks_stream as socks;
    use std::io;
    use std::str::FromStr;

    #[test]
    fn no_acceptable_method() {
        let req = ConnectRequest {
            version: 5.into(),
            command: Command::Connect,
            connect_to: Address::from_str("192.168.0.1:5123").unwrap(),
        };
        let mut session = Session::new(
            5.into(),
            BufferConnector {
                addrs: vec![req.connect_to.clone()],
                rd_buff: vec![],
                wr_buff: vec![],
            },
            RejectService,
            "0.0.0.0:1080".parse().unwrap(),
            ConnectRule::any(),
        );
        println!("session: {:?}", session);
        let mut src = BufferStream::new(vec![5, 1, 0].into(), vec![].into());
        assert_eq!(
            session.make_session(src).unwrap_err().kind(),
            &ErrorKind::NoAcceptableMethod
        );
    }

    #[test]
    fn command_not_supported() {
        use crate::auth_service::NoAuthService;
        let mcand = MethodCandidates {
            version: 5.into(),
            method: vec![model::Method::NoAuth],
        };
        let req = ConnectRequest {
            version: 5.into(),
            // udp is not unsupported
            command: Command::UdpAssociate,
            connect_to: Address::from_str("192.168.0.1:5123").unwrap(),
        };
        let mut session = Session::new(
            5.into(),
            BufferConnector {
                addrs: vec![req.connect_to.clone()],
                rd_buff: vec![],
                wr_buff: vec![],
            },
            NoAuthService::new(),
            "0.0.0.0:1080".parse().unwrap(),
            ConnectRule::any(),
        );
        println!("session: {:?}", session);

        let mut buff = {
            let mut cursor = io::Cursor::new(vec![]);
            socks::test::write_method_candidates(&mut cursor, &mcand.into()).unwrap();
            socks::test::write_connect_request(&mut cursor, &req.into()).unwrap();
            cursor.into_inner()
        };
        let mut src = BufferStream::new(buff.into(), vec![].into());
        assert_eq!(
            session.make_session(src).unwrap_err().kind(),
            &ErrorKind::command_not_supported(Command::UdpAssociate)
        );
    }
}
