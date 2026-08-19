use crate::config::ServerConfig;
use crate::http::{HttpRequest, HttpResponse};
use std::io::Write;
use std::process::{Command, Stdio};

pub struct CgiHandler {
    pub script_path: String,
}

impl CgiHandler {
    pub fn new(script_path: String) -> Self {
        CgiHandler { script_path }
    }

    pub fn execute(&self, req: &HttpRequest, config: &ServerConfig) -> Result<HttpResponse, String> {
        let (path_info, query_string) = match req.uri.split_once('?') {
            Some((p, q)) => (p, q),
            None => (req.uri.as_str(), ""),
        };

        let mut cmd = Command::new(&self.script_path);

        cmd.env("REQUEST_METHOD", &req.method)
            .env("QUERY_STRING", query_string)
            .env("SERVER_PROTOCOL", &req.version)
            .env("PATH_INFO", path_info)
            .env("SCRIPT_FILENAME", &self.script_path)
            .env("SERVER_NAME", &config.server_name)
            .env("SERVER_PORT", config.port.to_string());

        if let Some(content_type) = req.headers.get("content-type") {
            cmd.env("CONTENT_TYPE", content_type);
        }

        if let Some(content_length) = req.headers.get("content-length") {
            cmd.env("CONTENT_LENGTH", content_length);
        }

        // Export HTTP headers into HTTP_ CGI variables
        for (key, val) in &req.headers {
            let env_key = format!("HTTP_{}", key.to_uppercase().replace('-', "_"));
            cmd.env(env_key, val);
        }

        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn CGI script: {}", e))?;

        if !req.body.is_empty() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(&req.body);
            }
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("CGI execution failed: {}", e))?;

        if !output.status.success() {
            return Err("CGI process returned non-zero exit code".to_string());
        }

        let mut response = HttpResponse::new(200, "OK");
        let raw_output = String::from_utf8_lossy(&output.stdout);

        // Parse CGI header output from body
        let delimiter = if raw_output.contains("\r\n\r\n") {
            "\r\n\r\n"
        } else {
            "\n\n"
        };

        if let Some((headers_part, body_part)) = raw_output.split_once(delimiter) {
            for line in headers_part.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    response.headers.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
            response.body = body_part.as_bytes().to_vec();
        } else {
            response.body = output.stdout;
        }

        if !response.headers.contains_key("Content-Length") {
            response.headers.insert("Content-Length".to_string(), response.body.len().to_string());
        }

        Ok(response)
    }
}