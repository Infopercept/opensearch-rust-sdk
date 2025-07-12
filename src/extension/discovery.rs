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
    
    pub async fn discover_extensions(&self) -> Result<Vec<DiscoveredExtension>, ExtensionError> {
        use crate::transport::TransportClient;
        
        let client = TransportClient::new(&self.service_url, 9300);
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
}