use std::io;
use std::thread::{self, JoinHandle};

use log::*;

use crate::auth_service::*;
use crate::byte_stream::{BoxedStream, ByteStream};
use crate::connector::Connector;
use crate::error::Error;
use crate::method_selector::MethodSelector;
use crate::rw_socks_stream::ReadWriteStream;

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
            Command::Connect => {}
            cmd @ Command::Bind | cmd @ Command::UdpAssociate => {
                debug!("command not supported: {:?}", cmd);
                let not_supported: model::Error = ErrorKind::command_not_supported(*cmd).into();
                // reply error
                let rep = self.connect_reply(Err(not_supported.kind().clone()));
                strm.send_connect_reply(rep)?;
                return Err(not_supported.into());
            }
        }

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
        Ok(())
    }
}
