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

    #[structopt(short = "i", long = "ip", default_value = "0.0.0.0")]
    /// Set ipaddress to listen on
    ipaddr: IpAddr,
}

fn set_handler(signals: &[i32], handler: impl Fn(i32) + Send + 'static) -> std::io::Result<()> {
    use signal_hook::*;
    let signals = iterator::Signals::new(signals)?;
    std::thread::spawn(move || signals.forever().for_each(handler));
    Ok(())
}

fn main() {
    use signal_hook::*;
    pretty_env_logger::init_timed();

    println!("gatekeeperd");
    let opt = Opt::from_args();
    debug!("option: {:?}", opt);

    let config = {
        let mut config = gk::config::ServerConfig::default();
        config.server_ip = opt.ipaddr;
        config.server_port = opt.port;
        config
    };

    let (server, tx) = gk::server::Server::new(
        config,
        gk::acceptor::TcpBinder,
        gk::connector::TcpUdpConnector,
    );
    set_handler(&[SIGTERM, SIGINT, SIGQUIT, SIGCHLD], move |_| {
        tx.send(gk::ServerCommand::Terminate).ok();
    })
    .expect("setting ctrl-c handler");

    if let Err(err) = server.serve() {
        error!("server error: {:?}", err);
    }
}
