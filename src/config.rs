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
        let content = fs::read_to_string(file_path)?;
        let mut servers = Vec::new();
        let mut current_server: Option<ServerConfig> = None;
        let mut current_route: Option<RouteConfig> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with("server {") {
                current_server = Some(ServerConfig {
                    host: "127.0.0.1".to_string(),
                    port: 8080,
                    server_name: "localhost".to_string(),
                    error_pages: HashMap::new(),
                    client_max_body_size: 10_000_000,
                    routes: Vec::new(),
                });
            } else if line.starts_with("location ") && line.ends_with('{') {
                let path = line["location ".len()..line.len() - 1].trim().to_string();
                current_route = Some(RouteConfig {
                    path,
                    allowed_methods: vec!["GET".to_string()],
                    root: "./www".to_string(),
                    default_file: None,
                    cgi_extension: None,
                    autoindex: false,
                    redirect: None,
                });
            } else if line == "}" {
                if let Some(route) = current_route.take() {
                    if let Some(ref mut server) = current_server {
                        server.routes.push(route);
                    }
                } else if let Some(server) = current_server.take() {
                    servers.push(server);
                }
            } else if let Some((key, val)) = line.split_once(' ') {
                let val = val.trim_matches(';').trim();

                if let Some(ref mut route) = current_route {
                    match key {
                        "root" => route.root = val.to_string(),
                        "index" => route.default_file = Some(val.to_string()),
                        "methods" => route.allowed_methods = val.split(',').map(|s| s.trim().to_string()).collect(),
                        "cgi_ext" => route.cgi_extension = Some(val.to_string()),
                        "autoindex" => route.autoindex = val == "on",
                        "return" => route.redirect = Some(val.to_string()),
                        _ => {}
                    }
                } else if let Some(ref mut server) = current_server {
                    match key {
                        "host" => server.host = val.to_string(),
                        "listen" => server.port = val.parse().unwrap_or(8080),
                        "server_name" => server.server_name = val.to_string(),
                        "client_max_body_size" => server.client_max_body_size = val.parse().unwrap_or(10_000_000),
                        "error_page" => {
                            let parts: Vec<&str> = val.split_whitespace().collect();
                            if parts.len() == 2 {
                                if let Ok(code) = parts[0].parse::<u16>() {
                                    server.error_pages.insert(code, parts[1].to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        Self::validate_configs(&servers)?;

        if servers.is_empty() {
            Ok(vec![ServerConfig::default_local(8080)])
        } else {
            Ok(servers)
        }
    }

    fn validate_configs(servers: &[ServerConfig]) -> Result<(), io::Error> {
        let mut seen = std::collections::HashSet::new();

        for s in servers {
            let key = (s.port, s.server_name.clone());
            if seen.contains(&key) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Duplicate server configuration found for port {} and server_name '{}'", s.port, s.server_name),
                ));
            }
            seen.insert(key);
        }

        Ok(())
    }
}