use std::io;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};

use log::*;

use crate::byte_stream::{BoxedStream, ByteStream};
use crate::model::{Error, ErrorKind};
use crate::server_command::ServerCommand;
use crate::session::{DisconnectGuard, SessionId};

pub fn spawn_relay<S>(
    id: SessionId,
    client_conn: BoxedStream,
    server_conn: impl ByteStream,
    rx: mpsc::Receiver<()>,
    tx: mpsc::SyncSender<ServerCommand<S>>,
) -> Result<(JoinHandle<()>, JoinHandle<()>), Error>
where
    S: Send + 'static,
{
    debug!("spawn_relay");
    let (read_client, write_client) = client_conn.split()?;
    let (read_server, write_server) = server_conn.split()?;
    let shared_rx = Arc::new(Mutex::new(rx));
    Ok((
        spawn_relay_half(
            id,
            "outbound",
            shared_rx.clone(),
            tx.clone(),
            read_client,
            write_server,
        )?,
        spawn_relay_half(
            id,
            "incoming",
            shared_rx.clone(),
            tx.clone(),
            read_server,
            write_client,
        )?,
    ))
}

fn spawn_relay_half<S>(
    id: SessionId,
    name: &'static str,
    rx: Arc<Mutex<mpsc::Receiver<()>>>,
    tx: mpsc::SyncSender<ServerCommand<S>>,
    mut src: impl io::Read + Send + 'static,
    mut dst: impl io::Write + Send + 'static,
) -> Result<JoinHandle<()>, Error>
where
    S: Send + 'static,
{
    spawn_thread(name, move || {
        debug!("spawned relay");
        let _guard = DisconnectGuard::new(id, tx);
        loop {
            use io::ErrorKind as K;
            if check_termination(&rx).expect("main thread must be alive") {
                return;
            }
            match io::copy(&mut src, &mut dst) {
                Ok(0) => return,
                Ok(size) => trace!("copy: {}", size),
                Err(err) if err.kind() == K::WouldBlock || err.kind() == K::TimedOut => {}
                Err(err) => {
                    error!("{}: {:?}", name, err);
                    return;
                }
            }
        }
    })
}

fn check_termination(rx: &Arc<Mutex<mpsc::Receiver<()>>>) -> Result<bool, Error> {
    use mpsc::TryRecvError;
    match rx.lock()?.try_recv() {
        Ok(()) => {
            info!("recv termination message");
            Ok(true)
        }
        Err(TryRecvError::Empty) => {
            trace!("message empty");
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
