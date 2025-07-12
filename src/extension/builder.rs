use std::sync::Arc;
use tokio::runtime::Runtime;

use crate::extension::{
    Extension, ExtensionContext, ExtensionError, ExtensionRunner,
    context::Settings,
};
use crate::transport::TransportClient;

pub struct ExtensionBuilder {
    name: String,
    unique_id: String,
    version: String,
    opensearch_version: String,
    settings: Settings,
    port: u16,
    transport_host: String,
    transport_port: u16,
    thread_pool: Option<Arc<Runtime>>,
}

impl ExtensionBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        ExtensionBuilder {
            name: name.into(),
            unique_id: String::new(),
            version: "1.0.0".to_string(),
            opensearch_version: "3.0.0".to_string(),
            settings: Settings::new(),
            port: 1234,
            transport_host: "localhost".to_string(),
            transport_port: 9300,
            thread_pool: None,
        }
    }
    
    pub fn unique_id(mut self, id: impl Into<String>) -> Self {
        self.unique_id = id.into();
        self
    }
    
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }
    
    pub fn opensearch_version(mut self, version: impl Into<String>) -> Self {
        self.opensearch_version = version.into();
        self
    }
    
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }
    
    pub fn transport_endpoint(mut self, host: impl Into<String>, port: u16) -> Self {
        self.transport_host = host.into();
        self.transport_port = port;
        self
    }
    
    pub fn setting<T: Into<crate::extension::context::SettingValue>>(
        mut self,
        key: impl Into<String>,
        value: T,
    ) -> Self {
        self.settings.set(key, value);
        self
    }
    
    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }
    
    pub fn thread_pool(mut self, pool: Arc<Runtime>) -> Self {
        self.thread_pool = Some(pool);
        self
    }
    
    pub fn build<E: Extension>(self, extension: E) -> Result<ExtensionRunner, ExtensionError> {
        if self.unique_id.is_empty() {
            return Err(ExtensionError::configuration("Unique ID is required"));
        }
        
        let provided_name = extension.name();
        if provided_name != self.name {
            return Err(ExtensionError::configuration(
                format!("Extension name mismatch: builder has '{}', extension has '{}'", 
                    self.name, provided_name)
            ));
        }
        
        let provided_id = extension.unique_id();
        if provided_id != self.unique_id {
            return Err(ExtensionError::configuration(
                format!("Extension ID mismatch: builder has '{}', extension has '{}'", 
                    self.unique_id, provided_id)
            ));
        }
        
        let provided_version = extension.version();
        if provided_version != self.version {
            return Err(ExtensionError::configuration(
                format!("Extension version mismatch: builder has '{}', extension has '{}'", 
                    self.version, provided_version)
            ));
        }
        
        let transport_client = Arc::new(
            TransportClient::new(self.transport_host, self.transport_port)
        );
        
        let thread_pool = self.thread_pool.unwrap_or_else(|| {
            Arc::new(
                Runtime::new()
                    .expect("Failed to create runtime")
            )
        });
        
        let context = ExtensionContext::builder()
            .settings(self.settings)
            .transport_client(transport_client)
            .thread_pool(thread_pool)
            .build()
            .map_err(ExtensionError::configuration)?;
        
        ExtensionRunner::new(Box::new(extension), context, self.port)
    }
}

impl Default for ExtensionBuilder {
    fn default() -> Self {
        Self::new("default-extension")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    
    struct TestExtension {
        name: String,
        unique_id: String,
        version: String,
    }
    
    #[async_trait]
    impl Extension for TestExtension {
        fn name(&self) -> &str { &self.name }
        fn unique_id(&self) -> &str { &self.unique_id }
        fn version(&self) -> &str { &self.version }
        fn opensearch_version(&self) -> &str { "3.0.0" }
        
        async fn initialize(&mut self, _context: &ExtensionContext) -> Result<(), ExtensionError> {
            Ok(())
        }
        
        async fn shutdown(&mut self) -> Result<(), ExtensionError> {
            Ok(())
        }
    }
    
    #[test]
    fn test_builder_validation() {
        let extension = TestExtension {
            name: "test".to_string(),
            unique_id: "test-ext".to_string(),
            version: "1.0.0".to_string(),
        };
        
        let result = ExtensionBuilder::new("test")
            .unique_id("test-ext")
            .version("1.0.0")
            .build(extension);
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_builder_validation_fails() {
        let extension = TestExtension {
            name: "test".to_string(),
            unique_id: "test-ext".to_string(),
            version: "1.0.0".to_string(),
        };
        
        let result = ExtensionBuilder::new("wrong-name")
            .unique_id("test-ext")
            .version("1.0.0")
            .build(extension);
        
        assert!(result.is_err());
    }
}