#![cfg(test)]
use std::path::PathBuf;

#[test]
#[ignore]
fn get_main() {
    use std::io::prelude::*;

    use log::*;
    use regex::Regex;
    use socks::*;

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
        write!(conn, "GET /src/main.rs HTTP/1.1\r\n").unwrap();
        write!(conn, "Host: myhttpd\r\n\r\n").unwrap();
        conn.flush().unwrap();

        let mut conn = std::io::BufReader::new(conn);
        // skip http headers
        let mut line = String::new();
        let mut content_length = None;
        let re = Regex::new(r"Content-Length: (\d+)\r\n").unwrap();
        while let Ok(_) = conn.read_line(&mut line) {
            debug!("line: {:?}", line);
            if line == "\r\n" {
                break;
            }
            if let Some(m) = re.captures(&line) {
                content_length = m.get(1).unwrap().as_str().parse().ok();
            }
            line.clear();
        }
        let mut buff = Vec::new();
        buff.resize(content_length.unwrap(), 0);
        conn.read_exact(&mut buff[..]).unwrap();
        String::from_utf8_lossy(&buff).to_string()
    };
    assert_eq!(act, exp)
}
