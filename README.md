# opensearch-sdk-rs

🦀 **OpenSearch Extension SDK for Rust** - A working hello world implementation

This is the beginning of an OpenSearch Extension SDK implementation in Rust, providing a foundation for building OpenSearch extensions using Rust.

## 🚀 Quick Start

### Prerequisites
- Rust 1.70+
- OpenSearch 3.0+ with extensions enabled

### Build & Run

```bash
# Build the project
cargo build

# Run the hello world extension
cargo run

# Run tests
cargo test
```

The extension will start listening on `localhost:1234` by default.

### Register Extension with OpenSearch

Once your extension is running, register it with OpenSearch:

```bash
curl -XPOST "http://localhost:9200/_extensions/initialize" \
  -H "Content-Type:application/json" \
  --data @examples/hello/hello.json
```

## 🏗️ Architecture

This SDK implements the OpenSearch transport protocol for extensions:

- **Transport Layer** (`src/transport.rs`) - Handles TCP communication with OpenSearch
- **Interface** (`src/interface.rs`) - Serialization/deserialization traits
- **Server** (`src/main.rs`) - Main extension server implementation

## 🔧 Recent Fixes

- ✅ Fixed all compilation warnings
- ✅ Completed basic serialization/deserialization
- ✅ Working TCP transport header parsing
- ✅ Proper error handling
- ✅ Hello world request/response handlers
- ✅ Clean, documented code

## 📚 References

Inspired by existing OpenSearch SDK implementations:

1. [OpenSearch Extensions Blog](https://opensearch.org/blog/introducing-extensions-for-opensearch)
2. [Python SDK](https://github.com/opensearch-project/opensearch-sdk-py)
3. [Java SDK](https://github.com/opensearch-project/opensearch-sdk-java)

## 🚧 Roadmap

This is a foundational hello world implementation. Future enhancements:

- [ ] Complete protobuf message handling
- [ ] REST action registration
- [ ] Cluster state management
- [ ] Settings management
- [ ] Advanced transport features
- [ ] Production-ready error handling
- [ ] Comprehensive test suite

## 📝 License

Apache License 2.0 - see [LICENSE.txt](LICENSE.txt)

---

**Note**: This is an early-stage implementation focused on establishing the basic transport protocol and communication patterns with OpenSearch. It successfully parses OpenSearch transport headers and responds to basic requests.
