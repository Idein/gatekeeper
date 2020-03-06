use log::*;
use std::net::IpAddr;
use structopt::*;

use gatekeeper as gk;

#[derive(StructOpt, Debug)]
#[structopt(name = "gatekeeper")]
struct Opt {
    #[structopt(short = "p", long = "port", default_value = "1080")]
    /// Set port to listen on
    port: u16,

    #[structopt(short = "i", long = "ip", default_value = "127.0.0.1")]
    /// Set ipaddress to listen on
    ipaddr: IpAddr,

    #[structopt(long = "packet_size", default_value = "4096")]
    udp_pkt_size: usize,
}

fn main() {
    pretty_env_logger::init_timed();

    println!("gatekeeperd");
    let opt = Opt::from_args();
    debug!("option: {:?}", opt);

    let config = gk::config::ServerConfig {
        server_ip: opt.ipaddr,
        server_port: opt.port,
        udp_pkt_size: opt.udp_pkt_size,
    };

    let (server, _tx) = gk::server::Server::new(
        config.clone(),
        gk::acceptor::TcpBinder,
        gk::connector::TcpUdpConnector::new(config.server_addr(), config.udp_pkt_size),
    );
    if let Err(err) = server.serve() {
        error!("server error: {:?}", err);
    }
}
