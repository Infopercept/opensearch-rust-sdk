use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub details: HashMap<String, serde_json::Value>,
    pub last_check: std::time::SystemTime,
}

#[derive(Clone)]
pub struct HealthService {
    checks: Arc<RwLock<HashMap<String, HealthCheck>>>,
}

impl HealthService {
    pub fn new() -> Self {
        HealthService {
            checks: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub async fn register_check(&self, name: impl Into<String>) {
        let name = name.into();
        let check = HealthCheck {
            name: name.clone(),
            status: HealthStatus::Healthy,
            message: None,
            details: HashMap::new(),
            last_check: std::time::SystemTime::now(),
        };
        
        let mut checks = self.checks.write().await;
        checks.insert(name, check);
    }
    
    pub async fn update_check(
        &self,
        name: &str,
        status: HealthStatus,
        message: Option<String>,
    ) -> Result<(), String> {
        let mut checks = self.checks.write().await;
        let check = checks.get_mut(name)
            .ok_or_else(|| format!("Health check '{}' not found", name))?;
        
        check.status = status;
        check.message = message;
        check.last_check = std::time::SystemTime::now();
        
        Ok(())
    }
    
    pub async fn add_detail(
        &self,
        name: &str,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Result<(), String> {
        let mut checks = self.checks.write().await;
        let check = checks.get_mut(name)
            .ok_or_else(|| format!("Health check '{}' not found", name))?;
        
        check.details.insert(key.into(), value);
        
        Ok(())
    }
    
    pub async fn get_check(&self, name: &str) -> Option<HealthCheck> {
        let checks = self.checks.read().await;
        checks.get(name).cloned()
    }
    
    pub async fn get_all_checks(&self) -> Vec<HealthCheck> {
        let checks = self.checks.read().await;
        checks.values().cloned().collect()
    }
    
    pub async fn get_overall_status(&self) -> HealthStatus {
        let checks = self.checks.read().await;
        
        if checks.is_empty() {
            return HealthStatus::Healthy;
        }
        
        let mut has_degraded = false;
        
        for check in checks.values() {
            match check.status {
                HealthStatus::Unhealthy => return HealthStatus::Unhealthy,
                HealthStatus::Degraded => has_degraded = true,
                HealthStatus::Healthy => {}
            }
        }
        
        if has_degraded {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        }
    }
    
    pub async fn get_health_report(&self) -> HealthReport {
        let checks = self.get_all_checks().await;
        let overall_status = self.get_overall_status().await;
        
        HealthReport {
            status: overall_status,
            checks,
            timestamp: std::time::SystemTime::now(),
        }
    }
}

impl Default for HealthService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: HealthStatus,
    pub checks: Vec<HealthCheck>,
    pub timestamp: std::time::SystemTime,
}

#[async_trait::async_trait]
pub trait HealthCheckProvider: Send + Sync {
    async fn check_health(&self) -> HealthCheck;
}

pub struct CompositeHealthChecker {
    providers: Vec<Box<dyn HealthCheckProvider>>,
}

impl CompositeHealthChecker {
    pub fn new() -> Self {
        CompositeHealthChecker {
            providers: Vec::new(),
        }
    }
    
    pub fn add_provider(&mut self, provider: Box<dyn HealthCheckProvider>) {
        self.providers.push(provider);
    }
    
    pub async fn check_all(&self) -> Vec<HealthCheck> {
        let mut checks = Vec::new();
        
        for provider in &self.providers {
            checks.push(provider.check_health().await);
        }
        
        checks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_health_service() {
        let service = HealthService::new();
        
        service.register_check("database").await;
        service.register_check("cache").await;
        
        assert_eq!(service.get_overall_status().await, HealthStatus::Healthy);
        
        service.update_check("database", HealthStatus::Degraded, Some("Slow response".to_string())).await.unwrap();
        assert_eq!(service.get_overall_status().await, HealthStatus::Degraded);
        
        service.update_check("cache", HealthStatus::Unhealthy, Some("Connection failed".to_string())).await.unwrap();
        assert_eq!(service.get_overall_status().await, HealthStatus::Unhealthy);
        
        let report = service.get_health_report().await;
        assert_eq!(report.status, HealthStatus::Unhealthy);
        assert_eq!(report.checks.len(), 2);
    }
}