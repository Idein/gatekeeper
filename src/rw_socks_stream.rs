use std::convert::TryInto;
use std::fmt;
use std::io;
use std::slice;

use failure::ResultExt;
use log::*;

use crate::model;
use crate::model::{Error, ErrorKind, SocksStream};
use crate::raw_message::{self as raw, *};

trait ReadSocksExt {
    fn read_u8(&mut self) -> Result<u8, Error>;
    fn read_u16(&mut self) -> Result<u16, Error>;
    fn read_rsv(&mut self) -> Result<u8, Error>;
    fn read_version(&mut self) -> Result<ProtocolVersion, Error>;
    fn read_methods(&mut self, nmethod: usize) -> Result<Vec<AuthMethods>, Error>;
    fn read_rep(&mut self) -> Result<ResponseCode, Error>;
    fn read_cmd(&mut self) -> Result<SockCommand, Error>;
    fn read_atyp(&mut self) -> Result<AddrType, Error>;
    fn read_addr(&mut self, atyp: AddrType) -> Result<Addr, Error>;
    fn read_udp(&mut self) -> Result<UdpHeader, Error>;
}

trait WriteSocksExt {
    fn write_u8(&mut self, v: u8) -> Result<(), Error>;
    fn write_u16(&mut self, v: u16) -> Result<(), Error>;
    fn write_cmd(&mut self, cmd: SockCommand) -> Result<(), Error>;
    fn write_atyp(&mut self, atyp: AddrType) -> Result<(), Error>;
    fn write_addr(&mut self, addr: &Addr) -> Result<(), Error>;
    fn write_version(&mut self, version: ProtocolVersion) -> Result<(), Error>;
    fn write_methods(&mut self, nmethods: &[AuthMethods]) -> Result<(), Error>;
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
        let rsv = self.read_u8()?;
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

    fn read_rep(&mut self) -> Result<ResponseCode, Error> {
        let rep = ResponseCode::from_u8(self.read_u8()?).context(ErrorKind::Io)?;
        Ok(rep)
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
    fn write_cmd(&mut self, cmd: SockCommand) -> Result<(), Error> {
        self.write_all(slice::from_ref(&(cmd as u8)))?;
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
            Addr::Domain(domain) => {
                if domain.len() > 255 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("domain name is too long: {:?}", domain),
                    )
                    .into());
                }
                self.write_u8(domain.len() as u8)?;
                self.write_all(&domain)?
            }
        }
        Ok(())
    }
    fn write_version(&mut self, version: ProtocolVersion) -> Result<(), Error> {
        self.write_all(slice::from_ref(&version.into()))?;
        Ok(())
    }
    fn write_methods(&mut self, nmethods: &[AuthMethods]) -> Result<(), Error> {
        let len = nmethods.len();
        let methods: Vec<u8> = nmethods.iter().map(|m| m.code()).collect();
        self.write_u8(len as u8)?;
        self.write_all(methods.as_ref())?;
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
            cur.write_version(connect_reply.ver)?;
            cur.write_rep(connect_reply.rep)?;
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
#[allow(dead_code)]
pub fn read_datagram(buf: &[u8]) -> Result<model::UdpDatagram<'_>, model::Error> {
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

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::byte_stream::test::BufferStream;

    pub fn write_method_candidates<T: io::Write>(
        mut strm: T,
        cand: model::MethodCandidates,
    ) -> Result<(), Error> {
        trace!("recv_method_candidates");
        let cand: raw::MethodCandidates = cand.into();
        strm.write_version(cand.ver)?;
        strm.write_methods(cand.methods.as_ref())?;
        Ok(())
    }

    pub fn write_connect_request<T: io::Write>(
        mut strm: T,
        req: model::ConnectRequest,
    ) -> Result<(), Error> {
        trace!("recv_connect_request");
        let req: raw::ConnectRequest = req.into();
        strm.write_version(req.ver)?;
        strm.write_cmd(req.cmd)?;
        strm.write_u8(req.rsv)?;
        strm.write_atyp(req.atyp)?;
        strm.write_addr(&req.dst_addr)?;
        strm.write_u16(req.dst_port)?;
        Ok(())
    }

    pub fn read_method_selection<T: io::Read>(
        mut strm: T,
    ) -> Result<model::MethodSelection, Error> {
        trace!("read_method_selection");
        let ver = strm.read_version()?;
        let method = strm.read_u8()?.into();
        Ok(raw::MethodSelection { ver, method }.into())
    }

    pub fn read_connect_reply<T: io::Read>(mut strm: T) -> Result<model::ConnectReply, Error> {
        trace!("read_connect_reply");
        let ver = strm.read_version()?;
        let rep = strm.read_rep()?;
        let rsv = strm.read_rsv()?;
        let atyp = strm.read_atyp()?;
        let bnd_addr = strm.read_addr(atyp)?;
        let bnd_port = strm.read_u16()?;
        raw::ConnectReply {
            ver,
            rep,
            rsv,
            atyp,
            bnd_addr,
            bnd_port,
        }
        .try_into()
        .map_err(Into::into)
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Prim {
        fu8: u8,
        fu16: u16,
        fatyp: AddrType,
        faddr: Addr,
        fver: ProtocolVersion,
        frep: ResponseCode,
        fudp: UdpHeader,
    }

    fn read_prims<T>(mut strm: T) -> Result<Prim, Error>
    where
        T: io::Read + io::Write,
    {
        let fu8 = strm.read_u8()?;
        let fu16 = strm.read_u16()?;
        let fatyp = strm.read_atyp()?;
        let faddr = strm.read_addr(fatyp)?;
        let fver = strm.read_version()?;
        let frep = ResponseCode::from_u8(strm.read_u8()?).unwrap();
        let fudp = strm.read_udp()?;
        println!("read_fudp: {:?}", fudp);

        let prim_read = Prim {
            fu8,
            fu16,
            fatyp,
            faddr,
            fver,
            frep,
            fudp,
        };
        println!("read_prim: {:?}", prim_read);

        Ok(prim_read)
    }

    fn write_prims<T: io::Write>(mut strm: T, prim: &Prim) -> Result<(), model::Error> {
        strm.write_u8(prim.fu8)?;
        strm.write_u16(prim.fu16)?;
        strm.write_atyp(prim.fatyp)?;
        strm.write_addr(&prim.faddr)?;
        strm.write_version(prim.fver)?;
        strm.write_rep(prim.frep)?;
        strm.write_udp(&prim.fudp)?;
        println!("write_udp: {:?}", prim.fudp);
        Ok(())
    }

    #[test]
    fn read_write_ext() {
        let prim = Prim {
            fu8: 42,
            fu16: 32854,
            fatyp: AddrType::V4,
            faddr: Addr::IpAddr(Ipv4Addr::new(1, 2, 3, 4).into()),
            fver: 5.into(),
            frep: ResponseCode::NetworkUnreachable,
            fudp: UdpHeader {
                rsv: 0,
                frag: 0,
                atyp: AddrType::V6,
                dst_addr: Addr::IpAddr(Ipv6Addr::new(7, 6, 5, 4, 3, 2, 1, 0).into()),
                dst_port: 835,
            },
        };

        let mut buff = [0u8; 256];
        {
            let mut cursor = io::Cursor::new(&mut buff[..]);
            write_prims(&mut cursor, &prim).unwrap();
        }

        let prim_ = {
            let mut cursor = io::Cursor::new(&mut buff[..]);
            read_prims(&mut cursor).unwrap()
        };

        println!("prim_: {:?}", prim_);
        assert_eq!(prim, prim_);
    }

    #[test]
    fn buffer_stream() {
        use model::dao::*;
        use model::{
            Address, Command, ConnectError, ConnectReply, ConnectRequest, Method, MethodSelection,
        };
        let input = vec![
            5, 1, 0, 5, 6, 0, 1, 2, 0x6a, 0xef, 0xff, 5, 1, 0, 1, 1, 2, 3, 4, 0, 5, 5, 1, 0, 3, 11,
            b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'c', b'o', b'm', 0x7d, 0x6c, 5, 2, 0,
            1, 0, 0, 0, 0, 0x1f, 0x90, 5, 3, 0, 3, 11, b'e', b'x', b'a', b'm', b'p', b'l', b'e',
            b'.', b'c', b'o', b'm', 0x7, 0xe4,
        ];
        let mut strm =
            ReadWriteStream::new(BufferStream::with_buffer((&input).into(), vec![].into()));
        assert_eq!(
            strm.recv_method_candidates().unwrap(),
            model::MethodCandidates {
                version: 5.into(),
                method: vec![model::Method::NoAuth]
            }
        );
        assert_eq!(
            strm.recv_method_candidates().unwrap(),
            model::MethodCandidates {
                version: 5.into(),
                method: vec![
                    Method::NoAuth,
                    Method::GssApi,
                    Method::UserPass,
                    Method::IANAMethod(0x6a),
                    Method::Private(0xef),
                    Method::NoMethods
                ]
            }
        );
        assert_eq!(
            strm.recv_connect_request().unwrap(),
            ConnectRequest {
                version: 5.into(),
                command: Command::Connect,
                connect_to: "1.2.3.4:5".parse().unwrap(),
            }
        );
        assert_eq!(
            strm.recv_connect_request().unwrap(),
            ConnectRequest {
                version: 5.into(),
                command: Command::Connect,
                connect_to: Address::Domain("example.com".into(), 32108),
            }
        );
        assert_eq!(
            strm.recv_connect_request().unwrap(),
            ConnectRequest {
                version: 5.into(),
                command: Command::Bind,
                connect_to: "0.0.0.0:8080".parse().unwrap(),
            }
        );
        assert_eq!(
            strm.recv_connect_request().unwrap(),
            ConnectRequest {
                version: 5.into(),
                command: Command::UdpAssociate,
                connect_to: Address::Domain("example.com".into(), 2020)
            }
        );

        strm.send_method_selection(MethodSelection {
            version: 5.into(),
            method: Method::NoAuth,
        })
        .unwrap();
        strm.send_method_selection(MethodSelection {
            version: 5.into(),
            method: Method::GssApi,
        })
        .unwrap();
        strm.send_method_selection(MethodSelection {
            version: 5.into(),
            method: Method::UserPass,
        })
        .unwrap();
        strm.send_method_selection(MethodSelection {
            version: 5.into(),
            method: Method::IANAMethod(0x7f),
        })
        .unwrap();
        strm.send_method_selection(MethodSelection {
            version: 5.into(),
            method: Method::Private(0xfe),
        })
        .unwrap();
        strm.send_method_selection(MethodSelection {
            version: 5.into(),
            method: Method::NoMethods,
        })
        .unwrap();
        strm.send_connect_reply(ConnectReply {
            version: 5.into(),
            connect_result: Ok(()),
            server_addr: "127.0.0.1:1080".parse().unwrap(),
        })
        .unwrap();
        strm.send_connect_reply(ConnectReply {
            version: 5.into(),
            connect_result: Err(ConnectError::ServerFailure),
            server_addr: Address::Domain("example.com".into(), 8335),
        })
        .unwrap();

        let inner = strm.into_inner();
        // consumed all bytes
        assert_eq!(inner.rd_buff.lock().unwrap().position(), input.len() as u64);
        let out_exp: Vec<u8> = [5, 0]
            .iter()
            .chain([5, 1].iter())
            .chain([5, 2].iter())
            .chain([5, 0x7f].iter())
            .chain([5, 0xfe].iter())
            .chain([5, 0xff].iter())
            .chain([5, 0, 0, 1, 127, 0, 0, 1, 0x4, 0x38].iter())
            .chain(
                [
                    5, 1, 0, 3, 11, b'e', b'x', b'a', b'm', b'p', b'l', b'e', b'.', b'c', b'o',
                    b'm', 0x20, 0x8f,
                ]
                .iter(),
            )
            .cloned()
            .collect();
        assert_eq!(inner.wr_buff.lock().unwrap().clone().into_inner(), out_exp);
        assert_eq!(
            inner.wr_buff.lock().unwrap().position(),
            out_exp.len() as u64
        );
    }
}
