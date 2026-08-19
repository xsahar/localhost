use crate::cgi::CgiHandler;
use crate::config::{RouteConfig, ServerConfig};
use crate::http::{HttpRequest, HttpResponse};
use libc::{
    accept, c_int, close, epoll_create1, epoll_ctl, epoll_event, epoll_wait, fcntl, read,
    sockaddr, sockaddr_in, socket, write, AF_INET, EPOLLIN, EPOLLOUT, EPOLL_CTL_ADD,
    EPOLL_CTL_MOD, EPOLL_CTL_DEL, F_SETFL, O_NONBLOCK, SOCK_STREAM, SOL_SOCKET, SO_REUSEADDR,
    EAGAIN, EWOULDBLOCK,
};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::mem;
use std::path::Path;

struct ClientState {
    port: u16,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
    bytes_written: usize,
}

pub struct Server {
    pub epoll_fd: c_int,
    pub listen_fds: HashMap<c_int, u16>,
    clients: HashMap<c_int, ClientState>,
    pub configs: Vec<ServerConfig>,
}

impl Server {
    pub fn new(configs: Vec<ServerConfig>) -> Result<Self, String> {
        let epoll_fd = unsafe { epoll_create1(0) };
        if epoll_fd < 0 {
            return Err("Failed to create epoll instance".to_string());
        }

        let mut server = Server {
            epoll_fd,
            listen_fds: HashMap::new(),
            clients: HashMap::new(),
            configs,
        };

        server.setup_sockets()?;
        Ok(server)
    }

    fn setup_sockets(&mut self) -> Result<(), String> {
        let mut ports: Vec<u16> = self.configs.iter().map(|c| c.port).collect();
        ports.sort();
        ports.dedup();

        for port in ports {
            unsafe {
                let fd = socket(AF_INET, SOCK_STREAM, 0);
                if fd < 0 {
                    return Err("Failed to create socket".to_string());
                }

                let opt: c_int = 1;
                libc::setsockopt(
                    fd,
                    SOL_SOCKET,
                    SO_REUSEADDR,
                    &opt as *const _ as *const libc::c_void,
                    mem::size_of::<c_int>() as u32,
                );

                let flags = fcntl(fd, libc::F_GETFL, 0);
                fcntl(fd, F_SETFL, flags | O_NONBLOCK);

                let mut addr: sockaddr_in = mem::zeroed();
                addr.sin_family = AF_INET as u16;
                addr.sin_port = port.to_be();
                addr.sin_addr.s_addr = libc::INADDR_ANY;

                if libc::bind(
                    fd,
                    &addr as *const _ as *const libc::sockaddr,
                    mem::size_of::<sockaddr_in>() as u32,
                ) < 0
                {
                    return Err(format!("Failed to bind to port {}", port));
                }

                if libc::listen(fd, 1024) < 0 {
                    return Err(format!("Failed to listen on port {}", port));
                }

                let mut ev = epoll_event {
                    events: EPOLLIN as u32,
                    u64: fd as u64,
                };

                if epoll_ctl(self.epoll_fd, EPOLL_CTL_ADD, fd, &mut ev) < 0 {
                    return Err("Failed to add socket to epoll".to_string());
                }

                self.listen_fds.insert(fd, port);
                println!("Listening on port {} [Socket FD: {}]", port, fd);
            }
        }
        Ok(())
    }

    pub fn match_config(&self, port: u16, host_header: Option<&String>) -> &ServerConfig {
        let matching_configs: Vec<&ServerConfig> = self
            .configs
            .iter()
            .filter(|c| c.port == port)
            .collect();

        if let Some(raw_host) = host_header {
            let hostname = raw_host.split(':').next().unwrap_or(raw_host);
            if let Some(matched) = matching_configs.iter().find(|c| c.server_name == hostname) {
                return matched;
            }
        }

        matching_configs.first().copied().unwrap_or(&self.configs[0])
    }

    fn close_client(&mut self, fd: c_int) {
        unsafe {
            epoll_ctl(self.epoll_fd, EPOLL_CTL_DEL, fd, std::ptr::null_mut());
            close(fd);
        }
        self.clients.remove(&fd);
    }

    pub fn run(&mut self) {
        println!("Epoll event loop starting...");
        let mut events: [epoll_event; 512] = unsafe { mem::zeroed() };

        loop {
            let num_events = unsafe { epoll_wait(self.epoll_fd, events.as_mut_ptr(), 512, -1) };
            if num_events < 0 {
                continue;
            }

            for i in 0..num_events as usize {
                let fd = events[i].u64 as c_int;
                let event_flags = events[i].events;

                if let Some(&port) = self.listen_fds.get(&fd) {
                    // Accept new connections
                    unsafe {
                        loop {
                            let mut client_addr: sockaddr_in = mem::zeroed();
                            let mut client_len = mem::size_of::<sockaddr_in>() as u32;
                            let client_fd = accept(
                                fd,
                                &mut client_addr as *mut _ as *mut sockaddr,
                                &mut client_len,
                            );

                            if client_fd < 0 {
                                break;
                            }

                            let flags = fcntl(client_fd, libc::F_GETFL, 0);
                            fcntl(client_fd, F_SETFL, flags | O_NONBLOCK);

                            let mut ev = epoll_event {
                                events: EPOLLIN as u32,
                                u64: client_fd as u64,
                            };
                            epoll_ctl(self.epoll_fd, EPOLL_CTL_ADD, client_fd, &mut ev);

                            self.clients.insert(
                                client_fd,
                                ClientState {
                                    port,
                                    read_buf: Vec::new(),
                                    write_buf: Vec::new(),
                                    bytes_written: 0,
                                },
                            );
                        }
                    }
                } else {
                    // Process active client socket
                    if (event_flags & EPOLLIN as u32) != 0 {
                        self.handle_client_read(fd);
                    }

                    if (event_flags & EPOLLOUT as u32) != 0 {
                        self.handle_client_write(fd);
                    }
                }
            }
        }
    }

  fn handle_client_read(&mut self, fd: c_int) {
    let mut temp_buf = [0u8; 4096];
    let bytes_read = unsafe {
        read(
            fd,
            temp_buf.as_mut_ptr() as *mut libc::c_void,
            temp_buf.len(),
        )
    };

    if bytes_read > 0 {
        // Step 1: Read incoming bytes into buffer and check for complete request header
        let req_data = if let Some(client) = self.clients.get_mut(&fd) {
            client.read_buf.extend_from_slice(&temp_buf[..bytes_read as usize]);
            if client.read_buf.windows(4).any(|w| w == b"\r\n\r\n") {
                Some((client.port, client.read_buf.clone()))
            } else {
                None
            }
        } else {
            None
        };

        // Step 2: Process request without holding a borrow on self.clients
        if let Some((port, read_buf)) = req_data {
            let response = match HttpRequest::parse(&read_buf) {
                Ok(req) => {
                    let config = self.match_config(port, req.headers.get("host"));
                    handle_request(&req, config)
                }
                Err(_) => {
                    let fallback_config = self.match_config(port, None);
                    build_error_response(fallback_config, 400, "Bad Request")
                }
            };

            // Step 3: Update client state with response payload
            if let Some(client) = self.clients.get_mut(&fd) {
                client.write_buf = response.to_bytes();
                client.bytes_written = 0;

                let mut ev = epoll_event {
                    events: EPOLLOUT as u32,
                    u64: fd as u64,
                };
                unsafe {
                    epoll_ctl(self.epoll_fd, EPOLL_CTL_MOD, fd, &mut ev);
                }
            }
        }
    } else if bytes_read == 0 {
        self.close_client(fd);
    } else {
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(EAGAIN) && err.raw_os_error() != Some(EWOULDBLOCK) {
            self.close_client(fd);
        }
    }
}

    fn handle_client_write(&mut self, fd: c_int) {
        if let Some(client) = self.clients.get_mut(&fd) {
            let remaining = &client.write_buf[client.bytes_written..];
            if remaining.is_empty() {
                self.close_client(fd);
                return;
            }

            let written = unsafe {
                write(
                    fd,
                    remaining.as_ptr() as *const libc::c_void,
                    remaining.len(),
                )
            };

            if written > 0 {
                client.bytes_written += written as usize;
                if client.bytes_written >= client.write_buf.len() {
                    self.close_client(fd);
                }
            } else {
                let err = io::Error::last_os_error();
                if err.raw_os_error() != Some(EAGAIN) && err.raw_os_error() != Some(EWOULDBLOCK) {
                    self.close_client(fd);
                }
            }
        } else {
            self.close_client(fd);
        }
    }
}

fn find_matching_route<'a>(config: &'a ServerConfig, uri: &str) -> Option<&'a RouteConfig> {
    config
        .routes
        .iter()
        .filter(|r| uri.starts_with(&r.path))
        .max_by_key(|r| r.path.len())
}

fn generate_autoindex(dir_path: &str, req_uri: &str) -> Result<HttpResponse, String> {
    let entries = fs::read_dir(dir_path).map_err(|e| e.to_string())?;
    let mut html = format!(
        "<html><head><title>Index of {}</title></head><body><h1>Index of {}</h1><hr><ul>",
        req_uri, req_uri
    );

    for entry in entries.flatten() {
        if let Ok(name) = entry.file_name().into_string() {
            let href = format!("{}/{}", req_uri.trim_end_matches('/'), name);
            html.push_str(&format!("<li><a href=\"{}\">{}</a></li>", href, name));
        }
    }
    html.push_str("</ul><hr></body></html>");

    let mut resp = HttpResponse::new(200, "OK");
    resp.headers.insert("Content-Type".to_string(), "text/html".to_string());
    resp.headers.insert("Content-Length".to_string(), html.len().to_string());
    resp.body = html.into_bytes();
    Ok(resp)
}

fn get_mime_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "text/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

fn handle_request(req: &HttpRequest, config: &ServerConfig) -> HttpResponse {
    if req.body.len() > config.client_max_body_size {
        return build_error_response(config, 413, "Payload Too Large");
    }

    let route = match find_matching_route(config, &req.uri) {
        Some(r) => r,
        None => return build_error_response(config, 404, "Not Found"),
    };

    if !route.allowed_methods.contains(&req.method) {
        return build_error_response(config, 405, "Method Not Allowed");
    }

    if let Some(ref redirect_url) = route.redirect {
        let mut resp = HttpResponse::new(301, "Moved Permanently");
        resp.headers.insert("Location".to_string(), redirect_url.clone());
        return resp;
    }

    let relative_path = if req.uri.starts_with(&route.path) {
        &req.uri[route.path.len()..]
    } else {
        &req.uri[..]
    };

    let clean_relative = relative_path.split('?').next().unwrap_or(relative_path);
    let trimmed = clean_relative.trim_start_matches('/');
    let mut target_path = format!("{}/{}", route.root.trim_end_matches('/'), trimmed);

    if let Some(ref ext) = route.cgi_extension {
        if target_path.ends_with(ext) && Path::new(&target_path).exists() {
            let handler = CgiHandler::new(target_path);
            return match handler.execute(req, config) {
                Ok(resp) => resp,
                Err(_) => build_error_response(config, 500, "Internal Server Error"),
            };
        }
    }

    if req.method == "POST" {
        if Path::new(&target_path).is_dir() || target_path.ends_with('/') {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            target_path = format!("{}/upload_{}.txt", target_path.trim_end_matches('/'), timestamp);
        }

        return match fs::write(&target_path, &req.body) {
            Ok(_) => {
                let mut resp = HttpResponse::new(201, "Created");
                resp.headers.insert("Location".to_string(), req.uri.clone());
                resp.headers.insert("Content-Type".to_string(), "text/plain".to_string());
                resp.body = b"File created successfully\n".to_vec();
                resp
            }
            Err(_) => build_error_response(config, 500, "Internal Server Error"),
        };
    }

    if req.method == "DELETE" {
        if !Path::new(&target_path).exists() {
            return build_error_response(config, 404, "Not Found");
        }

        if Path::new(&target_path).is_dir() {
            return build_error_response(config, 403, "Forbidden");
        }

        return match fs::remove_file(&target_path) {
            Ok(_) => {
                let mut resp = HttpResponse::new(200, "OK");
                resp.headers.insert("Content-Type".to_string(), "text/plain".to_string());
                resp.body = b"File deleted successfully\n".to_vec();
                resp
            }
            Err(_) => build_error_response(config, 500, "Internal Server Error"),
        };
    }

    if Path::new(&target_path).is_dir() {
        if let Some(ref default_file) = route.default_file {
            let index_path = format!("{}/{}", target_path.trim_end_matches('/'), default_file);
            if Path::new(&index_path).exists() {
                target_path = index_path;
            } else if route.autoindex {
                return generate_autoindex(&target_path, &req.uri)
                    .unwrap_or_else(|_| build_error_response(config, 500, "Internal Server Error"));
            }
        } else if route.autoindex {
            return generate_autoindex(&target_path, &req.uri)
                .unwrap_or_else(|_| build_error_response(config, 500, "Internal Server Error"));
        }
    }

    match fs::read(&target_path) {
        Ok(content) => {
            let mut resp = HttpResponse::new(200, "OK");
            let mime = get_mime_type(&target_path);
            resp.headers.insert("Content-Type".to_string(), mime.to_string());
            resp.headers.insert("Content-Length".to_string(), content.len().to_string());
            resp.body = content;
            resp
        }
        Err(_) => build_error_response(config, 404, "Not Found"),
    }
}

fn build_error_response(config: &ServerConfig, code: u16, reason: &str) -> HttpResponse {
    if let Some(error_page_path) = config.error_pages.get(&code) {
        if let Ok(content) = fs::read_to_string(error_page_path) {
            let mut resp = HttpResponse::new(code, reason);
            resp.headers.insert("Content-Type".to_string(), "text/html".to_string());
            resp.headers.insert("Content-Length".to_string(), content.len().to_string());
            resp.body = content.into_bytes();
            return resp;
        }
    }

    let body = format!("<h1>{} {}</h1>", code, reason);
    let mut resp = HttpResponse::new(code, reason);
    resp.headers.insert("Content-Type".to_string(), "text/html".to_string());
    resp.headers.insert("Content-Length".to_string(), body.len().to_string());
    resp.body = body.into_bytes();
    resp
}