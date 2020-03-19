use std::fmt;

use crate::byte_stream::{BoxedStream, ByteStream};

use model::Error;

pub trait AuthService: fmt::Debug {
    /// authentication then return Wrapped stream
    fn auth<'a, B>(&self, conn: B) -> Result<BoxedStream<'a>, Error>
    where
        B: ByteStream + 'a;
}

#[derive(Debug)]
pub struct NoAuthService;

impl AuthService for NoAuthService {
    fn auth<'a, B>(&self, conn: B) -> Result<BoxedStream<'a>, Error>
    where
        B: ByteStream + 'a,
    {
        // pass through without any authentication
        Ok(Box::new(conn))
    }
}
