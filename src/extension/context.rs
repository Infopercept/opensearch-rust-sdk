use std::sync::Arc;
use tokio::runtime::Runtime;
use tracing::Level;
use crate::transport::TransportClient;
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
    
    pub fn set(&self, key: impl Into<String>, value: impl Into<SettingValue>) {
        let mut values = self.values.write().unwrap();
        values.insert(key.into(), value.into());
    }
    
    pub fn get(&self, key: &str) -> Option<SettingValue> {
        let values = self.values.read().unwrap();
        values.get(key).cloned()
    }
    
    pub fn get_string(&self, key: &str) -> Option<String> {
        match self.get(key)? {
            SettingValue::String(s) => Some(s),
            _ => None,
        }
    }
    
    pub fn get_integer(&self, key: &str) -> Option<i64> {
        match self.get(key)? {
            SettingValue::Integer(i) => Some(i),
            _ => None,
        }
    }
    
    pub fn get_float(&self, key: &str) -> Option<f64> {
        match self.get(key)? {
            SettingValue::Float(f) => Some(f),
            _ => None,
        }
    }
    
    pub fn get_boolean(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            SettingValue::Boolean(b) => Some(b),
            _ => None,
        }
    }
    
    pub fn merge(&mut self, other: Settings) {
        let mut values = self.values.write().unwrap();
        let other_values = other.values.read().unwrap();
        for (key, value) in other_values.iter() {
            values.insert(key.clone(), value.clone());
        }
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
    
    pub fn build(self) -> Result<ExtensionContext, String> {
        let transport_client = self.transport_client
            .ok_or_else(|| "Transport client is required".to_string())?;
        
        let thread_pool = self.thread_pool
            .unwrap_or_else(|| {
                Arc::new(
                    tokio::runtime::Builder::new_multi_thread()
                        .enable_all()
                        .build()
                        .expect("Failed to create thread pool")
                )
            });
        
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
        
        settings.set("test.string", "value");
        settings.set("test.integer", 42);
        settings.set("test.float", 3.14159);
        settings.set("test.boolean", true);
        
        assert_eq!(settings.get_string("test.string"), Some("value".to_string()));
        assert_eq!(settings.get_integer("test.integer"), Some(42));
        assert_eq!(settings.get_float("test.float"), Some(3.14159));
        assert_eq!(settings.get_boolean("test.boolean"), Some(true));
    }
    
    #[test]
    fn test_settings_merge() {
        let mut settings1 = Settings::new();
        settings1.set("key1", "value1");
        settings1.set("key2", "value2");
        
        let settings2 = Settings::new();
        settings2.set("key2", "updated");
        settings2.set("key3", "value3");
        
        settings1.merge(settings2);
        
        assert_eq!(settings1.get_string("key1"), Some("value1".to_string()));
        assert_eq!(settings1.get_string("key2"), Some("updated".to_string()));
        assert_eq!(settings1.get_string("key3"), Some("value3".to_string()));
    }
}