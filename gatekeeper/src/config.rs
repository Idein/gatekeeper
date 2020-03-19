use model::{ConnectRule, IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub server_ip: IpAddr,
    pub server_port: u16,
    pub conn_rule: ConnectRule,
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
