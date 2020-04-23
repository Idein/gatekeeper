use std::convert::TryInto;
use std::io;
use std::mem;
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6, TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::time::Duration;

use nix::sys::time::{TimeVal, TimeValLike};

pub trait TcpListenerExt {
    fn accept_timeout(&self, timeout: Duration) -> io::Result<(TcpStream, SocketAddr)>;
}

impl TcpListenerExt for TcpListener {
    fn accept_timeout(&self, timeout: Duration) -> io::Result<(TcpStream, SocketAddr)> {
        use nix::sys::select::*;

        let fd = self.as_raw_fd();

        let mut tm = dur_to_timeval::<TimeVal>(timeout)?;

        let mut fds = FdSet::new();
        fds.insert(fd);
        let r = select(None, &mut fds, None, None, &mut tm)
            .map_err(|err| io::Error::from_raw_os_error(err.as_errno().unwrap() as i32))?;
        if r == 0 {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "select accept"));
        }
        assert!(r == 1);
        assert!(fds.contains(fd));

        let mut storage: libc::sockaddr_storage = unsafe { mem::zeroed() };
        let mut len = mem::size_of_val(&storage) as libc::socklen_t;
        unsafe {
            let accepted =
                libc::accept(fd, &mut storage as *mut _ as *mut libc::sockaddr, &mut len);
            if accepted < 0 {
                return Err(io::Error::last_os_error());
            }
            let addr = sockaddr_to_addr(&storage, len as usize)?;
            Ok((TcpStream::from_raw_fd(accepted), addr))
        }
    }
}

fn dur_to_timeval<T: TimeValLike>(dur: Duration) -> io::Result<T> {
    dur.as_micros()
        .try_into()
        .map(T::microseconds)
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("timeout convert error: {:?}", dur),
            )
        })
}

fn sockaddr_to_addr(storage: &libc::sockaddr_storage, len: usize) -> io::Result<SocketAddr> {
    match storage.ss_family as libc::c_int {
        libc::AF_INET => {
            assert!(len as usize >= mem::size_of::<libc::sockaddr_in>());
            let addr = unsafe { *(storage as *const _ as *const libc::sockaddr_in) };
            Ok(SocketAddr::V4(SocketAddrV4::new(
                addr.sin_addr.s_addr.into(),
                addr.sin_port,
            )))
        }
        libc::AF_INET6 => {
            assert!(len as usize >= mem::size_of::<libc::sockaddr_in6>());
            let addr = unsafe { *(storage as *const _ as *const libc::sockaddr_in6) };
            Ok(SocketAddr::V6(SocketAddrV6::new(
                addr.sin6_addr.s6_addr.into(),
                addr.sin6_port,
                addr.sin6_flowinfo,
                addr.sin6_scope_id,
            )))
        }
        af_family => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid argument: {}", af_family),
        )),
    }
}
