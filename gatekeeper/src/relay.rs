use std::io;
use std::thread::{self, JoinHandle};

use log::*;
use model;

use crate::byte_stream::{BoxedStream, ByteStream};

pub fn spawn_relay(
    client_conn: BoxedStream,
    server_conn: impl ByteStream,
) -> Result<(JoinHandle<()>, JoinHandle<()>), model::Error> {
    debug!("spawn_relay");
    let (read_client, write_client) = client_conn.split()?;
    let (read_server, write_server) = server_conn.split()?;
    Ok((
        spawn_relay_half("relay: outbound", read_client, write_server)?,
        spawn_relay_half("relay: incoming", read_server, write_client)?,
    ))
}

fn spawn_relay_half(
    name: &str,
    mut src: impl io::Read + Send + 'static,
    mut dst: impl io::Write + Send + 'static,
) -> Result<JoinHandle<()>, model::Error> {
    debug!("spawn_relay_half");
    let name = name.to_owned();
    thread::Builder::new()
        .name(name.clone())
        .spawn(move || {
            debug!("spawned: {}", name);
            if let Err(err) = io::copy(&mut src, &mut dst) {
                error!("Err: relay ({}): {}", name, err);
            }
        })
        .map_err(Into::into)
}
