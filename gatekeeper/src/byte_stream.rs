use std::fmt;
use std::io;
use std::net::TcpStream;

use model::Error;

use crate::try_clone::TryClone;

/// read/write operations on byte stream
pub trait ByteStream: fmt::Debug + io::Read + io::Write /*+ TryClone */ + Send {
    fn split(&self) -> io::Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>)>;
}

/// byte stream on tcp connection
impl ByteStream for TcpStream {
    fn split(&self) -> io::Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>)> {
        let rd = self.try_clone()?;
        let wr = self.try_clone()?;
        Ok((Box::new(rd), Box::new(wr)))
    }
}

/// Boxed stream
impl<S: ByteStream> ByteStream for Box<S> {
    fn split(&self) -> io::Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>)> {
        self.split()
    }
}

pub type BoxedStream = Box<dyn ByteStream>;

pub enum EitherStream<T, U> {
    Left(T),
    Right(U),
}

impl<T, U> fmt::Debug for EitherStream<T, U>
where
    T: fmt::Debug,
    U: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EitherStream::Left(s) => s.fmt(f),
            EitherStream::Right(s) => s.fmt(f),
        }
    }
}

/*
impl<T, U> io::Read for EitherStream<T, U>
where
    T: io::Read,
    U: io::Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            EitherStream::Left(s) => s.read(buf),
            EitherStream::Right(s) => s.read(buf),
        }
    }
}

impl<T, U> io::Write for EitherStream<T, U>
where
    T: io::Write,
    U: io::Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            EitherStream::Left(s) => s.write(buf),
            EitherStream::Right(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            EitherStream::Left(s) => s.flush(),
            EitherStream::Right(s) => s.flush(),
        }
    }
}

impl<T, U> TryClone for EitherStream<T, U>
where
    T: ByteStream,
    U: ByteStream,
{
    fn try_clone(&self) -> Result<Self, Error> {
        match self {
            EitherStream::Left(s) => s.try_clone(),
            EitherStream::Right(s) => s.try_clone(),
        }
    }
}

impl<T, U> ByteStream for EitherStream<T, U>
where
    T: ByteStream,
    U: ByteStream,
{
}

#[cfg(test)]
pub mod test {
    use super::*;
    use std::borrow::Cow;

    #[derive(Debug, Clone)]
    pub struct BufferStream {
        cursor: io::Cursor<Vec<u8>>,
    }

    impl BufferStream {
        #[allow(unused)]
        pub fn new(buffer: Cow<[u8]>) -> Self {
            Self {
                cursor: io::Cursor::new(buffer.into_owned()),
            }
        }
    }

    impl io::Read for BufferStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.cursor.read(buf)
        }
    }

    impl io::Write for BufferStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.cursor.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.cursor.flush()
        }
    }

    impl ByteStream for BufferStream {}
}
*/
