use crate::byte_stream::BoxedStream;

#[derive(Debug)]
pub struct RelayConnector {
    pub tcp: BoxedStream,
    // TODO: impl
    pub udp: (),
}
