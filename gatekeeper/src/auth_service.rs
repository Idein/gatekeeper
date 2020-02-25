use std::fmt;

use crate::byte_stream::ByteStream;
use crate::relay_connector::RelayConnector;

use model::Error;

pub trait AuthService: fmt::Debug {
    fn auth(&self, conn: impl ByteStream + 'static) -> Result<RelayConnector, Error>;
}

#[derive(Debug)]
pub struct NoAuthService;

impl AuthService for NoAuthService {
    fn auth(&self, conn: impl ByteStream + 'static) -> Result<RelayConnector, Error> {
        // pass through without any authentication
        Ok(RelayConnector::new(conn))
    }
}
