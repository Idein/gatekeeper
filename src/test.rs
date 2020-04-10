#![cfg(test)]
use std::path::PathBuf;

#[test]
#[ignore]
fn get_main() {
    use log::*;
    use socks::*;
    use std::io::prelude::*;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    println!("root: {}", root.display());

    let exp = std::fs::read_to_string(root.join("src/main.rs")).unwrap();

    let act = {
        // connect to socks proxy
        let mut conn = Socks5Stream::connect(
            "localhost:1080",
            TargetAddr::Domain("myhttpd".to_owned(), 80),
        )
        .unwrap();

        // request main.rs
        write!(conn, "GET /gatekeeper/src/main.rs HTTP/1.1\r\n").unwrap();
        write!(conn, "Host: myhttpd\r\n\r\n").unwrap();
        conn.flush().unwrap();

        let mut conn = std::io::BufReader::new(conn);
        // skip http headers
        let mut line = String::new();
        while let Ok(_) = conn.read_line(&mut line) {
            debug!("line: {:?}", line);
            if line == "\r\n" {
                break;
            }
            line.clear();
        }
        let mut buff = vec![0; exp.len()];
        conn.read_exact(&mut buff[..]).unwrap();
        String::from_utf8_lossy(&buff).to_string()
    };
    assert_eq!(act, exp)
}
