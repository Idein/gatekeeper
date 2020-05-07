//! A SOCKS5 proxy server implemented in Rust
//!
//! Gatekeeperd is an SOCKS5 proxy built on gatekeeper crate.
//!
use std::io;
use std::net::IpAddr;
use std::path::PathBuf;

use log::*;
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

    #[structopt(short = "r", long = "rule")]
    /// Set path to connection rule file (format: yaml)
    rulefile: Option<PathBuf>,
}

fn set_handler(signals: &[i32], handler: impl Fn(i32) + Send + 'static) -> io::Result<()> {
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

    let config = match opt.rulefile {
        Some(ref path) => gk::ServerConfig::with_file(opt.ipaddr, opt.port, path),
        None => Ok(gk::ServerConfig::new(
            opt.ipaddr,
            opt.port,
            gk::ConnectRule::any(),
        )),
    }
    .expect("server config");

    let (mut server, tx) = gk::server::Server::new(config);
    set_handler(&[SIGTERM, SIGINT, SIGQUIT, SIGCHLD], move |_| {
        tx.send(gk::ServerCommand::Terminate).ok();
    })
    .expect("setting ctrl-c handler");

    if let Err(err) = server.serve() {
        error!("server error: {:?}", err);
    }
}
