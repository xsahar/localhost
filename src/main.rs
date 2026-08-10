mod cgi;
mod config;
mod http;
mod server;

use config::ServerConfig;
use server::Server;

fn main() {
    println!("Starting Localhost Server...");

    let configs = match ServerConfig::parse_file("config/default.conf") {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("Error loading config ({}), falling back to default.", e);
            vec![ServerConfig::default_local(8080)]
        }
    };

    let mut server = match Server::new(configs) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Initialization error: {}", e);
            return;
        }
    };

    server.run();
}