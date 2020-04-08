use std::io;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};

use log::*;
use model;

use crate::byte_stream::{BoxedStream, ByteStream};

pub fn spawn_relay(
    client_conn: BoxedStream,
    server_conn: impl ByteStream,
    rx: mpsc::Receiver<()>,
) -> Result<(JoinHandle<()>, JoinHandle<()>), model::Error> {
    debug!("spawn_relay");
    let (read_client, write_client) = client_conn.split()?;
    let (read_server, write_server) = server_conn.split()?;
    let shared_rx = Arc::new(Mutex::new(rx));
    Ok((
        spawn_relay_half("outbound", shared_rx.clone(), read_client, write_server)?,
        spawn_relay_half("incoming", shared_rx.clone(), read_server, write_client)?,
    ))
}

fn spawn_relay_half(
    name: &str,
    rx: Arc<Mutex<mpsc::Receiver<()>>>,
    mut src: impl io::Read + Send + 'static,
    mut dst: impl io::Write + Send + 'static,
) -> Result<JoinHandle<()>, model::Error> {
    use mpsc::TryRecvError;

    let name = name.to_owned();
    thread::Builder::new()
        .name(name.clone())
        .spawn(move || {
            debug!("spawned: {}", name);
            loop {
                use io::ErrorKind as K;
                match io::copy(&mut src, &mut dst) {
                    Ok(size) => {
                        trace!("{}: {}", name, size);
                        if size == 0 {
                            return;
                        }
                    }
                    Err(err) if err.kind() == K::WouldBlock || err.kind() == K::TimedOut => {}
                    Err(err) => {
                        error!("{}: {:?}", name, err);
                        return;
                    }
                }
                match rx.lock() {
                    Ok(rx) => match rx.try_recv() {
                        Ok(()) => {
                            info!("{}: recv termination message", name);
                            return;
                        }
                        Err(TryRecvError::Empty) => trace!("{}: message empty", name),
                        Err(TryRecvError::Disconnected) => {
                            error!("{}: disconnected", name);
                            return;
                        }
                    },
                    Err(err) => {
                        error!("rx.lock: {:?}", err);
                        return;
                    }
                }
            }
        })
        .map_err(Into::into)
}
