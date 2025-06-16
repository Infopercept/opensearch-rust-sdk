use byteorder::WriteBytesExt;
use std::io::{self, Read, Write};

pub trait Serialize {
    /// Serialize to a `Write`able buffer
    fn serialize(&self, buf: &mut impl Write) -> io::Result<usize>;
}

pub trait Deserialize {
    type Output;
    /// Deserialize from a `Read`able buffer
    fn deserialize(buf: &mut impl Read) -> io::Result<Self::Output>;
}

/// Request object (client -> server)
/// Reference: https://github.com/opensearch-project/opensearch-sdk-py/blob/main/src/opensearch_sdk_py/transport/transport_status.py#L9
#[derive(Debug)]
pub enum Request {
    RequestResponse(String),
    TransportError(String),
    Compress(String),
    Handshake(String),
}

/// Encode the request type as a single byte (as long as we don't exceed 255 types)
impl From<&Request> for u8 {
    fn from(req: &Request) -> Self {
        match req {
            Request::RequestResponse(_) => 1 << 0,
            Request::TransportError(_) => 1 << 1,
            Request::Compress(_) => 1 << 2,
            Request::Handshake(_) => 1 << 3,
        }
    }
}

impl Serialize for Request {
    /// Serialize Request to bytes to send to OpenSearch server
    fn serialize(&self, buf: &mut impl Write) -> io::Result<usize> {
        let type_byte: u8 = self.into();
        buf.write_u8(type_byte)?;

        let content = match self {
            Request::RequestResponse(s) => s,
            Request::TransportError(s) => s,
            Request::Compress(s) => s,
            Request::Handshake(s) => s,
        };

        let content_bytes = content.as_bytes();
        buf.write_all(content_bytes)?;

        Ok(1 + content_bytes.len())
    }
}

impl Deserialize for Request {
    type Output = Request;

    /// Deserialize Request from bytes (to receive from TcpStream)
    fn deserialize(buf: &mut impl Read) -> io::Result<Self::Output> {
        let mut type_buf = [0u8; 1];
        buf.read_exact(&mut type_buf)?;

        let mut content_buf = Vec::new();
        buf.read_to_end(&mut content_buf)?;
        let content = String::from_utf8(content_buf)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        match type_buf[0] {
            1 => Ok(Request::RequestResponse(content)),
            2 => Ok(Request::TransportError(content)),
            4 => Ok(Request::Compress(content)),
            8 => Ok(Request::Handshake(content)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid request type",
            )),
        }
    }
}
