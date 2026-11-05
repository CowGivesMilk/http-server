//request
use super::file_handler::FileHandler;
use super::response::{Response, StatusCode};
use crate::response::Compression;
use anyhow::{Context, Result, anyhow, bail};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};
const MAX_BODY: usize = 1024 * 1024; // 1 MiB
#[derive(Debug)]
pub enum Method {
    GET,
    POST,
    PUT,
    PATCH,
    TRACE,
    DELETE,
    OPTIONS,
    HEAD,
    QUERY,
    CONNECT,
}

impl TryFrom<&str> for Method {
    type Error = String;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "GET" => Ok(Method::GET),
            "POST" => Ok(Method::POST),
            "PUT" => Ok(Method::PUT),
            "PATCH" => Ok(Method::PATCH),
            "TRACE" => Ok(Method::TRACE),
            "DELETE" => Ok(Method::DELETE),
            "OPTIONS" => Ok(Method::OPTIONS),
            "HEAD" => Ok(Method::HEAD),
            "QUERY" => Ok(Method::QUERY),
            "CONNECT" => Ok(Method::CONNECT),
            _ => Err(format!("Invalid method {}", value)),
        }
    }
}
pub struct RequestLines {
    pub method: Method,
    pub uri: String,
    pub version: String,
}

pub struct Request {
    pub request_lines: RequestLines,
    pub headers: Vec<(String, String)>,
    pub message: String,
}

/// Reads one CRLF-terminated line and returns it without the CRLF.
async fn read_crlf_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<String> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        bail!("connection closed before the request was complete");
    }
    if !line.ends_with("\r\n") {
        bail!("line not terminated with CRLF");
    }
    line.truncate(line.len() - 2);
    Ok(line)
}

impl RequestLines {
    pub async fn parse_request_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Self> {
        let line = read_crlf_line(reader).await?;
        let mut parts = line.split(' ');

        let (Some(method), Some(uri), Some(version), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            bail!("malformed request line");
        };

        Ok(RequestLines {
            method: Method::try_from(method).map_err(|e| anyhow!(e))?,
            uri: uri.to_string(),
            version: version.to_string(),
        })
    }
}

impl Request {
    pub async fn parse_request<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Self> {
        let request_lines = RequestLines::parse_request_line(reader).await?;

        // Headers: read until the empty line
        let mut headers: Vec<(String, String)> = Vec::new();
        loop {
            let line = read_crlf_line(reader).await?;
            if line.is_empty() {
                break;
            }
            let (name, value) = line
                .split_once(':')
                .ok_or_else(|| anyhow!("malformed header: {line}"))?;
            headers.push((name.to_string(), value.trim().to_string()));
        }

        // Body: exactly Content-Length bytes, or none
        let content_length = headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
            .map(|(_, v)| v.parse::<usize>())
            .transpose()
            .context("invalid Content-Length")?
            .unwrap_or(0);
        // Problem on multiple Content-Length
        if headers
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
            .nth(1)
            .is_some()
        {
            bail!("Multiple content length");
        }

        if content_length > MAX_BODY {
            bail!("body too large: {content_length} bytes");
        }

        let mut body = vec![0u8; content_length];
        reader.read_exact(&mut body).await?;
        let message = String::from_utf8(body).context("body is not valid UTF-8")?;

        Ok(Request {
            request_lines,
            headers,
            message,
        })
    }
}
impl Request {
    pub async fn route(self, file_handler: &FileHandler) -> Response {
        // Destructure once: `headers` is borrowed (never moved) below,
        // so it can still be inspected for `Accept-Encoding` after the match.
        let Request {
            request_lines,
            headers,
            message,
        } = self;
        let method = request_lines.method;
        let uri = request_lines.uri;

        let result = match (method, uri.as_str()) {
            (Method::GET, "/") => Response::new(StatusCode::Ok, None::<Vec<u8>>, "HTTP/1.1")
                .header("Content-Type", "text/plain"),

            (Method::GET, "/user-agent") => {
                let user_agent = headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("user-agent"))
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();

                Response::new(StatusCode::Ok, Some(user_agent), "HTTP/1.1")
                    .header("Content-Type", "text/plain")
            }

            (Method::GET, path) if path.starts_with("/echo/") => {
                let echoed = &path["/echo/".len()..];
                Response::new(StatusCode::Ok, Some(echoed.as_bytes().to_vec()), "HTTP/1.1")
                    .header("Content-Type", "text/plain")
            }

            (Method::GET, file) if file.starts_with("/files/") => {
                let file_path = &file["/files/".len()..];
                match file_handler.get_file_data(file_path) {
                    Ok(Some(bytes)) => Response::new(StatusCode::Ok, Some(bytes), "HTTP/1.1")
                        .header("Content-Type", "application/octet-stream"),
                    Ok(None) => Response::new(StatusCode::NotFound, None::<Vec<u8>>, "HTTP/1.1"),
                    Err(e) => {
                        eprintln!("file read error for {uri}: {e:#}");
                        Response::new(StatusCode::NotFound, None::<Vec<u8>>, "HTTP/1.1")
                    }
                }
            }

            (Method::POST, file) if file.starts_with("/files/") => {
                let file_path = &file["/files/".len()..];
                if file_path.is_empty() {
                    Response::new(StatusCode::BadRequest, None::<Vec<u8>>, "HTTP/1.1")
                } else if file_handler.write_to_file(file_path, &message) {
                    Response::new(StatusCode::Created, None::<Vec<u8>>, "HTTP/1.1")
                } else {
                    Response::new(StatusCode::InternalServerError, None::<Vec<u8>>, "HTTP/1.1")
                }
            }

            _ => Response::new(StatusCode::NotFound, None::<Vec<u8>>, "HTTP/1.1"),
        };

        // A client "accepts gzip" if any encoding in the comma-separated
        // list (ignoring ;q=… parameters) equals "gzip", case-insensitively.
        let wants_gzip = headers.iter().any(|(key, value)| {
            key.eq_ignore_ascii_case("accept-encoding")
                && value
                    .split(',')
                    .map(|item| item.split(';').next().unwrap_or("").trim())
                    .any(|enc| enc.eq_ignore_ascii_case("gzip"))
        });

        // `compress` is the only place that sets `Content-Encoding: gzip`.
        if wants_gzip {
            result.compress(Compression::Gzip)
        } else {
            result
        }
    }
}
