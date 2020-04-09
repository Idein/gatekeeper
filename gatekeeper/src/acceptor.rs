use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use failure::Fail;
use log::*;

use crate::byte_stream::ByteStream;
use crate::error::Error;

pub struct TcpAcceptor {
    listener: TcpListener,
    rw_timeout: Option<Duration>,
}

impl Iterator for TcpAcceptor {
    type Item = (TcpStream, SocketAddr);
    fn next(&mut self) -> Option<Self::Item> {
        match self.listener.accept() {
            Ok((tcp, addr)) => {
                if let Err(err) = tcp.set_read_timeout(self.rw_timeout.clone()) {
                    error!("set_read_timeout({:?}): {:?}", self.rw_timeout, err);
                    return None;
                }
                if let Err(err) = tcp.set_write_timeout(self.rw_timeout.clone()) {
                    error!("set_write_timeout({:?}): {:?}", self.rw_timeout, err);
                    return None;
                }
                Some((tcp, addr))
            }
            Err(err) => {
                error!("accept error: {}", err);
                trace!("accept error: {:?}", err);
                None
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
}

impl TcpBinder {
    pub fn new(rw_timeout: Option<Duration>) -> Self {
        Self { rw_timeout }
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
        Ok(TcpAcceptor {
            listener: tcp.listen(0)?,
            rw_timeout: self.rw_timeout,
        })
    }
}

fn addr_error(io_err: io::Error, addr: SocketAddr) -> model::Error {
    use model::ErrorKind;
    match io_err.kind() {
        io::ErrorKind::AddrInUse => ErrorKind::AddressAlreadInUse { addr }.into(),
        io::ErrorKind::AddrNotAvailable => ErrorKind::AddressNotAvailable { addr }.into(),
        _ => io_err.context(ErrorKind::Io),
    }
    .into()
}
