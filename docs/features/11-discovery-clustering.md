# Discovery & Clustering

## Overview

Discovery and Clustering features enable extensions to participate in OpenSearch's distributed architecture, discover nodes and services, maintain cluster state awareness, and coordinate distributed operations across the cluster.

## Goals

- Enable extension discovery by OpenSearch nodes
- Support service discovery for extension-to-extension communication
- Provide cluster state awareness and monitoring
- Enable distributed coordination and leader election
- Support fault tolerance and high availability
- Maintain cluster membership information

## Design

### Discovery Framework

```rust
/// Extension discovery service
pub struct DiscoveryService {
    /// Local extension information
    local_extension: ExtensionInfo,
    /// Discovered extensions
    extensions: Arc<RwLock<HashMap<String, ExtensionInfo>>>,
    /// Cluster nodes
    nodes: Arc<RwLock<HashMap<String, NodeInfo>>>,
    /// Discovery transport
    transport: Arc<TransportClient>,
    /// Event listeners
    listeners: Arc<RwLock<Vec<Box<dyn DiscoveryListener>>>>,
}

impl DiscoveryService {
    /// Start discovery service
    pub async fn start(&mut self) -> Result<(), DiscoveryError> {
        // Register with OpenSearch
        self.register_extension().await?;
        
        // Start heartbeat
        self.start_heartbeat().await?;
        
        // Start discovery polling
        self.start_discovery_task().await?;
        
        Ok(())
    }
    
    /// Register extension with OpenSearch
    async fn register_extension(&self) -> Result<(), DiscoveryError> {
        let request = ExtensionDiscoveryRequest {
            extension_id: self.local_extension.id.clone(),
            name: self.local_extension.name.clone(),
            version: self.local_extension.version.clone(),
            capabilities: self.local_extension.capabilities.clone(),
            endpoints: self.local_extension.endpoints.clone(),
        };
        
        self.transport.send_request(
            "internal:discovery/register_extension",
            request,
        ).await?;
        
        Ok(())
    }
    
    /// Discover other extensions
    pub async fn discover_extensions(&self) -> Result<Vec<ExtensionInfo>, DiscoveryError> {
        let response: ExtensionDiscoveryResponse = self.transport
            .send_request("internal:discovery/extensions", EmptyRequest {})
            .await?;
        
        let extensions = response.extensions;
        
        // Update local cache
        let mut cached = self.extensions.write().await;
        for ext in &extensions {
            cached.insert(ext.id.clone(), ext.clone());
        }
        
        // Notify listeners
        self.notify_extensions_discovered(&extensions).await;
        
        Ok(extensions)
    }
    
    /// Find extension by capability
    pub async fn find_by_capability(
        &self,
        capability: &str,
    ) -> Vec<ExtensionInfo> {
        let extensions = self.extensions.read().await;
        extensions.values()
            .filter(|ext| ext.capabilities.contains(&capability.to_string()))
            .cloned()
            .collect()
    }
}

/// Extension information
#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub host: String,
    pub port: u16,
    pub capabilities: Vec<String>,
    pub endpoints: HashMap<String, EndpointInfo>,
    pub metadata: HashMap<String, String>,
    pub status: ExtensionStatus,
    pub last_seen: DateTime<Utc>,
}

/// Extension status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtensionStatus {
    Starting,
    Healthy,
    Degraded,
    Unhealthy,
    Stopping,
}

/// Discovery event listener
#[async_trait]
pub trait DiscoveryListener: Send + Sync {
    /// Called when new extensions are discovered
    async fn on_extensions_discovered(&self, extensions: &[ExtensionInfo]);
    
    /// Called when extensions are removed
    async fn on_extensions_removed(&self, extension_ids: &[String]);
    
    /// Called when extension status changes
    async fn on_extension_status_changed(
        &self,
        extension_id: &str,
        old_status: ExtensionStatus,
        new_status: ExtensionStatus,
    );
}
```

### Cluster State Awareness

```rust
/// Cluster state service
pub struct ClusterStateService {
    /// Current cluster state
    state: Arc<RwLock<ClusterState>>,
    /// State update listeners
    listeners: Arc<RwLock<Vec<Box<dyn ClusterStateListener>>>>,
    /// Transport client
    transport: Arc<TransportClient>,
}

impl ClusterStateService {
    /// Get current cluster state
    pub async fn state(&self) -> ClusterState {
        self.state.read().await.clone()
    }
    
    /// Subscribe to cluster state updates
    pub async fn subscribe(&self) -> Result<(), ClusterError> {
        let request = ClusterStateSubscriptionRequest {
            subscriber_id: self.local_node_id(),
            interested_in: vec![
                StateComponent::Nodes,
                StateComponent::Metadata,
                StateComponent::RoutingTable,
            ],
        };
        
        self.transport.send_request(
            "internal:cluster/state/subscribe",
            request,
        ).await?;
        
        Ok(())
    }
    
    /// Handle cluster state update
    pub async fn handle_state_update(
        &self,
        update: ClusterStateUpdate,
    ) -> Result<(), ClusterError> {
        let mut state = self.state.write().await;
        let old_version = state.version;
        
        // Apply update
        state.apply_update(update)?;
        
        // Notify listeners
        if state.version > old_version {
            self.notify_state_changed(&state).await;
        }
        
        Ok(())
    }
}

/// Cluster state representation
#[derive(Debug, Clone)]
pub struct ClusterState {
    pub version: u64,
    pub cluster_uuid: String,
    pub nodes: HashMap<String, NodeInfo>,
    pub metadata: ClusterMetadata,
    pub routing_table: RoutingTable,
    pub blocks: ClusterBlocks,
}

/// Node information
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub id: String,
    pub name: String,
    pub address: SocketAddr,
    pub roles: Vec<NodeRole>,
    pub attributes: HashMap<String, String>,
    pub status: NodeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeRole {
    Master,
    Data,
    Ingest,
    Coordinator,
}

/// Cluster state listener
#[async_trait]
pub trait ClusterStateListener: Send + Sync {
    /// Called when cluster state changes
    async fn on_state_changed(
        &self,
        old_state: &ClusterState,
        new_state: &ClusterState,
    );
}
```

### Service Mesh Integration

```rust
/// Service mesh for extension communication
pub struct ExtensionServiceMesh {
    /// Service registry
    registry: Arc<ServiceRegistry>,
    /// Load balancer
    load_balancer: Arc<dyn LoadBalancer>,
    /// Circuit breakers
    circuit_breakers: Arc<CircuitBreakerRegistry>,
    /// Retry policies
    retry_policies: HashMap<String, RetryPolicy>,
}

impl ExtensionServiceMesh {
    /// Call another extension's service
    pub async fn call<Req, Res>(
        &self,
        service: &str,
        method: &str,
        request: Req,
    ) -> Result<Res, ServiceMeshError>
    where
        Req: Serialize + Send,
        Res: DeserializeOwned,
    {
        // Find service endpoints
        let endpoints = self.registry.get_endpoints(service).await?;
        if endpoints.is_empty() {
            return Err(ServiceMeshError::ServiceNotFound(service.to_string()));
        }
        
        // Select endpoint using load balancer
        let endpoint = self.load_balancer.select(&endpoints)?;
        
        // Check circuit breaker
        let breaker = self.circuit_breakers.get(service);
        if !breaker.allow_request() {
            return Err(ServiceMeshError::CircuitOpen(service.to_string()));
        }
        
        // Get retry policy
        let retry_policy = self.retry_policies.get(service)
            .cloned()
            .unwrap_or_default();
        
        // Execute with retries
        let result = retry_policy.execute(|| async {
            self.execute_call(&endpoint, method, &request).await
        }).await;
        
        // Update circuit breaker
        match &result {
            Ok(_) => breaker.record_success(),
            Err(_) => breaker.record_failure(),
        }
        
        result
    }
    
    /// Register a service
    pub async fn register_service(
        &self,
        service: ServiceDefinition,
    ) -> Result<(), ServiceMeshError> {
        self.registry.register(service).await
    }
}

/// Service registry
pub struct ServiceRegistry {
    services: Arc<RwLock<HashMap<String, Vec<ServiceEndpoint>>>>,
    discovery: Arc<DiscoveryService>,
}

/// Service endpoint
#[derive(Debug, Clone)]
pub struct ServiceEndpoint {
    pub extension_id: String,
    pub address: SocketAddr,
    pub metadata: HashMap<String, String>,
    pub health_check_url: Option<String>,
    pub weight: u32,
}

/// Load balancer trait
#[async_trait]
pub trait LoadBalancer: Send + Sync {
    /// Select an endpoint
    fn select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint, ServiceMeshError>;
}

/// Round-robin load balancer
pub struct RoundRobinLoadBalancer {
    counter: AtomicUsize,
}

impl LoadBalancer for RoundRobinLoadBalancer {
    fn select(&self, endpoints: &[ServiceEndpoint]) -> Result<ServiceEndpoint, ServiceMeshError> {
        if endpoints.is_empty() {
            return Err(ServiceMeshError::NoHealthyEndpoints);
        }
        
        let index = self.counter.fetch_add(1, Ordering::Relaxed) % endpoints.len();
        Ok(endpoints[index].clone())
    }
}
```

### Coordination and Leader Election

```rust
/// Distributed coordination service
pub struct CoordinationService {
    /// Leader election
    leader_election: Arc<LeaderElection>,
    /// Distributed locks
    locks: Arc<DistributedLockManager>,
    /// Distributed barriers
    barriers: Arc<DistributedBarrierManager>,
}

/// Leader election
pub struct LeaderElection {
    election_id: String,
    node_id: String,
    transport: Arc<TransportClient>,
    is_leader: AtomicBool,
    listeners: Arc<RwLock<Vec<Box<dyn LeadershipListener>>>>,
}

impl LeaderElection {
    /// Participate in leader election
    pub async fn participate(&self) -> Result<(), CoordinationError> {
        let request = LeaderElectionRequest {
            election_id: self.election_id.clone(),
            candidate_id: self.node_id.clone(),
            priority: 100,
        };
        
        loop {
            let response: LeaderElectionResponse = self.transport
                .send_request("internal:coordination/leader/elect", request.clone())
                .await?;
            
            if response.leader_id == self.node_id {
                self.become_leader().await?;
            } else {
                self.become_follower(&response.leader_id).await?;
            }
            
            // Wait for next election cycle
            tokio::time::sleep(response.next_election_in).await;
        }
    }
    
    /// Check if this node is the leader
    pub fn is_leader(&self) -> bool {
        self.is_leader.load(Ordering::Relaxed)
    }
    
    async fn become_leader(&self) -> Result<(), CoordinationError> {
        self.is_leader.store(true, Ordering::Relaxed);
        self.notify_leadership_acquired().await;
        Ok(())
    }
    
    async fn become_follower(&self, leader_id: &str) -> Result<(), CoordinationError> {
        let was_leader = self.is_leader.swap(false, Ordering::Relaxed);
        if was_leader {
            self.notify_leadership_lost().await;
        }
        Ok(())
    }
}

/// Leadership change listener
#[async_trait]
pub trait LeadershipListener: Send + Sync {
    /// Called when leadership is acquired
    async fn on_leadership_acquired(&self);
    
    /// Called when leadership is lost
    async fn on_leadership_lost(&self);
}

/// Distributed lock
pub struct DistributedLock {
    lock_id: String,
    owner_id: String,
    expiry: DateTime<Utc>,
}

/// Distributed lock manager
pub struct DistributedLockManager {
    locks: Arc<RwLock<HashMap<String, DistributedLock>>>,
    transport: Arc<TransportClient>,
}

impl DistributedLockManager {
    /// Acquire a distributed lock
    pub async fn acquire(
        &self,
        lock_id: &str,
        timeout: Duration,
    ) -> Result<LockGuard, CoordinationError> {
        let request = AcquireLockRequest {
            lock_id: lock_id.to_string(),
            owner_id: self.node_id(),
            timeout,
        };
        
        let response: AcquireLockResponse = self.transport
            .send_request("internal:coordination/lock/acquire", request)
            .await?;
        
        if response.acquired {
            Ok(LockGuard {
                lock_id: lock_id.to_string(),
                manager: Arc::clone(self),
            })
        } else {
            Err(CoordinationError::LockNotAcquired)
        }
    }
}
```

## Implementation Plan

### Phase 1: Basic Discovery (Week 1)
- [ ] Extension registration
- [ ] Service discovery
- [ ] Health checking
- [ ] Event notifications

### Phase 2: Cluster Integration (Week 2)
- [ ] Cluster state monitoring
- [ ] Node discovery
- [ ] Routing awareness
- [ ] Metadata synchronization

### Phase 3: Service Mesh (Week 3)
- [ ] Service registry
- [ ] Load balancing
- [ ] Circuit breakers
- [ ] Retry policies

### Phase 4: Coordination (Week 4)
- [ ] Leader election
- [ ] Distributed locks
- [ ] Consensus protocols
- [ ] Fault tolerance

## Testing Strategy

### Unit Tests
- Discovery logic
- State management
- Load balancing
- Leader election

### Integration Tests
- Multi-node scenarios
- Network partitions
- Node failures
- Recovery testing

## Future Enhancements

- Consul/etcd integration
- Advanced routing policies
- Traffic shaping
- Canary deployments
- Blue-green deployments
- Distributed tracing integration