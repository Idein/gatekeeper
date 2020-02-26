use crate::byte_stream::ByteStream;
use crate::pkt_stream::PktStream;

pub trait GeneralStream {
    type B: ByteStream;
    type P: PktStream;

    fn byte_stream(&mut self) -> &mut Self::B;
    fn pkt_stream(&mut self) -> Option<&mut Self::P>;

    fn into_inner(self) -> (Self::B, Option<Self::P>);
}

pub struct WrapGeneralStream<B, P> {
    pub byte_stream: B,
    pub pkt_stream: Option<P>,
}

impl<B, P> GeneralStream for WrapGeneralStream<B, P>
where
    B: ByteStream,
    P: PktStream,
{
    type B = B;
    type P = P;
    fn byte_stream(&mut self) -> &mut Self::B {
        &mut self.byte_stream
    }
    fn pkt_stream(&mut self) -> Option<&mut Self::P> {
        self.pkt_stream.as_mut()
    }

    fn into_inner(self) -> (Self::B, Option<Self::P>) {
        (self.byte_stream, self.pkt_stream)
    }
}
