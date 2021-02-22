use std::io;
use std::net::SocketAddr;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};

use log::*;

use crate::byte_stream::{BoxedStream, ByteStream};
use crate::model::{Error, ErrorKind};
use crate::session::DisconnectGuard;
use crate::thread::spawn_thread;

#[derive(Debug)]
pub struct RelayHandle {
    /// client address
    client_addr: SocketAddr,
    /// server address
    server_addr: SocketAddr,
    /// handle to relay: client -> external network
    outbound_th: JoinHandle<Result<(), Error>>,
    /// handle to relay: client <- external network
    incoming_th: JoinHandle<Result<(), Error>>,
}

impl RelayHandle {
    fn new(
        client_addr: SocketAddr,
        server_addr: SocketAddr,
        outbound_th: JoinHandle<Result<(), Error>>,
        incoming_th: JoinHandle<Result<(), Error>>,
    ) -> Self {
        Self {
            client_addr,
            server_addr,
            outbound_th,
            incoming_th,
        }
    }

    pub fn join(self) -> thread::Result<Result<(), Error>> {
        self.outbound_th.join().and(self.incoming_th.join())
    }
}

/// Spawn relay thread(s)
///
/// * `client_addr`
///    The address of the client of this session.
/// * `server_addr`
///    The address of the server to connect to.
/// * `client_conn`
///    Connection between client and this proxy.
/// * `server_conn`
///    Connection between external host and this proxy.
/// * `rx`
///    Relay termination message Receiver.
///    It is needed to send 2 messages for terminates 2 relays.
/// * `guard`
///    Send `Disconnect` to the main thread when the relay thread is completed.
pub fn spawn_relay<S>(
    client_addr: SocketAddr,
    server_addr: SocketAddr,
    client_conn: BoxedStream,
    server_conn: impl ByteStream,
    rx: Arc<Mutex<mpsc::Receiver<()>>>,
    guard: Arc<Mutex<DisconnectGuard<S>>>,
) -> Result<RelayHandle, Error>
where
    S: Send + 'static,
{
    let (read_client, write_client) = client_conn.split()?;
    let (read_server, write_server) = server_conn.split()?;

    let outbound_th = {
        let guard = guard.clone();
        let rx = rx.clone();
        spawn_thread("outbound", move || {
            let _guard = guard;
            spawn_relay_half(rx, client_addr, server_addr, read_client, write_server)
        })?
    };
    let incoming_th = {
        spawn_thread("incoming", move || {
            let _guard = guard;
            spawn_relay_half(rx, server_addr, client_addr, read_server, write_client)
        })?
    };
    Ok(RelayHandle::new(
        client_addr,
        server_addr,
        outbound_th,
        incoming_th,
    ))
}

fn spawn_relay_half(
    rx: Arc<Mutex<mpsc::Receiver<()>>>,
    src_addr: SocketAddr,
    dst_addr: SocketAddr,
    mut src: impl io::Read + Send + 'static,
    mut dst: impl io::Write + Send + 'static,
) -> Result<(), Error> {
    // thread_name
    let name = thread::current().name().unwrap_or("<anonymous>").to_owned();
    info!("spawned relay: {}: {} ==> {}", name, src_addr, dst_addr);
    loop {
        use io::ErrorKind as K;
        if check_termination(&rx).expect("main thread must be alive") {
            info!(
                "relay thread is requested termination: {} ==> {}",
                src_addr, dst_addr
            );
            return Ok(());
        }
        match io::copy(&mut src, &mut dst) {
            Ok(0) => {
                info!(
                    "relay thread has been finished: {}: {} ==> {}",
                    name, src_addr, dst_addr
                );
                return Ok(());
            }
            Ok(size) => trace!("{}: {} ==> {}: {} bytes", name, src_addr, dst_addr, size),
            Err(err) if err.kind() == K::WouldBlock || err.kind() == K::TimedOut => {}
            Err(err) => {
                return Err(err.into());
            }
        }
    }
}

fn check_termination(rx: &Arc<Mutex<mpsc::Receiver<()>>>) -> Result<bool, Error> {
    use mpsc::TryRecvError;
    match rx.lock()?.try_recv() {
        Ok(()) => Ok(true),
        Err(TryRecvError::Empty) => {
            // trace!("message empty");
            Ok(false)
        }
        Err(TryRecvError::Disconnected) => {
            Err(ErrorKind::disconnected(thread::current().name().unwrap_or("<anonymous>")).into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_relay() {
        use crate::byte_stream::test::IterBuffer;
        use crate::server_command::ServerCommand;
        use crate::session::SessionId;

        let client_writer = Arc::new(Mutex::new(io::Cursor::new(vec![])));
        let client_addr = "192.168.1.1:45678".parse().unwrap();
        let dummy_client_conn = Box::new(IterBuffer {
            iter: vec![b"hello".to_vec(), b" ".to_vec(), b"client".to_vec()].into_iter(),
            wr_buff: client_writer.clone(),
        }) as Box<dyn ByteStream>;

        let server_writer = Arc::new(Mutex::new(io::Cursor::new(vec![])));
        let server_addr = "192.168.1.1:45679".parse().unwrap();
        let dummy_server_conn = IterBuffer {
            iter: vec![b"hello".to_vec(), b" ".to_vec(), b"server".to_vec()].into_iter(),
            wr_buff: server_writer.clone(),
        };

        let (tx_relay, rx_relay) = mpsc::channel();
        let (tx_server, rx_server) = mpsc::channel();
        let guard = Arc::new(Mutex::new(DisconnectGuard::<()>::new(0.into(), tx_server)));

        let handle = {
            let rx_relay = Arc::new(Mutex::new(rx_relay));
            spawn_relay(
                client_addr,
                server_addr,
                dummy_client_conn,
                dummy_server_conn,
                rx_relay,
                guard,
            )
            .unwrap()
        };

        assert!(
            if let ServerCommand::Disconnect(SessionId(0)) = rx_server.recv().unwrap() {
                true
            } else {
                false
            }
        );

        tx_relay.send(()).unwrap_err();
        handle.join().unwrap().unwrap();

        assert_eq!(
            client_writer.lock().unwrap().get_ref().as_slice(),
            &b"hello server"[..]
        );
        assert_eq!(
            server_writer.lock().unwrap().get_ref().as_slice(),
            &b"hello client"[..]
        );
    }
}
