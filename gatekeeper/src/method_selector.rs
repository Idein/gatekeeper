use model::{Error, Method};

pub trait MethodSelector {
    type A: AuthService;
    /// decide auth method from candidates
    fn select(&self, candidates: &[Method]) -> Result<Option<(Method, Self::A)>, Error>;
    /// enumerate supported auth method
    fn supported(&self) -> &[Method];
}

pub trait AuthService {
    // TODO: impl
    // fn auth(&self) -> Result<RelayConnector, Error>;
}
