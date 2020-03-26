use std::io;
use std::thread::{self, JoinHandle};

use log::*;
use model;

use crate::byte_stream::{BoxedStream, ByteStream};

pub fn spawn_relay<R>(
    client_conn: BoxedStream,
    server_conn: R,
) -> Result<(JoinHandle<()>, JoinHandle<()>), model::Error>
where
    R: ByteStream,
{
    println!("spawn_relay");
    let (read_client, write_client) = client_conn.split()?;
    let (read_server, write_server) = server_conn.split()?;
    Ok((
        spawn_relay_half("relay: outbound", read_client, write_server)?,
        spawn_relay_half("relay: incoming", read_server, write_client)?,
    ))
}

fn spawn_relay_half<S, D>(
    name: &str,
    mut src: S,
    mut dst: D,
) -> Result<JoinHandle<()>, model::Error>
where
    S: io::Read + Send + 'static,
    D: io::Write + Send + 'static,
{
    println!("spawn_relay_half");
    let name = name.to_owned();
    thread::Builder::new()
        .name(name.clone())
        .spawn(move || {
            println!("spawned: {}", name);
            if let Err(err) = io::copy(&mut src, &mut dst) {
                println!("Err: relay ({}): {}", name, err);
            } else {
                println!("relay ({})", name);
            }
        })
        .map_err(Into::into)
}
