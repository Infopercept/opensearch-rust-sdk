use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::time::Duration;
use crate::extension::ExtensionError;

#[derive(Clone)]
pub struct TransportClient {
    host: String,
    port: u16,
    timeout: Duration,
}

impl TransportClient {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        TransportClient {
            host: host.into(),
            port,
            timeout: Duration::from_secs(30),
        }
    }
    
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
    
    pub async fn connect(&self) -> Result<TcpStream, ExtensionError> {
        let addr = format!("{}:{}", self.host, self.port);
        let stream = tokio::time::timeout(
            self.timeout,
            TcpStream::connect(&addr)
        )
        .await
        .map_err(|_| ExtensionError::timeout("Connection timeout"))?
        .map_err(|e| ExtensionError::transport(format!("Failed to connect: {}", e)))?;
        
        Ok(stream)
    }
    
    pub async fn send_request(&self, _action: &str, data: &[u8]) -> Result<Vec<u8>, ExtensionError> {
        let mut stream = self.connect().await?;
        
        stream.write_all(data).await
            .map_err(|e| ExtensionError::transport(format!("Failed to send request: {}", e)))?;
        
        let mut response = Vec::new();
        stream.read_to_end(&mut response).await
            .map_err(|e| ExtensionError::transport(format!("Failed to read response: {}", e)))?;
        
        Ok(response)
    }
}

pub struct TransportConnectionPool {
    client: Arc<TransportClient>,
    connections: Arc<tokio::sync::Mutex<Vec<TcpStream>>>,
    max_connections: usize,
}

impl TransportConnectionPool {
    pub fn new(client: Arc<TransportClient>, max_connections: usize) -> Self {
        TransportConnectionPool {
            client,
            connections: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            max_connections,
        }
    }
    
    pub async fn get_connection(&self) -> Result<TcpStream, ExtensionError> {
        let mut pool = self.connections.lock().await;
        
        if let Some(conn) = pool.pop() {
            Ok(conn)
        } else {
            self.client.connect().await
        }
    }
    
    pub async fn return_connection(&self, conn: TcpStream) {
        let mut pool = self.connections.lock().await;
        
        if pool.len() < self.max_connections {
            pool.push(conn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[test]
    fn test_transport_client_creation() {
        let client = TransportClient::new("localhost", 9200)
            .with_timeout(Duration::from_secs(60));
        
        assert_eq!(client.host, "localhost");
        assert_eq!(client.port, 9200);
        assert_eq!(client.timeout, Duration::from_secs(60));
    }

    #[tokio::test]
    async fn test_connection_failure() {
        let client = TransportClient::new("invalid-host", 9999)
            .with_timeout(Duration::from_millis(100));
        
        let result = client.connect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_send_request_timeout() {
        let client = TransportClient::new("localhost", 9999)
            .with_timeout(Duration::from_millis(100));
        
        let result = timeout(
            Duration::from_millis(200),
            client.send_request("test", &[1, 2, 3])
        ).await;
        
        assert!(result.is_ok()); // Timeout wrapper succeeded
        assert!(result.unwrap().is_err()); // Inner operation failed
    }

    #[tokio::test]
    async fn test_connection_pool() {
        let client = Arc::new(TransportClient::new("localhost", 9999));
        let pool = TransportConnectionPool::new(client, 5);
        
        // Getting a connection should fail but not panic
        let result = pool.get_connection().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_connection_pool_return() {
        let client = Arc::new(TransportClient::new("localhost", 9999));
        let pool = TransportConnectionPool::new(client.clone(), 2);
        
        // Create a mock connection (this will fail in real scenario)
        if let Ok(conn) = client.connect().await {
            pool.return_connection(conn).await;
            
            // Check pool size by attempting to get connection
            let pool_guard = pool.connections.lock().await;
            assert!(pool_guard.len() <= 2);
        }
    }
}