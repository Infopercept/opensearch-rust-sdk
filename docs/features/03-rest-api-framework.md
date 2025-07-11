# REST API Framework

## Overview

The REST API Framework enables extensions to expose custom HTTP endpoints that integrate seamlessly with OpenSearch's REST layer. This framework handles routing, request parsing, response formatting, and integration with OpenSearch's security and authentication mechanisms.

## Goals

- Type-safe REST endpoint definition
- Automatic route registration with OpenSearch
- Support for all HTTP methods and content types
- Integration with OpenSearch's REST conventions
- Async request handling with proper backpressure
- OpenAPI/Swagger documentation generation

## Design

### REST Handler Traits

```rust
/// Base trait for REST handlers
#[async_trait]
pub trait RestHandler: Send + Sync + 'static {
    /// HTTP methods this handler supports
    fn methods(&self) -> &[Method];
    
    /// Path pattern for this handler (e.g., "/_extensions/{extension_id}/users/{id}")
    fn path(&self) -> &str;
    
    /// Handle the REST request
    async fn handle(&self, request: RestRequest) -> Result<RestResponse, RestError>;
    
    /// Optional: provide OpenAPI schema for documentation
    fn openapi_schema(&self) -> Option<OpenApiOperation> {
        None
    }
}

/// REST request representation
#[derive(Debug)]
pub struct RestRequest {
    /// HTTP method
    pub method: Method,
    /// Request path with parameters extracted
    pub path: String,
    /// Path parameters (e.g., {id} -> actual value)
    pub path_params: HashMap<String, String>,
    /// Query parameters
    pub query_params: MultiMap<String, String>,
    /// Request headers
    pub headers: HeaderMap,
    /// Request body as bytes
    pub body: Bytes,
    /// Content type of the request
    pub content_type: Option<ContentType>,
    /// Remote address
    pub remote_addr: Option<SocketAddr>,
}

impl RestRequest {
    /// Parse body as JSON
    pub fn json<T: DeserializeOwned>(&self) -> Result<T, RestError> {
        serde_json::from_slice(&self.body)
            .map_err(|e| RestError::InvalidJson(e))
    }
    
    /// Get body as string
    pub fn text(&self) -> Result<String, RestError> {
        String::from_utf8(self.body.to_vec())
            .map_err(|e| RestError::InvalidUtf8(e))
    }
}

/// REST response
#[derive(Debug)]
pub struct RestResponse {
    /// HTTP status code
    pub status: StatusCode,
    /// Response headers
    pub headers: HeaderMap,
    /// Response body
    pub body: RestResponseBody,
}

/// Response body variants
#[derive(Debug)]
pub enum RestResponseBody {
    /// JSON response
    Json(serde_json::Value),
    /// Plain text response
    Text(String),
    /// Binary response
    Binary(Bytes),
    /// Empty response
    Empty,
}

impl RestResponse {
    /// Create a JSON response
    pub fn json<T: Serialize>(value: T) -> Result<Self, RestError> {
        Ok(Self {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: RestResponseBody::Json(serde_json::to_value(value)?),
        })
    }
    
    /// Create an error response
    pub fn error(status: StatusCode, message: &str) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            body: RestResponseBody::Json(json!({
                "error": {
                    "type": status.canonical_reason().unwrap_or("error"),
                    "reason": message
                }
            })),
        }
    }
}
```

### REST Router

```rust
/// Router for managing REST endpoints
pub struct RestRouter {
    routes: Arc<RwLock<HashMap<String, Route>>>,
    prefix: String,
}

struct Route {
    pattern: PathPattern,
    methods: HashSet<Method>,
    handler: Arc<dyn RestHandler>,
}

impl RestRouter {
    /// Create a new router with extension prefix
    pub fn new(extension_id: &str) -> Self {
        Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
            prefix: format!("/_extensions/{}", extension_id),
        }
    }
    
    /// Register a REST handler
    pub fn route<H: RestHandler>(&mut self, handler: H) -> &mut Self {
        let path = format!("{}{}", self.prefix, handler.path());
        let pattern = PathPattern::new(&path);
        
        let route = Route {
            pattern,
            methods: handler.methods().iter().cloned().collect(),
            handler: Arc::new(handler),
        };
        
        self.routes.write().unwrap()
            .insert(path, route);
        
        self
    }
    
    /// Match a request to a handler
    pub async fn handle(&self, request: RestRequest) -> Result<RestResponse, RestError> {
        let routes = self.routes.read().unwrap();
        
        for (_, route) in routes.iter() {
            if let Some(params) = route.pattern.matches(&request.path) {
                if route.methods.contains(&request.method) {
                    let mut request = request;
                    request.path_params = params;
                    return route.handler.handle(request).await;
                }
            }
        }
        
        Ok(RestResponse::error(
            StatusCode::NOT_FOUND,
            "No handler found for this endpoint",
        ))
    }
}
```

### REST Extension Integration

```rust
/// Extension trait for REST support
#[async_trait]
pub trait RestExtension: Extension {
    /// Configure REST routes
    fn rest_routes(&self, router: &mut RestRouter);
    
    /// Optional: configure REST settings
    fn rest_settings(&self) -> RestSettings {
        RestSettings::default()
    }
}

/// REST-specific settings
#[derive(Debug, Clone)]
pub struct RestSettings {
    /// Maximum request body size
    pub max_body_size: usize,
    /// Request timeout
    pub request_timeout: Duration,
    /// Enable CORS
    pub enable_cors: bool,
    /// Allowed CORS origins
    pub cors_origins: Vec<String>,
}
```

### Example REST Handler

```rust
/// Example: User management endpoints
struct UserHandler {
    user_service: Arc<UserService>,
}

#[async_trait]
impl RestHandler for UserHandler {
    fn methods(&self) -> &[Method] {
        &[Method::GET, Method::POST, Method::PUT, Method::DELETE]
    }
    
    fn path(&self) -> &str {
        "/users/{id}"
    }
    
    async fn handle(&self, request: RestRequest) -> Result<RestResponse, RestError> {
        match request.method {
            Method::GET => {
                let id = request.path_params.get("id")
                    .ok_or_else(|| RestError::BadRequest("Missing user ID"))?;
                
                match self.user_service.get_user(id).await? {
                    Some(user) => RestResponse::json(user),
                    None => Ok(RestResponse::error(
                        StatusCode::NOT_FOUND,
                        "User not found",
                    )),
                }
            }
            Method::POST => {
                let user: CreateUserRequest = request.json()?;
                let created = self.user_service.create_user(user).await?;
                
                let mut response = RestResponse::json(created)?;
                response.status = StatusCode::CREATED;
                Ok(response)
            }
            Method::PUT => {
                let id = request.path_params.get("id")
                    .ok_or_else(|| RestError::BadRequest("Missing user ID"))?;
                let update: UpdateUserRequest = request.json()?;
                
                match self.user_service.update_user(id, update).await? {
                    Some(user) => RestResponse::json(user),
                    None => Ok(RestResponse::error(
                        StatusCode::NOT_FOUND,
                        "User not found",
                    )),
                }
            }
            Method::DELETE => {
                let id = request.path_params.get("id")
                    .ok_or_else(|| RestError::BadRequest("Missing user ID"))?;
                
                self.user_service.delete_user(id).await?;
                Ok(RestResponse {
                    status: StatusCode::NO_CONTENT,
                    headers: HeaderMap::new(),
                    body: RestResponseBody::Empty,
                })
            }
            _ => unreachable!(),
        }
    }
}
```

### REST Action Registration

```rust
/// Register REST actions with OpenSearch
pub async fn register_rest_actions(
    transport_client: &TransportClient,
    router: &RestRouter,
) -> Result<(), TransportError> {
    let mut actions = Vec::new();
    
    for (path, route) in router.routes.read().unwrap().iter() {
        for method in &route.methods {
            actions.push(RestActionRegistration {
                method: method.as_str().to_string(),
                path: path.clone(),
                unique_name: format!("{}:{}", method.as_str(), path),
            });
        }
    }
    
    let request = RegisterRestActionsRequest {
        identity: ExtensionIdentity {
            unique_id: "my-extension".to_string(),
            // ... other fields
        },
        rest_actions: actions,
    };
    
    transport_client.send_request(
        "internal:extensions/registerrestactions",
        request,
    ).await?;
    
    Ok(())
}
```

## Implementation Plan

### Phase 1: Core REST Framework (Week 1)
- [ ] Define REST handler traits
- [ ] Implement request/response types
- [ ] Create path pattern matching
- [ ] Basic router implementation

### Phase 2: OpenSearch Integration (Week 2)
- [ ] REST action registration protocol
- [ ] Transport-to-REST bridge
- [ ] Error handling and formatting
- [ ] Security integration

### Phase 3: Advanced Features (Week 3)
- [ ] Request validation
- [ ] Response compression
- [ ] CORS support
- [ ] Rate limiting

### Phase 4: Developer Experience (Week 4)
- [ ] Macro for easy handler definition
- [ ] OpenAPI documentation generation
- [ ] Testing utilities
- [ ] Example handlers

## Usage Example

```rust
use opensearch_sdk::{RestExtension, RestRouter, RestHandler, async_trait};

struct MyExtension;

impl RestExtension for MyExtension {
    fn rest_routes(&self, router: &mut RestRouter) {
        router
            .route(HealthHandler)
            .route(UserHandler::new())
            .route(DocumentHandler::new());
    }
}

// Simple health check handler
struct HealthHandler;

#[async_trait]
impl RestHandler for HealthHandler {
    fn methods(&self) -> &[Method] {
        &[Method::GET]
    }
    
    fn path(&self) -> &str {
        "/health"
    }
    
    async fn handle(&self, _request: RestRequest) -> Result<RestResponse, RestError> {
        RestResponse::json(json!({
            "status": "healthy",
            "timestamp": Utc::now().to_rfc3339(),
        }))
    }
}
```

## Testing Strategy

### Unit Tests
- Path pattern matching
- Request parsing
- Response serialization
- Error handling

### Integration Tests
- Full REST request cycle
- Multiple handlers
- Concurrent requests
- Error scenarios

### Performance Tests
- Request throughput
- Response latency
- Memory usage
- Connection handling

## Security Considerations

- Input validation on all requests
- Authentication token forwarding
- Authorization checks
- Rate limiting per client
- Request size limits
- XSS and injection prevention

## Future Enhancements

- WebSocket support
- Server-sent events
- GraphQL integration
- Request/response interceptors
- Automatic API versioning
- Request retry mechanisms