use std::fmt;

use log::*;

use crate::byte_stream::ByteStream;
use crate::relay_connector::{EitherRelayConnector, RelayConnector, WrapRelayConnector};

use model::{model::*, Error};

/// type parameter `S` - type of input connection
pub trait AuthService<S: ByteStream>: fmt::Debug {
    type Relay: RelayConnector<Stream = S> + Send;
    fn auth(&self, conn: S) -> Result<Self::Relay, Error>;
}

#[derive(Debug)]
pub struct NoAuthService;

impl<S> AuthService<S> for NoAuthService
where
    S: ByteStream,
{
    type Relay = WrapRelayConnector<S>;
    fn auth(&self, conn: S) -> Result<Self::Relay, Error> {
        // pass through without any authentication
        Ok(WrapRelayConnector::new(conn))
    }
}

pub fn auth_with_selection<A, S>(
    src_conn: S,
    selection: Option<(Method, A)>,
) -> Result<EitherRelayConnector<A::Relay, WrapRelayConnector<S>>, Error>
where
    S: ByteStream,
    A: AuthService<S>,
{
    use EitherRelayConnector::*;
    if let Some((method, service)) = selection {
        info!("authentication: {}", method);
        // authorize
        service.auth(src_conn).map(LeftRelay)
    } else {
        info!("authentication is not necessary");
        // take over original src connection
        Ok(RightRelay(WrapRelayConnector::new(src_conn)))
    }
}
