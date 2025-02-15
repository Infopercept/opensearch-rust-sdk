use std::io::Read;
use std::net::{Ipv4Addr, TcpListener};

use opensearch_sdk_rs::transport::TransportTcpHeader;

const DEFAULT_PORT: u32 = 7878;

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
    pub fn default() -> Host {
        Host {
            address: Ipv4Addr::new(127, 0, 0, 1),
            port: DEFAULT_PORT,
        }
    }
    pub fn run(&self) {
        let listener = TcpListener::bind(format!("{}:{}", &self.address.to_string(), &self.port))
            .expect(&format!("Unable to bind to port: {}", &self.port));
        let mut count = 0;

        for stream in listener.incoming() {
            // TODO: use actual OpenSearch TCP stream - this one will not work
            //
            let stream = stream.unwrap();
            count += 1;
            // reading stream from Ok(127.0.0.1:64932)
            // Parsed header: OpenSearchTcpHeader { message_length: 39, request_id: 25, status: 8, version: 136357827, variable_header_size: 26 }
            // reading stream from Ok(127.0.0.1:64933)
            // Unable to parse prefix
            // Error parsing header: Error { kind: UnexpectedEof, message: "failed to fill whole buffer" }
            // reading stream from Ok(127.0.0.1:64934)

            // let mut v = Vec::<u8>::new();
            // let _ = stream.try_clone().unwrap().read_to_end(&mut v);
            // let mut s = String::new();
            // let res = stream.try_clone().unwrap().read_to_string(&mut s);
            // if res.is_ok() {
            //     println!("Reading header: {:?}", res.unwrap());
            // }
            println!("[{}] reading stream from {:?}", &count, &stream.peer_addr());

            match TransportTcpHeader::from_stream(stream) {
                Ok(h) => {
                    // Can finally successfully run and parse a single opensearch header
                    // Parsed header: OpenSearchTcpHeader { message_length: 39, request_id: 49, status: 8, version: 136357827, variable_header_size: 26 }

                    println!(
                        "[{}] Parsed header: {:?}, is_handshake? {:?}",
                        &count,
                        h,
                        h.is_handshake()
                    );
                    if h.is_handshake() {
                        // TODO: actually handle this case
                    }
                }
                Err(e) => {
                    eprintln!("[{}] Error parsing header: {:?}", count, e);
                }
            }
            // let buf_reader = BufReader::new(&mut stream);
            // let http_request: Vec<_> = buf_reader
            //     .lines()
            //     .map(|result| result.map_or("<unknown>".to_string(), |v| v))
            //     .take_while(|line| !line.is_empty())
            //     .collect();
            // dbg!(http_request);
            //
            // let response = "HTTP/1.1 200 OK\r\n\r\n";
            // stream.write_all(response.as_bytes()).unwrap();
        }
    }
}

fn main() {
    let host = Host::new(1234);
    host.run();

    // register the "hello" extension with command line
    // curl -XPOST "localhost:9200/_extensions/initialize" -H "Content-Type:application/json" --data @hello.json
}
