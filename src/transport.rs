use std::io::{Error, ErrorKind, Read, Write};
use std::net::TcpStream;

const MARKER_BYTES: &[u8; 2] = b"ES";
const REQUEST_ID_SIZE: usize = 8;
const STATUS_SIZE: usize = 1;
const VERSION_ID_SIZE: usize = 4;

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

    pub fn from_stream(mut stream: TcpStream) -> Result<Self, Error> {
        let mut prefix = [0u8; 2];
        stream.read_exact(&mut prefix)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to parse prefix"))?;

        if &prefix != MARKER_BYTES {
            return Err(Error::new(ErrorKind::InvalidData, "Invalid header prefix"));
        }

        let mut size = [0u8; 4];
        stream.read_exact(&mut size)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to parse size"))?;

        let mut request_id = [0u8; 8];
        stream.read_exact(&mut request_id)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Cannot parse request_id"))?;

        let mut status = [0u8; 1];
        stream.read_exact(&mut status)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to parse status"))?;

        let mut version = [0u8; 4];
        stream.read_exact(&mut version)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to parse version"))?;

        let mut variable_header_size = [0u8; 4];
        stream.read_exact(&mut variable_header_size)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "Unable to parse variable_header_size"))?;

        let message_length = u32::from_be_bytes(size);

        Ok(Self {
            request_id: u64::from_be_bytes(request_id),
            status: status[0],
            variable_header_size: u32::from_be_bytes(variable_header_size),
            version: u32::from_be_bytes(version),
            message_length,
        })
    }

    pub fn write_response(&self, stream: &mut TcpStream, content: &[u8]) -> Result<(), Error> {
        // Write OpenSearch transport header
        stream.write_all(MARKER_BYTES)?;
        stream.write_all(&self.message_length.to_be_bytes())?;
        stream.write_all(&self.request_id.to_be_bytes())?;
        stream.write_all(&[self.status])?;
        stream.write_all(&self.version.to_be_bytes())?;
        stream.write_all(&self.variable_header_size.to_be_bytes())?;

        // Write content
        stream.write_all(content)?;
        stream.flush()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_creation() {
        let header = TransportTcpHeader::new(123, 1, 1000, 100, 50);
        assert_eq!(header.request_id, 123);
        assert_eq!(header.status, 1);
        assert_eq!(header.version, 1000);
        assert!(header.is_request_response());
        assert!(!header.is_handshake());
    }
}
