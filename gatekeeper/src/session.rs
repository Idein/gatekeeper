use std::ops::{Deref, DerefMut};
use std::sync::mpsc;
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
    // termination message receiver
    rx: mpsc::Receiver<()>,
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
    ) -> (Self, mpsc::SyncSender<()>) {
        let (tx, rx) = mpsc::sync_channel(1);
        (
            Self {
                version,
                dst_connector,
                authorizer,
                server_addr,
                conn_rule,
                rx,
            },
            tx,
        )
    }

    fn connect_reply(&self, connect_result: Result<(), ConnectError>) -> ConnectReply {
        ConnectReply {
            version: self.version,
            connect_result,
            server_addr: self.server_addr.clone().into(),
        }
    }

    fn make_session<'a>(
        self,
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

        relay::spawn_relay(socks.into_inner(), conn, self.rx)
    }

    pub fn start<'a>(
        self,
        _addr: SocketAddr,
        src_conn: impl ByteStream + 'a,
    ) -> Result<(JoinHandle<()>, JoinHandle<()>), Error> {
        self.make_session(src_conn).map_err(|err| {
            error!("session error: {}", err);
            trace!("session error: {:?}", err);
            err.into()
        })
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
        let (session, _) = Session::new(
            5.into(),
            BufferConnector::<BufferStream> {
                strms: vec![(
                    req.connect_to.clone(),
                    Ok(BufferStream::new(vec![].into(), vec![].into())),
                )]
                .into_iter()
                .collect(),
            },
            RejectService,
            "0.0.0.0:1080".parse().unwrap(),
            ConnectRule::any(),
        );
        println!("session: {:?}", session);
        let src = BufferStream::new(vec![5, 1, 0].into(), vec![].into());
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
        let (session, _) = Session::new(
            5.into(),
            BufferConnector::<BufferStream> {
                strms: vec![(
                    req.connect_to.clone(),
                    Ok(BufferStream::new(vec![].into(), vec![].into())),
                )]
                .into_iter()
                .collect(),
            },
            NoAuthService::new(),
            "0.0.0.0:1080".parse().unwrap(),
            ConnectRule::any(),
        );
        println!("session: {:?}", session);

        let buff = {
            let mut cursor = io::Cursor::new(vec![]);
            socks::test::write_method_candidates(&mut cursor, mcand).unwrap();
            socks::test::write_connect_request(&mut cursor, req).unwrap();
            cursor.into_inner()
        };
        let src = BufferStream::new(buff.into(), vec![].into());
        assert_eq!(
            session.make_session(src).unwrap_err().kind(),
            &ErrorKind::command_not_supported(Command::UdpAssociate)
        );
    }

    #[test]
    fn connect_not_allowed() {
        use crate::auth_service::NoAuthService;
        let version: ProtocolVersion = 5.into();
        let connect_to = Address::from_str("192.168.0.1:5123").unwrap();
        let (session, _) = Session::new(
            version,
            BufferConnector::<BufferStream> {
                strms: vec![(
                    connect_to.clone(),
                    Ok(BufferStream::new(vec![].into(), vec![].into())),
                )]
                .into_iter()
                .collect(),
            },
            NoAuthService::new(),
            "0.0.0.0:1080".parse().unwrap(),
            ConnectRule::none(),
        );
        println!("session: {:?}", session);

        let buff = {
            let mut cursor = io::Cursor::new(vec![]);
            socks::test::write_method_candidates(
                &mut cursor,
                MethodCandidates {
                    version,
                    method: vec![model::Method::NoAuth],
                },
            )
            .unwrap();
            socks::test::write_connect_request(
                &mut cursor,
                ConnectRequest {
                    version,
                    command: Command::Connect,
                    connect_to: connect_to.clone(),
                },
            )
            .unwrap();
            cursor.into_inner()
        };
        let src = BufferStream::new(buff.into(), vec![].into());
        assert_eq!(
            session.make_session(src).unwrap_err().kind(),
            &ErrorKind::connection_not_allowed(connect_to, L4Protocol::Tcp)
        );
    }

    #[test]
    fn connection_refused() {
        use crate::auth_service::NoAuthService;
        let version: ProtocolVersion = 5.into();
        let connect_to = Address::from_str("192.168.0.1:5123").unwrap();
        let (session, _) = Session::new(
            version,
            BufferConnector::<BufferStream> {
                strms: vec![(connect_to.clone(), Err(ConnectError::ConnectionRefused))]
                    .into_iter()
                    .collect(),
            },
            NoAuthService::new(),
            "0.0.0.0:1080".parse().unwrap(),
            ConnectRule::any(),
        );
        println!("session: {:?}", session);

        let buff = {
            let mut cursor = io::Cursor::new(vec![]);
            socks::test::write_method_candidates(
                &mut cursor,
                MethodCandidates {
                    version,
                    method: vec![model::Method::NoAuth],
                },
            )
            .unwrap();
            socks::test::write_connect_request(
                &mut cursor,
                ConnectRequest {
                    version,
                    command: Command::Connect,
                    connect_to: connect_to.clone(),
                },
            )
            .unwrap();
            cursor.into_inner()
        };
        let src = BufferStream::new(buff.into(), vec![].into());
        assert_eq!(
            session.make_session(src).unwrap_err().kind(),
            &ErrorKind::connection_refused(connect_to, L4Protocol::Tcp)
        );
    }

    fn gen_random_vec(size: usize) -> Vec<u8> {
        use rand::distributions::Standard;
        use rand::{thread_rng, Rng};
        let rng = thread_rng();
        rng.sample_iter(Standard).take(size).collect()
    }

    fn vec_from_read<T: io::Read>(mut reader: T) -> Vec<u8> {
        let mut buff = vec![];
        reader.read_to_end(&mut buff).unwrap();
        buff
    }

    #[test]
    fn relay_contents() {
        use crate::auth_service::NoAuthService;
        use io::Write;
        let version: ProtocolVersion = 5.into();
        let connect_to = Address::Domain("example.com".into(), 5123);
        let (session, tx) = Session::new(
            version,
            BufferConnector {
                strms: vec![(
                    connect_to.clone(),
                    Ok(BufferStream::new(
                        gen_random_vec(8200).into(),
                        vec![].into(),
                    )),
                )]
                .into_iter()
                .collect(),
            },
            NoAuthService::new(),
            "0.0.0.0:1080".parse().unwrap(),
            ConnectRule::any(),
        );

        // length of SOCKS message (len MethodCandidates + len ConnectRequest)
        let input_stream_pos;
        let src = {
            // input from socks client
            let mut cursor = io::Cursor::new(vec![]);
            socks::test::write_method_candidates(
                &mut cursor,
                MethodCandidates {
                    version,
                    method: vec![model::Method::NoAuth],
                },
            )
            .unwrap();
            socks::test::write_connect_request(
                &mut cursor,
                ConnectRequest {
                    version,
                    // udp is not unsupported
                    command: Command::Connect,
                    connect_to: connect_to.clone(),
                },
            )
            .unwrap();
            input_stream_pos = cursor.position();
            // binaries from client
            cursor.write_all(&gen_random_vec(8200)).unwrap();
            BufferStream::new(cursor.into_inner().into(), vec![].into())
        };
        let dst_connector = session.dst_connector.clone();
        // start relay
        let (relay_out, relay_in) = session.make_session(src.clone()).unwrap();
        tx.send(()).unwrap();
        assert!(relay_out.join().is_ok());
        assert!(relay_in.join().is_ok());

        // check for replied command from Session to client
        {
            // read output buffer from pos(0)
            src.wr_buff().set_position(0);
            assert_eq!(
                socks::test::read_method_selection(&mut *src.wr_buff()).unwrap(),
                MethodSelection {
                    version,
                    method: model::Method::NoAuth
                }
            );
            assert_eq!(
                socks::test::read_connect_reply(&mut *src.wr_buff()).unwrap(),
                ConnectReply {
                    version,
                    connect_result: Ok(()),
                    server_addr: Address::IpAddr("0.0.0.0".parse().unwrap(), 1080),
                }
            );
        }

        // check for relayed contents
        // client <-- target
        assert_eq!(vec_from_read(&mut *src.wr_buff()), {
            let mut rd_buff = dst_connector.stream(&connect_to).rd_buff();
            rd_buff.set_position(0);
            vec_from_read(&mut *rd_buff)
        });
        // client --> target
        assert_eq!(
            {
                let mut rd_buff = src.rd_buff();
                rd_buff.set_position(input_stream_pos);
                vec_from_read(&mut *rd_buff)
            },
            {
                let mut wr_buff = dst_connector.stream(&connect_to).wr_buff();
                wr_buff.set_position(0);
                vec_from_read(&mut *wr_buff)
            }
        );
    }
}
