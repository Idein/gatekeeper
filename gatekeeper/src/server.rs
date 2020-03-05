use std::sync::mpsc::{self, SyncSender};
use std::thread;

use log::*;

use crate::acceptor::Binder;
use crate::byte_stream::ByteStream;
use crate::config::ServerConfig;
use crate::connector::Connector;
use crate::error::Error;
use crate::method_selector::{MethodSelector, OnlyNoAuth};
use crate::server_command::ServerCommand;
use crate::session::Session;

use model::{IpAddr, ProtocolVersion, SocketAddr};

pub struct Server<S, T, C> {
    config: ServerConfig,
    tx_cmd: mpsc::SyncSender<ServerCommand<S>>,
    rx_cmd: mpsc::Receiver<ServerCommand<S>>,
    /// bind server address
    binder: T,
    /// make connection to service host
    connector: C,
    protocol_version: ProtocolVersion,
}

/// spawn a thread send accepted stream to `tx`
fn spawn_acceptor<S>(
    acceptor: impl Iterator<Item = (S, SocketAddr)> + Send + 'static,
    tx: SyncSender<ServerCommand<S>>,
) -> thread::JoinHandle<()>
where
    S: ByteStream + 'static + Send,
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
fn spawn_session<S, D, M>(mut session: Session<S, D, M>) -> thread::JoinHandle<Result<(), Error>>
where
    S: ByteStream + 'static,
    D: Connector + 'static,
    M: MethodSelector + 'static,
{
    thread::spawn(move || session.start())
}

impl<S, T, C> Server<S, T, C>
where
    S: ByteStream + 'static,
    T: Binder<Stream = S>,
    C: Connector + Clone + 'static,
{
    pub fn new(
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
            },
            tx,
        )
    }

    pub fn serve(&self) -> Result<(), Error> {
        let acceptor = self
            .binder
            .bind((self.config.server_address, self.config.server_port))?;
        spawn_acceptor(acceptor, self.tx_cmd.clone());

        while let Ok(cmd) = self.rx_cmd.recv() {
            use ServerCommand::*;
            info!("cmd: {:?}", cmd);
            match cmd {
                Terminate => break,
                Connect(stream, addr) => {
                    info!("connect from: {}", addr);
                    let session = Session::new(
                        self.protocol_version,
                        stream,
                        addr,
                        self.connector.clone(),
                        OnlyNoAuth::new(),
                        SocketAddr::from(([127, 0, 0, 1], 1080)).into(),
                    );
                    spawn_session(session);
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

    use std::borrow::Cow;
    use std::net::*;
    use std::ops::Deref;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime};

    #[test]
    fn server_shutdown() {
        let (server, tx) = Server::new(TcpBinder);
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
        fn bind<A: ToSocketAddrs>(&self, addr: A) -> Result<Self::Iter, Error> {
            let mut addr = addr.to_socket_addrs().unwrap();
            println!("bind: {}", addr.next().unwrap());
            Ok(std::iter::once((self.stream.clone(), self.src_addr)))
        }
    }

    #[test]
    fn dummy_binder() {
        let binder = DummyBinder {
            stream: BufferStream::new(Cow::from(b"dummy".to_vec())),
            src_addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 1080)),
        };
        let (server, tx) = Server::new(binder);
        let th = thread::spawn(move || {
            server.serve().ok();
        });

        thread::sleep(Duration::from_secs(1));
        tx.send(ServerCommand::Terminate).unwrap();
        th.join().unwrap();
    }
}
