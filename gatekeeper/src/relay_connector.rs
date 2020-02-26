use std::fmt;
use std::io;

use crate::byte_stream::ByteStream;
use crate::either::Either;
use crate::pkt_stream::PktStream;
use crate::rw_socks_stream::ReadWriteStream;

use model::{dao::*, Error, L4Protocol};

pub trait RelayConnector: fmt::Debug {
    type Byte: ByteStream;

    fn split(self) -> Result<(Self::Byte, Self::Byte), Error>;
}

pub struct WrapRelayConnector<B> {
    socks_stream: ReadWriteStream<B>,
}

impl<B> WrapRelayConnector<B>
where
    B: ByteStream,
{
    pub fn new(byte_stream: B) -> Self {
        Self {
            socks_stream: ReadWriteStream::new(byte_stream),
        }
    }
}

impl<B: fmt::Debug> fmt::Debug for WrapRelayConnector<B> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "WrapRelayConnector({:?})", self.socks_stream)
    }
}

/*
impl<B> RelayConnector for WrapRelayConnector<B>
where
    B: ByteStream,
{
    type Byte = B;
    type Socks = ReadWriteStream<Self::Byte>;

    fn socks(&mut self) -> &mut Self::Socks {
        &mut self.socks_stream
    }
    fn relay_stream(&self) -> Result<Self::Byte, Error> {
        self.socks_stream.try_clone()
    }
}
*/

pub enum EitherRelayConnector<T, U> {
    LeftRelay(T),
    RightRelay(U),
}

impl<T, U> fmt::Debug for EitherRelayConnector<T, U>
where
    T: RelayConnector,
    U: RelayConnector,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use EitherRelayConnector::*;
        match self {
            LeftRelay(r) => r.fmt(f),
            RightRelay(r) => r.fmt(f),
        }
    }
}

/*
impl<T, U> RelayConnector for EitherRelayConnector<T, U>
where
    T: RelayConnector + Send,
    U: RelayConnector + Send,
{
    type Byte = EitherRelayConnector<T, U>;
    type Socks = Self;

    fn socks(&mut self) -> &mut Self::Socks {
        &mut self
    }
    fn relay_stream(self) -> Self::Byte {
        self
    }
}

impl<T, U> io::Read for EitherRelayConnector<T, U>
where
    T: RelayConnector,
    U: RelayConnector,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use EitherRelayConnector::*;
        match self {
            LeftRelay(r) => r.byte_stream().read(buf),
            RightRelay(r) => r.byte_stream().read(buf),
        }
    }
}

impl<T, U> io::Write for EitherRelayConnector<T, U>
where
    T: RelayConnector,
    U: RelayConnector,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use EitherRelayConnector::*;
        match self {
            LeftRelay(r) => r.byte_stream().write(buf),
            RightRelay(r) => r.byte_stream().write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        use EitherRelayConnector::*;
        match self {
            LeftRelay(r) => r.byte_stream().flush(),
            RightRelay(r) => r.byte_stream().flush(),
        }
    }
}
*/
