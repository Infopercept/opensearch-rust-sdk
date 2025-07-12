use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{info, error, warn};

use crate::extension::{
    Extension, ExtensionContext, ExtensionError,
    lifecycle::{LifecycleManager, ExtensionState, LoggingStateListener},
};

pub struct ExtensionRunner {
    extension: Arc<RwLock<Box<dyn Extension>>>,
    context: Arc<ExtensionContext>,
    lifecycle: Arc<LifecycleManager>,
    port: u16,
}

impl ExtensionRunner {
    pub fn new(
        extension: Box<dyn Extension>,
        context: ExtensionContext,
        port: u16,
    ) -> Result<Self, ExtensionError> {
        let lifecycle = Arc::new(LifecycleManager::new());
        
        Ok(ExtensionRunner {
            extension: Arc::new(RwLock::new(extension)),
            context: Arc::new(context),
            lifecycle,
            port,
        })
    }
    
    pub async fn run(&mut self) -> Result<(), ExtensionError> {
        self.lifecycle.add_listener(Box::new(LoggingStateListener)).await;
        
        self.lifecycle.transition_to(ExtensionState::Initializing).await?;
        
        {
            let mut ext = self.extension.write().await;
            ext.initialize(&self.context).await?;
        }
        
        self.lifecycle.transition_to(ExtensionState::Initialized).await?;
        
        self.register_with_opensearch().await?;
        
        self.lifecycle.transition_to(ExtensionState::Running).await?;
        
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port))
            .await
            .map_err(|e| ExtensionError::initialization(
                format!("Failed to bind to port {}: {}", self.port, e)
            ))?;
        
        info!("Extension listening on port {}", self.port);
        
        let shutdown_signal = Self::create_shutdown_signal();
        let server_loop = self.run_server(listener);
        
        tokio::select! {
            result = server_loop => {
                if let Err(e) = result {
                    error!("Server error: {}", e);
                    self.lifecycle.transition_to(ExtensionState::Failed).await?;
                }
            }
            _ = shutdown_signal => {
                info!("Shutdown signal received");
            }
        }
        
        self.shutdown().await
    }
    
    async fn run_server(&self, listener: TcpListener) -> Result<(), ExtensionError> {
        loop {
            if !self.lifecycle.is_running().await {
                break;
            }
            
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from {}", addr);
                    
                    let extension = self.extension.clone();
                    let context = self.context.clone();
                    
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, extension, context).await {
                            error!("Error handling connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        
        Ok(())
    }
    
    async fn handle_connection(
        mut stream: tokio::net::TcpStream,
        _extension: Arc<RwLock<Box<dyn Extension>>>,
        _context: Arc<ExtensionContext>,
    ) -> Result<(), ExtensionError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        
        let mut buffer = vec![0u8; 1024];
        let n = stream.read(&mut buffer).await
            .map_err(|e| ExtensionError::transport(format!("Failed to read from stream: {}", e)))?;
        
        if n == 0 {
            return Ok(());
        }
        
        let response = b"Hello from extension";
        stream.write_all(response).await
            .map_err(|e| ExtensionError::transport(format!("Failed to write response: {}", e)))?;
        
        Ok(())
    }
    
    async fn register_with_opensearch(&self) -> Result<(), ExtensionError> {
        use crate::extension::registration::{ExtensionIdentity, ExtensionRegistration, RegistrationProtocol};
        
        let ext = self.extension.read().await;
        
        info!(
            "Registering extension '{}' (ID: {}, version: {}) with OpenSearch",
            ext.name(),
            ext.unique_id(),
            ext.version()
        );
        
        let identity = ExtensionIdentity::from_extension(&**ext);
        let registration = ExtensionRegistration::new(
            identity,
            "0.0.0.0".to_string(),
            self.port,
        );
        
        let protocol = RegistrationProtocol::new(registration);
        
        match protocol.register_with_opensearch("localhost").await {
            Ok(response) => {
                if response.success {
                    info!("Successfully registered with OpenSearch cluster: {:?}", response.cluster_name);
                } else {
                    warn!("Registration failed: {:?}", response.message);
                }
            }
            Err(e) => {
                warn!("Failed to register with OpenSearch: {}", e);
            }
        }
        
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), ExtensionError> {
        info!("Shutting down extension");
        
        self.lifecycle.transition_to(ExtensionState::Stopping).await?;
        
        {
            let mut ext = self.extension.write().await;
            ext.shutdown().await?;
        }
        
        self.lifecycle.transition_to(ExtensionState::Stopped).await?;
        
        info!("Extension shutdown complete");
        Ok(())
    }
    
    async fn create_shutdown_signal() {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };
        
        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install signal handler")
                .recv()
                .await;
        };
        
        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();
        
        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }
}

pub struct ExtensionHandle {
    lifecycle: Arc<LifecycleManager>,
}

impl ExtensionHandle {
    pub fn new(lifecycle: Arc<LifecycleManager>) -> Self {
        ExtensionHandle { lifecycle }
    }
    
    pub async fn state(&self) -> ExtensionState {
        self.lifecycle.current_state().await
    }
    
    pub async fn is_running(&self) -> bool {
        self.lifecycle.is_running().await
    }
    
    pub async fn shutdown(&self) -> Result<(), ExtensionError> {
        if self.lifecycle.is_terminal().await {
            return Ok(());
        }
        
        self.lifecycle.transition_to(ExtensionState::Stopping).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extension::ExtensionContext;
    use crate::transport::TransportClient;
    
    struct TestExtension;
    
    #[async_trait::async_trait]
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
    fn test_extension_runner_creation() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let extension = Box::new(TestExtension);
        let transport_client = Arc::new(TransportClient::new("localhost", 9200));
        let context = ExtensionContext::builder()
            .transport_client(transport_client)
            .thread_pool(Arc::new(runtime))
            .build()
            .unwrap();
        
        let runner = ExtensionRunner::new(extension, context, 1234);
        assert!(runner.is_ok());
    }
}