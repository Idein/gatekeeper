//! This crate provides a library for constructing [SOCKS5](ftp://ftp.rfc-editor.org/in-notes/rfc1928.txt) proxy server.
//!
//! # Feature
//! ## Authentication
//!
//! Any authentication method is not supported.
//!
//! The client connects to the server is required for sending `X'00'` (`NO AUTHENTICATION REQUIRED`) as a method selection message.
//!
//! ## Command
//!
//! Only `CONNECT` command is supported.
//! Then, some protocols connecting from server to client are not able to proxy.
//! And also protocols using UDP is not supported.
//!
//! ## Filter Rule
//!
//! By default, `gatekeeper` accepts all connection requests.
//! However, it is possible to filter out some requests along with filtering rules.
//!
//!
//!
//! # Usage
//!
//! This crate is on [crates.io](https://crates.io/crates/gatekeeper), and can be used by adding `gatekeeper` to your dependencies in your project's `Cargo.toml`.
//!
//! ```toml
//! [dependencies]
//! gatekeeper = "1.0.0"
//! ```
//!
//! You can find an example server implementation [Example Server](#Server).
//!
//! ## Server
//!
//! Here is a minimum server example.
//!
//! ```rust
//! use std::{time::Duration, thread};
//! use gatekeeper::*;
//! let (mut server, tx) = Server::new(ServerConfig::default());
//! let th = thread::spawn(move || server.serve());
//! thread::sleep(Duration::from_secs(1));
//! tx.send(ServerCommand::Terminate).unwrap();
//! th.join().unwrap();
//! ```
//!
//! ## FilterRule
//!
//! It is possible to constructing proxy server with complex filter rules like below:
//!
//! ```rust
//! use std::{time::Duration, thread};
//! use gatekeeper::*;
//! use AddressPattern as Pat;
//! use RulePattern::*;
//! use regex::Regex;
//! let mut rule = ConnectRule::none();
//! // allow local ipv4 network 192.168.0.1/16
//! rule.allow(
//!     Specif(Pat::IpAddr { addr: "192.168.0.1".parse().unwrap(), prefix: 16, }),
//!     Specif(80),
//!     Any,
//! );
//! // allow local ipv4 network 192.168.0.1/16 port 443
//! rule.allow(
//!     Specif(Pat::IpAddr { addr: "192.168.0.1".parse().unwrap(), prefix: 16, }),
//!     Specif(443),
//!     Any,
//! );
//! // allow connecting to actcast.io
//! rule.allow(
//!     Specif(Regex::new(r"\A(.+\.)?actcast\.io\z").unwrap().into()),
//!     Any,
//!     Specif(L4Protocol::Tcp),
//! );
//! // deny facebook.com
//! rule.allow(
//!     Specif(Regex::new(r"\A(www\.)?facebook\.com\z").unwrap().into()),
//!     Any,
//!     Specif(L4Protocol::Tcp),
//! );
//! let mut config = ServerConfig::default();
//! config.server_port = 1081; // conflict to other example
//! config.set_connect_rule(rule);
//! let (mut server, tx) = Server::new(config);
//! let th = thread::spawn(move || server.serve());
//! thread::sleep(Duration::from_secs(1));
//! tx.send(ServerCommand::Terminate).unwrap();
//! th.join().unwrap();
//! ```

pub mod acceptor;
mod auth_service;
mod byte_stream;
pub mod config;
pub mod connector;
pub mod error;
pub mod model;
mod pkt_stream;
mod raw_message;
mod relay;
mod rw_socks_stream;
pub mod server;
pub mod server_command;
mod session;
mod tcp_listener_ext;
mod test;
mod thread;

pub use config::*;
pub use model::model::*;
pub use server::*;
pub use server_command::*;
