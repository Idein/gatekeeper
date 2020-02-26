use std::io;
use std::net::TcpStream;

pub trait TryClone: Sized {
    fn try_clone(&self) -> io::Result<Self>;
}

impl TryClone for TcpStream {
    fn try_clone(&self) -> io::Result<Self> {
        TcpStream::try_clone(self)
    }
}

impl<T: TryClone> TryClone for Box<T> {
    fn try_clone(&self) -> io::Result<Self> {
        T::try_clone(self).map(Box::new)
    }
}
