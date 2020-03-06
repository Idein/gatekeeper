use std::io;
use std::net::{self, SocketAddr};

use model::{Error, ErrorKind};

pub trait PktStream {
    fn pkt_size(&self) -> usize;
    fn recv_pkt(&mut self) -> Result<(&[u8], SocketAddr), Error>;
    fn send_pkt(&self, pkt: &[u8], addr: SocketAddr) -> Result<(), Error>;
}

pub struct UdpPktStream {
    pkt_size: usize,
    socket: net::UdpSocket,
    buf: Vec<u8>,
}

impl UdpPktStream {
    pub fn new(pkt_size: usize, socket: net::UdpSocket) -> Self {
        Self {
            pkt_size,
            socket,
            buf: Vec::with_capacity(pkt_size),
        }
    }
}

impl PktStream for UdpPktStream {
    fn pkt_size(&self) -> usize {
        self.pkt_size
    }

    fn recv_pkt(&mut self) -> Result<(&[u8], SocketAddr), Error> {
        let (len, addr) = self.socket.recv_from(&mut self.buf)?;
        Ok((&self.buf[..len], addr))
    }

    fn send_pkt(&self, pkt: &[u8], addr: SocketAddr) -> Result<(), Error> {
        if pkt.len() > self.pkt_size {
            return Err(ErrorKind::PacketSizeLimitExceeded {
                size: pkt.len(),
                limit: self.pkt_size,
            }
            .into());
        }
        self.socket
            .send_to(pkt, addr)
            .and_then(|size| {
                if size == pkt.len() {
                    Ok(())
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("UdpPktStream::send: {} != {}", size, pkt.len()),
                    ))
                }
            })
            .map_err(Into::into)
    }
}
