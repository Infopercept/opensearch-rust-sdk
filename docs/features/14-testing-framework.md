# Testing Framework

## Overview

The Testing Framework provides comprehensive tools and utilities for testing OpenSearch extensions. It includes unit testing helpers, integration testing infrastructure, performance benchmarking tools, and test fixtures that make it easy to write reliable tests for extensions.

## Goals

- Provide easy-to-use testing utilities for extension developers
- Support unit, integration, and performance testing
- Enable testing without a full OpenSearch cluster
- Provide mock implementations of OpenSearch services
- Support property-based and fuzz testing
- Enable reproducible test environments

## Design

### Test Extension Framework

```rust
/// Base trait for testable extensions
#[async_trait]
pub trait TestableExtension: Extension {
    /// Setup test environment
    async fn setup_test(&mut self) -> Result<(), TestError> {
        Ok(())
    }
    
    /// Teardown test environment
    async fn teardown_test(&mut self) -> Result<(), TestError> {
        Ok(())
    }
    
    /// Get test configuration
    fn test_config(&self) -> TestConfig {
        TestConfig::default()
    }
}

/// Test configuration
#[derive(Debug, Clone, Default)]
pub struct TestConfig {
    /// Use in-memory transport
    pub use_mock_transport: bool,
    /// Use in-memory storage
    pub use_mock_storage: bool,
    /// Enable debug logging
    pub enable_debug_logging: bool,
    /// Test timeout
    pub timeout: Option<Duration>,
    /// Random seed for reproducibility
    pub seed: Option<u64>,
}

/// Test harness for extensions
pub struct ExtensionTestHarness {
    extension: Box<dyn TestableExtension>,
    mock_cluster: MockCluster,
    test_client: TestClient,
    fixtures: FixtureManager,
}

impl ExtensionTestHarness {
    /// Create a new test harness
    pub async fn new(extension: Box<dyn TestableExtension>) -> Result<Self, TestError> {
        let config = extension.test_config();
        let mock_cluster = MockCluster::new(config.clone()).await?;
        let test_client = TestClient::new(&mock_cluster);
        let fixtures = FixtureManager::new();
        
        Ok(Self {
            extension,
            mock_cluster,
            test_client,
            fixtures,
        })
    }
    
    /// Run a test
    pub async fn run_test<F, Fut>(&mut self, test: F) -> Result<(), TestError>
    where
        F: FnOnce(&mut Self) -> Fut,
        Fut: Future<Output = Result<(), TestError>>,
    {
        // Setup
        self.extension.setup_test().await?;
        self.mock_cluster.start().await?;
        
        // Run test with timeout
        let result = if let Some(timeout) = self.extension.test_config().timeout {
            tokio::time::timeout(timeout, test(self)).await
                .map_err(|_| TestError::Timeout)?
        } else {
            test(self).await
        };
        
        // Teardown
        self.mock_cluster.stop().await?;
        self.extension.teardown_test().await?;
        
        result
    }
}
```

### Mock OpenSearch Cluster

```rust
/// Mock OpenSearch cluster for testing
pub struct MockCluster {
    nodes: Vec<MockNode>,
    state: Arc<RwLock<ClusterState>>,
    transport: MockTransport,
    settings: ClusterSettings,
}

impl MockCluster {
    /// Create a new mock cluster
    pub async fn new(config: TestConfig) -> Result<Self, TestError> {
        let nodes = vec![
            MockNode::new("node-1", vec![NodeRole::Master, NodeRole::Data]),
            MockNode::new("node-2", vec![NodeRole::Data]),
        ];
        
        let state = Arc::new(RwLock::new(ClusterState::default()));
        let transport = MockTransport::new();
        let settings = ClusterSettings::default();
        
        Ok(Self {
            nodes,
            state,
            transport,
            settings,
        })
    }
    
    /// Add a node to the cluster
    pub async fn add_node(&mut self, node: MockNode) -> Result<(), TestError> {
        self.nodes.push(node);
        self.update_cluster_state().await
    }
    
    /// Simulate node failure
    pub async fn fail_node(&mut self, node_id: &str) -> Result<(), TestError> {
        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
            node.status = NodeStatus::Failed;
            self.update_cluster_state().await?;
        }
        Ok(())
    }
    
    /// Get cluster health
    pub async fn health(&self) -> ClusterHealth {
        let active_nodes = self.nodes.iter()
            .filter(|n| n.status == NodeStatus::Active)
            .count();
        
        ClusterHealth {
            status: if active_nodes == self.nodes.len() {
                HealthStatus::Green
            } else if active_nodes > 0 {
                HealthStatus::Yellow
            } else {
                HealthStatus::Red
            },
            number_of_nodes: self.nodes.len(),
            active_nodes,
        }
    }
}

/// Mock node
#[derive(Debug, Clone)]
pub struct MockNode {
    pub id: String,
    pub name: String,
    pub roles: Vec<NodeRole>,
    pub status: NodeStatus,
    pub attributes: HashMap<String, String>,
}

/// Mock transport for testing
pub struct MockTransport {
    handlers: Arc<RwLock<HashMap<String, Box<dyn MockHandler>>>>,
    messages: Arc<Mutex<Vec<TransportMessage>>>,
}

impl MockTransport {
    /// Register a mock handler
    pub async fn register_handler<H: MockHandler + 'static>(
        &self,
        action: &str,
        handler: H,
    ) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(action.to_string(), Box::new(handler));
    }
    
    /// Get recorded messages
    pub async fn get_messages(&self) -> Vec<TransportMessage> {
        self.messages.lock().await.clone()
    }
    
    /// Clear recorded messages
    pub async fn clear_messages(&self) {
        self.messages.lock().await.clear();
    }
}

/// Mock handler trait
#[async_trait]
pub trait MockHandler: Send + Sync {
    async fn handle(
        &self,
        request: &[u8],
    ) -> Result<Vec<u8>, TransportError>;
}
```

### Test Fixtures

```rust
/// Fixture manager for test data
pub struct FixtureManager {
    fixtures: HashMap<String, Box<dyn Fixture>>,
}

impl FixtureManager {
    /// Register a fixture
    pub fn register<F: Fixture + 'static>(&mut self, name: &str, fixture: F) {
        self.fixtures.insert(name.to_string(), Box::new(fixture));
    }
    
    /// Get a fixture
    pub fn get<T: 'static>(&self, name: &str) -> Option<&T> {
        self.fixtures.get(name)
            .and_then(|f| f.as_any().downcast_ref::<T>())
    }
    
    /// Load fixtures from file
    pub async fn load_from_file(&mut self, path: &Path) -> Result<(), TestError> {
        let content = tokio::fs::read_to_string(path).await?;
        let fixtures: HashMap<String, serde_json::Value> = serde_json::from_str(&content)?;
        
        for (name, value) in fixtures {
            self.register(name, JsonFixture(value));
        }
        
        Ok(())
    }
}

/// Base trait for fixtures
pub trait Fixture: Send + Sync {
    fn as_any(&self) -> &dyn Any;
}

/// JSON fixture
struct JsonFixture(serde_json::Value);

impl Fixture for JsonFixture {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Document fixtures
pub struct DocumentFixtures {
    documents: Vec<TestDocument>,
}

impl DocumentFixtures {
    /// Generate random documents
    pub fn generate(count: usize, seed: Option<u64>) -> Self {
        use rand::{Rng, SeedableRng};
        let mut rng = seed
            .map(|s| rand::rngs::StdRng::seed_from_u64(s))
            .unwrap_or_else(|| rand::rngs::StdRng::from_entropy());
        
        let documents = (0..count)
            .map(|i| TestDocument {
                id: format!("doc-{}", i),
                title: format!("Test Document {}", i),
                content: generate_lorem_ipsum(&mut rng, 100),
                timestamp: Utc::now() - Duration::seconds(rng.gen_range(0..86400)),
                tags: generate_tags(&mut rng, 3),
                score: rng.gen_range(0.0..1.0),
            })
            .collect();
        
        Self { documents }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestDocument {
    pub id: String,
    pub title: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub tags: Vec<String>,
    pub score: f64,
}
```

### Test Assertions

```rust
/// Custom assertions for OpenSearch testing
pub trait OpenSearchAssertions {
    /// Assert that a search returns expected results
    async fn assert_search_results<T>(
        &self,
        query: Query,
        expected: &[T],
    ) -> Result<(), TestError>
    where
        T: PartialEq + Debug;
    
    /// Assert index exists
    async fn assert_index_exists(&self, index: &str) -> Result<(), TestError>;
    
    /// Assert document exists
    async fn assert_document_exists(
        &self,
        index: &str,
        id: &str,
    ) -> Result<(), TestError>;
    
    /// Assert cluster health
    async fn assert_cluster_health(
        &self,
        expected: HealthStatus,
    ) -> Result<(), TestError>;
}

impl OpenSearchAssertions for TestClient {
    async fn assert_search_results<T>(
        &self,
        query: Query,
        expected: &[T],
    ) -> Result<(), TestError>
    where
        T: PartialEq + Debug,
    {
        let response = self.search(SearchRequest {
            body: SearchBody {
                query: Some(query),
                size: Some(expected.len()),
                ..Default::default()
            },
            ..Default::default()
        }).await?;
        
        let actual: Vec<T> = response.hits.hits
            .into_iter()
            .map(|hit| hit.source)
            .collect();
        
        assert_eq!(actual, expected, "Search results do not match expected");
        Ok(())
    }
    
    async fn assert_index_exists(&self, index: &str) -> Result<(), TestError> {
        let exists = self.indices().exists(index).await?;
        assert!(exists, "Index {} does not exist", index);
        Ok(())
    }
}

/// Property-based testing support
#[cfg(test)]
mod proptest_support {
    use proptest::prelude::*;
    
    /// Strategy for generating queries
    pub fn query_strategy() -> impl Strategy<Value = Query> {
        prop_oneof![
            Just(Query::MatchAll(MatchAllQuery {})),
            (
                any::<String>(),
                any::<String>()
            ).prop_map(|(field, value)| Query::Term(TermQuery { field, value })),
            (
                vec(query_strategy(), 0..5),
                vec(query_strategy(), 0..5)
            ).prop_map(|(must, should)| Query::Bool(BoolQuery {
                must: Some(must),
                should: Some(should),
                ..Default::default()
            })),
        ]
    }
    
    /// Strategy for generating documents
    pub fn document_strategy() -> impl Strategy<Value = TestDocument> {
        (
            "[a-z]{5,10}",
            "[A-Za-z ]{10,50}",
            "[A-Za-z ]{50,200}",
            0i64..86400,
            vec("[a-z]{3,8}", 0..5),
            0.0f64..1.0,
        ).prop_map(|(id, title, content, timestamp_offset, tags, score)| {
            TestDocument {
                id,
                title,
                content,
                timestamp: Utc::now() - Duration::seconds(timestamp_offset),
                tags,
                score,
            }
        })
    }
}
```

### Performance Testing

```rust
/// Performance test harness
pub struct PerfTestHarness {
    harness: ExtensionTestHarness,
    metrics: Arc<Mutex<PerfMetrics>>,
}

impl PerfTestHarness {
    /// Run a performance test
    pub async fn run_perf_test<F, Fut>(
        &mut self,
        name: &str,
        iterations: usize,
        test: F,
    ) -> Result<PerfResult, TestError>
    where
        F: Fn(&mut Self) -> Fut,
        Fut: Future<Output = Result<(), TestError>>,
    {
        let mut durations = Vec::with_capacity(iterations);
        let mut errors = 0;
        
        // Warmup
        for _ in 0..min(10, iterations / 10) {
            let _ = test(self).await;
        }
        
        // Actual test
        for _ in 0..iterations {
            let start = Instant::now();
            match test(self).await {
                Ok(_) => durations.push(start.elapsed()),
                Err(_) => errors += 1,
            }
        }
        
        // Calculate statistics
        durations.sort();
        let total: Duration = durations.iter().sum();
        let mean = total / iterations as u32;
        let median = durations[durations.len() / 2];
        let p95 = durations[(durations.len() * 95) / 100];
        let p99 = durations[(durations.len() * 99) / 100];
        
        Ok(PerfResult {
            name: name.to_string(),
            iterations,
            errors,
            mean,
            median,
            p95,
            p99,
            min: durations.first().copied().unwrap_or_default(),
            max: durations.last().copied().unwrap_or_default(),
        })
    }
    
    /// Run throughput test
    pub async fn run_throughput_test<F, Fut>(
        &mut self,
        name: &str,
        duration: Duration,
        concurrency: usize,
        test: F,
    ) -> Result<ThroughputResult, TestError>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), TestError>> + Send,
    {
        let start = Instant::now();
        let counter = Arc::new(AtomicUsize::new(0));
        let error_counter = Arc::new(AtomicUsize::new(0));
        
        let mut handles = vec![];
        
        for _ in 0..concurrency {
            let counter = Arc::clone(&counter);
            let error_counter = Arc::clone(&error_counter);
            let test = test.clone();
            
            let handle = tokio::spawn(async move {
                while start.elapsed() < duration {
                    match test().await {
                        Ok(_) => counter.fetch_add(1, Ordering::Relaxed),
                        Err(_) => error_counter.fetch_add(1, Ordering::Relaxed),
                    };
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all tasks
        for handle in handles {
            handle.await?;
        }
        
        let total_ops = counter.load(Ordering::Relaxed);
        let total_errors = error_counter.load(Ordering::Relaxed);
        let elapsed = start.elapsed();
        
        Ok(ThroughputResult {
            name: name.to_string(),
            total_operations: total_ops,
            total_errors,
            duration: elapsed,
            operations_per_second: total_ops as f64 / elapsed.as_secs_f64(),
            concurrency,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PerfResult {
    pub name: String,
    pub iterations: usize,
    pub errors: usize,
    pub mean: Duration,
    pub median: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub min: Duration,
    pub max: Duration,
}

#[derive(Debug, Clone)]
pub struct ThroughputResult {
    pub name: String,
    pub total_operations: usize,
    pub total_errors: usize,
    pub duration: Duration,
    pub operations_per_second: f64,
    pub concurrency: usize,
}
```

## Implementation Plan

### Phase 1: Core Framework (Week 1)
- [ ] Test harness
- [ ] Mock cluster
- [ ] Basic assertions
- [ ] Fixture support

### Phase 2: Mock Services (Week 2)
- [ ] Mock transport
- [ ] Mock storage
- [ ] Mock indices
- [ ] Mock search

### Phase 3: Test Utilities (Week 3)
- [ ] Property testing
- [ ] Fuzz testing
- [ ] Performance testing
- [ ] Snapshot testing

### Phase 4: Integration (Week 4)
- [ ] CI/CD integration
- [ ] Test reporting
- [ ] Coverage tools
- [ ] Documentation

## Usage Example

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use opensearch_sdk_test::*;
    
    #[tokio::test]
    async fn test_search_extension() {
        let extension = MySearchExtension::new();
        let mut harness = ExtensionTestHarness::new(Box::new(extension))
            .await
            .unwrap();
        
        harness.run_test(|h| async {
            // Create test index
            h.test_client.indices().create("test-index").await?;
            
            // Index test documents
            let docs = DocumentFixtures::generate(100, Some(42));
            for doc in &docs.documents {
                h.test_client.index(IndexRequest {
                    index: "test-index".to_string(),
                    document: doc,
                    ..Default::default()
                }).await?;
            }
            
            // Test custom query
            let results = h.test_client.search(SearchRequest {
                indices: Some(vec!["test-index".to_string()]),
                body: SearchBody {
                    query: Some(custom_query()),
                    ..Default::default()
                },
                ..Default::default()
            }).await?;
            
            // Assert results
            assert_eq!(results.hits.total.value, 10);
            Ok(())
        }).await.unwrap();
    }
    
    #[test]
    fn prop_test_query_parsing() {
        proptest!(|(query in query_strategy())| {
            let serialized = serde_json::to_string(&query).unwrap();
            let deserialized: Query = serde_json::from_str(&serialized).unwrap();
            prop_assert_eq!(query, deserialized);
        });
    }
}
```

## Testing Best Practices

1. **Isolation**: Each test should be independent
2. **Reproducibility**: Use seeds for random data
3. **Performance**: Track performance regressions
4. **Coverage**: Aim for high code coverage
5. **Documentation**: Document test scenarios

## Future Enhancements

- Chaos testing support
- Distributed testing
- Load testing framework
- Security testing tools
- Compatibility testing
- Mutation testing