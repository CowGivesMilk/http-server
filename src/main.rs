use anyhow::Result;
use http_server::file_handler::FileHandler;
use http_server::request::Request;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    let target_dir = match args.get(2) {
        Some(dir) => PathBuf::from(dir),
        None => anyhow::bail!("usage: {} --directory <target-dir>", args[0]),
    };

    // Built once, validated once, shared by every connection.
    let file_handler = Arc::new(FileHandler::try_new(target_dir)?);

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
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
    let req = Request::parse_request(&mut reader).await?;

    println!(
        "{:?} {} ({} headers, {} body bytes)\n{}",
        req.request_lines.method,
        req.request_lines.uri,
        req.headers.len(),
        req.message.len(),
        req.message
    );

    let response = req.route(&file_handler).await;

    let stream = reader.get_mut();
    stream.write_all(&response.to_bytes()).await?;
    stream.shutdown().await?;
    Ok(())
}
