use std::fmt;

use log::*;

use crate::byte_stream::{BoxedStream, ByteStream};
use crate::pkt_stream::PktStream;
use crate::relay_connector::{EitherRelayConnector, RelayConnector, WrapRelayConnector};

use model::{dao::*, model::*, Error};

pub trait AuthService: fmt::Debug {
    /// input byte stream
    // type Byte: ByteStream;
    /// wrapped stream
    // type Relay: RelayConnector;

    /// authentication then return Wrapped stream
    fn auth<B>(&self, conn: B) -> Result<BoxedStream, Error>
    where
        B: ByteStream + 'static;
}

#[derive(Debug)]
pub struct NoAuthService;

impl AuthService for NoAuthService {
    // type Relay = WrapRelayConnector<B>;
    fn auth<B>(&self, conn: B) -> Result<BoxedStream, Error>
    where
        B: ByteStream + 'static,
    {
        // pass through without any authentication
        Ok(Box::new(conn))
    }
}

/*
pub fn auth_with_selection<A, B>(
    src_conn: B,
    selection: (Method, A),
) -> Result<impl RelayConnector, Error>
where
    B: ByteStream,
    A: AuthService,
{
    let (method, service) = selection;
    info!("authentication: {}", method);
    // authorize
    service.auth(src_conn).map(LeftRelay)
}
*/
