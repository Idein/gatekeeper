//! Proxy server main process
//!
//! # Server workflow summary
//!
//! ```text
//! Client     Acceptor        Server                      External Service
//!   |            |             |                                 |
//!   |            |             |                                 |
//!   |----------->|             |                                 |
//!   |connect(2)  x accept(2)   |                                 |
//!   |            |------------>|                                 |
//!   |            |Connect      |                                 |
//!   |            |             |                                 |
//!   |            |             x Session::new                    |
//!   .            .             .                                 |
//!   .            .             .                                 |
//!   |                          |                                 |
//!   | [ establish connection ] |                                 |
//!   |     - authorize          |                                 |
//!   |     - filtering          |                                 |
//!   .                          .                                 |
//!   .                          .                                 |
//!   |                          |        incoming    outgoing     |
//!   |                          |          relay      relay       |
//!   |                          x -----------x--------> x         |
//!   |                          |spawn_relay |          |         |
//!   |                          .            |          |         |
//!   |                          .            |          |         |
//!   |                                       |          |         |
//!
//! [ repeat ]                                |          |         |
//!   |------------------------------------------------->|         |
//!   |write(2)                               |          |-------->|
//!   |                                       |          |write(2) |
//!   |                                       |          |         |
//!   |                                       |          |<--------|
//!   |<-------------------------------------------------|read(2)  |
//!   |read(2)                                |          |         |
//!   |                                       |          |         |
//!
//! [ alt ]
//! [ relay completed ]                       |          |
//!   |                          .            |          |
//!   |                          .            .          |
//!   |                          |            .          |
//!   |                          |            x complete .
//!   |                          |                       .
//!   |                          |<----------------------x complete
//!   |                          |             Disconnect
//!   |                          |
//!
//! [ alt ]
//! [ abort relay ]              |            |          |
//!   |                          x recv Terminate        |
//!   |                          |            |          |
//!   |                          |----------->|          |
//!   |                          |send(())    x          |
//!   |                          |                       |
//!   |                          |---------------------->|
//!   |                          |send(())               |
//!   |                          |                       |
//!   |                          |<----------------------x
//!   |                          |             Disconnect
//!   |                          |
//! ```
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{
    mpsc::{self, Receiver, Sender, SyncSender},
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
use crate::thread::spawn_thread;

pub struct Server<S, T, C> {
    config: ServerConfig,
    tx_cmd: Sender<ServerCommand<S>>,
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
    id_rng: StdRng,
}

/// spawn a thread send accepted stream to `tx`
fn spawn_acceptor<S>(
    acceptor: impl Iterator<Item = (S, SocketAddr)> + Send + 'static,
    tx: Sender<ServerCommand<S>>,
) -> Result<thread::JoinHandle<()>, Error>
where
    S: ByteStream + 'static,
{
    use ServerCommand::*;
    Ok(spawn_thread("acceptor", move || {
        for (strm, addr) in acceptor {
            if tx.send(Connect(strm, addr)).is_err() {
                info!("disconnected ServerCommand chan");
                break;
            }
        }
    })?)
}

/// spawn a thread perform `Session.start`
///
///
/// - *session*
///   Session to spawn.
/// - *tx*
///   Sender of session termination message.
/// - *addr*
///   Address of the client connects to this server.
/// - *strm*
///   Established connection between a client and this server.
fn spawn_session<S, D, M>(
    session: Session<D, M, S>,
    tx: SyncSender<()>,
    addr: SocketAddr,
    strm: S,
) -> SessionHandle
where
    S: ByteStream + 'static,
    D: Connector + 'static,
    M: AuthService + 'static,
{
    let session_th = spawn_thread(&format!("{}: {}", session.id, addr), move || {
        session.start(addr, strm)
    })
    .unwrap();
    SessionHandle::new(addr, session_th, tx)
}

impl Server<TcpStream, TcpBinder, TcpUdpConnector> {
    pub fn new(config: ServerConfig) -> (Self, mpsc::Sender<ServerCommand<TcpStream>>) {
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
    ) -> (Self, Sender<ServerCommand<S>>) {
        let (tx, rx) = mpsc::channel();
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
                id_rng: StdRng::from_entropy(),
            },
            tx,
        )
    }

    fn next_session_id(&mut self) -> SessionId {
        loop {
            let next_candidate = self.id_rng.next_u32().into();
            if self.session.contains_key(&next_candidate) {
                continue;
            }
            debug!("next session id is issued: {}", next_candidate);
            return next_candidate;
        }
    }

    /// Server main loop
    pub fn serve(&mut self) -> Result<(), Error> {
        let acceptor = self.binder.bind(self.config.server_addr())?;
        let accept_th = spawn_acceptor(acceptor, self.tx_cmd.clone())?;

        while let Ok(cmd) = self.rx_cmd.recv() {
            use ServerCommand::*;
            info!("cmd: {:?}", cmd);
            match cmd {
                Terminate => {
                    self.tx_acceptor_done.send(()).ok();
                    self.session.iter().for_each(|(_, ss)| ss.stop());

                    self.session.drain().for_each(|(_, ss)| {
                        ss.join().ok();
                    });
                    debug!("join accept thread");
                    accept_th.join().ok();
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
                        let addr = session.client_addr();
                        session.stop();
                        match session.join() {
                            Ok(Ok(())) => info!("session is stopped: {}: {}", addr, id),
                            Ok(Err(err)) => error!("session error: {}: {}: {}", addr, id, err),
                            Err(err) => error!("session panic: {}: {}: {:?}", addr, id, err),
                        }
                    } else {
                        error!("session has already been stopped: {}", id);
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
        let req_shutdown = Arc::new(Mutex::new(SystemTime::now()));

        let th = {
            let req_shutdown = req_shutdown.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(1));
                *req_shutdown.lock().unwrap() = SystemTime::now();
                tx.send(ServerCommand::Terminate).unwrap();
            })
        };

        server.serve().ok();
        let shutdown = SystemTime::now();
        th.join().unwrap();
        assert!(&shutdown > req_shutdown.lock().unwrap().deref());
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
            stream: BufferStream::with_buffer(
                Cow::from(b"dummy read".to_vec()),
                Cow::from(b"dummy write".to_vec()),
            ),
            src_addr: "127.0.0.1:1080".parse().unwrap(),
        };
        let tx = Arc::new(Mutex::new(None));
        let th = {
            let tx = tx.clone();
            thread::spawn(move || {
                let (tx_done, _rx_done) = mpsc::sync_channel(1);
                let (mut server, stx) = Server::with_binder(
                    ServerConfig::default(),
                    binder,
                    tx_done,
                    TcpUdpConnector::new(None),
                );
                *tx.lock().unwrap() = Some(stx);
                server.serve().ok();
            })
        };

        thread::sleep(Duration::from_secs(1));
        tx.lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .send(ServerCommand::Terminate)
            .unwrap();
        th.join().unwrap();
    }
}
