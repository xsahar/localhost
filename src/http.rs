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
        let text = String::from_utf8_lossy(buffer);
        let mut lines = text.lines();

        // 1. Parse Request Line (e.g., "GET /about.html HTTP/1.1")
        let request_line = lines.next().ok_or("Empty request")?;
        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 3 {
            return Err("Invalid HTTP request line".to_string());
        }

        let method = parts[0].to_string();
        let uri = parts[1].to_string();
        let version = parts[2].to_string();

        // 2. Parse Headers (e.g., "Host: 127.0.0.1:8080")
        let mut headers = HashMap::new();
        for line in lines {
            if line.is_empty() {
                break; // Blank line marks the end of headers
            }
            if let Some((key, value)) = line.split_once(':') {
                headers.insert(key.trim().to_lowercase(), value.trim().to_string());
            }
        }

        Ok(HttpRequest {
            method,
            uri,
            version,
            headers,
            body: Vec::new(),
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