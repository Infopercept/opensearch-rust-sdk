use async_trait::async_trait;
use crate::extension::{ExtensionContext, ExtensionDependency, ExtensionError};

#[async_trait]
pub trait Extension: Send + Sync + 'static {
    fn name(&self) -> &str;
    
    fn unique_id(&self) -> &str;
    
    fn version(&self) -> &str;
    
    fn opensearch_version(&self) -> &str;
    
    fn java_version(&self) -> &str {
        "11"
    }
    
    fn dependencies(&self) -> Vec<ExtensionDependency> {
        vec![]
    }
    
    async fn initialize(&mut self, context: &ExtensionContext) -> Result<(), ExtensionError>;
    
    async fn shutdown(&mut self) -> Result<(), ExtensionError>;
}