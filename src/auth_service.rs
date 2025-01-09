use std::io;

use crate::byte_stream::{BoxedStream, ByteStream};
use crate::model::{Error, Method};

pub trait AuthService: Send {
    /// decide auth method from candidates
    ///
    /// # Details
    /// returns `None` means that no acceptable methods.
    fn select(&self, candidates: &[Method]) -> Result<Option<Method>, Error>;

    /// authentication then return Wrapped stream
    fn authorize<'a, B>(&self, method: Method, conn: B) -> Result<BoxedStream<'a>, Error>
    where
        B: ByteStream + 'a;
}

/// `NoAuth` method compeller
#[derive(Debug, Clone)]
pub struct NoAuthService {}

impl NoAuthService {
    pub fn new() -> Self {
        Self {}
    }
}

impl AuthService for NoAuthService {
    fn select(&self, candidates: &[Method]) -> Result<Option<Method>, Error> {
        if candidates.contains(&Method::NoAuth) {
            Ok(Some(Method::NoAuth))
        } else {
            Ok(None)
        }
    }

    fn authorize<'a, B>(&self, method: Method, conn: B) -> Result<BoxedStream<'a>, Error>
    where
        B: ByteStream + 'a,
    {
        if method != Method::NoAuth {
            let e = io::Error::new(io::ErrorKind::InvalidInput, method.to_string());
            return Err(e.into());
        }
        // pass through without any authentication
        Ok(Box::new(conn))
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::model::ErrorKind;

    #[derive(Debug)]
    pub struct RejectService;

    impl AuthService for RejectService {
        fn select(&self, _candidates: &[Method]) -> Result<Option<Method>, Error> {
            Ok(None)
        }

        /// authentication then return Wrapped stream
        fn authorize<'a, B>(&self, _method: Method, _conn: B) -> Result<BoxedStream<'a>, Error>
        where
            B: ByteStream + 'a,
        {
            Err(ErrorKind::Authentication.into())
        }
    }
}
