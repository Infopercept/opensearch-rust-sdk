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

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct TestExtension;

    #[async_trait]
    impl Extension for TestExtension {
        fn name(&self) -> &str { "test" }
        fn unique_id(&self) -> &str { "test-id" }
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
    fn test_default_java_version() {
        let ext = TestExtension;
        assert_eq!(ext.java_version(), "11");
    }

    #[test]
    fn test_default_dependencies() {
        let ext = TestExtension;
        assert_eq!(ext.dependencies(), vec![]);
    }
}