use anyhow::{Ok, Result};
use http_server::response::{Response, StatusCode};
use std::{io::Write, net::TcpListener};
fn main() -> Result<()> {
    let s: StatusCode = StatusCode::Ok;
    let response = Response::new(s, "Hello world!").header("Content-Type", "text/plain");
    let listner = TcpListener::bind("127.0.0.1:4221")?;
    for stream in listner.incoming() {
        let mut stream = stream?;
        stream.write_all(&response.to_bytes())?;
    }

    Ok(())
}
