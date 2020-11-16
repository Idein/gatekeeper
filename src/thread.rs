use std::io;

use std::thread::{self, JoinHandle};

/// spawn `name`d thread performs `f`
pub fn spawn_thread<F, R>(name: &str, f: F) -> io::Result<JoinHandle<R>>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    thread::Builder::new().name(name.into()).spawn(move || f())
}
