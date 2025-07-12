use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::Level;
use crate::transport::TransportClient;
use crate::extension::ExtensionError;
use std::collections::HashMap;

pub type Logger = tracing::Span;

#[derive(Clone)]
pub struct Settings {
    values: Arc<std::sync::RwLock<HashMap<String, SettingValue>>>,
}

#[derive(Clone, Debug)]
pub enum SettingValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<SettingValue>),
    Map(HashMap<String, SettingValue>),
}

impl Settings {
    pub fn new() -> Self {
        Settings {
            values: Arc::new(std::sync::RwLock::new(HashMap::new())),
        }
    }
    
    pub fn set(&self, key: impl Into<String>, value: impl Into<SettingValue>) -> Result<(), ExtensionError> {
        let mut values = self.values.write()
            .map_err(|_| ExtensionError::configuration("Settings lock poisoned"))?;
        values.insert(key.into(), value.into());
        Ok(())
    }
    
    pub fn get(&self, key: &str) -> Result<Option<SettingValue>, ExtensionError> {
        let values = self.values.read()
            .map_err(|_| ExtensionError::configuration("Settings lock poisoned"))?;
        Ok(values.get(key).cloned())
    }
    
    pub fn get_string(&self, key: &str) -> Result<Option<String>, ExtensionError> {
        match self.get(key)? {
            Some(SettingValue::String(s)) => Ok(Some(s)),
            Some(_) => Ok(None),
            None => Ok(None),
        }
    }
    
    pub fn get_integer(&self, key: &str) -> Result<Option<i64>, ExtensionError> {
        match self.get(key)? {
            Some(SettingValue::Integer(i)) => Ok(Some(i)),
            Some(_) => Ok(None),
            None => Ok(None),
        }
    }
    
    pub fn get_float(&self, key: &str) -> Result<Option<f64>, ExtensionError> {
        match self.get(key)? {
            Some(SettingValue::Float(f)) => Ok(Some(f)),
            Some(_) => Ok(None),
            None => Ok(None),
        }
    }
    
    pub fn get_boolean(&self, key: &str) -> Result<Option<bool>, ExtensionError> {
        match self.get(key)? {
            Some(SettingValue::Boolean(b)) => Ok(Some(b)),
            Some(_) => Ok(None),
            None => Ok(None),
        }
    }
    
    pub fn merge(&mut self, other: &Settings) -> Result<(), ExtensionError> {
        let mut values = self.values.write()
            .map_err(|_| ExtensionError::configuration("Settings lock poisoned"))?;
        let other_values = other.values.read()
            .map_err(|_| ExtensionError::configuration("Settings lock poisoned"))?;
        for (key, value) in other_values.iter() {
            values.insert(key.clone(), value.clone());
        }
        Ok(())
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for SettingValue {
    fn from(value: String) -> Self {
        SettingValue::String(value)
    }
}

impl From<&str> for SettingValue {
    fn from(value: &str) -> Self {
        SettingValue::String(value.to_string())
    }
}

impl From<i64> for SettingValue {
    fn from(value: i64) -> Self {
        SettingValue::Integer(value)
    }
}

impl From<i32> for SettingValue {
    fn from(value: i32) -> Self {
        SettingValue::Integer(value as i64)
    }
}

impl From<f64> for SettingValue {
    fn from(value: f64) -> Self {
        SettingValue::Float(value)
    }
}

impl From<f32> for SettingValue {
    fn from(value: f32) -> Self {
        SettingValue::Float(value as f64)
    }
}

impl From<bool> for SettingValue {
    fn from(value: bool) -> Self {
        SettingValue::Boolean(value)
    }
}

pub struct ExtensionContext {
    pub settings: Settings,
    pub transport_client: Arc<TransportClient>,
    pub thread_pool: Arc<Runtime>,
    pub logger: Logger,
}

impl ExtensionContext {
    pub fn new(
        settings: Settings,
        transport_client: Arc<TransportClient>,
        thread_pool: Arc<Runtime>,
    ) -> Self {
        let logger = tracing::span!(Level::INFO, "extension");
        
        ExtensionContext {
            settings,
            transport_client,
            thread_pool,
            logger,
        }
    }
    
    pub fn builder() -> ExtensionContextBuilder {
        ExtensionContextBuilder::new()
    }
}

pub struct ExtensionContextBuilder {
    settings: Settings,
    transport_client: Option<Arc<TransportClient>>,
    thread_pool: Option<Arc<Runtime>>,
}

impl ExtensionContextBuilder {
    pub fn new() -> Self {
        ExtensionContextBuilder {
            settings: Settings::new(),
            transport_client: None,
            thread_pool: None,
        }
    }
    
    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }
    
    pub fn transport_client(mut self, client: Arc<TransportClient>) -> Self {
        self.transport_client = Some(client);
        self
    }
    
    pub fn thread_pool(mut self, pool: Arc<Runtime>) -> Self {
        self.thread_pool = Some(pool);
        self
    }
    
    pub fn build(self) -> Result<ExtensionContext, ExtensionError> {
        let transport_client = self.transport_client
            .ok_or_else(|| ExtensionError::configuration("Transport client is required"))?;
        
        let thread_pool = match self.thread_pool {
            Some(pool) => pool,
            None => {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .map(Arc::new)
                    .map_err(|e| ExtensionError::initialization(
                        format!("Failed to create thread pool: {}", e)
                    ))?
            }
        };
        
        Ok(ExtensionContext::new(
            self.settings,
            transport_client,
            thread_pool,
        ))
    }
}

impl Default for ExtensionContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_settings() {
        let settings = Settings::new();
        
        settings.set("test.string", "value").unwrap();
        settings.set("test.integer", 42).unwrap();
        settings.set("test.float", 3.14159).unwrap();
        settings.set("test.boolean", true).unwrap();
        
        assert_eq!(settings.get_string("test.string").unwrap(), Some("value".to_string()));
        assert_eq!(settings.get_integer("test.integer").unwrap(), Some(42));
        assert_eq!(settings.get_float("test.float").unwrap(), Some(3.14159));
        assert_eq!(settings.get_boolean("test.boolean").unwrap(), Some(true));
    }
    
    #[test]
    fn test_settings_merge() {
        let mut settings1 = Settings::new();
        settings1.set("key1", "value1").unwrap();
        settings1.set("key2", "value2").unwrap();
        
        let settings2 = Settings::new();
        settings2.set("key2", "updated").unwrap();
        settings2.set("key3", "value3").unwrap();
        
        settings1.merge(&settings2).unwrap();
        
        assert_eq!(settings1.get_string("key1").unwrap(), Some("value1".to_string()));
        assert_eq!(settings1.get_string("key2").unwrap(), Some("updated".to_string()));
        assert_eq!(settings1.get_string("key3").unwrap(), Some("value3".to_string()));
    }
}