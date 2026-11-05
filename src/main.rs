//main
use anyhow::{Ok, Result};
use http_server::file_handler::FileHandler;
use http_server::request::Request;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

const TCP_BIND_ADDRESS: &'static str = "127.0.0.1:4221";
#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let target_dir = match args.get(2) {
        Some(dir) => PathBuf::from(dir),
        None => anyhow::bail!("usage: {} --directory <target-dir>", args[0]),
    };

    // Built once, validated once, shared by every connection.
    let file_handler = Arc::new(FileHandler::try_new(target_dir)?);

    let listener = TcpListener::bind(TCP_BIND_ADDRESS).await?;
    println!("listening on {}", listener.local_addr()?);

    loop {
        let (stream, peer) = listener.accept().await?;
        let fh = Arc::clone(&file_handler);
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, fh).await {
                eprintln!("{peer}: {e:#}");
            }
        });
    }
}
async fn handle_connection(stream: TcpStream, file_handler: Arc<FileHandler>) -> Result<()> {
    let mut reader = BufReader::new(stream);
    loop {
        let buf = reader.fill_buf().await?;
        if buf.is_empty() {
            return Ok(()); // clean close, no error
        }

        let req = Request::parse_request(&mut reader).await?; // real error here = malformed/truncated request
        let close = req.request_lines.version.as_str() == "HTTP/1.0"
            || req.headers.iter().any(|(h, v)| {
                h.eq_ignore_ascii_case("connection") && v.eq_ignore_ascii_case("close")
            });

        let response = match close {
            true => req.route(&file_handler).await.header("connection", "clode"),
            false => req
                .route(&file_handler)
                .await
                .header("connection", "keep-alive"),
        };
        let stream = reader.get_mut();
        stream.write_all(&response.to_bytes()).await?;
        if close {
            stream.shutdown().await?;
            return Ok(());
        }
    }
}
