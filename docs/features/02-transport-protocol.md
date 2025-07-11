# Transport Protocol Implementation

## Overview

The Transport Protocol is the binary communication layer between OpenSearch and extensions. It provides high-performance, type-safe message passing with support for request-response patterns, streaming, and async operations. This implementation builds upon the existing hello world prototype to provide full protocol support.

## Goals

- Complete binary protocol implementation compatible with OpenSearch
- High-performance async I/O using tokio
- Type-safe message serialization/deserialization
- Support for all transport actions and message types
- Robust error handling and recovery
- Extension-to-extension communication via proxy

## Protocol Specification

### Message Structure

```
┌─────────────────────────────────────────────┐
│ Fixed Header (6 bytes)                      │
├─────────────────────────────────────────────┤
│ - Message Type (1 byte): 'E'/'I'            │
│ - Protocol Type (1 byte): 'S'/'R'           │
│ - Status (1 byte): 0x00/0x01                │
│ - Version (1 byte): Protocol version        │
│ - Request ID (4 bytes): Unique identifier   │
├─────────────────────────────────────────────┤
│ Variable Header (variable length)           │
├─────────────────────────────────────────────┤
│ - Features (optional)                       │
│ - Action name (for requests)                │
│ - Thread context headers                    │
├─────────────────────────────────────────────┤
│ Message Content (variable length)           │
├─────────────────────────────────────────────┤
│ - Protocol buffer encoded payload           │
└─────────────────────────────────────────────┘
```

## Design

### Core Types

```rust
/// Transport message types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageType {
    Request,
    Response,
}

/// Protocol versions
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Version(pub u8);

/// Unique request identifier
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct RequestId(pub u32);

/// Transport header
#[derive(Debug, Clone)]
pub struct TransportHeader {
    pub message_type: MessageType,
    pub version: Version,
    pub request_id: RequestId,
    pub status: TransportStatus,
    pub features: Features,
    pub action: Option<String>,
    pub thread_context: ThreadContext,
}

/// Transport status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransportStatus {
    Success = 0,
    Error = 1,
}

/// Thread context for distributed tracing
#[derive(Debug, Clone, Default)]
pub struct ThreadContext {
    pub headers: HashMap<String, String>,
}
```

### Message Parsing

```rust
use nom::{IResult, bytes::complete::take, number::complete::{be_u32, u8}};

/// Parse transport header from bytes
pub fn parse_transport_header(input: &[u8]) -> IResult<&[u8], TransportHeader> {
    let (input, message_marker) = u8(input)?;
    let (input, protocol_type) = u8(input)?;
    let (input, status) = u8(input)?;
    let (input, version) = u8(input)?;
    let (input, request_id) = be_u32(input)?;
    
    // Parse variable header
    let (input, features) = parse_features(input)?;
    let (input, action) = if message_marker == b'E' {
        parse_string(input).map(|(i, s)| (i, Some(s)))?
    } else {
        (input, None)
    };
    let (input, thread_context) = parse_thread_context(input)?;
    
    Ok((input, TransportHeader {
        message_type: if protocol_type == b'S' { 
            MessageType::Request 
        } else { 
            MessageType::Response 
        },
        version: Version(version),
        request_id: RequestId(request_id),
        status: if status == 0 { 
            TransportStatus::Success 
        } else { 
            TransportStatus::Error 
        },
        features,
        action,
        thread_context,
    }))
}
```

### Transport Client

```rust
/// Client for sending transport messages
pub struct TransportClient {
    connection: Arc<Mutex<TcpStream>>,
    pending_requests: Arc<DashMap<RequestId, oneshot::Sender<TransportResponse>>>,
    next_request_id: AtomicU32,
}

impl TransportClient {
    /// Send a request and await response
    pub async fn send_request<Req, Res>(
        &self,
        action: &str,
        request: Req,
    ) -> Result<Res, TransportError>
    where
        Req: TransportMessage + Serialize,
        Res: TransportMessage + DeserializeOwned,
    {
        let request_id = self.next_request_id();
        let (tx, rx) = oneshot::channel();
        
        // Register pending request
        self.pending_requests.insert(request_id, tx);
        
        // Serialize and send
        let header = TransportHeader {
            message_type: MessageType::Request,
            request_id,
            action: Some(action.to_string()),
            ..Default::default()
        };
        
        self.send_message(header, request).await?;
        
        // Await response
        let response = rx.await
            .map_err(|_| TransportError::ResponseTimeout)?;
        
        Ok(response.deserialize()?)
    }
    
    /// Send a response to a request
    pub async fn send_response<Res>(
        &self,
        request_id: RequestId,
        response: Res,
    ) -> Result<(), TransportError>
    where
        Res: TransportMessage + Serialize,
    {
        let header = TransportHeader {
            message_type: MessageType::Response,
            request_id,
            ..Default::default()
        };
        
        self.send_message(header, response).await
    }
}
```

### Transport Server

```rust
/// Server for handling incoming transport connections
pub struct TransportServer {
    listener: TcpListener,
    handlers: Arc<TransportHandlers>,
    shutdown: broadcast::Receiver<()>,
}

impl TransportServer {
    /// Start the transport server
    pub async fn start(self) -> Result<(), TransportError> {
        loop {
            tokio::select! {
                Ok((stream, addr)) = self.listener.accept() => {
                    let handlers = Arc::clone(&self.handlers);
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, addr, handlers).await {
                            error!("Connection error: {}", e);
                        }
                    });
                }
                _ = self.shutdown.recv() => {
                    info!("Transport server shutting down");
                    break;
                }
            }
        }
        Ok(())
    }
}

/// Handle a single connection
async fn handle_connection(
    mut stream: TcpStream,
    addr: SocketAddr,
    handlers: Arc<TransportHandlers>,
) -> Result<(), TransportError> {
    let (reader, writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let writer = Arc::new(Mutex::new(writer));
    
    loop {
        // Read header
        let header = read_transport_header(&mut reader).await?;
        
        match header.message_type {
            MessageType::Request => {
                let action = header.action.as_ref()
                    .ok_or(TransportError::MissingAction)?;
                
                // Find handler
                if let Some(handler) = handlers.get(action) {
                    let writer = Arc::clone(&writer);
                    let request_id = header.request_id;
                    
                    tokio::spawn(async move {
                        let response = handler.handle(header, reader).await;
                        send_response(writer, request_id, response).await;
                    });
                } else {
                    // Send error response
                    send_error_response(
                        Arc::clone(&writer),
                        header.request_id,
                        TransportError::UnknownAction(action.clone()),
                    ).await?;
                }
            }
            MessageType::Response => {
                // Handle response for pending request
                if let Some(tx) = handlers.pending_requests.remove(&header.request_id) {
                    let response = read_response(header, &mut reader).await?;
                    let _ = tx.send(response);
                }
            }
        }
    }
}
```

### Transport Actions

```rust
/// Base trait for transport message handlers
#[async_trait]
pub trait TransportHandler: Send + Sync {
    async fn handle(
        &self,
        header: TransportHeader,
        reader: &mut (dyn AsyncRead + Unpin + Send),
    ) -> Result<Box<dyn TransportMessage>, TransportError>;
}

/// Registry for transport handlers
pub struct TransportHandlers {
    handlers: HashMap<String, Box<dyn TransportHandler>>,
    pending_requests: Arc<DashMap<RequestId, oneshot::Sender<TransportResponse>>>,
}

impl TransportHandlers {
    pub fn new() -> Self {
        let mut handlers = HashMap::new();
        
        // Register built-in handlers
        handlers.insert(
            "internal:transport/handshake".to_string(),
            Box::new(HandshakeHandler) as Box<dyn TransportHandler>,
        );
        handlers.insert(
            "internal:discovery/extensions".to_string(),
            Box::new(DiscoveryHandler) as Box<dyn TransportHandler>,
        );
        
        Self {
            handlers,
            pending_requests: Arc::new(DashMap::new()),
        }
    }
    
    /// Register a custom handler
    pub fn register<H: TransportHandler + 'static>(
        &mut self,
        action: &str,
        handler: H,
    ) {
        self.handlers.insert(action.to_string(), Box::new(handler));
    }
}
```

## Implementation Plan

### Phase 1: Protocol Foundation (Week 1)
- [x] Basic header parsing (existing)
- [ ] Complete header serialization
- [ ] Variable header support
- [ ] Thread context handling

### Phase 2: Message Handling (Week 2)
- [ ] Request/response correlation
- [ ] Message type registry
- [ ] Protocol buffer integration
- [ ] Error message handling

### Phase 3: Transport Actions (Week 3)
- [ ] Handshake implementation
- [ ] Discovery protocol
- [ ] Extension registration
- [ ] Environment settings

### Phase 4: Advanced Features (Week 4)
- [ ] Connection pooling
- [ ] Streaming support
- [ ] Extension-to-extension proxy
- [ ] Circuit breakers

## Protocol Buffer Messages

```protobuf
// Transport handshake
message TransportHandshakeRequest {
    string source_node = 1;
    string target_node = 2;
    string version = 3;
}

message TransportHandshakeResponse {
    string discovery_node = 1;
    string cluster_name = 2;
    string version = 3;
}

// Extension registration
message ExtensionRegistrationRequest {
    string unique_id = 1;
    string name = 2;
    string version = 3;
    repeated string dependencies = 4;
}

// Environment settings
message EnvironmentSettingsRequest {
    repeated EnvironmentSetting settings = 1;
}

message EnvironmentSetting {
    string key = 1;
    string value = 2;
    string type = 3;
}
```

## Testing Strategy

### Unit Tests
- Protocol parsing with known byte sequences
- Header serialization/deserialization
- Request/response correlation
- Error handling scenarios

### Integration Tests
- Full handshake sequence
- Multiple concurrent connections
- Large message handling
- Connection failure recovery

### Performance Tests
- Throughput benchmarks
- Latency measurements
- Connection pooling efficiency
- Memory usage under load

## Performance Optimizations

- Zero-copy parsing where possible
- Connection pooling and reuse
- Batch message sending
- Efficient buffer management
- Lock-free data structures for hot paths

## Security Considerations

- TLS support for encrypted transport
- Authentication via transport headers
- Request validation and sanitization
- Rate limiting per connection
- Audit logging for sensitive operations

## Migration from Current Code

1. Extend existing `TcpHeader` to full `TransportHeader`
2. Replace ad-hoc handlers with `TransportHandler` trait
3. Implement proper request/response correlation
4. Add connection pooling and reuse
5. Integrate protocol buffer messages

## Future Enhancements

- HTTP/2 transport option
- WebSocket support for browsers
- gRPC compatibility layer
- Custom compression algorithms
- Transport-level caching