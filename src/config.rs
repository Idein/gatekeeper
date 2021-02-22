use std::fs::File;
use std::path::Path;
use std::time::Duration;

use crate::error::{Error, ErrorKind};
use crate::model::{ConnectRule, IpAddr, Ipv4Addr, SocketAddr};

use failure::ResultExt;

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// ip address for listening connections. (default: 0.0.0.0)
    pub server_ip: IpAddr,
    /// port number for listening connections. (default: 1080)
    pub server_port: u16,
    /// rule set for filtering connection requests (default: allow any connection)
    pub conn_rule: ConnectRule,
    /// timeout of relaying data chunk from client to external network. (default: 2000ms)
    pub client_rw_timeout: Option<Duration>,
    /// timeout of relaying data chunk from external network to client. (default: 5000ms)
    pub server_rw_timeout: Option<Duration>,
    /// timeout of accpet connection from client. (default 3s)
    pub accept_timeout: Option<Duration>,
}

impl ServerConfig {
    pub fn new(server_ip: IpAddr, server_port: u16, conn_rule: ConnectRule) -> Self {
        Self {
            server_ip,
            server_port,
            conn_rule,
            ..Self::default()
        }
    }

    /// Config with filter rule from file
    ///
    /// * `server_ip`
    ///   Listening ip address of the Server.
    /// * `server_port`
    ///   Listening port number of the Server.
    /// * `rulefile`
    ///   Path to file specify filtering rules in yaml.
    ///
    /// # Example
    ///
    /// Here is an example of filtering rule written in yaml.
    ///
    /// ```
    /// use std::fs;
    /// # use std::path::Path;
    /// # use gatekeeper::error::Error;
    /// # use gatekeeper::model::L4Protocol::*;
    /// use gatekeeper::config::ServerConfig;
    /// # fn main() -> Result<(), Error> {
    /// fs::write("rule.yml", r#"
    /// ---
    /// # # default deny
    /// - Deny:
    ///     address: Any
    ///     port: Any
    ///     protocol: Any
    /// # # allow local ipv4 network 192.168.0.1/16
    /// - Allow:
    ///     address:
    ///       Specif:
    ///         IpAddr:
    ///           addr: 192.168.0.1
    ///           prefix: 16
    ///     port: Any
    ///     protocol: Any
    /// "#.as_bytes())?;
    /// let config = ServerConfig::with_file("192.168.0.1".parse().unwrap(), 1080, Path::new("rule.yml"))?;
    /// assert!(config.conn_rule.check("192.168.0.2:80".parse().unwrap(), Tcp));
    /// assert!(!config.conn_rule.check("192.167.0.2:80".parse().unwrap(), Udp));
    /// # Ok(())
    /// # }
    /// ```
    ///
    pub fn with_file(server_ip: IpAddr, server_port: u16, rulefile: &Path) -> Result<Self, Error> {
        let path = File::open(rulefile)?;
        let conn_rule = serde_yaml::from_reader(path).context(ErrorKind::Config)?;
        Ok(ServerConfig {
            server_ip,
            server_port,
            conn_rule,
            ..Self::default()
        })
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            server_ip: Ipv4Addr::new(0, 0, 0, 0).into(),
            server_port: 1080,
            conn_rule: ConnectRule::any(),
            client_rw_timeout: Some(Duration::from_millis(2000)),
            server_rw_timeout: Some(Duration::from_millis(5000)),
            accept_timeout: Some(Duration::from_secs(3)),
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

    pub fn set_server_addr(&mut self, addr: SocketAddr) -> &mut Self {
        self.server_ip = addr.ip();
        self.server_port = addr.port();
        self
    }

    pub fn set_connect_rule(&mut self, rule: ConnectRule) -> &mut Self {
        self.conn_rule = rule;
        self
    }

    pub fn set_client_rw_timeout(&mut self, dur: Option<Duration>) -> &mut Self {
        self.client_rw_timeout = dur;
        self
    }

    pub fn set_server_rw_timeout(&mut self, dur: Option<Duration>) -> &mut Self {
        self.server_rw_timeout = dur;
        self
    }

    pub fn set_accept_timeout(&mut self, dur: Option<Duration>) -> &mut Self {
        self.accept_timeout = dur;
        self
    }
}
