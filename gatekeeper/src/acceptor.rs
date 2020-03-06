use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};

use log::*;

use crate::byte_stream::ByteStream;
use crate::error::Error;

pub struct TcpAcceptor {
    tcp_listener: TcpListener,
    udp_socket: UdpSocket,
}

impl Iterator for TcpAcceptor {
    type Item = (TcpStream, SocketAddr);
    fn next(&mut self) -> Option<Self::Item> {
        match self.tcp_listener.accept() {
            Ok(x) => Some(x),
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
    fn bind<A: ToSocketAddrs + Clone>(&self, addr: A) -> Result<Self::Iter, Error>;
}

pub struct TcpUdpBinder;

impl Binder for TcpUdpBinder {
    type Stream = TcpStream;
    type Iter = TcpAcceptor;
    fn bind<A: ToSocketAddrs + Clone>(&self, addr: A) -> Result<Self::Iter, Error> {
        let tcp_listener = TcpListener::bind(addr.clone())?;
        let udp_socket = UdpSocket::bind(addr)?;
        Ok(TcpAcceptor {
            tcp_listener,
            udp_socket,
        })
    }
}
