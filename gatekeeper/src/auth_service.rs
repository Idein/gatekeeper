use crate::byte_stream::ByteStream;
use crate::relay_connector::RelayConnector;

use model::Error;

pub trait AuthService {
    fn auth(&self, conn: impl ByteStream + 'static) -> Result<RelayConnector, Error>;
}

pub struct NoAuthService;

impl AuthService for NoAuthService {
    fn auth(&self, conn: impl ByteStream + 'static) -> Result<RelayConnector, Error> {
        Ok(RelayConnector {
            tcp: Box::new(conn),
            udp: (),
        })
    }
}
