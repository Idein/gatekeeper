use std::fmt;
use std::io;
use std::net::TcpStream;
use std::ops::Deref;

use crate::model::Error;

/// read/write operations on byte stream
pub trait ByteStream: fmt::Debug + io::Read + io::Write + Send {
    #[allow(clippy::type_complexity)]
    fn split(&self) -> Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>), Error>;
}

/// byte stream on tcp connection
impl ByteStream for TcpStream {
    #[allow(clippy::type_complexity)]
    fn split(&self) -> Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>), Error> {
        let rd = self.try_clone()?;
        let wr = self.try_clone()?;
        Ok((Box::new(rd), Box::new(wr)))
    }
}

/// Boxed stream
impl<S: ByteStream> ByteStream for Box<S> {
    #[allow(clippy::type_complexity)]
    fn split(&self) -> Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>), Error> {
        self.deref().split()
    }
}

pub type BoxedStream<'a> = Box<dyn ByteStream + 'a>;

#[cfg(test)]
pub mod test {
    use super::*;
    use std::borrow::Cow;
    use std::io::{self};
    use std::sync::{Arc, Mutex, MutexGuard};

    #[derive(Debug, Clone)]
    pub struct BufferStream {
        pub rd_buff: Arc<Mutex<io::Cursor<Vec<u8>>>>,
        pub wr_buff: Arc<Mutex<io::Cursor<Vec<u8>>>>,
    }

    impl BufferStream {
        pub fn new() -> Self {
            BufferStream::with_buffer(vec![].into(), vec![].into())
        }

        pub fn with_buffer(rd: Cow<[u8]>, wr: Cow<[u8]>) -> Self {
            Self {
                rd_buff: Arc::new(Mutex::new(io::Cursor::new(rd.into_owned()))),
                wr_buff: Arc::new(Mutex::new(io::Cursor::new(wr.into_owned()))),
            }
        }

        pub fn rd_buff<'a>(&'a self) -> MutexGuard<'a, io::Cursor<Vec<u8>>> {
            self.rd_buff.lock().unwrap()
        }

        pub fn wr_buff<'a>(&'a self) -> MutexGuard<'a, io::Cursor<Vec<u8>>> {
            self.wr_buff.lock().unwrap()
        }
    }

    impl io::Read for BufferStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.rd_buff.lock().unwrap().read(buf)
        }
    }

    impl io::Write for BufferStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.wr_buff.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.wr_buff.lock().unwrap().flush()
        }
    }

    impl ByteStream for BufferStream {
        fn split(&self) -> Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>), Error> {
            let rd = Self {
                rd_buff: self.rd_buff.clone(),
                wr_buff: self.wr_buff.clone(),
            };
            let wr = Self {
                rd_buff: self.rd_buff.clone(),
                wr_buff: self.wr_buff.clone(),
            };
            Ok((Box::new(rd), Box::new(wr)))
        }
    }

    #[derive(Debug, Clone)]
    pub struct IterBuffer<T> {
        pub iter: T,
        pub wr_buff: Arc<Mutex<io::Cursor<Vec<u8>>>>,
    }

    impl<T> IterBuffer<T>
    where
        T: Iterator<Item = Vec<u8>>,
    {
        pub fn new(iter: T, wr_buff: io::Cursor<Vec<u8>>) -> Self {
            Self {
                iter,
                wr_buff: Arc::new(Mutex::new(wr_buff)),
            }
        }
    }

    impl<T> io::Read for IterBuffer<T>
    where
        T: Iterator<Item = Vec<u8>>,
    {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if let Some(chunk) = self.iter.next() {
                (&chunk[..]).read(buf)
            } else {
                Ok(0)
            }
        }
    }

    impl<T> io::Write for IterBuffer<T>
    where
        T: Iterator<Item = Vec<u8>>,
    {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            log::debug!("IterBuffer::write({})", String::from_utf8_lossy(buf));
            self.wr_buff.lock().unwrap().write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.wr_buff.lock().unwrap().flush()
        }
    }

    impl<T> ByteStream for IterBuffer<T>
    where
        T: fmt::Debug + Iterator<Item = Vec<u8>> + Clone + Send + 'static,
    {
        fn split(&self) -> Result<(Box<dyn io::Read + Send>, Box<dyn io::Write + Send>), Error> {
            let rd = Box::new(self.clone()) as Box<dyn io::Read + Send>;
            let wr = Box::new(self.clone()) as Box<dyn io::Write + Send>;
            Ok((rd, wr))
        }
    }

    #[test]
    fn iter_buffer() {
        use io::{Read, Write};

        let mut iter_buffer = IterBuffer::new(
            vec![b"hello".to_vec(), b"world".to_vec()].into_iter(),
            io::Cursor::new(vec![]),
        );
        let mut buff: Vec<u8> = std::iter::repeat(0).take(256).collect();

        let size = iter_buffer.read(&mut buff).unwrap();
        assert_eq!(&b"hello"[..], &buff[..size]);
        let size = iter_buffer.read(&mut buff).unwrap();
        assert_eq!(&b"world"[..], &buff[..size]);

        iter_buffer.write(&b"hello"[..]).unwrap();
        iter_buffer.write(&b" "[..]).unwrap();
        iter_buffer.write(&b"world"[..]).unwrap();
        let wr_buff = iter_buffer.wr_buff.lock().unwrap();
        assert_eq!(wr_buff.get_ref().as_slice(), &b"hello world"[..])
    }
}
