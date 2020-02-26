use std::convert::TryInto;
use std::fmt;
use std::io;

use log::*;
use model::{Error, ErrorKind, SocksStream};

use crate::raw_message::{self as raw, *};

/// Wrapper of Read/Write stream
/// for impl SocksStream.
pub struct ReadWriteStreamRef<'a, T> {
    strm: &'a mut T,
}

impl<'a, T> ReadWriteStreamRef<'a, T>
where
    T: io::Read + io::Write,
{
    pub fn new(strm: &'a mut T) -> Self {
        Self { strm }
    }

    fn read_u8(&mut self) -> Result<u8, Error> {
        let mut buf = [0u8; 1];
        self.strm.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn read_u16(&mut self) -> Result<u16, Error> {
        let mut buf = [0u8; 2];
        self.strm.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes([buf[0], buf[1]]))
    }

    fn read_rsv(&mut self) -> Result<u8, Error> {
        let rsv = self.read_u8()?.into();
        if rsv != RESERVED {
            Err(ErrorKind::message_fmt(format_args!("value of rsv is not 0({})", rsv)).into())
        } else {
            Ok(rsv)
        }
    }

    fn read_protocol_version(&mut self) -> Result<ProtocolVersion, Error> {
        let version = self.read_u8()?.into();
        Ok(version)
    }

    fn read_methods(&mut self, nmethod: usize) -> Result<Vec<AuthMethods>, Error> {
        let mut methods = vec![0u8; nmethod];
        self.strm.read_exact(&mut methods)?;
        Ok(methods.into_iter().map(Into::into).collect())
    }

    fn read_addr(&mut self, atyp: AddrType) -> Result<Addr, Error> {
        use AddrType::*;
        match atyp {
            V4 => {
                let mut buf = [0u8; 4];
                self.strm.read_exact(&mut buf)?;
                Ok(Addr::IpAddr(
                    Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]).into(),
                ))
            }
            Domain => {
                let len = self.read_u8()? as usize;
                let mut buf = vec![0u8; len];
                self.strm.read_exact(&mut buf)?;
                Ok(Addr::Domain(buf))
            }
            V6 => {
                let mut buf = [0u8; 16];
                self.strm.read_exact(&mut buf)?;
                let addr: Vec<_> = buf
                    .chunks_exact(2)
                    .map(|c| u16::from_ne_bytes([c[0], c[1]]))
                    .collect();
                Ok(Addr::IpAddr(
                    Ipv6Addr::new(
                        addr[0], addr[1], addr[2], addr[3], addr[4], addr[5], addr[6], addr[7],
                    )
                    .into(),
                ))
            }
        }
    }

    fn write_addr(&self, buf: &mut Vec<u8>, atyp: AddrType, addr: Addr) -> Result<(), Error> {
        use AddrType::*;
        match (atyp, addr) {
            (V4, Addr::IpAddr(IpAddr::V4(addr))) => buf.extend_from_slice(&addr.octets()),
            (V6, Addr::IpAddr(IpAddr::V6(addr))) => buf.extend_from_slice(&addr.octets()),
            (Domain, Addr::Domain(domain)) => buf.extend_from_slice(&domain),
            other => Err(ErrorKind::message_fmt(format_args!(
                "Invalid Address: {:?}",
                other
            )))?,
        }
        Ok(())
    }
}

impl<'a, T> SocksStream for ReadWriteStreamRef<'a, T>
where
    T: io::Read + io::Write,
{
    fn recv_method_candidates(&mut self) -> Result<model::MethodCandidates, Error> {
        trace!("recv_method_candidates");
        let ver = self.read_protocol_version()?;
        let nmethods = self.read_u8()?;
        let methods = self.read_methods(nmethods as usize)?;
        Ok(raw::MethodCandidates { ver, methods }.into())
    }

    fn send_method_selection(
        &mut self,
        method_selection: model::MethodSelection,
    ) -> Result<(), Error> {
        trace!("send_method_selection: {:?}", method_selection);
        let method_selection: raw::MethodSelection = method_selection.into();

        let mut buf = [0u8; 2];
        buf[0] = method_selection.ver.into();
        buf[1] = method_selection.method.code();
        self.strm.write_all(&buf)?;
        Ok(())
    }

    fn recv_connect_request(&mut self) -> Result<model::ConnectRequest, Error> {
        trace!("recv_connect_request");
        let ver = self.read_protocol_version()?;
        let cmd = self
            .read_u8()?
            .try_into()
            .map_err(|_| ErrorKind::message_fmt(format_args!("ConnectRequest::cmd")))?;
        let rsv = self.read_rsv()?;
        let atyp = self
            .read_u8()?
            .try_into()
            .map_err(|_| ErrorKind::message_fmt(format_args!("ConnectRequest::atyp")))?;
        let dst_addr = self.read_addr(atyp)?;
        let dst_port = self.read_u16()?;
        Ok(raw::ConnectRequest {
            ver,
            cmd,
            rsv,
            atyp,
            dst_addr,
            dst_port,
        }
        .try_into()
        .map_err(|err| ErrorKind::message_fmt(format_args!("{}", err)))?)
    }

    fn send_connect_reply(&mut self, connect_reply: model::ConnectReply) -> Result<(), Error> {
        trace!("send_connect_reply: {:?}", connect_reply);
        let connect_reply: raw::ConnectReply = connect_reply.into();
        let mut buf: Vec<u8> = Vec::with_capacity(256);
        buf.push(connect_reply.ver.into());
        buf.push(connect_reply.rep.code());
        buf.push(connect_reply.rsv.into());
        buf.push(connect_reply.atyp as u8);
        self.write_addr(&mut buf, connect_reply.atyp, connect_reply.bnd_addr)?;
        buf.extend_from_slice(&connect_reply.bnd_port.to_be_bytes());
        self.strm.write_all(&buf)?;
        Ok(())
    }
}

pub struct ReadWriteStream<T> {
    strm: T,
}

impl<T: fmt::Debug> fmt::Debug for ReadWriteStream<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ReadWriteStream({:?})", self.strm)
    }
}

impl<T> ReadWriteStream<T>
where
    T: io::Read + io::Write,
{
    pub fn new(strm: T) -> Self {
        Self { strm }
    }
    pub fn into_inner(self) -> T {
        self.strm
    }
    fn rw_stream(&mut self) -> ReadWriteStreamRef<T> {
        ReadWriteStreamRef::new(&mut self.strm)
    }
}

impl<T> SocksStream for ReadWriteStream<T>
where
    T: io::Read + io::Write,
{
    fn recv_method_candidates(&mut self) -> Result<model::MethodCandidates, Error> {
        self.rw_stream().recv_method_candidates()
    }
    fn send_method_selection(
        &mut self,
        method_selection: model::MethodSelection,
    ) -> Result<(), Error> {
        self.rw_stream().send_method_selection(method_selection)
    }
    fn recv_connect_request(&mut self) -> Result<model::ConnectRequest, Error> {
        self.rw_stream().recv_connect_request()
    }
    fn send_connect_reply(&mut self, connect_reply: model::ConnectReply) -> Result<(), Error> {
        self.rw_stream().send_connect_reply(connect_reply)
    }
}
