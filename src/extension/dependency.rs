use semver::Version;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionDependency {
    pub unique_id: String,
    pub version: Version,
}

impl ExtensionDependency {
    pub fn new(unique_id: impl Into<String>, version: Version) -> Self {
        ExtensionDependency {
            unique_id: unique_id.into(),
            version,
        }
    }
    
    pub fn from_str(unique_id: impl Into<String>, version_str: &str) -> Result<Self, semver::Error> {
        let version = Version::parse(version_str)?;
        Ok(Self::new(unique_id, version))
    }
    
    pub fn satisfies(&self, other_version: &Version) -> bool {
        self.version <= *other_version
    }
}

impl fmt::Display for ExtensionDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.unique_id, self.version)
    }
}

#[derive(Debug, Clone)]
pub struct DependencyResolver {
    extensions: Vec<ExtensionInfo>,
}

#[derive(Debug, Clone)]
struct ExtensionInfo {
    unique_id: String,
    version: Version,
    dependencies: Vec<ExtensionDependency>,
}

impl DependencyResolver {
    pub fn new() -> Self {
        DependencyResolver {
            extensions: Vec::new(),
        }
    }
    
    pub fn add_extension(
        &mut self,
        unique_id: impl Into<String>,
        version: Version,
        dependencies: Vec<ExtensionDependency>,
    ) {
        self.extensions.push(ExtensionInfo {
            unique_id: unique_id.into(),
            version,
            dependencies,
        });
    }
    
    pub fn resolve(&self) -> Result<Vec<String>, String> {
        let mut resolved = Vec::new();
        let mut visited = std::collections::HashSet::new();
        
        for ext in &self.extensions {
            self.resolve_extension(&ext.unique_id, &mut resolved, &mut visited)?;
        }
        
        Ok(resolved)
    }
    
    fn resolve_extension(
        &self,
        unique_id: &str,
        resolved: &mut Vec<String>,
        visited: &mut std::collections::HashSet<String>,
    ) -> Result<(), String> {
        if resolved.contains(&unique_id.to_string()) {
            return Ok(());
        }
        
        if !visited.insert(unique_id.to_string()) {
            return Err(format!("Circular dependency detected for extension: {}", unique_id));
        }
        
        if let Some(ext) = self.extensions.iter().find(|e| e.unique_id == unique_id) {
            for dep in &ext.dependencies {
                if let Some(dep_ext) = self.extensions.iter().find(|e| e.unique_id == dep.unique_id) {
                    if !dep.satisfies(&dep_ext.version) {
                        return Err(format!(
                            "Dependency version mismatch: {} requires {} {}, but found {}",
                            unique_id, dep.unique_id, dep.version, dep_ext.version
                        ));
                    }
                    self.resolve_extension(&dep.unique_id, resolved, visited)?;
                } else {
                    return Err(format!("Missing dependency: {} requires {}", unique_id, dep.unique_id));
                }
            }
            resolved.push(unique_id.to_string());
        }
        
        visited.remove(unique_id);
        Ok(())
    }
}

impl Default for DependencyResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dependency_creation() {
        let dep = ExtensionDependency::from_str("test-ext", "1.0.0").unwrap();
        assert_eq!(dep.unique_id, "test-ext");
        assert_eq!(dep.version, Version::new(1, 0, 0));
    }
    
    #[test]
    fn test_dependency_satisfies() {
        let dep = ExtensionDependency::from_str("test-ext", "1.0.0").unwrap();
        assert!(dep.satisfies(&Version::new(1, 0, 0)));
        assert!(dep.satisfies(&Version::new(1, 1, 0)));
        assert!(dep.satisfies(&Version::new(2, 0, 0)));
        assert!(!dep.satisfies(&Version::new(0, 9, 0)));
    }
    
    #[test]
    fn test_dependency_resolver() {
        let mut resolver = DependencyResolver::new();
        
        resolver.add_extension("ext-a", Version::new(1, 0, 0), vec![]);
        resolver.add_extension(
            "ext-b",
            Version::new(1, 0, 0),
            vec![ExtensionDependency::from_str("ext-a", "1.0.0").unwrap()],
        );
        resolver.add_extension(
            "ext-c",
            Version::new(1, 0, 0),
            vec![
                ExtensionDependency::from_str("ext-a", "1.0.0").unwrap(),
                ExtensionDependency::from_str("ext-b", "1.0.0").unwrap(),
            ],
        );
        
        let resolved = resolver.resolve().unwrap();
        assert_eq!(resolved, vec!["ext-a", "ext-b", "ext-c"]);
    }
    
    #[test]
    fn test_circular_dependency_detection() {
        let mut resolver = DependencyResolver::new();
        
        resolver.add_extension(
            "ext-a",
            Version::new(1, 0, 0),
            vec![ExtensionDependency::from_str("ext-b", "1.0.0").unwrap()],
        );
        resolver.add_extension(
            "ext-b",
            Version::new(1, 0, 0),
            vec![ExtensionDependency::from_str("ext-a", "1.0.0").unwrap()],
        );
        
        let result = resolver.resolve();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Circular dependency"));
    }
}