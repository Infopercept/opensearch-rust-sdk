use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use semver::Version;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionMetadata {
    pub manifest: ExtensionManifest,
    pub runtime_info: RuntimeInfo,
    pub metrics: ExtensionMetrics,
    pub custom_metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub name: String,
    pub unique_id: String,
    pub version: Version,
    pub opensearch_min_version: Version,
    pub opensearch_max_version: Option<Version>,
    pub java_version: String,
    pub description: String,
    pub vendor: String,
    pub license: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub issues: Option<String>,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub authors: Vec<Author>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeInfo {
    pub startup_time: std::time::SystemTime,
    pub pid: Option<u32>,
    pub host: String,
    pub port: u16,
    pub rust_version: String,
    pub os_info: OsInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OsInfo {
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionMetrics {
    pub requests_total: u64,
    pub requests_failed: u64,
    pub requests_duration_ms: Vec<f64>,
    pub memory_usage_bytes: Option<u64>,
    pub cpu_usage_percent: Option<f32>,
    pub uptime_seconds: u64,
}

impl ExtensionMetrics {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn record_request(&mut self, duration_ms: f64, success: bool) {
        self.requests_total += 1;
        if !success {
            self.requests_failed += 1;
        }
        self.requests_duration_ms.push(duration_ms);
        
        if self.requests_duration_ms.len() > 1000 {
            self.requests_duration_ms.remove(0);
        }
    }
    
    pub fn average_request_duration(&self) -> Option<f64> {
        if self.requests_duration_ms.is_empty() {
            None
        } else {
            let sum: f64 = self.requests_duration_ms.iter().sum();
            Some(sum / self.requests_duration_ms.len() as f64)
        }
    }
    
    pub fn success_rate(&self) -> f64 {
        if self.requests_total == 0 {
            1.0
        } else {
            (self.requests_total - self.requests_failed) as f64 / self.requests_total as f64
        }
    }
}

pub struct MetadataBuilder {
    manifest: ExtensionManifest,
    runtime_info: Option<RuntimeInfo>,
    custom_metadata: HashMap<String, serde_json::Value>,
}

impl MetadataBuilder {
    pub fn new(manifest: ExtensionManifest) -> Self {
        MetadataBuilder {
            manifest,
            runtime_info: None,
            custom_metadata: HashMap::new(),
        }
    }
    
    pub fn runtime_info(mut self, info: RuntimeInfo) -> Self {
        self.runtime_info = Some(info);
        self
    }
    
    pub fn custom_field(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.custom_metadata.insert(key.into(), value);
        self
    }
    
    pub fn build(self) -> ExtensionMetadata {
        let runtime_info = self.runtime_info.unwrap_or_else(|| RuntimeInfo {
            startup_time: std::time::SystemTime::now(),
            pid: std::process::id().into(),
            host: "localhost".to_string(),
            port: 0,
            rust_version: "unknown".to_string(),
            os_info: OsInfo {
                os_type: std::env::consts::OS.to_string(),
                os_version: "unknown".to_string(),
                architecture: std::env::consts::ARCH.to_string(),
            },
        });
        
        ExtensionMetadata {
            manifest: self.manifest,
            runtime_info,
            metrics: ExtensionMetrics::new(),
            custom_metadata: self.custom_metadata,
        }
    }
}

pub trait MetadataProvider {
    fn get_metadata(&self) -> ExtensionMetadata;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extension_metrics() {
        let mut metrics = ExtensionMetrics::new();
        
        metrics.record_request(100.0, true);
        metrics.record_request(200.0, true);
        metrics.record_request(150.0, false);
        
        assert_eq!(metrics.requests_total, 3);
        assert_eq!(metrics.requests_failed, 1);
        assert_eq!(metrics.average_request_duration(), Some(150.0));
        assert!((metrics.success_rate() - 0.666).abs() < 0.01);
    }
    
    #[test]
    fn test_metadata_builder() {
        let manifest = ExtensionManifest {
            name: "test-extension".to_string(),
            unique_id: "test-ext".to_string(),
            version: Version::new(1, 0, 0),
            opensearch_min_version: Version::new(3, 0, 0),
            opensearch_max_version: None,
            java_version: "11".to_string(),
            description: "Test extension".to_string(),
            vendor: "Test Inc".to_string(),
            license: "MIT".to_string(),
            homepage: None,
            repository: None,
            issues: None,
            categories: vec!["test".to_string()],
            keywords: vec!["test".to_string(), "extension".to_string()],
            authors: vec![Author {
                name: "Test Author".to_string(),
                email: Some("test@example.com".to_string()),
                url: None,
            }],
        };
        
        let metadata = MetadataBuilder::new(manifest.clone())
            .custom_field("test_field", serde_json::json!("test_value"))
            .build();
        
        assert_eq!(metadata.manifest.name, "test-extension");
        assert_eq!(metadata.custom_metadata.get("test_field").unwrap(), "test_value");
    }
}