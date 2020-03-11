use std::convert::TryInto;
use std::fmt;
use std::io;
use std::slice;

use failure::ResultExt;
use log::*;
use model::{Error, ErrorKind, SocksStream};

use crate::raw_message::{self as raw, *};

trait ReadSocksExt {
    fn read_u8(&mut self) -> Result<u8, Error>;
    fn read_u16(&mut self) -> Result<u16, Error>;
    fn read_rsv(&mut self) -> Result<u8, Error>;
    fn read_version(&mut self) -> Result<ProtocolVersion, Error>;
    fn read_methods(&mut self, nmethod: usize) -> Result<Vec<AuthMethods>, Error>;
    fn read_cmd(&mut self) -> Result<SockCommand, Error>;
    fn read_atyp(&mut self) -> Result<AddrType, Error>;
    fn read_addr(&mut self, atyp: AddrType) -> Result<Addr, Error>;
    fn read_udp(&mut self) -> Result<UdpHeader, Error>;
}

trait WriteSocksExt {
    fn write_u8(&mut self, v: u8) -> Result<(), Error>;
    fn write_u16(&mut self, v: u16) -> Result<(), Error>;
    fn write_atyp(&mut self, atyp: AddrType) -> Result<(), Error>;
    fn write_addr(&mut self, addr: &Addr) -> Result<(), Error>;
    fn write_version(&mut self, version: ProtocolVersion) -> Result<(), Error>;
    fn write_rep(&mut self, rep: ResponseCode) -> Result<(), Error>;
    fn write_udp(&mut self, header: &UdpHeader) -> Result<(), Error>;
}

impl<T> ReadSocksExt for T
where
    T: io::Read,
{
    fn read_u8(&mut self) -> Result<u8, Error> {
        let mut buf = [0u8; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn read_u16(&mut self) -> Result<u16, Error> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
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

    fn read_version(&mut self) -> Result<ProtocolVersion, Error> {
        let version = self.read_u8()?.into();
        Ok(version)
    }

    fn read_methods(&mut self, nmethod: usize) -> Result<Vec<AuthMethods>, Error> {
        let mut methods = vec![0u8; nmethod];
        self.read_exact(&mut methods)?;
        Ok(methods.into_iter().map(Into::into).collect())
    }

    fn read_cmd(&mut self) -> Result<SockCommand, Error> {
        let cmd = TryInto::<SockCommand>::try_into(self.read_u8()?)
            .context(ErrorKind::message_fmt(format_args!("ConnectRequest::cmd")))?;
        Ok(cmd)
    }

    fn read_atyp(&mut self) -> Result<AddrType, Error> {
        let atyp = TryInto::<AddrType>::try_into(self.read_u8()?)
            .context(ErrorKind::message_fmt(format_args!("ConnectRequest::atyp")))?;
        Ok(atyp)
    }

    fn read_addr(&mut self, atyp: AddrType) -> Result<Addr, Error> {
        use AddrType::*;
        match atyp {
            V4 => {
                let mut buf = [0u8; 4];
                self.read_exact(&mut buf)?;
                Ok(Addr::IpAddr(
                    Ipv4Addr::new(buf[0], buf[1], buf[2], buf[3]).into(),
                ))
            }
            Domain => {
                let len = self.read_u8()? as usize;
                let mut buf = vec![0u8; len];
                self.read_exact(&mut buf)?;
                Ok(Addr::Domain(buf))
            }
            V6 => {
                let mut buf = [0u8; 16];
                self.read_exact(&mut buf)?;
                let addr: Vec<_> = buf
                    .chunks_exact(2)
                    .map(|c| u16::from_be_bytes([c[0], c[1]]))
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

    fn read_udp(&mut self) -> Result<UdpHeader, Error> {
        self.read_rsv()?;
        self.read_rsv()?;
        let frag = self.read_u8()?;
        let atyp = self.read_atyp()?;
        let dst_addr = self.read_addr(atyp)?;
        let dst_port = self.read_u16()?;
        Ok(UdpHeader {
            rsv: 0,
            frag,
            atyp,
            dst_addr,
            dst_port,
        })
    }
}

impl<T> WriteSocksExt for T
where
    T: io::Write,
{
    fn write_u8(&mut self, v: u8) -> Result<(), Error> {
        self.write_all(slice::from_ref(&v))?;
        Ok(())
    }
    fn write_u16(&mut self, v: u16) -> Result<(), Error> {
        self.write_all(&v.to_be_bytes())?;
        Ok(())
    }
    fn write_atyp(&mut self, atyp: AddrType) -> Result<(), Error> {
        self.write_all(slice::from_ref(&(atyp as u8)))?;
        Ok(())
    }
    fn write_addr(&mut self, addr: &Addr) -> Result<(), Error> {
        match addr {
            Addr::IpAddr(IpAddr::V4(addr)) => self.write_all(&addr.octets())?,
            Addr::IpAddr(IpAddr::V6(addr)) => self.write_all(&addr.octets())?,
            Addr::Domain(domain) => self.write_all(&domain)?,
        }
        Ok(())
    }
    fn write_version(&mut self, version: ProtocolVersion) -> Result<(), Error> {
        self.write_all(slice::from_ref(&version.into()))?;
        Ok(())
    }
    fn write_rep(&mut self, rep: ResponseCode) -> Result<(), Error> {
        self.write_all(slice::from_ref(&rep.code()))?;
        Ok(())
    }
    fn write_udp(&mut self, header: &UdpHeader) -> Result<(), Error> {
        self.write_u16(header.rsv)?;
        self.write_u8(header.frag)?;
        self.write_atyp(header.atyp)?;
        self.write_addr(&header.dst_addr)?;
        self.write_u16(header.dst_port)?;
        Ok(())
    }
}

/// Wrapper of Read/Write stream
/// for impl SocksStream.
pub struct ReadWriteStreamRef<'a, T> {
    strm: &'a mut T,
}

impl<'a, T> ReadWriteStreamRef<'a, T> {
    pub fn new(strm: &'a mut T) -> Self {
        Self { strm }
    }
}

impl<'a, T> SocksStream for ReadWriteStreamRef<'a, T>
where
    T: io::Read + io::Write,
{
    fn recv_method_candidates(&mut self) -> Result<model::MethodCandidates, Error> {
        trace!("recv_method_candidates");
        let ver = self.strm.read_version()?;
        let nmethods = self.strm.read_u8()?;
        let methods = self.strm.read_methods(nmethods as usize)?;
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
        let ver = self.strm.read_version()?;
        let cmd = self.strm.read_cmd()?;
        let rsv = self.strm.read_rsv()?;
        let atyp = self.strm.read_atyp()?;
        let dst_addr = self.strm.read_addr(atyp)?;
        let dst_port = self.strm.read_u16()?;
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
        let mut buf: [u8; 256] = [0; 256];
        let pos = {
            let mut cur = io::Cursor::new(&mut buf[..]);
            cur.write_version(connect_reply.ver.clone())?;
            cur.write_rep(connect_reply.rep.into())?;
            cur.write_u8(connect_reply.rsv)?;
            cur.write_atyp(connect_reply.atyp)?;
            cur.write_addr(&connect_reply.bnd_addr)?;
            cur.write_u16(connect_reply.bnd_port)?;
            cur.position() as usize
        };
        self.strm.write_all(&buf[..pos])?;
        Ok(())
    }
}

/// Parse socks5 udp header expected for UDP_ASSOCIATE-d socket
pub fn read_datagram<'a>(buf: &'a [u8]) -> Result<model::UdpDatagram<'a>, model::Error> {
    let mut cur = io::Cursor::new(buf);
    let header = cur.read_udp()?;
    let dst_addr = AddrTriple::new(header.atyp, header.dst_addr, header.dst_port).try_into()?;
    let pos = cur.position() as usize;
    let data = cur.into_inner();
    Ok(model::UdpDatagram {
        frag: header.frag,
        dst_addr,
        data: &data[pos..],
    })
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
