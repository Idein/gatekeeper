use std::collections::HashSet;
use std::io::{self, Write};
use std::net::ToSocketAddrs;
use std::os::unix::io::AsRawFd;
use std::thread::{self, JoinHandle};

use log::*;

use crate::auth_service::*;
use crate::byte_stream::{BoxedStream, ByteStream};
use crate::connector::Connector;
use crate::error::Error;
use crate::method_selector::MethodSelector;
use crate::pkt_stream::PktStream;
use crate::raw_message::{AddrType, UdpHeader};
use crate::rw_socks_stream::{read_datagram, ReadWriteStream, WriteSocksExt};

use model::dao::*;
use model::error::ErrorKind;
use model::model::*;

pub struct Session<C, D, S> {
    pub version: ProtocolVersion,
    /// connection from client for auth method negotiation
    pub src_conn: Option<C>,
    pub src_addr: SocketAddr,
    pub dst_connector: D,
    pub method_selector: S,
    pub server_addr: SocketAddr,
}

fn spawn_relay_half<S, D>(
    name: &str,
    mut src: S,
    mut dst: D,
) -> Result<JoinHandle<()>, model::Error>
where
    S: io::Read + Send + 'static,
    D: io::Write + Send + 'static,
{
    info!("spawn_relay_half");
    let name = name.to_owned();
    thread::Builder::new()
        .name(name.clone())
        .spawn(move || {
            info!("spawned: {}", name);
            if let Err(err) = io::copy(&mut src, &mut dst) {
                error!("relay ({}): {}", name, err);
            }
        })
        .map_err(Into::into)
}

fn spawn_relay<R>(
    client_conn: BoxedStream,
    server_conn: R,
) -> Result<(JoinHandle<()>, JoinHandle<()>), model::Error>
where
    R: ByteStream,
{
    info!("spawn_relay");
    let (read_client, write_client) = client_conn.split()?;
    let (read_server, write_server) = server_conn.split()?;
    Ok((
        spawn_relay_half("relay: outbound", read_client, write_server)?,
        spawn_relay_half("relay: incoming", read_server, write_client)?,
    ))
}

///
/// - `client_addr`
///   IP address of the client that will send datagrams to the BND.PORT
fn spawn_udp_relay(
    socks_conn: impl SocksStream + Send + 'static,
    relay: std::net::UdpSocket,
    mut client_addr: SocketAddr,
    server_addr: SocketAddr,
) -> Result<JoinHandle<()>, model::Error> {
    info!("spawn_udp_relay");
    debug!("client_addr: {:?}", client_addr);
    client_addr = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 31338);
    debug!("server_addr: {:?}", server_addr);
    Ok(thread::spawn(move || {
        let _socks_conn = socks_conn;
        let mut buf = [0; 4096];
        let mut dst_set = HashSet::new();
        loop {
            let (size, addr) = relay.recv_from(&mut buf).unwrap();
            debug!("recv_from: {}:{:?}", size, addr);

            if let Some(_) = dst_set.get(&addr) {
                debug!("server: {} -> {}", addr, client_addr);
                let mut udp_buf = [0; 4096];
                let pos = {
                    let mut cur = io::Cursor::new(&mut udp_buf[..]);
                    cur.write_udp(&UdpHeader {
                        rsv: 0,
                        frag: 0,
                        atyp: if addr.ip().is_ipv4() {
                            AddrType::V4
                        } else {
                            AddrType::V6
                        },
                        dst_addr: addr.ip().into(),
                        dst_port: addr.port(),
                    })
                    .unwrap();
                    cur.write_all(&buf[0..size]).unwrap();
                    cur.position()
                };
                relay
                    .send_to(&udp_buf[..pos as usize], client_addr)
                    .unwrap();
            } else if addr.ip() == client_addr.ip() {
                debug!("client: {} -> {}", client_addr, server_addr);
                let datagram = read_datagram(&buf[..size]).unwrap();
                debug!(
                    "datagram: {:?}> {:?}",
                    datagram.dst_addr,
                    String::from_utf8_lossy(datagram.data)
                );
                let addrs: Vec<_> = datagram.dst_addr.to_socket_addrs().unwrap().collect();
                addrs.iter().for_each(|addr| {
                    dst_set.insert(addr.clone());
                });
                debug!("addrs: {:?}", addrs);
                relay.send_to(datagram.data, &addrs[..]).unwrap();
                debug!("send_to: {:?}", datagram.dst_addr);
            }
        }
    }))
}

impl<C, D, S> Session<C, D, S>
where
    C: ByteStream + 'static,
    D: Connector,
    S: MethodSelector,
{
    pub fn new(
        version: ProtocolVersion,
        src_conn: C,
        src_addr: SocketAddr,
        dst_connector: D,
        method_selector: S,
        server_addr: SocketAddr,
    ) -> Self {
        Self {
            version,
            src_conn: Some(src_conn),
            src_addr,
            dst_connector,
            method_selector,
            server_addr,
        }
    }

    fn connect_reply<R>(&self, connect_result: Result<(), R>) -> ConnectReply
    where
        ConnectError: From<R>,
    {
        ConnectReply {
            version: self.version,
            connect_result: connect_result.map_err(Into::into),
            server_addr: self.server_addr.clone().into(),
        }
    }

    pub fn start(&mut self) -> std::result::Result<(), Error> {
        let (src_conn, method) = {
            let mut src_conn = self.src_conn.take().unwrap();
            let mut strm = ReadWriteStream::new(&mut src_conn);
            let candidates = strm.recv_method_candidates()?;
            trace!("candidates: {:?}", candidates);

            let selection = self.method_selector.select(&candidates.method)?;
            trace!("selection: {:?}", selection);

            match selection {
                Some((method, auth)) => {
                    strm.send_method_selection(MethodSelection {
                        version: self.version,
                        method: method.clone(),
                    })?;
                    (src_conn, auth)
                }
                None => {
                    // no acceptable method
                    strm.send_method_selection(MethodSelection {
                        version: self.version,
                        method: Method::NoMethods,
                    })?;
                    return Err(model::Error::from(ErrorKind::NoAcceptableMethod).into());
                }
            }
        };

        let relay = method.auth(src_conn)?;

        let mut strm = ReadWriteStream::new(relay);
        let conn_req = strm.recv_connect_request()?;
        debug!("connect request: {:?}", conn_req);
        match &conn_req.command {
            Command::Connect => {
                let dst_conn = match self
                    .dst_connector
                    .connect_byte_stream(conn_req.connect_to.clone())
                {
                    Ok(conn) => {
                        strm.send_connect_reply(self.connect_reply::<model::ErrorKind>(Ok(())))?;
                        conn
                    }
                    Err(err) => {
                        error!("connect error: {:?}", err);
                        strm.send_connect_reply(self.connect_reply(Err(err.kind().clone())))?;
                        return Err(err.into());
                    }
                };
                spawn_relay(strm.into_inner(), dst_conn)?;
            }
            // UDP接続をTCP接続へ関連付け
            Command::UdpAssociate => {
                trace!("UdpAssociate");
                let udp_relay = net2::UdpBuilder::new_v4()?
                    .reuse_address(true)?
                    .bind(self.server_addr.clone())?;
                trace!("bind: {:?}", self.server_addr.clone());
                strm.send_connect_reply(self.connect_reply::<model::ErrorKind>(Ok(())))?;
                trace!(
                    "send_connect_reply: {:?}",
                    self.connect_reply::<model::ErrorKind>(Ok(()))
                );
                let server_addr = conn_req.connect_to.clone();
                trace!("server_addr: {:?}", server_addr);
                spawn_udp_relay(
                    strm,
                    udp_relay,
                    self.src_addr.clone(),
                    server_addr.to_socket_addrs()?.next().unwrap(),
                )?;
            }
            // サーバからクライアントへの接続を中継
            cmd @ Command::Bind => {
                debug!("command not supported: {:?}", cmd);
                let not_supported: model::Error = ErrorKind::command_not_supported(*cmd).into();
                // reply error
                let rep = self.connect_reply(Err(not_supported.kind().clone()));
                strm.send_connect_reply(rep)?;
                return Err(not_supported.into());
            }
        }

        Ok(())
    }
}
