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
    
    #[test]
    fn test_transport_client_creation() {
        let client = TransportClient::new("localhost", 9200)
            .with_timeout(Duration::from_secs(60));
        
        assert_eq!(client.host, "localhost");
        assert_eq!(client.port, 9200);
        assert_eq!(client.timeout, Duration::from_secs(60));
    }
}