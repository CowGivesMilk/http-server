//response
const GZIP_COMPRESSION_LEVEL: u8 = 6;
#[derive(Copy, Clone)]
#[repr(u16)]
pub enum StatusCode {
    Ok = 200,
    Created = 201,
    BadRequest = 400,
    NotFound = 404,
    InternalServerError = 500,
}

impl StatusCode {
    pub fn phrase(self) -> &'static str {
        match self {
            StatusCode::Ok => "OK",
            StatusCode::Created => "Created",
            StatusCode::BadRequest => "Bad request",
            StatusCode::NotFound => "Not Found",
            StatusCode::InternalServerError => "Internal Server Error",
        }
    }
}
impl TryFrom<u16> for StatusCode {
    type Error = &'static str;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            200 => Ok(StatusCode::Ok),
            404 => Ok(StatusCode::NotFound),
            _ => Err("Invalid status code"),
        }
    }
}
pub enum Compression {
    Gzip,
}
pub struct Response {
    pub status_code: StatusCode,
    pub header_fields: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    version: String,
}

impl Response {
    pub fn new(
        status_code: StatusCode,
        body: Option<impl Into<Vec<u8>>>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            status_code,
            header_fields: Vec::new(),
            body: body.map(Into::into),
            version: version.into(),
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.header_fields.push((name.into(), value.into()));
        self
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        use std::fmt::Write;

        let mut out = String::new();

        write!(
            out,
            "{} {} {}\r\n",
            self.version,
            self.status_code as u16,
            self.status_code.phrase()
        )
        .unwrap();

        for (name, value) in &self.header_fields {
            // We compute this ourselves below; a caller-supplied one could disagree with the body
            if name.eq_ignore_ascii_case("content-length") {
                continue;
            }
            write!(out, "{}: {}\r\n", name, value).unwrap();
        }

        // Content-Length semantics:
        //   Some(bytes) => exact byte length (0 for empty file)
        //   None        => no body; RFC 7230 lets us omit Content-Length for
        //                  responses that aren't 1xx/204/304 only if we close
        //                  the connection. Safer to send 0.
        let content_length = self.body.as_ref().map_or(0, |b| b.len());
        write!(out, "Content-Length: {}\r\n", content_length).unwrap();

        out.push_str("\r\n");

        let mut bytes = out.into_bytes();
        if let Some(body) = &self.body {
            bytes.extend_from_slice(body);
        }
        bytes
    }
}
impl Response {
    pub fn compress(self, format: Compression) -> Self {
        match format {
            Compression::Gzip => {
                if let Some(data) = self.body {
                    let data = gzippy::compress(&data, GZIP_COMPRESSION_LEVEL).unwrap();
                    return Response {
                        status_code: self.status_code,
                        header_fields: self.header_fields,
                        body: Some(data),
                        version: self.version,
                    }
                    .header("Content-Encoding", "gzip");
                }
                self
            }
        }
    }
}
