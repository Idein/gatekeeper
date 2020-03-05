use model::{IpAddr, Ipv4Addr};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServerConfig {
    pub server_address: IpAddr,
    pub server_port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            server_address: Ipv4Addr::new(127, 0, 0, 1).into(),
            server_port: 1080,
        }
    }
}
