use std::io;
use std::net::TcpStream;

pub trait ByteStream: io::Read + io::Write {}

impl ByteStream for TcpStream {}

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
