# Action System

## Overview

The Action System provides a framework for defining, registering, and executing transport actions in OpenSearch extensions. It enables type-safe request/response patterns, async execution, and integration with OpenSearch's transport layer for both client and server operations.

## Goals

- Type-safe action definitions with compile-time guarantees
- Async-first design with proper cancellation
- Support for streaming responses
- Action versioning and compatibility
- Client and server action implementations
- Performance monitoring and metrics

## Design

### Core Action Traits

```rust
/// Base trait for all actions
pub trait Action: Send + Sync + 'static {
    /// Action name (e.g., "indices:data/read/search")
    fn name() -> &'static str where Self: Sized;
    
    /// Request type for this action
    type Request: ActionRequest;
    
    /// Response type for this action
    type Response: ActionResponse;
    
    /// Whether this action supports streaming responses
    fn supports_streaming() -> bool {
        false
    }
}

/// Request that can be sent via transport
pub trait ActionRequest: Send + Sync + 'static {
    /// Validate the request
    fn validate(&self) -> Result<(), ValidationError> {
        Ok(())
    }
    
    /// Get timeout for this request
    fn timeout(&self) -> Option<Duration> {
        None
    }
    
    /// Serialize to bytes
    fn serialize(&self) -> Result<Bytes, SerializationError>;
    
    /// Deserialize from bytes
    fn deserialize(bytes: &[u8]) -> Result<Self, SerializationError> 
    where 
        Self: Sized;
}

/// Response that can be received via transport
pub trait ActionResponse: Send + Sync + 'static {
    /// Whether this is an error response
    fn is_error(&self) -> bool {
        false
    }
    
    /// Serialize to bytes
    fn serialize(&self) -> Result<Bytes, SerializationError>;
    
    /// Deserialize from bytes
    fn deserialize(bytes: &[u8]) -> Result<Self, SerializationError>
    where
        Self: Sized;
}
```

### Action Handler

```rust
/// Handler that processes action requests
#[async_trait]
pub trait ActionHandler<A: Action>: Send + Sync + 'static {
    /// Handle the action request
    async fn handle(
        &self,
        request: A::Request,
        context: ActionContext,
    ) -> Result<A::Response, ActionError>;
}

/// Context provided to action handlers
pub struct ActionContext {
    /// Transport channel for sending additional requests
    pub transport: Arc<TransportClient>,
    /// Current user/security context
    pub security_context: Option<SecurityContext>,
    /// Request metadata
    pub metadata: ActionMetadata,
    /// Cancellation token
    pub cancellation: CancellationToken,
}

/// Action metadata
#[derive(Debug, Clone)]
pub struct ActionMetadata {
    /// Source node ID
    pub source_node: String,
    /// Request ID for correlation
    pub request_id: RequestId,
    /// Request timestamp
    pub timestamp: Instant,
    /// Custom headers
    pub headers: HashMap<String, String>,
}
```

### Action Registry

```rust
/// Registry for action handlers
pub struct ActionRegistry {
    handlers: HashMap<&'static str, Box<dyn AnyActionHandler>>,
    client_actions: HashMap<&'static str, ActionMetadata>,
}

impl ActionRegistry {
    /// Register an action handler
    pub fn register_handler<A, H>(&mut self, handler: H)
    where
        A: Action,
        H: ActionHandler<A> + 'static,
    {
        self.handlers.insert(
            A::name().to_string(),
            Box::new(TypedActionHandler::<A, H>::new(handler)),
        );
    }
    
    /// Register a client action
    pub fn register_client_action<A: Action>(&mut self) {
        self.client_actions.insert(
            A::name().to_string(),
            ActionMetadata::for_action::<A>(),
        );
    }
    
    /// Handle incoming transport request
    pub async fn handle_transport_request(
        &self,
        action_name: &str,
        request_bytes: &[u8],
        context: ActionContext,
    ) -> Result<Bytes, ActionError> {
        let handler = self.handlers.get(action_name)
            .ok_or_else(|| ActionError::UnknownAction(action_name.to_string()))?;
        
        handler.handle_bytes(request_bytes, context).await
    }
}

/// Type-erased action handler
#[async_trait]
trait AnyActionHandler: Send + Sync {
    async fn handle_bytes(
        &self,
        request_bytes: &[u8],
        context: ActionContext,
    ) -> Result<Bytes, ActionError>;
}
```

### Client Actions

```rust
/// Client for executing actions
pub struct ActionClient {
    transport: Arc<TransportClient>,
    registry: Arc<ActionRegistry>,
}

impl ActionClient {
    /// Execute an action and get response
    pub async fn execute<A: Action>(
        &self,
        request: A::Request,
    ) -> Result<A::Response, ActionError> {
        // Validate request
        request.validate()?;
        
        // Serialize request
        let request_bytes = request.serialize()?;
        
        // Send via transport
        let response_bytes = self.transport
            .send_request(A::NAME, request_bytes)
            .await?;
        
        // Deserialize response
        A::Response::deserialize(&response_bytes)
            .map_err(Into::into)
    }
    
    /// Execute an action with custom timeout
    pub async fn execute_with_timeout<A: Action>(
        &self,
        request: A::Request,
        timeout: Duration,
    ) -> Result<A::Response, ActionError> {
        tokio::time::timeout(timeout, self.execute(request))
            .await
            .map_err(|_| ActionError::Timeout)?
    }
    
    /// Execute an action and stream responses
    pub async fn execute_streaming<A, R>(
        &self,
        request: A::Request,
    ) -> Result<impl Stream<Item = Result<R, ActionError>>, ActionError>
    where
        A: Action<Response = R>,
        R: StreamableResponse,
    {
        // Implementation for streaming responses
        unimplemented!()
    }
}
```

### Built-in Actions

```rust
/// Search action
pub struct SearchAction;

impl Action for SearchAction {
    fn name() -> &'static str {
        "indices:data/read/search"
    }
    
    type Request = SearchRequest;
    type Response = SearchResponse;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub indices: Vec<String>,
    pub query: Query,
    pub size: Option<usize>,
    pub from: Option<usize>,
    pub sort: Option<Vec<SortField>>,
    pub aggregations: Option<HashMap<String, Aggregation>>,
}

impl ActionRequest for SearchRequest {
    fn validate(&self) -> Result<(), ValidationError> {
        if self.indices.is_empty() {
            return Err(ValidationError::new("At least one index is required"));
        }
        Ok(())
    }
    
    fn timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }
    
    fn serialize(&self) -> Result<Bytes, SerializationError> {
        // Use protocol buffers or other serialization
        Ok(Bytes::from(serde_json::to_vec(self)?))
    }
    
    fn deserialize(bytes: &[u8]) -> Result<Self, SerializationError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

/// Bulk action for indexing multiple documents
pub struct BulkAction;

impl Action for BulkAction {
    fn name() -> &'static str {
        "indices:data/write/bulk"
    }
    
    type Request = BulkRequest;
    type Response = BulkResponse;
    
    fn supports_streaming() -> bool {
        true
    }
}
```

### Action Macros

```rust
/// Macro for defining actions easily
#[macro_export]
macro_rules! define_action {
    (
        $name:ident,
        $action_name:expr,
        request = $request:ty,
        response = $response:ty
        $(, streaming = $streaming:expr)?
    ) => {
        pub struct $name;
        
        impl Action for $name {
            fn name() -> &'static str {
                $action_name
            }
            type Request = $request;
            type Response = $response;
            
            $(
                fn supports_streaming() -> bool {
                    $streaming
                }
            )?
        }
    };
}

// Usage
define_action!(
    GetDocumentAction,
    "indices:data/read/get",
    request = GetRequest,
    response = GetResponse
);

define_action!(
    ScrollAction,
    "indices:data/read/scroll",
    request = ScrollRequest,
    response = ScrollResponse,
    streaming = true
);
```

### Extension Actions

```rust
/// Extension-specific action
pub struct ExtensionAction<Req, Res> {
    name: String,
    _phantom: PhantomData<(Req, Res)>,
}

impl<Req, Res> ExtensionAction<Req, Res> 
where
    Req: ActionRequest,
    Res: ActionResponse,
{
    pub fn new(extension_id: &str, action_name: &str) -> Self {
        Self {
            name: format!("extensions:{}:{}", extension_id, action_name),
            _phantom: PhantomData,
        }
    }
}

impl<Req, Res> Action for ExtensionAction<Req, Res>
where
    Req: ActionRequest,
    Res: ActionResponse,
{
    fn name() -> &'static str {
        // For dynamic actions, we'll need a different approach
        // This is a limitation - dynamic actions need special handling
        "extensions:dynamic"
    }
    
    type Request = Req;
    type Response = Res;
}

/// Helper for creating extension actions
pub struct ExtensionActionBuilder {
    extension_id: String,
}

impl ExtensionActionBuilder {
    pub fn new(extension_id: impl Into<String>) -> Self {
        Self {
            extension_id: extension_id.into(),
        }
    }
    
    pub fn action<Req, Res>(
        &self,
        name: &str,
    ) -> ExtensionAction<Req, Res>
    where
        Req: ActionRequest,
        Res: ActionResponse,
    {
        ExtensionAction::new(&self.extension_id, name)
    }
}
```

## Implementation Plan

### Phase 1: Core Action System (Week 1)
- [ ] Define action traits and types
- [ ] Implement action registry
- [ ] Create action client
- [ ] Basic serialization support

### Phase 2: Built-in Actions (Week 2)
- [ ] Search action
- [ ] Index/Delete actions
- [ ] Bulk operations
- [ ] Cluster actions

### Phase 3: Advanced Features (Week 3)
- [ ] Streaming responses
- [ ] Action versioning
- [ ] Request validation
- [ ] Timeout handling

### Phase 4: Extension Support (Week 4)
- [ ] Extension action framework
- [ ] Action discovery
- [ ] Inter-extension communication
- [ ] Action metrics

## Usage Example

```rust
use opensearch_sdk::{ActionClient, SearchAction, SearchRequest, Query};

// In extension initialization
let mut registry = ActionRegistry::new();

// Register handlers for actions we implement
registry.register_handler::<MyCustomAction, _>(MyCustomHandler);

// Register client actions we might call
registry.register_client_action::<SearchAction>();

// Create action client
let client = ActionClient::new(transport, registry);

// Execute a search
let search_request = SearchRequest {
    indices: vec!["my-index".to_string()],
    query: Query::match_all(),
    size: Some(10),
    ..Default::default()
};

let response = client.execute::<SearchAction>(search_request).await?;
println!("Found {} documents", response.hits.total.value);

// Execute with timeout
let response = client
    .execute_with_timeout::<GetDocumentAction>(get_request, Duration::from_secs(5))
    .await?;
```

## Testing Strategy

### Unit Tests
- Action serialization/deserialization
- Request validation
- Handler registration
- Error scenarios

### Integration Tests
- End-to-end action execution
- Concurrent requests
- Timeout handling
- Streaming responses

### Performance Tests
- Action throughput
- Serialization overhead
- Registry lookup performance
- Memory usage

## Security Considerations

- Validate all incoming requests
- Enforce action-level permissions
- Audit sensitive actions
- Rate limit by action type
- Sanitize error messages

## Future Enhancements

- Action pipelining
- Batch action execution
- Action result caching
- Circuit breakers per action
- Action replay for debugging
- GraphQL-style field selection