# Client Libraries

## Overview

Client Libraries provide high-level, idiomatic Rust APIs for interacting with OpenSearch from extensions and external applications. These libraries handle connection management, request serialization, response parsing, and error handling while providing a type-safe interface.

## Goals

- Provide ergonomic Rust APIs for OpenSearch operations
- Support both synchronous and asynchronous clients
- Enable connection pooling and load balancing
- Provide type-safe request/response handling
- Support streaming operations for large datasets
- Maintain compatibility with OpenSearch versions

## Design

### Client Architecture

```rust
/// Main OpenSearch client
pub struct OpenSearchClient {
    transport: Arc<Transport>,
    default_headers: HeaderMap,
    default_timeout: Duration,
}

impl OpenSearchClient {
    /// Create a new client
    pub fn new(config: ClientConfig) -> Result<Self, ClientError> {
        let transport = Transport::new(config.transport)?;
        
        Ok(Self {
            transport: Arc::new(transport),
            default_headers: config.default_headers,
            default_timeout: config.default_timeout,
        })
    }
    
    /// Get index operations client
    pub fn indices(&self) -> IndicesClient {
        IndicesClient::new(Arc::clone(&self.transport))
    }
    
    /// Get document operations client
    pub fn docs(&self) -> DocumentClient {
        DocumentClient::new(Arc::clone(&self.transport))
    }
    
    /// Get search operations client
    pub fn search(&self) -> SearchClient {
        SearchClient::new(Arc::clone(&self.transport))
    }
    
    /// Get cluster operations client
    pub fn cluster(&self) -> ClusterClient {
        ClusterClient::new(Arc::clone(&self.transport))
    }
    
    /// Execute a raw request
    pub async fn request<B>(&self, request: Request<B>) -> Result<Response, ClientError>
    where
        B: Into<Body>,
    {
        self.transport.send(request).await
    }
}

/// Client configuration
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub transport: TransportConfig,
    pub default_headers: HeaderMap,
    pub default_timeout: Duration,
    pub retry_policy: RetryPolicy,
}

/// Transport configuration
#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub urls: Vec<Url>,
    pub auth: Option<Auth>,
    pub tls: Option<TlsConfig>,
    pub proxy: Option<ProxyConfig>,
    pub connection_pool: ConnectionPoolConfig,
}
```

### Document Operations

```rust
/// Document operations client
pub struct DocumentClient {
    transport: Arc<Transport>,
}

impl DocumentClient {
    /// Index a document
    pub async fn index<T>(&self, request: IndexRequest<T>) -> Result<IndexResponse, ClientError>
    where
        T: Serialize,
    {
        let path = match &request.id {
            Some(id) => format!("/{}/_doc/{}", request.index, id),
            None => format!("/{}/_doc", request.index),
        };
        let method = if request.id.is_some() { Method::PUT } else { Method::POST };
        let body = serde_json::to_vec(&request.document)?;
        
        let response = self.transport
            .request(method, &path)
            .body(body)
            .send()
            .await?;
        
        response.json().await
    }
    
    /// Get a document
    pub async fn get<T>(&self, request: GetRequest) -> Result<GetResponse<T>, ClientError>
    where
        T: DeserializeOwned,
    {
        let path = format!("/{}/{}", request.index, request.id);
        
        let response = self.transport
            .request(Method::GET, &path)
            .query(&request.params)
            .send()
            .await?;
        
        response.json().await
    }
    
    /// Update a document
    pub async fn update<T>(&self, request: UpdateRequest<T>) -> Result<UpdateResponse, ClientError>
    where
        T: Serialize,
    {
        let path = format!("/{}/_update/{}", request.index, request.id);
        let body = serde_json::to_vec(&request.update)?;
        
        let response = self.transport
            .request(Method::POST, &path)
            .body(body)
            .send()
            .await?;
        
        response.json().await
    }
    
    /// Delete a document
    pub async fn delete(&self, request: DeleteRequest) -> Result<DeleteResponse, ClientError> {
        let path = format!("/{}/{}", request.index, request.id);
        
        let response = self.transport
            .request(Method::DELETE, &path)
            .send()
            .await?;
        
        response.json().await
    }
    
    /// Bulk operations
    pub async fn bulk(&self, request: BulkRequest) -> Result<BulkResponse, ClientError> {
        let body = request.to_ndjson()?;
        
        let response = self.transport
            .request(Method::POST, "/_bulk")
            .header("Content-Type", "application/x-ndjson")
            .body(body)
            .send()
            .await?;
        
        response.json().await
    }
}

/// Index request
#[derive(Debug, Clone)]
pub struct IndexRequest<T> {
    pub index: String,
    pub id: Option<String>,
    pub document: T,
    pub refresh: Option<RefreshPolicy>,
    pub version: Option<i64>,
    pub version_type: Option<VersionType>,
}

/// Bulk request builder
pub struct BulkRequest {
    operations: Vec<BulkOperation>,
}

impl BulkRequest {
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
        }
    }
    
    pub fn index<T: Serialize>(mut self, index: &str, doc: T) -> Self {
        self.operations.push(BulkOperation::Index {
            index: index.to_string(),
            document: serde_json::to_value(doc).unwrap(),
        });
        self
    }
    
    pub fn delete(mut self, index: &str, id: &str) -> Self {
        self.operations.push(BulkOperation::Delete {
            index: index.to_string(),
            id: id.to_string(),
        });
        self
    }
    
    fn to_ndjson(&self) -> Result<Vec<u8>, ClientError> {
        let mut buffer = Vec::new();
        
        for op in &self.operations {
            match op {
                BulkOperation::Index { index, document } => {
                    serde_json::to_writer(&mut buffer, &json!({
                        "index": { "_index": index }
                    }))?;
                    buffer.push(b'\n');
                    serde_json::to_writer(&mut buffer, document)?;
                    buffer.push(b'\n');
                }
                BulkOperation::Delete { index, id } => {
                    serde_json::to_writer(&mut buffer, &json!({
                        "delete": { "_index": index, "_id": id }
                    }))?;
                    buffer.push(b'\n');
                }
            }
        }
        
        Ok(buffer)
    }
}
```

### Search Operations

```rust
/// Search operations client
pub struct SearchClient {
    transport: Arc<Transport>,
}

impl SearchClient {
    /// Execute a search
    pub async fn search<T>(&self, request: SearchRequest) -> Result<SearchResponse<T>, ClientError>
    where
        T: DeserializeOwned,
    {
        let path = match &request.indices {
            Some(indices) => format!("/{}/_search", indices.join(",")),
            None => "/_search".to_string(),
        };
        
        let body = serde_json::to_vec(&request.body)?;
        
        let response = self.transport
            .request(Method::POST, &path)
            .query(&request.params)
            .body(body)
            .send()
            .await?;
        
        response.json().await
    }
    
    /// Multi-search
    pub async fn msearch<T>(
        &self,
        request: MultiSearchRequest,
    ) -> Result<MultiSearchResponse<T>, ClientError>
    where
        T: DeserializeOwned,
    {
        let body = request.to_ndjson()?;
        
        let response = self.transport
            .request(Method::POST, "/_msearch")
            .header("Content-Type", "application/x-ndjson")
            .body(body)
            .send()
            .await?;
        
        response.json().await
    }
    
    /// Scroll through results
    pub async fn scroll<T>(
        &self,
        request: ScrollRequest,
    ) -> Result<SearchResponse<T>, ClientError>
    where
        T: DeserializeOwned,
    {
        let response = self.transport
            .request(Method::POST, "/_search/scroll")
            .json(&request)
            .send()
            .await?;
        
        response.json().await
    }
    
    /// Point in time search
    pub async fn create_pit(
        &self,
        indices: &[String],
        keep_alive: &str,
    ) -> Result<CreatePitResponse, ClientError> {
        let path = format!("/{}/_pit", indices.join(","));
        
        let response = self.transport
            .request(Method::POST, &path)
            .query(&[("keep_alive", keep_alive)])
            .send()
            .await?;
        
        response.json().await
    }
}

/// Search request builder
#[derive(Debug, Default)]
pub struct SearchRequest {
    pub indices: Option<Vec<String>>,
    pub body: SearchBody,
    pub params: HashMap<String, String>,
}

#[derive(Debug, Default, Serialize)]
pub struct SearchBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<Query>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<Vec<Sort>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggs: Option<HashMap<String, Aggregation>>,
}

/// Query DSL builder
pub mod dsl {
    use super::*;
    
    pub fn match_all() -> Query {
        Query::MatchAll(MatchAllQuery {})
    }
    
    pub fn term(field: &str, value: impl Into<serde_json::Value>) -> Query {
        Query::Term(TermQuery {
            field: field.to_string(),
            value: value.into(),
        })
    }
    
    pub fn bool() -> BoolQueryBuilder {
        BoolQueryBuilder::default()
    }
    
    #[derive(Default)]
    pub struct BoolQueryBuilder {
        must: Vec<Query>,
        should: Vec<Query>,
        must_not: Vec<Query>,
        filter: Vec<Query>,
    }
    
    impl BoolQueryBuilder {
        pub fn must(mut self, query: Query) -> Self {
            self.must.push(query);
            self
        }
        
        pub fn should(mut self, query: Query) -> Self {
            self.should.push(query);
            self
        }
        
        pub fn must_not(mut self, query: Query) -> Self {
            self.must_not.push(query);
            self
        }
        
        pub fn filter(mut self, query: Query) -> Self {
            self.filter.push(query);
            self
        }
        
        pub fn build(self) -> Query {
            Query::Bool(BoolQuery {
                must: if self.must.is_empty() { None } else { Some(self.must) },
                should: if self.should.is_empty() { None } else { Some(self.should) },
                must_not: if self.must_not.is_empty() { None } else { Some(self.must_not) },
                filter: if self.filter.is_empty() { None } else { Some(self.filter) },
            })
        }
    }
}
```

### Streaming Operations

```rust
/// Streaming client for large operations
pub struct StreamingClient {
    transport: Arc<Transport>,
}

impl StreamingClient {
    /// Stream search results
    pub async fn search_stream<T>(
        &self,
        request: SearchRequest,
    ) -> Result<impl Stream<Item = Result<T, ClientError>>, ClientError>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let initial = self.search::<T>(request).await?;
        let scroll_id = initial.scroll_id.clone();
        
        Ok(SearchStream {
            client: self.clone(),
            scroll_id,
            buffer: VecDeque::from(initial.hits.hits),
            keep_alive: "1m".to_string(),
            finished: false,
        })
    }
    
    /// Stream bulk indexing
    pub fn bulk_stream<T>(
        &self,
        batch_size: usize,
    ) -> (BulkSender<T>, BulkReceiver)
    where
        T: Serialize + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(100);
        let (result_tx, result_rx) = mpsc::channel(100);
        
        let client = self.clone();
        tokio::spawn(async move {
            let mut buffer = Vec::with_capacity(batch_size);
            
            while let Some(op) = rx.recv().await {
                buffer.push(op);
                
                if buffer.len() >= batch_size {
                    let request = BulkRequest { operations: buffer.drain(..).collect() };
                    let result = client.bulk(request).await;
                    let _ = result_tx.send(result).await;
                }
            }
            
            // Flush remaining
            if !buffer.is_empty() {
                let request = BulkRequest { operations: buffer };
                let result = client.bulk(request).await;
                let _ = result_tx.send(result).await;
            }
        });
        
        (BulkSender { tx }, BulkReceiver { rx: result_rx })
    }
}

use futures::stream::Stream;
use std::future::Future;
use std::task::{Context, Poll};
use std::pin::Pin;

/// Search results stream
struct SearchStream<T> {
    client: StreamingClient,
    scroll_id: Option<String>,
    buffer: VecDeque<Hit<T>>,
    keep_alive: String,
    finished: bool,
    // Store the in-flight future to avoid creating it in poll
    pending_future: Option<Pin<Box<dyn Future<Output = Result<ScrollResponse<T>, ClientError>> + Send>>>,
}

impl<T> Stream for SearchStream<T>
where
    T: DeserializeOwned + Send + Unpin,
{
    type Item = Result<T, ClientError>;
    
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        
        // Return buffered items first
        if let Some(hit) = this.buffer.pop_front() {
            return Poll::Ready(Some(Ok(hit.source)));
        }
        
        if this.finished {
            return Poll::Ready(None);
        }
        
        // Create the future if we don't have one pending
        if this.pending_future.is_none() {
            let scroll_id = match &this.scroll_id {
                Some(id) => id.clone(),
                None => {
                    this.finished = true;
                    return Poll::Ready(None);
                }
            };
            
            let client = this.client.clone();
            let keep_alive = this.keep_alive.clone();
            
            let future = async move {
                client.scroll::<T>(ScrollRequest {
                    scroll_id,
                    scroll: keep_alive,
                }).await
            };
            
            this.pending_future = Some(Box::pin(future));
        }
        
        // Poll the pending future
        if let Some(future) = &mut this.pending_future {
            match future.as_mut().poll(cx) {
                Poll::Ready(Ok(response)) => {
                    this.pending_future = None; // Clear the future
                    
                    if response.hits.hits.is_empty() {
                        this.finished = true;
                        Poll::Ready(None)
                    } else {
                        this.buffer.extend(response.hits.hits);
                        this.scroll_id = response.scroll_id;
                        // Return to poll_next to yield the first item
                        self.poll_next(cx)
                    }
                }
                Poll::Ready(Err(e)) => {
                    this.pending_future = None; // Clear the future
                    Poll::Ready(Some(Err(e)))
                }
                Poll::Pending => Poll::Pending,
            }
        } else {
            unreachable!("pending_future should be Some")
        }
    }
}
```

## Implementation Plan

### Phase 1: Core Client (Week 1)
- [ ] Transport layer
- [ ] Connection pooling
- [ ] Basic authentication
- [ ] Error handling

### Phase 2: Document APIs (Week 2)
- [ ] CRUD operations
- [ ] Bulk operations
- [ ] Multi-get
- [ ] Update by query

### Phase 3: Search APIs (Week 3)
- [ ] Query DSL
- [ ] Aggregations
- [ ] Scroll/PIT
- [ ] Highlighting

### Phase 4: Advanced Features (Week 4)
- [ ] Streaming support
- [ ] Async/await
- [ ] Retry policies
- [ ] Circuit breakers

## Testing Strategy

### Unit Tests
- Request building
- Response parsing
- Error handling
- DSL construction

### Integration Tests
- End-to-end operations
- Connection handling
- Bulk operations
- Error recovery

## Performance Considerations

- Connection pooling
- Request pipelining
- Response streaming
- Memory-efficient parsing
- Zero-copy where possible

## Future Enhancements

- GraphQL client
- SQL client
- Reactive streams
- WebSocket support
- gRPC transport
- Client-side caching