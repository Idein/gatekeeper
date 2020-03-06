use model::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServerConfig {
    pub server_ip: IpAddr,
    pub server_port: u16,
    pub udp_pkt_size: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            server_ip: Ipv4Addr::new(127, 0, 0, 1).into(),
            server_port: 1080,
            udp_pkt_size: 4096,
        }
    }
}

impl ServerConfig {
    pub fn server_addr(&self) -> SocketAddr {
        SocketAddr::new(self.server_ip, self.server_port)
    }
}
