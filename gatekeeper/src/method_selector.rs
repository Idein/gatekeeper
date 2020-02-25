use model::{Error, Method};

pub trait MethodSelector: Send {
    type A: AuthService;
    /// decide auth method from candidates
    ///
    /// # Details
    /// returns `None` means that no acceptable methods.
    fn select(&self, candidates: &[Method]) -> Result<Option<(Method, Self::A)>, Error>;
    /// enumerate supported auth method
    fn supported(&self) -> &[Method];
}

pub trait AuthService {
    // TODO: impl
    // fn auth(&self) -> Result<RelayConnector, Error>;
}

/// `NoAuth` method compeller
pub struct OnlyNoAuth {
    no_auth: Method,
}

impl OnlyNoAuth {
    pub fn new() -> Self {
        Self {
            no_auth: Method::NoAuth,
        }
    }
}

impl MethodSelector for OnlyNoAuth {
    type A = NoAuthService;
    fn select(&self, candidates: &[Method]) -> Result<Option<(Method, Self::A)>, Error> {
        if candidates.contains(&Method::NoAuth) {
            Ok(Some((Method::NoAuth, NoAuthService)))
        } else {
            Ok(None)
        }
    }
    fn supported(&self) -> &[Method] {
        std::slice::from_ref(&self.no_auth)
    }
}

pub struct NoAuthService;

impl AuthService for NoAuthService {}
