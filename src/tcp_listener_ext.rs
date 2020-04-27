use std::convert::TryInto;
use std::io;
use std::mem;
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6, TcpListener, TcpStream};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::time::Duration;

use nix::sys::time::{TimeVal, TimeValLike};

pub trait TcpListenerExt {
    fn accept_timeout(&self, timeout: Option<Duration>) -> io::Result<(TcpStream, SocketAddr)>;
}

impl TcpListenerExt for TcpListener {
    /// accept(2) with timeout
    ///
    /// * `timeout`
    ///   Timeout for _accept_. If the value is `None`, wait connection indefinitely.
    fn accept_timeout(&self, timeout: Option<Duration>) -> io::Result<(TcpStream, SocketAddr)> {
        use nix::sys::select::*;

        let fd = self.as_raw_fd();

        let mut tm = timeout.map(dur_to_timeval::<TimeVal>).transpose()?;

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

/// Convert Duration to timeval in microseconds
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

/// Convert sockaddr_storage to SocketAddr
///
/// * `storage`
///   The target value contains an address.
///   The actual type of the value depends on it's address family.
/// * `len`
///   The sizeof `storage` in bytes.
///   This should larger than or equals to the size of the *actual* type of `storage`.
fn sockaddr_to_addr(storage: &libc::sockaddr_storage, len: usize) -> io::Result<SocketAddr> {
    match storage.ss_family as libc::c_int {
        libc::AF_INET => {
            assert!(len as usize >= mem::size_of::<libc::sockaddr_in>());
            let addr = unsafe { *(storage as *const _ as *const libc::sockaddr_in) };
            Ok(SocketAddrV4::new(addr.sin_addr.s_addr.into(), addr.sin_port).into())
        }
        libc::AF_INET6 => {
            assert!(len as usize >= mem::size_of::<libc::sockaddr_in6>());
            let addr = unsafe { *(storage as *const _ as *const libc::sockaddr_in6) };
            Ok(SocketAddrV6::new(
                addr.sin6_addr.s6_addr.into(),
                addr.sin6_port,
                addr.sin6_flowinfo,
                addr.sin6_scope_id,
            )
            .into())
        }
        af_family => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid argument: {}", af_family),
        )),
    }
}
