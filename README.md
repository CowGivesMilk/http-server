# http_server

A small HTTP/1.1 server written from scratch in Rust on top of `tokio`. It parses requests off raw TCP streams, routes them, serves and stores files, and gzip-compresses responses on request. No web framework is used: request parsing, routing and response serialization are all hand-written.

## Features

- **Async, concurrent**: one `tokio` task per connection, all sharing a single validated `FileHandler` via `Arc`.
- **Persistent connections**: HTTP/1.1 keep-alive by default; closes on `Connection: close` or HTTP/1.0.
- **Hand-rolled parser**: CRLF-strict request line and header parsing, `Content-Length` bodies, `\r\n` line discipline.
- **Gzip compression**: applied when the client's `Accept-Encoding` includes `gzip` (handles comma-separated lists, `q=` parameters and case-insensitivity).
- **Safe file serving**: path sanitization plus canonicalization to block directory traversal (`..`, absolute paths, symlink escapes).
- **Request hardening**: 1 MiB body cap, rejects invalid or duplicate `Content-Length`, rejects non-UTF-8 bodies.

## Endpoints

| Method | Path | Behavior |
|--------|------|----------|
| `GET`  | `/` | `200 OK`, empty body |
| `GET`  | `/echo/{text}` | Returns `{text}` as `text/plain` |
| `GET`  | `/user-agent` | Returns the request's `User-Agent` header as `text/plain` |
| `GET`  | `/files/{path}` | Returns the file as `application/octet-stream`, or `404` |
| `POST` | `/files/{path}` | Writes the request body to `{path}` under the target directory, `201 Created` (parent directories are created as needed) |
| any    | anything else | `404 Not Found` |

## Getting started

Requires a recent stable Rust toolchain.

```bash
cargo run -- --directory /tmp/files
```

The server listens on `127.0.0.1:4221`. The target directory must exist; it is canonicalized at startup and every file operation is confined to it.

### Try it

```bash
# Basic routes
curl -i http://127.0.0.1:4221/
curl -i http://127.0.0.1:4221/echo/hello
curl -i -A "my-agent/1.0" http://127.0.0.1:4221/user-agent

# Store and fetch a file
curl -i -X POST --data 'hello world' http://127.0.0.1:4221/files/greeting.txt
curl -i http://127.0.0.1:4221/files/greeting.txt

# Gzip negotiation
curl -i --compressed http://127.0.0.1:4221/echo/hello

# Traversal attempt -> 404
curl -i --path-as-is http://127.0.0.1:4221/files/../../etc/passwd

# Keep-alive: two requests over one connection
curl -i http://127.0.0.1:4221/echo/a http://127.0.0.1:4221/echo/b
```

## Design

```
src/
├── main.rs          # startup, accept loop, per-connection keep-alive loop
├── lib.rs           # module exports
├── request.rs       # Method, request-line/header/body parsing, routing
├── response.rs      # StatusCode, Response builder, serialization, gzip
└── file_handler.rs  # sandboxed file read/write, path sanitization
```

**Connection lifecycle.** `main` accepts connections and spawns a task for each. The task wraps the stream in a `BufReader` and loops: peek with `fill_buf` to distinguish a clean client close (empty buffer) from a truncated request (parse error), parse one request, route it, write the response, and either continue or shut down depending on the connection semantics.

**Parsing.** `Request::parse_request` reads the request line, then headers until the blank line, then exactly `Content-Length` bytes. Reads are generic over `AsyncBufRead`, which keeps the parser independent of `TcpStream`.

**Path safety.** File access is two-layered: `sanitize` rejects `..` and Windows prefixes lexically and drops `/` and `.` components; then the joined path is canonicalized and checked with `starts_with` against the canonical target directory, which also catches symlink escapes.

**Responses.** `Response` is a small builder (`Response::new(...).header(...)`). `to_bytes` computes `Content-Length` itself and ignores any caller-supplied one so the header can never disagree with the body. `compress` replaces the body with its gzip form and adds `Content-Encoding: gzip`.

## Limitations

- Bind address and port are hardcoded (`127.0.0.1:4221`).
- Only `Content-Length` bodies are supported (no chunked transfer encoding).
- File reads and writes use blocking `std::fs` calls inside async handlers.
- No read timeouts on connections.
- Only the methods listed above are routed; others return `404` rather than `405`.
- Compression is gzip only.

## Tech

Rust, `tokio`, `anyhow`, `gzippy`.
