use std::thread::{self, JoinHandle};

use crate::model::Error;

/// spawn `name`d thread performs `f`
pub fn spawn_thread<F, R>(name: &str, f: F) -> Result<JoinHandle<R>, Error>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    thread::Builder::new()
        .name(name.into())
        .spawn(move || f())
        .map_err(Into::into)
}
