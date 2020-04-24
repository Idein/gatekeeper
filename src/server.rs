use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{
    mpsc::{self, Receiver, SyncSender},
    Arc, Mutex,
};
use std::thread;

use log::*;
use rand::prelude::*;

use crate::acceptor::{Binder, TcpBinder};
use crate::auth_service::{AuthService, NoAuthService};
use crate::byte_stream::ByteStream;
use crate::config::ServerConfig;
use crate::connector::{Connector, TcpUdpConnector};
use crate::error::Error;
use crate::model::{ProtocolVersion, SocketAddr};
use crate::server_command::ServerCommand;
use crate::session::{Session, SessionHandle, SessionId};

pub struct Server<S, T, C> {
    config: ServerConfig,
    tx_cmd: SyncSender<ServerCommand<S>>,
    rx_cmd: Receiver<ServerCommand<S>>,
    /// bind server address
    binder: T,
    /// send termination message to the acceptor
    tx_acceptor_done: SyncSender<()>,
    /// make connection to service host
    connector: C,
    protocol_version: ProtocolVersion,
    session: HashMap<SessionId, SessionHandle>,
    /// random context for generating SessionIds
    id_rng: ThreadRng,
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
    session: Session<D, M, S>,
    // termination message sender
    tx: SyncSender<()>,
    addr: SocketAddr,
    strm: S,
) -> SessionHandle
where
    S: ByteStream + 'static,
    D: Connector + 'static,
    M: AuthService + 'static,
{
    SessionHandle::new(thread::spawn(move || session.start(addr, strm)), tx)
}

impl Server<TcpStream, TcpBinder, TcpUdpConnector> {
    pub fn new(config: ServerConfig) -> (Self, mpsc::SyncSender<ServerCommand<TcpStream>>) {
        let (tx_done, rx_done) = mpsc::sync_channel(1);
        Server::<TcpStream, TcpBinder, TcpUdpConnector>::with_binder(
            config.clone(),
            TcpBinder::new(
                config.client_rw_timeout,
                Arc::new(Mutex::new(rx_done)),
                config.accept_timeout,
            ),
            tx_done,
            TcpUdpConnector::new(config.server_rw_timeout),
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
        tx_acceptor_done: SyncSender<()>,
        connector: C,
    ) -> (Self, SyncSender<ServerCommand<S>>) {
        let (tx, rx) = mpsc::sync_channel(0);
        (
            Self {
                config,
                tx_cmd: tx.clone(),
                rx_cmd: rx,
                binder,
                tx_acceptor_done,
                connector,
                protocol_version: ProtocolVersion::from(5),
                session: HashMap::new(),
                id_rng: thread_rng(),
            },
            tx,
        )
    }

    fn next_session_id(&mut self) -> SessionId {
        loop {
            let next_candidate = self.id_rng.next_u64().into();
            if self.session.contains_key(&next_candidate) {
                continue;
            }
            debug!("next session id is issued: {}", next_candidate);
            return next_candidate;
        }
    }

    pub fn serve(&mut self) -> Result<(), Error> {
        let acceptor = self.binder.bind(self.config.server_addr())?;
        let accept_th = spawn_acceptor(acceptor, self.tx_cmd.clone());

        while let Ok(cmd) = self.rx_cmd.recv() {
            use ServerCommand::*;
            info!("cmd: {:?}", cmd);
            match cmd {
                Terminate => {
                    trace!("stopping accept thread...");
                    self.tx_acceptor_done.send(()).ok();
                    trace!("stopping session threads...");
                    self.session.iter().for_each(|(_, ss)| {
                        ss.stop().ok();
                    });

                    self.session.drain().for_each(|(_, ss)| {
                        ss.join().ok();
                    });
                    trace!("session threads are stopped");
                    accept_th.join().ok();
                    trace!("accept thread is stopped");
                    break;
                }
                Connect(stream, addr) => {
                    let (session, tx) = Session::new(
                        self.next_session_id(),
                        self.protocol_version,
                        self.connector.clone(),
                        NoAuthService::new(),
                        self.config.server_addr(),
                        self.config.connect_rule(),
                        self.tx_cmd.clone(),
                    );
                    self.session
                        .insert(session.id, spawn_session(session, tx, addr, stream));
                }
                Disconnect(id) => {
                    if let Some(session) = self.session.remove(&id) {
                        debug!("stopping session: {}", id);
                        session.stop().ok();
                        match session.join() {
                            Ok(Ok(())) => info!("session is stopped: {}", id),
                            Ok(Err(err)) => {
                                error!("session error: {}: {}", id, err);
                                trace!("session error: {}: {}", id, err);
                            }
                            Err(err) => error!("session panic: {}: {:?}", id, err),
                        }
                    } else {
                        error!("session not found: {}", id);
                    }
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
    use crate::model;

    use std::borrow::Cow;
    use std::ops::Deref;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime};

    #[test]
    fn server_shutdown() {
        let config = ServerConfig::default();
        let (tx_done, rx_done) = mpsc::sync_channel(1);

        let (mut server, tx) = Server::with_binder(
            config,
            TcpBinder::new(
                None,
                Arc::new(Mutex::new(rx_done)),
                Some(Duration::from_secs(3)),
            ),
            tx_done,
            TcpUdpConnector::new(None),
        );
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
        fn bind(&self, addr: SocketAddr) -> Result<Self::Iter, model::Error> {
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
        let (tx_done, _rx_done) = mpsc::sync_channel(1);
        let (mut server, tx) = Server::with_binder(
            ServerConfig::default(),
            binder,
            tx_done,
            TcpUdpConnector::new(None),
        );
        let th = thread::spawn(move || {
            server.serve().ok();
        });

        thread::sleep(Duration::from_secs(1));
        tx.send(ServerCommand::Terminate).unwrap();
        th.join().unwrap();
    }
}
