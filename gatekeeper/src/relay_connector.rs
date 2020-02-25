use std::fmt;
use std::io;

use crate::byte_stream::ByteStream;

pub trait RelayConnector: fmt::Debug {
    type Stream: ByteStream;
    fn tcp_stream(&mut self) -> &mut Self::Stream;
}

#[derive(Debug)]
pub struct WrapRelayConnector<T> {
    tcp: T,
    // TODO: impl
    udp: (),
}

impl<T> WrapRelayConnector<T> {
    pub fn new(tcp: T) -> Self {
        Self { tcp, udp: () }
    }
}

impl<T> RelayConnector for WrapRelayConnector<T>
where
    T: ByteStream,
{
    type Stream = T;
    fn tcp_stream(&mut self) -> &mut Self::Stream {
        &mut self.tcp
    }
}

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

impl<T, U> RelayConnector for EitherRelayConnector<T, U>
where
    T: RelayConnector + Send,
    U: RelayConnector + Send,
{
    type Stream = EitherRelayConnector<T, U>;
    fn tcp_stream(&mut self) -> &mut Self::Stream {
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
            LeftRelay(r) => r.tcp_stream().read(buf),
            RightRelay(r) => r.tcp_stream().read(buf),
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
            LeftRelay(r) => r.tcp_stream().write(buf),
            RightRelay(r) => r.tcp_stream().write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        use EitherRelayConnector::*;
        match self {
            LeftRelay(r) => r.tcp_stream().flush(),
            RightRelay(r) => r.tcp_stream().flush(),
        }
    }
}

impl<T, U> ByteStream for EitherRelayConnector<T, U>
where
    T: RelayConnector + Send,
    U: RelayConnector + Send,
{
}
