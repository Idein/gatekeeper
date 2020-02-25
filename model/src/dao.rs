use crate::error::Error;
use crate::model::*;

pub trait HaveDao {
    type Stream: SocksStream;

    fn stream(&self) -> &Self::Stream;
}

pub trait SocksStream {
    fn recv_method_candidates(&self) -> Result<MethodCandidates, Error>;
    fn send_method_selection(&self, method: MethodSelection) -> Result<(), Error>;
    fn recv_connect_request(&self) -> Result<ConnectRequest, Error>;
    fn send_connect_reply(&self, reply: ConnectReply) -> Result<(), Error>;
}
