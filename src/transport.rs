use core::fmt;
use std::io::{Error, ErrorKind, Read, Write};
use std::net::TcpStream;

const MARKER_BYTES: &[u8; 2] = b"ES";
const REQUEST_ID_SIZE: usize = 8;
const STATUS_SIZE: usize = 1;
const VERSION_ID_SIZE: usize = 4;

#[derive(Debug, Clone)]
struct OSTransportHeaderError;

impl fmt::Display for OSTransportHeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid header prefix bytes")
    }
}

// Reference: https://github.com/opensearch-project/opensearch-sdk-py/blob/main/src/opensearch_sdk_py/transport/tcp_header.py
#[derive(Debug)]
pub struct TransportTcpHeader {
    pub message_length: u32,
    pub request_id: u64,
    pub status: u8,
    pub version: u32,
    pub variable_header_size: u32,
}

// https://github.com/opensearch-project/opensearch-sdk-py/blob/main/src/opensearch_sdk_py/transport/transport_status.py#L9
pub mod transport_status {
    pub static STATUS_REQRES: u8 = 1 << 0;
    pub static STATUS_ERROR: u8 = 1 << 1;
    pub static STATUS_COMPRESS: u8 = 1 << 2;
    pub static STATUS_HANDSHAKE: u8 = 1 << 3;
}

// https://github.com/opensearch-project/opensearch-sdk-java/blob/main/DEVELOPER_GUIDE.md

// https://github.com/opensearch-project/opensearch-rs/blob/main/opensearch/src/http/transport.rs
impl TransportTcpHeader {
    pub fn new(
        request_id: u64,
        status: u8,
        version: u32,
        content_size: u32,
        variable_header_size: u32,
    ) -> Self {
        let message_length = content_size as usize
            + REQUEST_ID_SIZE
            + STATUS_SIZE
            + VERSION_ID_SIZE
            + variable_header_size as usize;
        Self {
            message_length: message_length
                .try_into()
                .expect("unable to convert into u32"),
            request_id,
            status,
            version,
            variable_header_size,
        }
    }

    pub fn is_handshake(&self) -> bool {
        self.status == transport_status::STATUS_HANDSHAKE
    }

    pub fn is_request_response(&self) -> bool {
        self.status == transport_status::STATUS_REQRES
    }

    pub fn is_error(&self) -> bool {
        self.status == transport_status::STATUS_ERROR
    }

    pub fn is_compressed(&self) -> bool {
        self.status == transport_status::STATUS_COMPRESS
    }

    // TODO: proper serialization/deserialization
    // https://github.com/thepacketgeek/rust-tcpstream-demo/tree/master/protocol

    // TODO: find how to simplify the byte reading (with nom??)
    pub fn from_stream(mut stream: TcpStream) -> Result<Self, Error> {
        let mut prefix = [0u8; 2];
        match stream.read_exact(&mut prefix) {
            Ok(_) => {
                if &prefix != MARKER_BYTES {
                    return Err(Error::new(ErrorKind::InvalidData, "invalid header prefix"));
                }
            }
            Err(e) => {
                eprintln!("Unable to parse prefix");
                return Err(e);
            }
        }
        let mut size = [0u8; 4]; // parse bytes for an integer
        match stream.read_exact(&mut size) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Unable to parse size");
                return Err(e);
            }
        }
        let mut request_id = [0u8; 8];
        match stream.read_exact(&mut request_id) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Cannot parse request_id");
                return Err(e);
            }
        }
        let mut status = [0u8; 1];
        match stream.read_exact(&mut status) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Unable to parse status");
                return Err(e);
            }
        }
        let mut version = [0u8; 4];
        match stream.read_exact(&mut version) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Unbale to parse version");
                return Err(e);
            }
        }
        let mut variable_header_size = [0u8; 4];
        match stream.read_exact(&mut variable_header_size) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Unable to parse variable_header_size");
                return Err(e);
            }
        }

        let message_length = REQUEST_ID_SIZE as u32
            + STATUS_SIZE as u32
            + VERSION_ID_SIZE as u32
            + u32::from_be_bytes(variable_header_size);

        Ok(Self {
            request_id: u64::from_be_bytes(request_id),
            status: status[0],
            variable_header_size: u32::from_be_bytes(variable_header_size),
            version: u32::from_be_bytes(version),
            message_length,
        })
    }

    // TODO: rewrite for OpenSearch header purposes
    pub fn write(stream: &mut TcpStream) -> Result<(), Error> {
        // Create a buffer for the TCP header

        let mut header = [0; 20];

        // Set source and destination ports

        header[0..2].copy_from_slice(&0u16.to_be_bytes());

        header[2..4].copy_from_slice(&80u16.to_be_bytes());

        // Set sequence number

        header[4..8].copy_from_slice(&0u32.to_be_bytes());

        // Set acknowledgement number

        header[8..12].copy_from_slice(&0u32.to_be_bytes());

        // Set header length, flags, etc

        header[12] = 0x50; // Header length = 5 words, flags = ACK set

        // Calculate and set checksum (skipped for brevity)

        // Write the header to the stream

        stream.write_all(&header)?;

        Ok(())
    }

    /*
        public static void writeHeader(
        StreamOutput output,
        long requestId,
        byte status,
        Version version,
        int contentSize,
        int variableHeaderSize
    ) throws IOException {
        output.writeBytes(PREFIX);
        // write the size, the size indicates the remaining message size, not including the size int
        output.writeInt(contentSize + REQUEST_ID_SIZE + STATUS_SIZE + VERSION_ID_SIZE + VARIABLE_HEADER_SIZE);
        output.writeLong(requestId);
        output.writeByte(status);
        output.writeInt(version.id);
        assert variableHeaderSize != -1 : "Variable header size not set";
        output.writeInt(variableHeaderSize);
    }
    */
}

mod test {
    #[cfg(test)]
    fn test_from_tcp_stream() {
        use std::net::TcpStream;
    }
}
