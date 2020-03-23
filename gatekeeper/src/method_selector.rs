use std::io;

use model::{Error, Method};

use crate::byte_stream::{BoxedStream, ByteStream};

pub trait MethodSelector: Send {
    /// decide auth method from candidates
    ///
    /// # Details
    /// returns `None` means that no acceptable methods.
    fn select(&self, candidates: &[Method]) -> Result<Option<Method>, Error>;

    /// enumerate supported auth method
    fn supported(&self) -> &[Method];

    /// authentication then return Wrapped stream
    fn authorize<'a, B>(&self, method: Method, conn: B) -> Result<BoxedStream<'a>, Error>
    where
        B: ByteStream + 'a;
}

/// `NoAuth` method compeller
#[derive(Debug, Clone)]
pub struct NoAuthService {
    no_auth: Method,
}

impl NoAuthService {
    pub fn new() -> Self {
        Self {
            no_auth: Method::NoAuth,
        }
    }
}

impl MethodSelector for NoAuthService {
    fn select(&self, candidates: &[Method]) -> Result<Option<Method>, Error> {
        if candidates.contains(&Method::NoAuth) {
            Ok(Some(Method::NoAuth))
        } else {
            Ok(None)
        }
    }

    fn supported(&self) -> &[Method] {
        std::slice::from_ref(&self.no_auth)
    }

    fn authorize<'a, B>(&self, method: Method, conn: B) -> Result<BoxedStream<'a>, Error>
    where
        B: ByteStream + 'a,
    {
        if method != Method::NoAuth {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                method.to_string(),
            ))?;
        }
        // pass through without any authentication
        Ok(Box::new(conn))
    }
}
