use crate::config::ServerConfig;
use crate::http::HttpRequest;
use libc::{
    accept, c_int, close, epoll_create1, epoll_ctl, epoll_event, epoll_wait, fcntl, read,
    sockaddr, sockaddr_in, socket, write, AF_INET, EPOLLIN, EPOLL_CTL_ADD, F_SETFL, O_NONBLOCK,
    SOCK_STREAM, SOL_SOCKET, SO_REUSEADDR,
};
use std::fs;
use std::mem;

pub struct Server {
    pub epoll_fd: c_int,
    pub listen_fds: Vec<c_int>,
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
            listen_fds: Vec::new(),
            configs,
        };

        server.setup_sockets()?;
        Ok(server)
    }

    fn setup_sockets(&mut self) -> Result<(), String> {
        for config in &self.configs {
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
                addr.sin_port = config.port.to_be();
                addr.sin_addr.s_addr = libc::INADDR_ANY;

                if libc::bind(
                    fd,
                    &addr as *const _ as *const libc::sockaddr,
                    mem::size_of::<sockaddr_in>() as u32,
                ) < 0
                {
                    return Err(format!("Failed to bind to port {}", config.port));
                }

                if libc::listen(fd, 128) < 0 {
                    return Err(format!("Failed to listen on port {}", config.port));
                }

                let mut ev = epoll_event {
                    events: EPOLLIN as u32,
                    u64: fd as u64,
                };

                if epoll_ctl(self.epoll_fd, EPOLL_CTL_ADD, fd, &mut ev) < 0 {
                    return Err("Failed to add socket to epoll".to_string());
                }

                self.listen_fds.push(fd);
                println!("Listening on port {} (Socket FD: {})", config.port, fd);
            }
        }
        Ok(())
    }

    pub fn run(&mut self) {
        println!("Epoll event loop starting...");
        let mut events: [epoll_event; 64] = unsafe { mem::zeroed() };

        loop {
            let num_events = unsafe { epoll_wait(self.epoll_fd, events.as_mut_ptr(), 64, -1) };

            if num_events < 0 {
                continue;
            }

            for i in 0..num_events as usize {
                let fd = events[i].u64 as c_int;

                if self.listen_fds.contains(&fd) {
                    unsafe {
                        let mut client_addr: sockaddr_in = mem::zeroed();
                        let mut client_len = mem::size_of::<sockaddr_in>() as u32;
                        let client_fd = accept(
                            fd,
                            &mut client_addr as *mut _ as *mut sockaddr,
                            &mut client_len,
                        );

                        if client_fd >= 0 {
                            let flags = fcntl(client_fd, libc::F_GETFL, 0);
                            fcntl(client_fd, F_SETFL, flags | O_NONBLOCK);

                            let mut ev = epoll_event {
                                events: EPOLLIN as u32,
                                u64: client_fd as u64,
                            };
                            epoll_ctl(self.epoll_fd, EPOLL_CTL_ADD, client_fd, &mut ev);
                        }
                    }
                } else {
                    let client_fd = fd;
                    let mut buffer = [0u8; 2048];
                    let bytes_read = unsafe {
                        read(client_fd, buffer.as_mut_ptr() as *mut libc::c_void, buffer.len())
                    };

                    if bytes_read > 0 {
                        let request_bytes = &buffer[..bytes_read as usize];

                        let (status_code, status_text, body_content) =
                            match HttpRequest::parse(request_bytes) {
                                Ok(req) => {
                                    println!("Request: {} {}", req.method, req.uri);

                                    // Map URI root "/" to "/index.html"
                                    let relative_path = if req.uri == "/" {
                                        "/index.html"
                                    } else {
                                        &req.uri
                                    };

                                    let file_path = format!("www{}", relative_path);

                                    match fs::read_to_string(&file_path) {
                                        Ok(content) => (200, "OK", content),
                                        Err(_) => (
                                            404,
                                            "Not Found",
                                            "<h1>404 Not Found</h1><p>Page does not exist.</p>"
                                                .to_string(),
                                        ),
                                    }
                                }
                                Err(_) => (
                                    400,
                                    "Bad Request",
                                    "<h1>400 Bad Request</h1>".to_string(),
                                ),
                            };

                        let response = format!(
                            "HTTP/1.1 {} {}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                            status_code, status_text, body_content.len(), body_content
                        );

                        unsafe {
                            write(
                                client_fd,
                                response.as_bytes().as_ptr() as *const libc::c_void,
                                response.len(),
                            );
                            close(client_fd);
                        }
                    } else {
                        unsafe {
                            close(client_fd);
                        }
                    }
                }
            }
        }
    }
}