use std::fmt;

use crate::byte_stream::{BoxedStream, ByteStream};

use model::Error;

pub trait AuthService: fmt::Debug {
    /// authentication then return Wrapped stream
    fn auth<B>(&self, conn: B) -> Result<BoxedStream, Error>
    where
        B: ByteStream + 'static;
}

#[derive(Debug)]
pub struct NoAuthService;

impl AuthService for NoAuthService {
    fn auth<B>(&self, conn: B) -> Result<BoxedStream, Error>
    where
        B: ByteStream + 'static,
    {
        // pass through without any authentication
        Ok(Box::new(conn))
    }
}
