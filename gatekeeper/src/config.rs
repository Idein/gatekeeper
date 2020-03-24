use std::fs::File;
use std::path::Path;

use crate::error::{Error, ErrorKind};
use model::{ConnectRule, IpAddr, Ipv4Addr, SocketAddr};

use failure::ResultExt;
use serde_yaml;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub server_ip: IpAddr,
    pub server_port: u16,
    pub conn_rule: ConnectRule,
}

impl ServerConfig {
    pub fn new(server_ip: IpAddr, server_port: u16, conn_rule: ConnectRule) -> Self {
        Self {
            server_ip,
            server_port,
            conn_rule,
        }
    }

    pub fn with_file(server_ip: IpAddr, server_port: u16, rulefile: &Path) -> Result<Self, Error> {
        let path = File::open(rulefile)?;
        let conn_rule = serde_yaml::from_reader(path).context(ErrorKind::Config)?;
        Ok(ServerConfig::new(server_ip, server_port, conn_rule))
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            server_ip: Ipv4Addr::new(0, 0, 0, 0).into(),
            server_port: 1080,
            conn_rule: ConnectRule::any(),
        }
    }
}

impl ServerConfig {
    pub fn server_addr(&self) -> SocketAddr {
        SocketAddr::new(self.server_ip, self.server_port)
    }
    pub fn connect_rule(&self) -> ConnectRule {
        self.conn_rule.clone()
    }
}
