use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use crate::extension::{ExtensionError, registration::ExtensionRegistration};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredExtension {
    pub registration: ExtensionRegistration,
    pub status: ExtensionStatus,
    pub last_seen: std::time::SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtensionStatus {
    Active,
    Inactive,
    Failed,
    Unknown,
}

pub struct DiscoveryService {
    extensions: Arc<RwLock<HashMap<String, DiscoveredExtension>>>,
    discovery_interval: std::time::Duration,
}

impl DiscoveryService {
    pub fn new(discovery_interval: std::time::Duration) -> Self {
        DiscoveryService {
            extensions: Arc::new(RwLock::new(HashMap::new())),
            discovery_interval,
        }
    }
    
    pub async fn register_extension(
        &self,
        registration: ExtensionRegistration,
    ) -> Result<(), ExtensionError> {
        let discovered = DiscoveredExtension {
            registration: registration.clone(),
            status: ExtensionStatus::Active,
            last_seen: std::time::SystemTime::now(),
        };
        
        let mut extensions = self.extensions.write().await;
        extensions.insert(registration.identity.unique_id.clone(), discovered);
        
        Ok(())
    }
    
    pub async fn unregister_extension(&self, unique_id: &str) -> Result<(), ExtensionError> {
        let mut extensions = self.extensions.write().await;
        extensions.remove(unique_id)
            .ok_or_else(|| ExtensionError::unknown(
                format!("Extension {} not found", unique_id)
            ))?;
        
        Ok(())
    }
    
    pub async fn get_extension(&self, unique_id: &str) -> Option<DiscoveredExtension> {
        let extensions = self.extensions.read().await;
        extensions.get(unique_id).cloned()
    }
    
    pub async fn list_extensions(&self) -> Vec<DiscoveredExtension> {
        let extensions = self.extensions.read().await;
        extensions.values().cloned().collect()
    }
    
    pub async fn list_active_extensions(&self) -> Vec<DiscoveredExtension> {
        let extensions = self.extensions.read().await;
        extensions
            .values()
            .filter(|ext| ext.status == ExtensionStatus::Active)
            .cloned()
            .collect()
    }
    
    pub async fn update_extension_status(
        &self,
        unique_id: &str,
        status: ExtensionStatus,
    ) -> Result<(), ExtensionError> {
        let mut extensions = self.extensions.write().await;
        let extension = extensions.get_mut(unique_id)
            .ok_or_else(|| ExtensionError::unknown(
                format!("Extension {} not found", unique_id)
            ))?;
        
        extension.status = status;
        extension.last_seen = std::time::SystemTime::now();
        
        Ok(())
    }
    
    pub async fn heartbeat(&self, unique_id: &str) -> Result<(), ExtensionError> {
        let mut extensions = self.extensions.write().await;
        let extension = extensions.get_mut(unique_id)
            .ok_or_else(|| ExtensionError::unknown(
                format!("Extension {} not found", unique_id)
            ))?;
        
        extension.last_seen = std::time::SystemTime::now();
        
        Ok(())
    }
    
    pub async fn check_stale_extensions(&self) -> Vec<String> {
        let mut stale_extensions = Vec::new();
        let mut extensions = self.extensions.write().await;
        let now = std::time::SystemTime::now();
        
        for (id, extension) in extensions.iter_mut() {
            if let Ok(elapsed) = now.duration_since(extension.last_seen) {
                if elapsed > self.discovery_interval * 3 {
                    extension.status = ExtensionStatus::Inactive;
                    stale_extensions.push(id.clone());
                }
            }
        }
        
        stale_extensions
    }
}

#[derive(Clone)]
pub struct DiscoveryClient {
    service_url: String,
}

impl DiscoveryClient {
    pub fn new(service_url: impl Into<String>) -> Self {
        DiscoveryClient {
            service_url: service_url.into(),
        }
    }
    
    fn parse_host_port(&self, url: &str) -> Result<(String, u16), ExtensionError> {
        // Simple host:port parsing
        if let Some(colon_pos) = url.rfind(':') {
            let host = &url[..colon_pos];
            let port_str = &url[colon_pos + 1..];
            let port = port_str.parse::<u16>()
                .map_err(|_| ExtensionError::configuration(
                    format!("Invalid port in service URL: {}", port_str)
                ))?;
            Ok((host.to_string(), port))
        } else {
            // Default to port 9300 if not specified
            Ok((url.to_string(), 9300))
        }
    }
    
    pub async fn discover_extensions(&self) -> Result<Vec<DiscoveredExtension>, ExtensionError> {
        use crate::transport::TransportClient;
        
        // Parse host and port from service_url
        let (host, port) = self.parse_host_port(&self.service_url)?;
        
        let client = TransportClient::new(host, port);
        let response = client
            .send_request("internal:discovery/list", &[])
            .await?;
        
        serde_json::from_slice(&response)
            .map_err(|e| ExtensionError::serialization(
                format!("Failed to deserialize discovery response: {}", e)
            ))
    }
    
    pub async fn query_extension(
        &self,
        unique_id: &str,
    ) -> Result<Option<DiscoveredExtension>, ExtensionError> {
        // First, try the optimized direct query endpoint
        match self.query_extension_direct(unique_id).await {
            Ok(result) => Ok(result),
            Err(e) => {
                // If the direct query fails (e.g., endpoint not available),
                // fall back to the less efficient list-and-filter approach
                tracing::debug!(
                    "Direct query failed ({}), falling back to list-and-filter", 
                    e
                );
                self.query_extension_fallback(unique_id).await
            }
        }
    }
    
    /// Direct query for a single extension - more efficient
    async fn query_extension_direct(
        &self,
        unique_id: &str,
    ) -> Result<Option<DiscoveredExtension>, ExtensionError> {
        use crate::transport::TransportClient;
        
        // Parse host and port from service_url
        let (host, port) = self.parse_host_port(&self.service_url)?;
        
        let client = TransportClient::new(host, port);
        
        // Create query request for specific extension
        let query_request = serde_json::json!({
            "unique_id": unique_id
        });
        
        let request_bytes = serde_json::to_vec(&query_request)
            .map_err(|e| ExtensionError::serialization(
                format!("Failed to serialize query request: {}", e)
            ))?;
        
        // Use targeted query endpoint
        let response = client
            .send_request("internal:discovery/query", &request_bytes)
            .await?;
        
        // Handle empty response as None
        if response.is_empty() {
            return Ok(None);
        }
        
        // Try to deserialize as a single extension
        serde_json::from_slice::<DiscoveredExtension>(&response)
            .map(Some)
            .or_else(|_| {
                // If that fails, try deserializing as an error response
                if let Ok(error_response) = serde_json::from_slice::<serde_json::Value>(&response) {
                    if error_response.get("found").and_then(|v| v.as_bool()) == Some(false) {
                        Ok(None)
                    } else {
                        Err(ExtensionError::serialization(
                            format!("Unexpected response format: {:?}", error_response)
                        ))
                    }
                } else {
                    Err(ExtensionError::serialization(
                        "Failed to deserialize query response"
                    ))
                }
            })
    }
    
    /// Fallback implementation that fetches all extensions and filters
    async fn query_extension_fallback(
        &self,
        unique_id: &str,
    ) -> Result<Option<DiscoveredExtension>, ExtensionError> {
        let extensions = self.discover_extensions().await?;
        Ok(extensions.into_iter().find(|ext| ext.registration.identity.unique_id == unique_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::registration::ExtensionIdentity;
    
    #[tokio::test]
    async fn test_discovery_service() {
        let service = DiscoveryService::new(std::time::Duration::from_secs(30));
        
        let identity = ExtensionIdentity {
            name: "test".to_string(),
            unique_id: "test-ext".to_string(),
            version: "1.0.0".to_string(),
            opensearch_version: "3.0.0".to_string(),
            java_version: "11".to_string(),
            description: None,
            vendor: None,
            license: None,
            dependencies: vec![],
        };
        
        let registration = ExtensionRegistration::new(
            identity,
            "127.0.0.1".to_string(),
            1234,
        );
        
        service.register_extension(registration).await.unwrap();
        
        let extensions = service.list_extensions().await;
        assert_eq!(extensions.len(), 1);
        assert_eq!(extensions[0].registration.identity.unique_id, "test-ext");
        
        let active = service.list_active_extensions().await;
        assert_eq!(active.len(), 1);
        
        service.update_extension_status("test-ext", ExtensionStatus::Inactive).await.unwrap();
        
        let active_after = service.list_active_extensions().await;
        assert_eq!(active_after.len(), 0);
        
        service.unregister_extension("test-ext").await.unwrap();
        
        let extensions_after = service.list_extensions().await;
        assert_eq!(extensions_after.len(), 0);
    }
    
    #[test]
    fn test_parse_host_port() {
        let client = DiscoveryClient::new("localhost:9300");
        
        let (host, port) = client.parse_host_port("localhost:9300").unwrap();
        assert_eq!(host, "localhost");
        assert_eq!(port, 9300);
        
        let (host, port) = client.parse_host_port("example.com").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 9300); // Default port
        
        let (host, port) = client.parse_host_port("192.168.1.1:8080").unwrap();
        assert_eq!(host, "192.168.1.1");
        assert_eq!(port, 8080);
        
        // Test invalid port
        let result = client.parse_host_port("localhost:invalid");
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_query_extension_direct() {
        let client = DiscoveryClient::new("localhost:9300");
        
        // This will fail since we don't have a real server, but it tests the logic
        let result = client.query_extension_direct("test-ext").await;
        assert!(result.is_err()); // Expected to fail without a server
    }
}