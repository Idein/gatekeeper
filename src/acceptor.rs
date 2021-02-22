use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    mpsc::{self, Receiver},
    Arc, Mutex,
};
use std::time::Duration;

use failure::Fail;
use log::*;

use crate::byte_stream::ByteStream;
use crate::model;
use crate::model::{Error, ErrorKind};
use crate::tcp_listener_ext::*;

pub struct TcpAcceptor {
    listener: TcpListener,
    rw_timeout: Option<Duration>,
    /// receive termination message
    rx: Arc<Mutex<Receiver<()>>>,
    /// timeout for accept
    accept_timeout: Option<Duration>,
}

impl TcpAcceptor {
    fn new(
        listener: TcpListener,
        rw_timeout: Option<Duration>,
        rx: Arc<Mutex<Receiver<()>>>,
        accept_timeout: Option<Duration>,
    ) -> Self {
        Self {
            listener,
            rw_timeout,
            rx,
            accept_timeout,
        }
    }

    fn accept_timeout(&self) -> io::Result<(TcpStream, SocketAddr)> {
        self.listener
            .accept_timeout(self.accept_timeout)
            .and_then(|(tcp, addr)| {
                tcp.set_read_timeout(self.rw_timeout)?;
                tcp.set_write_timeout(self.rw_timeout)?;
                Ok((tcp, addr))
            })
    }
}

fn check_message(rx: &Arc<Mutex<Receiver<()>>>) -> Result<bool, Error> {
    use mpsc::TryRecvError;
    match rx.lock()?.try_recv() {
        Ok(()) => Ok(true),
        Err(TryRecvError::Empty) => Ok(false),
        Err(TryRecvError::Disconnected) => Err(ErrorKind::disconnected("acceptor").into()),
    }
}

macro_rules! check_done {
    ($rx:expr) => {
        match check_message($rx) {
            Ok(true) => return None,
            Ok(false) => {}
            Err(_) => return None,
        }
    };
}

impl Iterator for TcpAcceptor {
    type Item = (TcpStream, SocketAddr);
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            check_done!(&self.rx);
            match self.accept_timeout() {
                Ok(x) => return Some(x),
                Err(err) if err.kind() == io::ErrorKind::TimedOut => {
                    // trace!("accept timeout: {}", err);
                }
                Err(err) => {
                    error!("accept error: {}", err);
                    trace!("accept error: {:?}", err);
                    return None;
                }
            }
        }
    }
}

pub trait Binder {
    type Stream: ByteStream + 'static;
    type Iter: Iterator<Item = (Self::Stream, SocketAddr)> + Send + 'static;
    fn bind(&self, addr: SocketAddr) -> Result<Self::Iter, Error>;
}

pub struct TcpBinder {
    rw_timeout: Option<Duration>,
    /// receiver for Acceptor termination message
    rx: Arc<Mutex<Receiver<()>>>,
    accept_timeout: Option<Duration>,
}

impl TcpBinder {
    pub fn new(
        rw_timeout: Option<Duration>,
        rx: Arc<Mutex<Receiver<()>>>,
        accept_timeout: Option<Duration>,
    ) -> Self {
        Self {
            rw_timeout,
            rx,
            accept_timeout,
        }
    }
}

impl Binder for TcpBinder {
    type Stream = TcpStream;
    type Iter = TcpAcceptor;
    fn bind(&self, addr: SocketAddr) -> Result<Self::Iter, Error> {
        let tcp = net2::TcpBuilder::new_v4()?;
        let tcp = tcp
            .reuse_address(true)?
            .bind(&addr)
            .map_err(|err| addr_error(err, addr))?;

        // `backlog` parameter to `TcpBuilder::listen() is directly passed to `listen(2)` system call.
        // If it is too small, clients may not `connect(2)` to the server.
        // Here, `backlog` is intended to be as large as `net.core.somaxconn` kernel parameter,
        let listener = tcp.listen(256)?;

        Ok(TcpAcceptor::new(
            listener,
            self.rw_timeout,
            self.rx.clone(),
            self.accept_timeout,
        ))
    }
}

fn addr_error(io_err: io::Error, addr: SocketAddr) -> model::Error {
    match io_err.kind() {
        io::ErrorKind::AddrInUse => ErrorKind::AddressAlreadInUse { addr }.into(),
        io::ErrorKind::AddrNotAvailable => ErrorKind::AddressNotAvailable { addr }.into(),
        _ => io_err.context(ErrorKind::Io),
    }
    .into()
}
