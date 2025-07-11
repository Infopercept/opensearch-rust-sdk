# Core Extension Framework

## Overview

The Core Extension Framework is the foundation of the OpenSearch Rust SDK. It provides the base traits, lifecycle management, and infrastructure that all extensions build upon. This framework enables extensions to run as separate processes, communicating with OpenSearch via the transport protocol.

## Goals

- Provide a clean, idiomatic Rust API for building extensions
- Ensure type safety and memory safety throughout the extension lifecycle
- Support async/await patterns for efficient I/O operations
- Enable easy testing and debugging of extensions
- Maintain compatibility with OpenSearch's extension protocol

## Design

### Core Traits

```rust
use async_trait::async_trait;

/// Base trait that all extensions must implement
#[async_trait]
pub trait Extension: Send + Sync + 'static {
    /// Extension name for identification
    fn name(&self) -> &str;
    
    /// Unique extension ID
    fn unique_id(&self) -> &str;
    
    /// Extension version
    fn version(&self) -> &str;
    
    /// OpenSearch version compatibility
    fn opensearch_version(&self) -> &str;
    
    /// Java version compatibility (for protocol compatibility)
    fn java_version(&self) -> &str {
        "11"
    }
    
    /// Extension dependencies
    fn dependencies(&self) -> Vec<ExtensionDependency> {
        vec![]
    }
    
    /// Initialize the extension
    async fn initialize(&mut self, context: &ExtensionContext) -> Result<(), ExtensionError>;
    
    /// Shutdown the extension gracefully
    async fn shutdown(&mut self) -> Result<(), ExtensionError>;
}

/// Context provided to extensions during initialization
pub struct ExtensionContext {
    /// Settings specific to this extension
    pub settings: Settings,
    /// Transport client for communicating with OpenSearch
    pub transport_client: Arc<TransportClient>,
    /// Thread pool for async operations
    pub thread_pool: Arc<Runtime>,
    /// Logger instance
    pub logger: Logger,
}

/// Extension dependency specification
pub struct ExtensionDependency {
    pub unique_id: String,
    pub version: Version,
}
```

### Extension Runner

```rust
/// Main extension runner that manages lifecycle
pub struct ExtensionRunner {
    extension: Box<dyn Extension>,
    transport_server: TransportServer,
    context: ExtensionContext,
}

impl ExtensionRunner {
    /// Create a new extension runner
    pub fn new(extension: Box<dyn Extension>) -> Result<Self, ExtensionError> {
        // Initialize transport, settings, etc.
    }
    
    /// Run the extension
    pub async fn run(&mut self) -> Result<(), ExtensionError> {
        // 1. Start transport server
        // 2. Register with OpenSearch
        // 3. Initialize extension
        // 4. Handle requests
        // 5. Shutdown on signal
    }
}
```

### Extension Builder Pattern

```rust
/// Builder for configuring extensions
pub struct ExtensionBuilder {
    name: String,
    unique_id: String,
    version: String,
    settings: Settings,
    // ... other fields
}

impl ExtensionBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        // Default configuration
    }
    
    pub fn unique_id(mut self, id: impl Into<String>) -> Self {
        self.unique_id = id.into();
        self
    }
    
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }
    
    pub fn setting<T: Setting>(mut self, key: &str, value: T) -> Self {
        self.settings.set(key, value);
        self
    }
    
    pub fn build<E: Extension>(self, extension: E) -> Result<ExtensionRunner, ExtensionError> {
        ExtensionRunner::new(Box::new(extension))
    }
}
```

## Implementation Plan

### Phase 1: Basic Framework (Week 1-2)
- [ ] Define core traits and structs
- [ ] Implement basic lifecycle management
- [ ] Create extension runner with signal handling
- [ ] Add logging infrastructure

### Phase 2: Registration & Discovery (Week 3)
- [ ] Implement extension registration protocol
- [ ] Add service discovery mechanisms
- [ ] Handle extension dependencies
- [ ] Create extension metadata management

### Phase 3: Error Handling & Recovery (Week 4)
- [ ] Design comprehensive error types
- [ ] Implement retry mechanisms
- [ ] Add circuit breakers for failures
- [ ] Create health check endpoints

### Phase 4: Testing & Documentation (Week 5)
- [ ] Create extension testing framework
- [ ] Write comprehensive documentation
- [ ] Add example extensions
- [ ] Performance benchmarks

## Usage Example

```rust
use opensearch_sdk::{Extension, ExtensionBuilder, ExtensionContext, ExtensionError};
use async_trait::async_trait;

struct MyExtension {
    // Extension state
}

#[async_trait]
impl Extension for MyExtension {
    fn name(&self) -> &str {
        "My Extension"
    }
    
    fn unique_id(&self) -> &str {
        "my-extension"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn opensearch_version(&self) -> &str {
        "3.0.0"
    }
    
    async fn initialize(&mut self, context: &ExtensionContext) -> Result<(), ExtensionError> {
        // Initialize extension resources
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), ExtensionError> {
        // Clean up resources
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let extension = MyExtension::new();
    
    let mut runner = ExtensionBuilder::new("My Extension")
        .unique_id("my-extension")
        .version("1.0.0")
        .setting("my.setting", "value")
        .build(extension)?;
    
    runner.run().await?;
    Ok(())
}
```

## Testing Strategy

### Unit Tests
- Test trait implementations
- Verify lifecycle transitions
- Mock transport interactions
- Test error scenarios

### Integration Tests
- Full extension lifecycle tests
- Registration and discovery tests
- Communication with mock OpenSearch
- Dependency resolution tests

### Example Test

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_extension_lifecycle() {
        let extension = TestExtension::new();
        let runner = ExtensionBuilder::new("test")
            .unique_id("test-ext")
            .build(extension)
            .unwrap();
        
        // Test initialization
        assert!(runner.initialize().await.is_ok());
        
        // Test shutdown
        assert!(runner.shutdown().await.is_ok());
    }
}
```

## Performance Considerations

- Use Arc for shared state to minimize cloning
- Leverage tokio's work-stealing scheduler
- Implement connection pooling for transport
- Use zero-copy serialization where possible
- Profile and optimize hot paths

## Security Considerations

- Validate all inputs from OpenSearch
- Implement secure transport options
- Sandbox extension execution
- Rate limit incoming requests
- Audit security-sensitive operations

## Migration from Current Code

The current hello world implementation will be refactored to use this framework:

1. Extract transport logic into reusable components
2. Convert main.rs handlers to Extension trait methods
3. Add proper error handling and recovery
4. Implement full protocol support
5. Add comprehensive tests

## Future Enhancements

- Hot reload support for development
- Distributed tracing integration
- Metrics and monitoring
- Extension marketplace support
- WebAssembly extensions