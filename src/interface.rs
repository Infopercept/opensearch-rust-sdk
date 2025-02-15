use byteorder::{NetworkEndian, ReadBytesExt, WriteBytesExt};
use std::io::{self, BufRead, BufReader, Read, Write};
// TODO: rewrite using Serde

const TAG_BYTES: &[u8; 2] = b"ES";

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
///
/// We use `&Request` since we don't actually need to own or mutat the request fields
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
        // may not be writing this correctly
        buf.write_u8(self.into())?; // Message type byte

        todo!("Finish implemetnation of serialize")
    }
}

impl Deserialize for Request {
    type Output = Request;

    /// Deserialize Request from bytes ( to receive from TcpStream)
    fn deserialize(buf: &mut impl Read) -> io::Result<Self::Output> {
        let mut buf_reader = BufReader::new(buf);
        let mut parse_location: usize = 0;
        let mut buffer: Vec<u8> = Vec::new();

        todo!();
        // match buf.read_u8()? {
        //     Ok(_) => {
        //     },
        //     _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid Request Header Bytes"))
        // }
    }
}
