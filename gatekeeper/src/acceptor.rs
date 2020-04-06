use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

use failure::Fail;
use log::*;

use crate::byte_stream::ByteStream;
use crate::error::Error;

pub struct TcpAcceptor {
    listener: TcpListener,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
}

impl Iterator for TcpAcceptor {
    type Item = (TcpStream, SocketAddr);
    fn next(&mut self) -> Option<Self::Item> {
        match self.listener.accept() {
            Ok((tcp, addr)) => {
                if let Err(err) = tcp.set_read_timeout(self.read_timeout.clone()) {
                    error!("set_read_timeout({:?}): {:?}", self.read_timeout, err);
                    return None;
                }
                if let Err(err) = tcp.set_write_timeout(self.write_timeout.clone()) {
                    error!("set_write_timeout({:?}): {:?}", self.write_timeout, err);
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
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
}

impl TcpBinder {
    pub fn new(read_timeout: Option<Duration>, write_timeout: Option<Duration>) -> Self {
        Self {
            read_timeout,
            write_timeout,
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
        Ok(TcpAcceptor {
            listener: tcp.listen(0)?,
            read_timeout: self.read_timeout,
            write_timeout: self.write_timeout,
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
