use async_trait::async_trait;
use opensearch_sdk_rs::extension::{
    Extension, ExtensionBuilder, ExtensionContext, ExtensionError, ExtensionDependency,
};
use tracing::info;
use tracing_subscriber;

struct HelloExtension {
    message_count: u64,
}

impl HelloExtension {
    fn new() -> Self {
        HelloExtension {
            message_count: 0,
        }
    }
}

#[async_trait]
impl Extension for HelloExtension {
    fn name(&self) -> &str {
        "Hello Extension"
    }
    
    fn unique_id(&self) -> &str {
        "hello-extension"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn opensearch_version(&self) -> &str {
        "3.0.0"
    }
    
    fn dependencies(&self) -> Vec<ExtensionDependency> {
        vec![]
    }
    
    async fn initialize(&mut self, context: &ExtensionContext) -> Result<(), ExtensionError> {
        info!("Initializing Hello Extension");
        
        if let Ok(Some(greeting)) = context.settings.get_string("hello.greeting") {
            info!("Custom greeting: {}", greeting);
        }
        
        info!("Extension initialized successfully");
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), ExtensionError> {
        info!("Shutting down Hello Extension");
        info!("Total messages processed: {}", self.message_count);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();
    
    info!("Starting Hello Extension example");
    
    let extension = HelloExtension::new();
    
    // Note: In production, you can load extension configuration from hello.json
    // using the `just register-extension` command which reads examples/hello/hello.json
    // and registers the extension with OpenSearch.
    //
    // The hello.json file contains:
    // {
    //     "name": "Hello World",
    //     "uniqueId": "hello-world-rs",
    //     "hostAddress": "127.0.0.1",
    //     "port": "1234",
    //     "version": "0.1.0",
    //     "opensearchVersion": "3.0.0",
    //     "minimumCompatibleVersion": "3.0.0"
    // }
    //
    // For now, we're building the extension programmatically:
    let mut runner = ExtensionBuilder::new("Hello Extension")
        .unique_id("hello-world-rs")  // Match the uniqueId in hello.json
        .version("0.1.0")             // Match the version in hello.json
        .port(1234)                   // Match the port in hello.json
        .transport_endpoint("localhost", 9300)
        .setting("hello.greeting", "Hello from Rust!")
        .setting("hello.max_messages", 1000i64)
        .build(extension)?;
    
    info!("Extension runner created, starting...");
    
    runner.run().await?;
    
    Ok(())
}