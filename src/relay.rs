use std::io;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};

use log::*;

use crate::byte_stream::{BoxedStream, ByteStream};
use crate::model::{Error, ErrorKind};
use crate::server_command::ServerCommand;
use crate::session::{DisconnectGuard, SessionId};

#[derive(Debug)]
pub struct RelayHandle {
    /// handle to relay: client -> external network
    outbound_th: JoinHandle<Result<(), Error>>,
    /// handle to relay: client <- external network
    incoming_th: JoinHandle<Result<(), Error>>,
}

impl RelayHandle {
    fn new(
        outbound_th: JoinHandle<Result<(), Error>>,
        incoming_th: JoinHandle<Result<(), Error>>,
    ) -> Self {
        Self {
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
/// * `id`
///    The id of session spawns this relay
/// * `client_conn`
///    Connection between client and this proxy.
/// * `server_conn`
///    Connection between external host and this proxy.
/// * `rx`
///    Relay termination message Receiver.
///    It is needed to send 2 messages for terminates 2 relays.
/// * `tx`
///    Notify relay thread termination with send `Disconnect` to main thread.
pub fn spawn_relay<S>(
    id: SessionId,
    client_conn: BoxedStream,
    server_conn: impl ByteStream,
    rx: Arc<Mutex<mpsc::Receiver<()>>>,
    tx: mpsc::Sender<ServerCommand<S>>,
) -> Result<RelayHandle, Error>
where
    S: Send + 'static,
{
    let (read_client, write_client) = client_conn.split()?;
    let (read_server, write_server) = server_conn.split()?;

    let outbound_th = {
        let tx = tx.clone();
        let rx = rx.clone();
        spawn_thread("outbound", move || {
            let _guard = DisconnectGuard::new(id, tx);
            spawn_relay_half(rx, read_client, write_server)
        })?
    };
    let incoming_th = {
        spawn_thread("incoming", move || {
            let _guard = DisconnectGuard::new(id, tx);
            spawn_relay_half(rx, read_server, write_client)
        })?
    };
    Ok(RelayHandle::new(outbound_th, incoming_th))
}

fn spawn_relay_half(
    rx: Arc<Mutex<mpsc::Receiver<()>>>,
    mut src: impl io::Read + Send + 'static,
    mut dst: impl io::Write + Send + 'static,
) -> Result<(), Error> {
    let thread_name = thread::current().name().unwrap_or("<anonymous>").to_owned();
    info!("spawned relay: {}", thread_name);
    loop {
        use io::ErrorKind as K;
        if check_termination(&rx).expect("main thread must be alive") {
            info!("relay thread is requested termination.");
            return Ok(());
        }
        match io::copy(&mut src, &mut dst) {
            Ok(0) => {
                info!("relay thread has been finished.");
                return Ok(());
            }
            Ok(size) => trace!("{} copy: {} bytes", thread_name, size),
            Err(err) if err.kind() == K::WouldBlock || err.kind() == K::TimedOut => {}
            Err(err) => Err(err)?,
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

/// spawn `name`d thread performs `f`
fn spawn_thread<F, R>(name: &str, f: F) -> Result<JoinHandle<R>, Error>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    thread::Builder::new()
        .name(name.into())
        .spawn(move || f())
        .map_err(Into::into)
}
