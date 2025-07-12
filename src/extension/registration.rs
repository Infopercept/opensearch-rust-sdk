use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use crate::extension::{Extension, ExtensionDependency, ExtensionError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionIdentity {
    pub name: String,
    pub unique_id: String,
    pub version: String,
    pub opensearch_version: String,
    pub java_version: String,
    pub description: Option<String>,
    pub vendor: Option<String>,
    pub license: Option<String>,
    pub dependencies: Vec<ExtensionDependency>,
}

impl ExtensionIdentity {
    pub fn from_extension<E: Extension + ?Sized>(extension: &E) -> Self {
        ExtensionIdentity {
            name: extension.name().to_string(),
            unique_id: extension.unique_id().to_string(),
            version: extension.version().to_string(),
            opensearch_version: extension.opensearch_version().to_string(),
            java_version: extension.java_version().to_string(),
            description: None,
            vendor: None,
            license: None,
            dependencies: extension.dependencies(),
        }
    }
    
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
    
    pub fn with_vendor(mut self, vendor: impl Into<String>) -> Self {
        self.vendor = Some(vendor.into());
        self
    }
    
    pub fn with_license(mut self, license: impl Into<String>) -> Self {
        self.license = Some(license.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRegistration {
    pub identity: ExtensionIdentity,
    pub host: String,
    pub port: u16,
    pub capabilities: ExtensionCapabilities,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionCapabilities {
    pub supports_rest_actions: bool,
    pub supports_named_writeable: bool,
    pub supports_action_extension: bool,
    pub supports_settings_extension: bool,
    pub supports_cluster_settings: bool,
}

impl ExtensionRegistration {
    pub fn new(identity: ExtensionIdentity, host: String, port: u16) -> Self {
        ExtensionRegistration {
            identity,
            host,
            port,
            capabilities: ExtensionCapabilities::default(),
        }
    }
    
    pub fn with_capabilities(mut self, capabilities: ExtensionCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }
    
    pub fn socket_address(&self) -> Result<SocketAddr, ExtensionError> {
        let addr_str = format!("{}:{}", self.host, self.port);
        addr_str.parse()
            .map_err(|e| ExtensionError::configuration(
                format!("Invalid socket address {}: {}", addr_str, e)
            ))
    }
}

pub struct RegistrationProtocol {
    registration: ExtensionRegistration,
}

impl RegistrationProtocol {
    pub fn new(registration: ExtensionRegistration) -> Self {
        RegistrationProtocol { registration }
    }
    
    pub async fn register_with_opensearch(
        &self,
        opensearch_addr: &str,
    ) -> Result<RegistrationResponse, ExtensionError> {
        use crate::transport::TransportClient;
        
        let client = TransportClient::new(opensearch_addr, 9300);
        
        let registration_bytes = self.serialize_registration()?;
        
        let response_bytes = client
            .send_request("internal:discovery/register", &registration_bytes)
            .await?;
        
        self.deserialize_response(&response_bytes)
    }
    
    fn serialize_registration(&self) -> Result<Vec<u8>, ExtensionError> {
        serde_json::to_vec(&self.registration)
            .map_err(|e| ExtensionError::serialization(
                format!("Failed to serialize registration: {}", e)
            ))
    }
    
    fn deserialize_response(&self, bytes: &[u8]) -> Result<RegistrationResponse, ExtensionError> {
        serde_json::from_slice(bytes)
            .map_err(|e| ExtensionError::serialization(
                format!("Failed to deserialize response: {}", e)
            ))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrationResponse {
    pub success: bool,
    pub extension_id: Option<String>,
    pub message: Option<String>,
    pub cluster_name: Option<String>,
    pub cluster_uuid: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::extension::{Extension, ExtensionContext};
    
    struct TestExtension;
    
    #[async_trait]
    impl Extension for TestExtension {
        fn name(&self) -> &str { "test" }
        fn unique_id(&self) -> &str { "test-ext" }
        fn version(&self) -> &str { "1.0.0" }
        fn opensearch_version(&self) -> &str { "3.0.0" }
        
        async fn initialize(&mut self, _context: &ExtensionContext) -> Result<(), ExtensionError> {
            Ok(())
        }
        
        async fn shutdown(&mut self) -> Result<(), ExtensionError> {
            Ok(())
        }
    }
    
    #[test]
    fn test_extension_identity_creation() {
        let extension = TestExtension;
        let identity = ExtensionIdentity::from_extension(&extension)
            .with_description("Test extension")
            .with_vendor("Test Inc")
            .with_license("MIT");
        
        assert_eq!(identity.name, "test");
        assert_eq!(identity.unique_id, "test-ext");
        assert_eq!(identity.version, "1.0.0");
        assert_eq!(identity.description, Some("Test extension".to_string()));
        assert_eq!(identity.vendor, Some("Test Inc".to_string()));
        assert_eq!(identity.license, Some("MIT".to_string()));
    }
    
    #[test]
    fn test_registration_socket_address() {
        let identity = ExtensionIdentity::from_extension(&TestExtension);
        let registration = ExtensionRegistration::new(identity, "127.0.0.1".to_string(), 1234);
        
        let addr = registration.socket_address().unwrap();
        assert_eq!(addr.to_string(), "127.0.0.1:1234");
    }
}