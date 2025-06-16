use std::net::{Ipv4Addr, TcpListener, TcpStream};

use opensearch_sdk_rs::transport::{transport_status, TransportTcpHeader};

const DEFAULT_PORT: u32 = 1234;

#[derive(Debug)]
pub struct Host {
    address: Ipv4Addr,
    port: u32,
}

impl Host {
    pub fn new(port: u32) -> Host {
        Host {
            address: Ipv4Addr::new(127, 0, 0, 1),
            port,
        }
    }

    pub fn run(&self) {
        let listener = TcpListener::bind(format!("{}:{}", &self.address, &self.port))
            .unwrap_or_else(|_| panic!("Unable to bind to port: {}", &self.port));

        println!(
            "🚀 OpenSearch Extension SDK (Rust) started on {}:{}",
            self.address, self.port
        );
        println!("📡 Waiting for OpenSearch connections...");

        let mut count = 0;

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    count += 1;
                    println!("[{}] 📨 Connection from {:?}", count, stream.peer_addr());

                    if let Err(e) = self.handle_connection(stream, count) {
                        eprintln!("[{}] ❌ Error handling connection: {:?}", count, e);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Error accepting connection: {:?}", e);
                }
            }
        }
    }

    fn handle_connection(
        &self,
        stream: TcpStream,
        connection_id: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match TransportTcpHeader::from_stream(stream.try_clone()?) {
            Ok(header) => {
                println!("[{}] 📋 Parsed header: {:?}", connection_id, header);

                if header.is_handshake() {
                    println!("[{}] 🤝 Handling handshake request", connection_id);
                    self.handle_handshake(stream, header, connection_id)?;
                } else if header.is_request_response() {
                    println!("[{}] 📨 Handling request/response", connection_id);
                    self.handle_request_response(stream, header, connection_id)?;
                } else {
                    println!(
                        "[{}] ❓ Unknown request type: {}",
                        connection_id, header.status
                    );
                }
            }
            Err(e) => {
                eprintln!("[{}] ❌ Error parsing header: {:?}", connection_id, e);
            }
        }
        Ok(())
    }

    fn handle_handshake(
        &self,
        mut stream: TcpStream,
        header: TransportTcpHeader,
        connection_id: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("[{}] 🤝 Processing handshake", connection_id);

        // Create a simple handshake response
        let response_content = b"Hello from OpenSearch Rust SDK!";

        let response_header = TransportTcpHeader::new(
            header.request_id,
            transport_status::STATUS_REQRES,
            header.version,
            response_content.len() as u32,
            0, // variable header size
        );

        response_header.write_response(&mut stream, response_content)?;
        println!("[{}] ✅ Handshake response sent", connection_id);

        Ok(())
    }

    fn handle_request_response(
        &self,
        mut stream: TcpStream,
        header: TransportTcpHeader,
        connection_id: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        println!("[{}] 📨 Processing request/response", connection_id);

        // Create a simple hello world response
        let response_content = br#"{"message": "Hello World from OpenSearch Rust Extension!", "status": "ok", "extension": "hello-world-rs"}"#;

        let response_header = TransportTcpHeader::new(
            header.request_id,
            transport_status::STATUS_REQRES,
            header.version,
            response_content.len() as u32,
            0, // variable header size
        );

        response_header.write_response(&mut stream, response_content)?;
        println!("[{}] ✅ Response sent", connection_id);

        Ok(())
    }
}

impl Default for Host {
    fn default() -> Self {
        Host {
            address: Ipv4Addr::new(127, 0, 0, 1),
            port: DEFAULT_PORT,
        }
    }
}

fn main() {
    println!("🦀 OpenSearch SDK for Rust - Hello World Extension");
    println!("==================================================");

    let host = Host::new(1234);
    host.run();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_creation() {
        let host = Host::new(8080);
        assert_eq!(host.port, 8080);
        assert_eq!(host.address, Ipv4Addr::new(127, 0, 0, 1));
    }

    #[test]
    fn test_default_host() {
        let host = Host::default();
        assert_eq!(host.port, DEFAULT_PORT);
    }
}
