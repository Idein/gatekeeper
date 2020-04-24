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
    outbound_th: JoinHandle<Result<(), Error>>,
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

pub fn spawn_relay<S>(
    id: SessionId,
    client_conn: BoxedStream,
    server_conn: impl ByteStream,
    rx: mpsc::Receiver<()>,
    tx: mpsc::SyncSender<ServerCommand<S>>,
) -> Result<RelayHandle, Error>
where
    S: Send + 'static,
{
    let (read_client, write_client) = client_conn.split()?;
    let (read_server, write_server) = server_conn.split()?;
    let rx = Arc::new(Mutex::new(rx));

    let outbound_th = {
        let tx = tx.clone();
        let rx = rx.clone();
        spawn_thread("outbound", move || {
            spawn_relay_half(id, rx, tx, read_client, write_server)
        })?
    };
    let incoming_th = spawn_thread("incoming", move || {
        spawn_relay_half(id, rx.clone(), tx.clone(), read_server, write_client)
    })?;
    Ok(RelayHandle::new(outbound_th, incoming_th))
}

fn spawn_relay_half<S>(
    id: SessionId,
    rx: Arc<Mutex<mpsc::Receiver<()>>>,
    tx: mpsc::SyncSender<ServerCommand<S>>,
    mut src: impl io::Read + Send + 'static,
    mut dst: impl io::Write + Send + 'static,
) -> Result<(), Error>
where
    S: Send + 'static,
{
    info!(
        "spawned relay: {}",
        thread::current().name().unwrap_or("<anonymous>")
    );
    let _guard = DisconnectGuard::new(id, tx);
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
            Ok(size) => trace!("copy: {}bytes", size),
            Err(err) if err.kind() == K::WouldBlock || err.kind() == K::TimedOut => {}
            Err(err) => {
                error!("relay error: {:?}", err);
                Err(err)?;
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
