use std::collections::HashMap;
use std::fs;
use std::io;

#[derive(Debug, Clone)]
pub struct RouteConfig {
    pub path: String,
    pub allowed_methods: Vec<String>,
    pub root: String,
    pub default_file: Option<String>,
    pub cgi_extension: Option<String>,
    pub autoindex: bool,
    pub redirect: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub server_name: String,
    pub error_pages: HashMap<u16, String>,
    pub client_max_body_size: usize,
    pub routes: Vec<RouteConfig>,
}

impl ServerConfig {
    pub fn default_local(port: u16) -> Self {
        ServerConfig {
            host: "127.0.0.1".to_string(),
            port,
            server_name: "localhost".to_string(),
            error_pages: HashMap::new(),
            client_max_body_size: 10_000_000,
            routes: vec![RouteConfig {
                path: "/".to_string(),
                allowed_methods: vec!["GET".to_string(), "POST".to_string(), "DELETE".to_string()],
                root: "./www".to_string(),
                default_file: Some("index.html".to_string()),
                cgi_extension: Some(".py".to_string()),
                autoindex: true,
                redirect: None,
            }],
        }
    }

    pub fn parse_file(file_path: &str) -> Result<Vec<ServerConfig>, io::Error> {
        let _content = fs::read_to_string(file_path)?;
        // TODO: Replace with full config block parsing
        Ok(vec![ServerConfig::default_local(8080)])
    }
}