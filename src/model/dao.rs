use super::error::Error;
use super::model::*;

pub trait HaveDao {
    type Stream: SocksStream;

    fn stream(&self) -> &Self::Stream;
}

pub trait SocksStream {
    fn recv_method_candidates(&mut self) -> Result<MethodCandidates, Error>;
    fn send_method_selection(&mut self, method: MethodSelection) -> Result<(), Error>;
    fn recv_connect_request(&mut self) -> Result<ConnectRequest, Error>;
    fn send_connect_reply(&mut self, reply: ConnectReply) -> Result<(), Error>;
}
