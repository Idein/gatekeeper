use std::fmt;
use std::io;
use std::net::TcpStream;
use std::ops::Deref;

use model::Error;

/// read/write operations on byte stream
pub trait ByteStream: fmt::Debug + io::Read + io::Write + Send {
    fn split(&self) -> Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>), Error>;
}

/// byte stream on tcp connection
impl ByteStream for TcpStream {
    fn split(&self) -> Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>), Error> {
        let rd = self.try_clone()?;
        let wr = self.try_clone()?;
        Ok((Box::new(rd), Box::new(wr)))
    }
}

/// Boxed stream
impl<S: ByteStream> ByteStream for Box<S> {
    fn split(&self) -> Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>), Error> {
        self.deref().split()
    }
}

pub type BoxedStream<'a> = Box<dyn ByteStream + 'a>;

#[cfg(test)]
pub mod test {
    use super::*;
    use std::borrow::Cow;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone)]
    pub struct BufferStream {
        cursor: Arc<Mutex<io::Cursor<Vec<u8>>>>,
    }

    impl BufferStream {
        #[allow(unused)]
        pub fn new(buffer: Cow<[u8]>) -> Self {
            Self {
                cursor: Arc::new(Mutex::new(io::Cursor::new(buffer.into_owned()))),
            }
        }
    }

    impl io::Read for BufferStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.cursor.lock().unwrap().read(buf)
        }
    }

    impl io::Write for BufferStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.cursor.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.cursor.lock().unwrap().flush()
        }
    }

    impl ByteStream for BufferStream {
        fn split(&self) -> Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>), Error> {
            let rd = Self {
                cursor: self.cursor.clone(),
            };
            let wr = Self {
                cursor: self.cursor.clone(),
            };
            Ok((Box::new(rd), Box::new(wr)))
        }
    }
}
