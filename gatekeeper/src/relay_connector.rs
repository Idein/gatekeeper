use crate::byte_stream::{BoxedStream, ByteStream};

#[derive(Debug)]
pub struct RelayConnector {
    tcp: BoxedStream,
    // TODO: impl
    udp: (),
}

impl RelayConnector {
    pub fn new<S>(tcp: S) -> Self
    where
        S: ByteStream + 'static,
    {
        Self {
            tcp: Box::new(tcp),
            udp: (),
        }
    }

    pub fn tcp(&self) -> &dyn ByteStream {
        &self.tcp
    }
}
