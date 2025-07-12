use std::fmt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtensionError {
    #[error("Initialization failed: {0}")]
    InitializationError(String),
    
    #[error("Transport error: {0}")]
    TransportError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Registration failed: {0}")]
    RegistrationError(String),
    
    #[error("Dependency error: {0}")]
    DependencyError(String),
    
    #[error("Shutdown error: {0}")]
    ShutdownError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    
    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl ExtensionError {
    pub fn initialization<S: Into<String>>(msg: S) -> Self {
        ExtensionError::InitializationError(msg.into())
    }
    
    pub fn transport<S: Into<String>>(msg: S) -> Self {
        ExtensionError::TransportError(msg.into())
    }
    
    pub fn configuration<S: Into<String>>(msg: S) -> Self {
        ExtensionError::ConfigurationError(msg.into())
    }
    
    pub fn registration<S: Into<String>>(msg: S) -> Self {
        ExtensionError::RegistrationError(msg.into())
    }
    
    pub fn dependency<S: Into<String>>(msg: S) -> Self {
        ExtensionError::DependencyError(msg.into())
    }
    
    pub fn shutdown<S: Into<String>>(msg: S) -> Self {
        ExtensionError::ShutdownError(msg.into())
    }
    
    pub fn serialization<S: Into<String>>(msg: S) -> Self {
        ExtensionError::SerializationError(msg.into())
    }
    
    pub fn protocol<S: Into<String>>(msg: S) -> Self {
        ExtensionError::ProtocolError(msg.into())
    }
    
    pub fn timeout<S: Into<String>>(msg: S) -> Self {
        ExtensionError::TimeoutError(msg.into())
    }
    
    pub fn unknown<S: Into<String>>(msg: S) -> Self {
        ExtensionError::Unknown(msg.into())
    }
}

#[derive(Debug)]
pub struct ExtensionResult<T> {
    pub value: Option<T>,
    pub error: Option<ExtensionError>,
}

impl<T> ExtensionResult<T> {
    pub fn ok(value: T) -> Self {
        ExtensionResult {
            value: Some(value),
            error: None,
        }
    }
    
    pub fn err(error: ExtensionError) -> Self {
        ExtensionResult {
            value: None,
            error: Some(error),
        }
    }
    
    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }
    
    pub fn is_err(&self) -> bool {
        self.error.is_some()
    }
}

impl<T> fmt::Display for ExtensionResult<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.error {
            Some(e) => write!(f, "Error: {}", e),
            None => write!(f, "Success"),
        }
    }
}