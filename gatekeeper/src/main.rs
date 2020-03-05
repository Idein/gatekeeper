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
}

fn main() {
    pretty_env_logger::init_timed();

    println!("gatekeeperd");
    let opt = Opt::from_args();
    debug!("option: {:?}", opt);

    let config = gk::config::ServerConfig {
        server_address: opt.ipaddr,
        server_port: opt.port,
    };

    let (server, tx) = gk::server::Server::new(
        config,
        gk::acceptor::TcpBinder,
        gk::connector::TcpUdpConnector,
    );
    server.serve();
}
