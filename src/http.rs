use std::collections::HashMap;

#[derive(Debug)]
pub struct HttpRequest {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn parse(buffer: &[u8]) -> Result<Self, String> {
        let mut headers = HashMap::new();

        // 1. Locate header / body delimiter (\r\n\r\n)
        let header_end = buffer
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .ok_or_else(|| "Malformed HTTP request: missing header delimiter".to_string())?;

        let header_str = std::str::from_utf8(&buffer[..header_end])
            .map_err(|_| "Invalid UTF-8 in HTTP headers".to_string())?;

        let mut lines = header_str.lines();

        // 2. Parse Request Line (e.g., "GET /index.html HTTP/1.1")
        let request_line = lines
            .next()
            .ok_or_else(|| "Empty HTTP request line".to_string())?;

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 3 {
            return Err("Invalid HTTP request line format".to_string());
        }

        let method = parts[0].to_string();
        let uri = parts[1].to_string();
        let version = parts[2].to_string();

        // 3. Parse Headers into case-insensitive keys
        for line in lines {
            if line.is_empty() {
                continue;
            }
            if let Some((key, val)) = line.split_once(':') {
                headers.insert(key.trim().to_lowercase(), val.trim().to_string());
            }
        }

        // 4. Extract Body according to Content-Length
        let body_bytes = &buffer[header_end + 4..];
        let content_length: usize = headers
            .get("content-length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let body = if content_length > 0 && body_bytes.len() >= content_length {
            body_bytes[..content_length].to_vec()
        } else {
            body_bytes.to_vec()
        };

        Ok(HttpRequest {
            method,
            uri,
            version,
            headers,
            body,
        })
    }
}

#[derive(Debug)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status_code: u16, status_text: &str) -> Self {
        let mut headers = HashMap::new();
        headers.insert("Server".to_string(), "Rust-Localhost/1.0".to_string());
        headers.insert("Connection".to_string(), "close".to_string());

        HttpResponse {
            status_code,
            status_text: status_text.to_string(),
            headers,
            body: Vec::new(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut response = format!(
            "HTTP/1.1 {} {}\r\n",
            self.status_code, self.status_text
        );

        for (key, val) in &self.headers {
            response.push_str(&format!("{}: {}\r\n", key, val));
        }

        response.push_str("\r\n");

        let mut bytes = response.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}