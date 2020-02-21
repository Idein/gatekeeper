use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;

use log::*;

use crate::error::Error;

#[derive(Debug)]
pub enum ServerCommand {
    Terminate,
    Accept(TcpStream, SocketAddr),
}

pub struct Server {
    tx_cmd: mpsc::SyncSender<ServerCommand>,
    rx_cmd: mpsc::Receiver<ServerCommand>,
}

impl Server {
    pub fn new() -> (Self, mpsc::SyncSender<ServerCommand>) {
        let (tx, rx) = mpsc::sync_channel(0);
        (
            Self {
                tx_cmd: tx.clone(),
                rx_cmd: rx,
            },
            tx,
        )
    }

    pub fn serve(&self) -> Result<(), Error> {
        let listener = TcpListener::bind("127.0.0.1:1080")?;
        let tx = self.tx_cmd.clone();
        thread::spawn(move || loop {
            match listener.accept() {
                Ok((stream, addr)) => {
                    info!("accept: {}", addr);
                    if tx.send(ServerCommand::Accept(stream, addr)).is_err() {
                        info!("disconnected ServerCommand chan");
                        break;
                    }
                }
                Err(err) => {
                    error!("error: {}", err);
                    trace!("error: {:?}", err);
                }
            }
        });

        while let Ok(cmd) = self.rx_cmd.recv() {
            use ServerCommand::*;
            debug!("cmd: {:?}", cmd);
            match cmd {
                Terminate => break,
                Accept(_stream, _addr) => {}
            }
        }
        info!("server shutdown");
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::ops::Deref;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, SystemTime};

    #[test]
    fn server() {
        let (server, tx) = Server::new();
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
}
