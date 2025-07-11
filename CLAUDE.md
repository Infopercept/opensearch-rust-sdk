# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is the OpenSearch Rust SDK - an early-stage implementation of OpenSearch Extensions in Rust. The project provides a foundation for building OpenSearch extensions using Rust, with a working "Hello World" implementation that demonstrates the transport protocol communication with OpenSearch.

## Build and Development Commands

### Core Commands
```bash
# Build (includes formatting)
just build
# or directly: cargo build

# Run tests
just test
# or directly: cargo test

# Run linter
just clippy
# or directly: cargo clippy --all --all-targets --all-features

# Format code
just fmt
# or directly: cargo fmt --all

# Run the extension server (listens on port 1234)
just run
# or directly: cargo run
```

### Development Workflow
```bash
# Complete setup (starts cluster + builds + registers extension)
just setup

# Start development cycle (builds and runs extension)
just dev

# OpenSearch cluster management
just start-cluster      # Start OpenSearch with extensions enabled
just stop-cluster       # Stop cluster
just restart-cluster    # Restart cluster

# Extension management
just register-extension # Register extension with OpenSearch
just test-extension    # Test the hello endpoint
just check-extension-health  # Check if extension is running

# Monitoring
just cluster-health    # Check cluster health
just extensions-list   # List registered extensions
just cluster-logs      # View cluster logs
```

### Testing a Single Test
```bash
# Run a specific test
cargo test test_name

# Run tests in a specific module
cargo test module_name::

# Run with output
cargo test -- --nocapture
```

## Architecture

### Transport Protocol Implementation
The SDK implements OpenSearch's binary transport protocol:

1. **TCP Server** (`src/main.rs`): Listens on port 1234 for OpenSearch connections
2. **Transport Layer** (`src/transport.rs`): 
   - Parses `TcpHeader` to identify request/response and request IDs
   - Routes requests based on transport action names
   - Handles serialization/deserialization of messages
3. **Protocol Buffers**: Messages defined in `src/*.proto` files, compiled at build time

### Key Components
- **TcpHeader**: Contains request ID, version, and request/response flag
- **Transport Actions**: Handle specific OpenSearch requests (e.g., "internal:transport/handshake")
- **Extension Identity**: Registered via `examples/hello/hello.json`

### Message Flow
1. OpenSearch connects to extension on port 1234
2. Extension parses TCP header from incoming bytes
3. Based on action name, appropriate handler processes the request
4. Response is serialized and sent back with matching request ID

## Extension Registration

The extension must be registered with OpenSearch using the configuration in `examples/hello/hello.json`. This file defines:
- Extension name, unique ID, and version
- Port (1234) and host (127.0.0.1)
- OpenSearch version compatibility (3.0.0+)

## Development Environment

### OpenSearch Setup
The project uses Docker Compose for OpenSearch:
- Configuration: `resources/compose/docker-compose-prod.yml`
- 2-node cluster with extensions enabled
- Admin credentials: `admin:$PASS` (where $PASS is from environment)
- Extensions feature flag: `-Dopensearch.experimental.feature.extensions.enabled=true`

### Security Notes
- Development uses HTTPS with self-signed certificates
- Curl commands use `-ku` flags for insecure connections
- Production deployments should use proper certificates

## Important Implementation Details

### Binary Protocol Parsing
The transport uses `nom` parser combinators to parse the binary protocol. Key structures:
- Fixed header: 6 bytes (TY, ES, status, version, request ID)
- Variable header: Contains features and action names
- Message content: Protocol buffer encoded

### Current Limitations
- Basic hello world implementation only
- Limited to transport protocol (no REST actions yet)
- No cluster state or settings management
- Development/testing focused (not production-ready)

## Testing Strategy

When adding new features:
1. Add unit tests in the same file using `#[cfg(test)]` modules
2. Test protocol parsing with known byte sequences
3. Use `cargo test -- --nocapture` to see debug output
4. Integration testing requires running OpenSearch cluster

## Common Development Tasks

### Adding a New Transport Action
1. Define the protobuf message in `src/`
2. Add to `build.rs` for compilation
3. Implement handler in transport layer
4. Add action name mapping in the request router

### Debugging Transport Issues
- Enable debug logging to see raw bytes
- Use Wireshark or similar to capture TCP traffic
- Check OpenSearch logs for extension-related messages
- Verify request IDs match between request and response