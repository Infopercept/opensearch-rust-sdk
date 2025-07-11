# Settings Management

## Overview

The Settings Management system provides type-safe configuration for extensions with validation, hot-reloading, and integration with OpenSearch's cluster settings. It supports various data types, custom validators, and hierarchical configuration with proper defaults and overrides.

## Goals

- Type-safe settings with compile-time guarantees
- Dynamic settings updates without restart
- Integration with OpenSearch cluster settings
- Validation with helpful error messages
- Support for all OpenSearch setting types
- Configuration file and environment variable support

## Design

### Core Setting Types

```rust
/// Base trait for all settings
pub trait Setting: Send + Sync + 'static {
    /// The value type this setting holds
    type Value: Clone + Send + Sync;
    
    /// Setting key (e.g., "my_extension.cache.size")
    fn key(&self) -> &str;
    
    /// Default value
    fn default(&self) -> Self::Value;
    
    /// Parse from string representation
    fn parse(&self, input: &str) -> Result<Self::Value, SettingError>;
    
    /// Validate the parsed value
    fn validate(&self, value: &Self::Value) -> Result<(), SettingError> {
        Ok(())
    }
    
    /// Setting metadata
    fn metadata(&self) -> SettingMetadata {
        SettingMetadata::default()
    }
}

/// Setting metadata
#[derive(Debug, Clone, Default)]
pub struct SettingMetadata {
    /// Human-readable description
    pub description: Option<String>,
    /// Whether this setting can be updated dynamically
    pub dynamic: bool,
    /// Deprecated message if applicable
    pub deprecated: Option<String>,
    /// Setting scope (node-level or index-level)
    pub scope: SettingScope,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SettingScope {
    Node,
    Index,
}
```

### Built-in Setting Types

```rust
/// Boolean setting
pub struct BoolSetting {
    key: String,
    default: bool,
    metadata: SettingMetadata,
}

impl Setting for BoolSetting {
    type Value = bool;
    
    fn key(&self) -> &str {
        &self.key
    }
    
    fn default(&self) -> bool {
        self.default
    }
    
    fn parse(&self, input: &str) -> Result<bool, SettingError> {
        match input.to_lowercase().as_str() {
            "true" | "yes" | "on" | "1" => Ok(true),
            "false" | "no" | "off" | "0" => Ok(false),
            _ => Err(SettingError::ParseError(
                format!("Invalid boolean value: {}", input)
            )),
        }
    }
}

/// Integer setting with bounds
pub struct IntegerSetting {
    key: String,
    default: i64,
    min: Option<i64>,
    max: Option<i64>,
    metadata: SettingMetadata,
}

impl Setting for IntegerSetting {
    type Value = i64;
    
    fn validate(&self, value: &i64) -> Result<(), SettingError> {
        if let Some(min) = self.min {
            if *value < min {
                return Err(SettingError::ValidationError(
                    format!("Value {} is below minimum {}", value, min)
                ));
            }
        }
        if let Some(max) = self.max {
            if *value > max {
                return Err(SettingError::ValidationError(
                    format!("Value {} is above maximum {}", value, max)
                ));
            }
        }
        Ok(())
    }
}

/// String setting with regex validation
pub struct StringSetting {
    key: String,
    default: String,
    pattern: Option<Regex>,
    metadata: SettingMetadata,
}

/// Duration setting (e.g., "10s", "5m", "1h")
pub struct DurationSetting {
    key: String,
    default: Duration,
    min: Option<Duration>,
    max: Option<Duration>,
    metadata: SettingMetadata,
}

impl Setting for DurationSetting {
    type Value = Duration;
    
    fn parse(&self, input: &str) -> Result<Duration, SettingError> {
        parse_duration(input)
            .ok_or_else(|| SettingError::ParseError(
                format!("Invalid duration format: {}", input)
            ))
    }
}

/// Byte size setting (e.g., "10kb", "5mb", "1gb")
pub struct ByteSizeSetting {
    key: String,
    default: ByteSize,
    min: Option<ByteSize>,
    max: Option<ByteSize>,
    metadata: SettingMetadata,
}

/// List setting
pub struct ListSetting<T: Setting> {
    key: String,
    element_setting: T,
    default: Vec<T::Value>,
    metadata: SettingMetadata,
}

/// Enum setting
pub struct EnumSetting<E: EnumType> {
    key: String,
    default: E,
    metadata: SettingMetadata,
}
```

### Settings Container

```rust
/// Container for managing all settings
pub struct Settings {
    values: Arc<RwLock<HashMap<String, SettingValue>>>,
    definitions: HashMap<String, Box<dyn AnySetting>>,
    listeners: Arc<RwLock<Vec<Box<dyn SettingListener>>>>,
}

impl Settings {
    /// Register a setting
    pub fn register<S: Setting>(&mut self, setting: S) -> SettingHandle<S::Value> {
        let key = setting.key().to_string();
        let default = setting.default();
        
        self.definitions.insert(
            key.clone(),
            Box::new(setting),
        );
        
        self.values.write().unwrap().insert(
            key.clone(),
            SettingValue::new(default),
        );
        
        SettingHandle {
            key,
            settings: Arc::downgrade(&self.values),
            _phantom: PhantomData,
        }
    }
    
    /// Get current value of a setting
    pub fn get<S: Setting>(&self, setting: &S) -> S::Value {
        self.values.read().unwrap()
            .get(setting.key())
            .and_then(|v| v.as_type::<S::Value>())
            .unwrap_or_else(|| setting.default())
    }
    
    /// Update a setting value
    pub fn set<S: Setting>(
        &self,
        setting: &S,
        value: S::Value,
    ) -> Result<(), SettingError> {
        setting.validate(&value)?;
        
        let key = setting.key();
        let old_value = self.get(setting);
        
        self.values.write().unwrap()
            .insert(key.to_string(), SettingValue::new(value.clone()));
        
        // Notify listeners
        self.notify_listeners(key, old_value, value);
        
        Ok(())
    }
    
    /// Load settings from various sources
    pub fn load(&mut self) -> Result<(), SettingError> {
        // 1. Load from default values
        // 2. Load from configuration file
        // 3. Load from environment variables
        // 4. Load from OpenSearch cluster settings
        
        Ok(())
    }
}

/// Handle for efficient setting access
pub struct SettingHandle<T> {
    key: String,
    settings: Weak<RwLock<HashMap<String, SettingValue>>>,
    _phantom: PhantomData<T>,
}

impl<T: Clone> SettingHandle<T> {
    /// Get current value
    pub fn get(&self) -> Option<T> {
        self.settings.upgrade()
            .and_then(|settings| {
                settings.read().unwrap()
                    .get(&self.key)
                    .and_then(|v| v.as_type::<T>())
            })
    }
}
```

### Setting Builders

```rust
/// Fluent API for building settings
pub struct SettingBuilder<S: Setting> {
    setting: S,
}

impl SettingBuilder<BoolSetting> {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            setting: BoolSetting {
                key: key.into(),
                default: false,
                metadata: SettingMetadata::default(),
            },
        }
    }
    
    pub fn default(mut self, value: bool) -> Self {
        self.setting.default = value;
        self
    }
    
    pub fn dynamic(mut self) -> Self {
        self.setting.metadata.dynamic = true;
        self
    }
    
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.setting.metadata.description = Some(desc.into());
        self
    }
    
    pub fn build(self) -> BoolSetting {
        self.setting
    }
}

// Similar builders for other setting types...
```

### Setting Groups

```rust
/// Group related settings together
pub trait SettingGroup: Send + Sync {
    /// Register all settings in this group
    fn register(&self, settings: &mut Settings);
    
    /// Group prefix for all settings
    fn prefix(&self) -> &str;
}

/// Example: Cache settings group
pub struct CacheSettings {
    pub enabled: SettingHandle<bool>,
    pub size: SettingHandle<ByteSize>,
    pub ttl: SettingHandle<Duration>,
    pub eviction_policy: SettingHandle<EvictionPolicy>,
}

impl CacheSettings {
    pub fn new(settings: &mut Settings) -> Self {
        let prefix = "my_extension.cache";
        
        Self {
            enabled: settings.register(
                SettingBuilder::bool_setting(&format!("{}.enabled", prefix))
                    .default(true)
                    .dynamic()
                    .description("Enable caching")
                    .build()
            ),
            size: settings.register(
                SettingBuilder::byte_size_setting(&format!("{}.size", prefix))
                    .default(ByteSize::mb(100))
                    .min(ByteSize::mb(1))
                    .max(ByteSize::gb(10))
                    .dynamic()
                    .description("Maximum cache size")
                    .build()
            ),
            ttl: settings.register(
                SettingBuilder::duration_setting(&format!("{}.ttl", prefix))
                    .default(Duration::from_secs(300))
                    .min(Duration::from_secs(1))
                    .dynamic()
                    .description("Cache entry time-to-live")
                    .build()
            ),
            eviction_policy: settings.register(
                SettingBuilder::enum_setting(&format!("{}.eviction_policy", prefix))
                    .default(EvictionPolicy::Lru)
                    .dynamic()
                    .description("Cache eviction policy")
                    .build()
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EvictionPolicy {
    Lru,
    Lfu,
    Fifo,
}
```

### Dynamic Setting Updates

```rust
/// Listener for setting changes
#[async_trait]
pub trait SettingListener: Send + Sync {
    /// Called when a setting value changes
    async fn on_change(
        &self,
        key: &str,
        old_value: &dyn Any,
        new_value: &dyn Any,
    );
}

/// Example: Dynamic cache resizing
struct CacheResizeListener {
    cache: Arc<Cache>,
}

#[async_trait]
impl SettingListener for CacheResizeListener {
    async fn on_change(
        &self,
        key: &str,
        _old_value: &dyn Any,
        new_value: &dyn Any,
    ) {
        if key == "my_extension.cache.size" {
            if let Some(size) = new_value.downcast_ref::<ByteSize>() {
                self.cache.resize(size.as_bytes()).await;
            }
        }
    }
}
```

## Implementation Plan

### Phase 1: Core Setting Types (Week 1)
- [ ] Base setting trait and types
- [ ] Built-in setting implementations
- [ ] Setting validation framework
- [ ] Basic settings container

### Phase 2: Setting Sources (Week 2)
- [ ] Configuration file parsing
- [ ] Environment variable support
- [ ] OpenSearch cluster settings integration
- [ ] Setting precedence rules

### Phase 3: Dynamic Updates (Week 3)
- [ ] Setting change listeners
- [ ] Hot-reload support
- [ ] Transactional updates
- [ ] Rollback on validation failure

### Phase 4: Advanced Features (Week 4)
- [ ] Setting groups and namespaces
- [ ] Setting documentation generation
- [ ] Setting migration tools
- [ ] Performance optimizations

## Usage Example

```rust
use opensearch_sdk::{Settings, SettingBuilder, Extension};

struct MyExtension {
    settings: Settings,
    cache_settings: CacheSettings,
}

impl MyExtension {
    pub fn new() -> Self {
        let mut settings = Settings::new();
        
        // Register individual settings
        let debug_mode = settings.register(
            SettingBuilder::bool_setting("my_extension.debug")
                .default(false)
                .dynamic()
                .description("Enable debug logging")
                .build()
        );
        
        // Register setting groups
        let cache_settings = CacheSettings::new(&mut settings);
        
        // Load from all sources
        settings.load().expect("Failed to load settings");
        
        Self {
            settings,
            cache_settings,
        }
    }
}

// Using settings in extension
impl MyExtension {
    async fn process_request(&self) -> Result<(), Error> {
        if self.cache_settings.enabled.get().unwrap_or(true) {
            let cache_size = self.cache_settings.size.get()
                .unwrap_or(ByteSize::mb(100));
            // Use cache with specified size
        }
        
        Ok(())
    }
}
```

## Testing Strategy

### Unit Tests
- Setting parsing and validation
- Type conversions
- Default values
- Validation rules

### Integration Tests
- Loading from multiple sources
- Setting precedence
- Dynamic updates
- Listener notifications

### Property Tests
- Parsing round-trips
- Validation consistency
- Thread safety
- Memory usage

## Future Enhancements

- Setting profiles for different environments
- A/B testing support for settings
- Setting change audit log
- Encrypted settings for sensitive data
- Setting dependency tracking
- Automatic setting documentation