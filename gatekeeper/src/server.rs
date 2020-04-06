use std::net::TcpStream;
use std::sync::mpsc::{self, SyncSender};
use std::thread;

use log::*;

use crate::acceptor::{Binder, TcpBinder};
use crate::auth_service::{AuthService, NoAuthService};
use crate::byte_stream::ByteStream;
use crate::config::ServerConfig;
use crate::connector::{Connector, TcpUdpConnector};
use crate::error::Error;
use crate::server_command::ServerCommand;
use crate::session::Session;

use model::{ProtocolVersion, SocketAddr};

pub struct Server<S, T, C> {
    config: ServerConfig,
    tx_cmd: mpsc::SyncSender<ServerCommand<S>>,
    rx_cmd: mpsc::Receiver<ServerCommand<S>>,
    /// bind server address
    binder: T,
    /// make connection to service host
    connector: C,
    protocol_version: ProtocolVersion,
    session: Vec<thread::JoinHandle<Result<(), Error>>>,
}

/// spawn a thread send accepted stream to `tx`
fn spawn_acceptor<S>(
    acceptor: impl Iterator<Item = (S, SocketAddr)> + Send + 'static,
    tx: SyncSender<ServerCommand<S>>,
) -> thread::JoinHandle<()>
where
    S: ByteStream + 'static,
{
    use ServerCommand::*;
    thread::spawn(move || {
        for (strm, addr) in acceptor {
            if tx.send(Connect(strm, addr)).is_err() {
                info!("disconnected ServerCommand chan");
                break;
            }
        }
    })
}

/// spawn a thread perform `Session.start`
fn spawn_session<S, D, M>(
    mut session: Session<D, M>,
    addr: SocketAddr,
    strm: S,
) -> thread::JoinHandle<Result<(), Error>>
where
    S: ByteStream + 'static,
    D: Connector + 'static,
    M: AuthService + 'static,
{
    thread::spawn(move || session.start(addr, strm))
}

impl Server<TcpStream, TcpBinder, TcpUdpConnector> {
    pub fn new(config: ServerConfig) -> (Self, mpsc::SyncSender<ServerCommand<TcpStream>>) {
        Server::<TcpStream, TcpBinder, TcpUdpConnector>::with_binder(
            config.clone(),
            TcpBinder::new(config.read_timeout, config.write_timeout),
            TcpUdpConnector,
        )
    }
}

impl<S, T, C> Server<S, T, C>
where
    S: ByteStream + 'static,
    T: Binder<Stream = S>,
    C: Connector + Clone + 'static,
{
    pub fn with_binder(
        config: ServerConfig,
        binder: T,
        connector: C,
    ) -> (Self, mpsc::SyncSender<ServerCommand<S>>) {
        let (tx, rx) = mpsc::sync_channel(0);
        (
            Self {
                config,
                tx_cmd: tx.clone(),
                rx_cmd: rx,
                binder,
                connector,
                protocol_version: ProtocolVersion::from(5),
                session: vec![],
            },
            tx,
        )
    }

    pub fn serve(&mut self) -> Result<(), Error> {
        let acceptor = self.binder.bind(self.config.server_addr())?;
        spawn_acceptor(acceptor, self.tx_cmd.clone());

        while let Ok(cmd) = self.rx_cmd.recv() {
            use ServerCommand::*;
            info!("cmd: {:?}", cmd);
            match cmd {
                Terminate => {
                    self.session.drain(..).for_each(|hnd| match hnd.join() {
                        Ok(res) => debug!("join session: {:?}", res),
                        Err(err) => error!("join session error: {:?}", err),
                    });
                    break;
                }
                Connect(stream, addr) => {
                    info!("connect from: {}", addr);
                    let session = Session::new(
                        self.protocol_version,
                        self.connector.clone(),
                        NoAuthService::new(),
                        self.config.server_addr(),
                        self.config.connect_rule(),
                    );
                    self.session.push(spawn_session(session, addr, stream));
                }
            }
        }
        info!("server shutdown");
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::acceptor::{Binder, TcpBinder};
    use crate::byte_stream::test::*;
    use crate::config::*;
    use crate::connector::*;

    use std::borrow::Cow;
    use std::ops::Deref;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime};

    #[test]
    fn server_shutdown() {
        let config = ServerConfig::default();

        let (mut server, tx) = Server::with_binder(config, TcpBinder, TcpUdpConnector);
        let shutdown = Arc::new(Mutex::new(SystemTime::now()));
        let th = {
            let shutdown = shutdown.clone();
            thread::spawn(move || {
                server.serve().ok();
                *shutdown.lock().unwrap() = SystemTime::now();
            })
        };
        thread::sleep(Duration::from_secs(1));
        let req_shutdown = SystemTime::now();
        tx.send(ServerCommand::Terminate).unwrap();
        th.join().unwrap();
        assert!(shutdown.lock().unwrap().deref() > &req_shutdown);
    }

    struct DummyBinder {
        stream: BufferStream,
        src_addr: SocketAddr,
    }

    impl Binder for DummyBinder {
        type Stream = BufferStream;
        type Iter = std::iter::Once<(Self::Stream, SocketAddr)>;
        fn bind(&self, addr: SocketAddr) -> Result<Self::Iter, Error> {
            println!("bind: {}", addr);
            Ok(std::iter::once((self.stream.clone(), self.src_addr)))
        }
    }

    #[test]
    fn dummy_binder() {
        let binder = DummyBinder {
            stream: BufferStream::new(
                Cow::from(b"dummy read".to_vec()),
                Cow::from(b"dummy write".to_vec()),
            ),
            src_addr: "127.0.0.1:1080".parse().unwrap(),
        };
        let (mut server, tx) =
            Server::with_binder(ServerConfig::default(), binder, TcpUdpConnector);
        let th = thread::spawn(move || {
            server.serve().ok();
        });

        thread::sleep(Duration::from_secs(1));
        tx.send(ServerCommand::Terminate).unwrap();
        th.join().unwrap();
    }
}
