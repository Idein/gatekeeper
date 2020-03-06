use std::io;
use std::net::ToSocketAddrs;
use std::thread::{self, JoinHandle};

use log::*;

use crate::auth_service::*;
use crate::byte_stream::{BoxedStream, ByteStream};
use crate::connector::Connector;
use crate::error::Error;
use crate::method_selector::MethodSelector;
use crate::pkt_stream::PktStream;
use crate::rw_socks_stream::{read_datagram, ReadWriteStream};

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
    client_addr: SocketAddr,
    server_addr: SocketAddr,
) -> Result<JoinHandle<()>, model::Error> {
    info!("spawn_udp_relay");
    Ok(thread::spawn(move || {
        let _socks_conn = socks_conn;
        let mut buf = [0; 4096];
        loop {
            let (size, addr) = relay.recv_from(&mut buf).unwrap();
            if addr == client_addr {
                debug!("client: {} -> {}", client_addr, server_addr);
                let datagram = read_datagram(&buf[..size]).unwrap();
                debug!("datagram: {:?}", datagram);
                relay.send_to(datagram.data, server_addr).unwrap();
            } else if addr == server_addr {
                debug!("server: {} -> {}", server_addr, client_addr);
                relay.send_to(&buf[..size], client_addr).unwrap();
            } else {
                // > It MUST drop any datagrams arriving from any source IP address
                // > other than the one recorded for the particular association.
                warn!("unknown src packet is comming (discarded): {:?}", addr);
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
                let udp_relay = std::net::UdpSocket::bind(self.server_addr.clone())?;
                strm.send_connect_reply(self.connect_reply::<model::ErrorKind>(Ok(())))?;
                let server_addr = conn_req.connect_to.clone();
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
