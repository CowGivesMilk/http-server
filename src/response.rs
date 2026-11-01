#[derive(Copy, Clone)]
#[repr(u16)]
pub enum StatusCode {
    Ok = 200,
    NotFound = 404,
}

impl StatusCode {
    pub fn phrase(self) -> &'static str {
        match self {
            StatusCode::Ok => "OK",
            StatusCode::NotFound => "Not Found",
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
pub struct Response {
    pub status_code: StatusCode,
    pub header_fields: Vec<(String, String)>,
    pub body: String,
    has_content_length_: bool,
}

impl Response {
    pub fn new(status_code: StatusCode, body: impl Into<String>) -> Self {
        Self {
            status_code,
            header_fields: Vec::new(),
            body: body.into(),
            has_content_length_: false,
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into();
        if name.eq_ignore_ascii_case("content-length") {
            self.has_content_length_ = true;
        }
        self.header_fields.push((name, value.into()));
        self
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        use std::fmt::Write;

        let mut out = String::new();

        // Status line
        writeln!(
            out,
            "HTTP/1.1 {} {}",
            self.status_code as u16,
            self.status_code.phrase()
        )
        .unwrap();

        // User-supplied headers
        for (name, value) in &self.header_fields {
            write!(out, "{}: {}\r\n", name, value).unwrap();
        }

        // Auto Content-Length unless caller set one
        if !self.has_content_length_ && !self.body.is_empty() {
            write!(out, "Content-Length: {}\r\n", self.body.len()).unwrap();
        }

        // Blank line separating headers from body
        out.push_str("\r\n");

        // Body
        out.push_str(&self.body);

        out.into_bytes()
    }
}
