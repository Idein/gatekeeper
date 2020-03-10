use std::net::{Ipv4Addr, SocketAddr};

use crate::config::ServerConfig;

#[test]
fn echo_udp() {
    let config = ServerConfig::default();
    let client = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 31338);
    println!("client: {:?}", client);
    let target = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 31337);
    println!("target: {:?}", target);
    let udp = socks::Socks5Datagram::bind(config.server_addr(), client).unwrap();
    println!("bind");
    let datagram = b"hello";
    udp.send_to(&datagram[..], target.clone()).unwrap();
    println!("send_to");
    let mut buff: [u8; 4096] = [0; 4096];
    let (size, src) = udp.recv_from(&mut buff[..]).unwrap();
    println!("recv_from");
    println!("size: {}", size);
    println!("src: {:?}", String::from_utf8_lossy(&buff[..size]));
    assert_eq!(size, 5);
    assert_eq!(datagram, &buff[..size]);
}
