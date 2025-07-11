# OpenSearch Extensions for Rust

*Note*: This document describes the design and architecture of the OpenSearch Rust SDK for building extensions.

## Overview

The OpenSearch Rust SDK enables developers to build extensions that run as separate processes, communicating with OpenSearch through a high-performance transport protocol. This architecture provides better isolation, security, and reliability compared to traditional plugins while leveraging Rust's memory safety and performance characteristics.

## Why Rust for Extensions?

- **Memory Safety**: Rust's ownership system prevents common bugs like null pointer dereferences and data races
- **Performance**: Zero-cost abstractions and no garbage collector ensure predictable performance
- **Concurrency**: Built-in async/await support with tokio for efficient I/O operations
- **Type Safety**: Strong type system catches errors at compile time
- **Ecosystem**: Rich ecosystem of libraries for networking, serialization, and web services

## Architecture Overview

```mermaid
graph TB
    subgraph "OpenSearch Cluster"
        OS[OpenSearch Node]
        EM[Extensions Manager]
        RC[RestController]
        TS[TransportService]
    end
    
    subgraph "Rust Extension Process"
        EXT[Extension]
        ER[ExtensionRunner]
        TH[Transport Handler]
        RH[REST Handlers]
        SC[SDK Client]
    end
    
    subgraph "Extension Components"
        REG[Registry]
        SET[Settings]
        SEC[Security]
        DISC[Discovery]
    end
    
    OS --> EM
    EM --> RC
    EM <--> TS
    
    TS <--> TH
    TH --> ER
    ER --> EXT
    EXT --> RH
    EXT --> SC
    
    ER --> REG
    ER --> SET
    ER --> SEC
    ER --> DISC
    
    style EXT fill:#f9f,stroke:#333,stroke-width:4px
    style ER fill:#bbf,stroke:#333,stroke-width:2px
```

## Core Components

### Extension Trait

The foundation of every Rust extension is the `Extension` trait:

```rust
#[async_trait]
pub trait Extension: Send + Sync + 'static {
    /// Extension name
    fn name(&self) -> &str;
    
    /// Unique identifier
    fn unique_id(&self) -> &str;
    
    /// Extension version
    fn version(&self) -> &str;
    
    /// Initialize the extension
    async fn initialize(&mut self, context: ExtensionContext) -> Result<(), ExtensionError>;
    
    /// Register REST handlers
    fn rest_handlers(&self) -> Vec<Box<dyn RestHandler>>;
    
    /// Register transport actions
    fn transport_actions(&self) -> Vec<Box<dyn TransportAction>>;
}
```

### Extension Lifecycle

```mermaid
sequenceDiagram
    participant Main
    participant ExtensionRunner
    participant Extension
    participant OpenSearch
    participant TransportService
    
    Main->>ExtensionRunner: run(extension)
    ExtensionRunner->>ExtensionRunner: bind to host:port
    ExtensionRunner->>Extension: initialize(context)
    ExtensionRunner->>TransportService: start listening
    
    Note over ExtensionRunner,OpenSearch: Extension Registration
    OpenSearch->>ExtensionRunner: InitializeExtensionRequest
    ExtensionRunner->>OpenSearch: InitializeExtensionResponse
    
    ExtensionRunner->>Extension: get rest_handlers()
    ExtensionRunner->>OpenSearch: RegisterRestActionsRequest
    OpenSearch->>OpenSearch: Register routes
    
    Note over OpenSearch,Extension: Runtime Operations
    loop Handle Requests
        OpenSearch->>ExtensionRunner: ExtensionRestRequest
        ExtensionRunner->>Extension: handle(request)
        Extension->>ExtensionRunner: response
        ExtensionRunner->>OpenSearch: ExtensionRestResponse
    end
```

## Transport Protocol Implementation

The Rust SDK implements the OpenSearch transport protocol for efficient binary communication:

```mermaid
graph LR
    subgraph "Message Structure"
        H[Header<br/>6 bytes]
        VH[Variable Header<br/>N bytes]
        P[Payload<br/>Protocol Buffer]
    end
    
    H --> VH
    VH --> P
    
    subgraph "Header Fields"
        MT[Message Type]
        V[Version]
        RID[Request ID]
        S[Status]
    end
    
    subgraph "Processing"
        PARSE[Parse Header]
        ROUTE[Route Message]
        HANDLE[Handle Request]
        RESP[Send Response]
    end
    
    H --> PARSE
    PARSE --> ROUTE
    ROUTE --> HANDLE
    HANDLE --> RESP
```

### Transport Message Flow

```rust
// Incoming message handling
pub async fn handle_connection(stream: TcpStream) {
    let (reader, writer) = stream.split();
    
    loop {
        // 1. Read and parse header
        let header = read_header(&mut reader).await?;
        
        // 2. Route based on message type
        match header.message_type {
            MessageType::Request => {
                let action = read_action(&mut reader).await?;
                let handler = find_handler(&action)?;
                
                // 3. Process request
                let response = handler.handle(reader).await?;
                
                // 4. Send response
                send_response(writer, header.request_id, response).await?;
            }
            MessageType::Response => {
                // Handle response for pending request
                handle_response(header, reader).await?;
            }
        }
    }
}
```

## REST Action Registration and Handling

```mermaid
sequenceDiagram
    participant User
    participant OpenSearch
    participant RestController
    participant ExtensionsManager
    participant RustExtension
    participant RestHandler
    
    Note over RustExtension: Startup & Registration
    RustExtension->>ExtensionsManager: RegisterRestActionsRequest
    ExtensionsManager->>RestController: Register routes
    
    Note over User,RestHandler: Request Handling
    User->>OpenSearch: GET /_extensions/my-ext/api
    OpenSearch->>RestController: Route request
    RestController->>ExtensionsManager: Forward to extension
    ExtensionsManager->>RustExtension: ExtensionRestRequest
    RustExtension->>RestHandler: handle(request)
    RestHandler->>RustExtension: RestResponse
    RustExtension->>ExtensionsManager: Response
    ExtensionsManager->>User: HTTP Response
```

### REST Handler Implementation

```rust
use async_trait::async_trait;
use std::borrow::Cow;

#[async_trait]
pub trait RestHandler: Send + Sync {
    /// HTTP methods this handler supports
    fn methods(&self) -> Cow<'static, [Method]>;
    
    /// Path pattern (e.g., "/users/{id}")
    fn path(&self) -> &str;
    
    /// Handle the request
    async fn handle(&self, request: RestRequest) -> Result<RestResponse, RestError>;
}

// Example implementation
struct UserHandler;

#[async_trait]
impl RestHandler for UserHandler {
    fn methods(&self) -> Cow<'static, [Method]> {
        Cow::Owned(vec![Method::GET, Method::POST])
    }
    
    fn path(&self) -> &str {
        "/users/{id}"
    }
    
    async fn handle(&self, request: RestRequest) -> Result<RestResponse, RestError> {
        match request.method {
            Method::GET => {
                let id = request.path_params.get("id").unwrap();
                // Fetch user logic
                RestResponse::json(user)
            }
            Method::POST => {
                let user: User = request.json()?;
                // Create user logic
                RestResponse::created(user)
            }
            _ => unreachable!()
        }
    }
}
```

## Extension Point Implementation

The Rust SDK provides various extension points similar to the Java SDK:

```mermaid
graph TD
    subgraph "Extension Points"
        EP[Extension]
        EP --> REST[REST Handlers]
        EP --> TRANS[Transport Actions]
        EP --> SEARCH[Search Extensions]
        EP --> ANALYSIS[Analysis Extensions]
        EP --> SCRIPT[Script Extensions]
        EP --> INGEST[Ingest Processors]
        EP --> MAPPER[Field Mappers]
    end
    
    subgraph "Core Services"
        SDK[SDK Client]
        SET[Settings]
        SEC[Security Context]
        DISC[Discovery Service]
    end
    
    EP --> SDK
    EP --> SET
    EP --> SEC
    EP --> DISC
```

### Named XContent Registration Example

```mermaid
sequenceDiagram
    participant Extension
    participant ExtensionRunner
    participant OpenSearch
    participant XContentRegistry
    
    Note over Extension,ExtensionRunner: Extension Startup
    Extension->>ExtensionRunner: new(extension)
    ExtensionRunner->>Extension: set_runner(self)
    Extension->>ExtensionRunner: get_named_xcontent()
    ExtensionRunner->>ExtensionRunner: Store custom XContent
    
    Note over ExtensionRunner,OpenSearch: OpenSearch Startup
    OpenSearch->>ExtensionRunner: InitializeExtensionRequest
    ExtensionRunner->>OpenSearch: Request environment settings
    OpenSearch->>ExtensionRunner: Environment settings
    
    ExtensionRunner->>XContentRegistry: Create with settings
    XContentRegistry->>XContentRegistry: Combine core + custom
    ExtensionRunner->>Extension: Registry available via getter
```

## Security Architecture

```mermaid
graph TB
    subgraph "Security Layers"
        TLS[TLS Transport]
        AUTH[Authentication]
        AUTHZ[Authorization]
        AUDIT[Audit Logging]
    end
    
    subgraph "Security Context"
        PRIN[Principal]
        PERM[Permissions]
        TOKEN[Access Token]
    end
    
    TLS --> AUTH
    AUTH --> AUTHZ
    AUTHZ --> AUDIT
    
    AUTH --> PRIN
    AUTHZ --> PERM
    AUTH --> TOKEN
    
    subgraph "Extension Access"
        REQ[Request]
        SEC_CHECK{Security Check}
        ALLOW[Allow]
        DENY[Deny]
    end
    
    REQ --> SEC_CHECK
    SEC_CHECK -->|Authorized| ALLOW
    SEC_CHECK -->|Unauthorized| DENY
    
    PRIN --> SEC_CHECK
    PERM --> SEC_CHECK
    TOKEN --> SEC_CHECK
```

## Service Discovery and Communication

```mermaid
graph LR
    subgraph "Discovery"
        EXT1[Extension 1]
        EXT2[Extension 2]
        EXT3[Extension 3]
        REG[Registry]
    end
    
    subgraph "Communication Patterns"
        P2P[Extension to Extension]
        PROXY[Proxy Actions]
        MESH[Service Mesh]
    end
    
    EXT1 --> REG
    EXT2 --> REG
    EXT3 --> REG
    
    EXT1 <--> P2P
    P2P <--> EXT2
    
    EXT1 --> PROXY
    PROXY --> EXT3
    
    MESH --> EXT1
    MESH --> EXT2
    MESH --> EXT3
```

### Extension-to-Extension Communication

```rust
// Proxy action for remote execution
pub async fn call_remote_extension(
    client: &SDKClient,
    target_extension: &str,
    action: &str,
    request: impl Serialize,
) -> Result<impl DeserializeOwned, Error> {
    let proxy_request = ProxyActionRequest {
        target_extension: target_extension.to_string(),
        action: action.to_string(),
        request: serde_json::to_value(request)?,
    };
    
    client.execute_action("proxy", proxy_request).await
}
```

## Performance Optimizations

The Rust SDK is designed for high performance:

1. **Zero-copy parsing**: Using `nom` for efficient protocol parsing
2. **Connection pooling**: Reusing TCP connections
3. **Async I/O**: Non-blocking operations with `tokio`
4. **Lock-free data structures**: For hot paths
5. **SIMD optimizations**: For data processing

```mermaid
graph TD
    subgraph "Performance Features"
        POOL[Connection Pool]
        CACHE[Response Cache]
        BATCH[Request Batching]
        STREAM[Streaming]
    end
    
    subgraph "Concurrency"
        ASYNC[Async Runtime]
        THREADS[Thread Pool]
        CHANNELS[Channels]
    end
    
    POOL --> ASYNC
    CACHE --> CHANNELS
    BATCH --> THREADS
    STREAM --> ASYNC
```

## Development Workflow

```mermaid
graph LR
    subgraph "Development"
        CODE[Write Extension]
        BUILD[cargo build]
        TEST[cargo test]
    end
    
    subgraph "Deployment"
        PKG[Package]
        REG[Register]
        RUN[Run Extension]
    end
    
    subgraph "Operations"
        MON[Monitor]
        LOG[Logging]
        METRIC[Metrics]
    end
    
    CODE --> BUILD
    BUILD --> TEST
    TEST --> PKG
    PKG --> REG
    REG --> RUN
    RUN --> MON
    RUN --> LOG
    RUN --> METRIC
```

### Quick Start Example

```rust
use opensearch_sdk::prelude::*;

#[derive(Default)]
struct HelloWorldExtension;

#[async_trait]
impl Extension for HelloWorldExtension {
    fn name(&self) -> &str {
        "hello-world"
    }
    
    fn unique_id(&self) -> &str {
        "hello-world-rust"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    async fn initialize(&mut self, _context: ExtensionContext) -> Result<(), ExtensionError> {
        println!("Hello World Extension initialized!");
        Ok(())
    }
    
    fn rest_handlers(&self) -> Vec<Box<dyn RestHandler>> {
        vec![Box::new(HelloHandler)]
    }
}

struct HelloHandler;

#[async_trait]
impl RestHandler for HelloHandler {
    fn methods(&self) -> &[Method] {
        &[Method::GET]
    }
    
    fn path(&self) -> &str {
        "/hello"
    }
    
    async fn handle(&self, _request: RestRequest) -> Result<RestResponse, RestError> {
        RestResponse::json(json!({
            "message": "Hello from Rust Extension!"
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let extension = HelloWorldExtension::default();
    ExtensionRunner::run(extension).await
}
```

## Testing Framework

```mermaid
graph TD
    subgraph "Test Types"
        UNIT[Unit Tests]
        INTEG[Integration Tests]
        PERF[Performance Tests]
        E2E[End-to-End Tests]
    end
    
    subgraph "Test Infrastructure"
        MOCK[Mock OpenSearch]
        FIX[Fixtures]
        ASSERT[Assertions]
        BENCH[Benchmarks]
    end
    
    UNIT --> MOCK
    INTEG --> FIX
    E2E --> ASSERT
    PERF --> BENCH
```

## Migration from Java/Python

For teams migrating from Java or Python SDKs:

```mermaid
graph LR
    subgraph "Java Extension"
        JAVA[Java Code]
        MAVEN[Maven/Gradle]
        JAR[JAR Package]
    end
    
    subgraph "Migration Tools"
        ANALYZE[Analyze]
        CONVERT[Convert]
        VALIDATE[Validate]
    end
    
    subgraph "Rust Extension"
        RUST[Rust Code]
        CARGO[Cargo]
        BIN[Binary]
    end
    
    JAVA --> ANALYZE
    ANALYZE --> CONVERT
    CONVERT --> RUST
    RUST --> VALIDATE
    RUST --> CARGO
    CARGO --> BIN
```

## Comparison with Plugin Architecture

| Feature | Plugins | Rust Extensions |
|---------|---------|-----------------|
| Process Isolation | Same process | Separate process |
| Memory Safety | JVM overhead | Rust guarantees |
| Failure Impact | Can crash cluster | Isolated failures |
| Resource Control | Shared resources | Independent limits |
| Security | Full access | Sandboxed |
| Deployment | Restart required | Hot deployment |
| Performance | Direct calls | Optimized transport |

## FAQ

**Q: What are the performance implications of using Rust extensions?**
A: Rust extensions have minimal overhead due to:
- Efficient binary transport protocol
- Zero-copy message parsing
- No garbage collection pauses
- Predictable memory usage
- Async I/O for high concurrency

**Q: Can Rust extensions communicate with Java/Python extensions?**
A: Yes, all extensions use the same transport protocol, enabling cross-language communication through proxy actions.

**Q: What OpenSearch versions are supported?**
A: The Rust SDK supports OpenSearch 2.x and above, with wire compatibility across minor versions.

**Q: How do I debug Rust extensions?**
A: The SDK provides:
- Structured logging with `tracing`
- Remote debugging support
- Metrics and telemetry
- Integration with OpenSearch's monitoring

**Q: What's the deployment model for Rust extensions?**
A: Rust extensions compile to single binary executables that can be:
- Deployed on the same node as OpenSearch
- Run on separate nodes for better isolation
- Containerized for cloud deployments
- Managed by orchestration systems

## Next Steps

1. Review the [Developer Guide](DEVELOPER_GUIDE.md) for detailed setup instructions
2. Explore the [example extensions](examples/) for common patterns
3. Check the [API documentation](https://docs.rs/opensearch-sdk) for comprehensive reference
4. Join the [community forums](https://forum.opensearch.org) for support

## Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md) for details on:
- Code style and standards
- Testing requirements
- Documentation guidelines
- Pull request process

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE.txt) file for details.