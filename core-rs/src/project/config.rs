/**
 * config.rs
 * Parser for .ckproject files (YAML format)
 *
 * Format:
 * ```yaml
 * apiVersion: conceptkernel/v1
 * kind: Project
 * metadata:
 *   name: project-name
 *   id: project-id
 * spec:
 *   domain: Org.Domain
 *   version: 1.3.14
 * ```
 *
 * Reference: Node.js v1.3.14 - MULTI_PROJECT_INFRASTRUCTURE.md section 2
 */

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::errors::CkpError;

/// .ckproject file structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    pub api_version: String,
    pub kind: String,
    pub metadata: Metadata,
    pub spec: Spec,
}

/// Project metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Metadata {
    pub name: String,
    pub id: String,
}

/// Port configuration in project spec
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PortConfig {
    pub base_port: u16,
    pub slot: u32,
}

/// Feature flags
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Features {
    pub use_edge_routing: bool,
}

/// Protocol domain resolution mapping
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtocolMapping {
    pub domain: String,
    pub url: String,
}

/// Default user for local development
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DefaultUser {
    pub username: String,
    pub password_hash: String,
    pub user_id: String,
    pub email: String,
    pub created_at: String,
    pub roles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Ontology library configuration (Phase 4 Stage 0)
/// Ontologies stored as ConceptKernel.Ontology kernel using standard URN resolution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OntologyConfig {
    /// Ontology kernel name (e.g., "ConceptKernel.Ontology") - Optional
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernel: Option<String>,
    /// Core ConceptKernel ontology URN (resolves via UrnResolver or file://)
    pub core: String,
    /// BFO upper ontology URN (resolves via UrnResolver or http://)
    pub bfo: String,
    /// Edge predicate mappings URN (resolves via UrnResolver or file://)
    pub predicates: String,
    /// Process ontology URN (optional, v1.3.18+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processes: Option<String>,
    /// RBAC ontology URN (optional, v1.3.18+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rbac: Option<String>,
    /// Self-improvement ontology URN (optional, v1.3.18+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub improvement: Option<String>,
    /// Workflow orchestration ontology URN (optional, v1.3.18+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
}

/// Project specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    pub domain: String,
    pub version: String,
    /// Optional port configuration (added during registration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<PortConfig>,
    /// Feature flags (v1.3.6+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub features: Option<Features>,
    /// Protocol domain resolution (v1.3.16+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protocol: Option<Vec<ProtocolMapping>>,
    /// Default user for local development (v1.3.11+)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_user: Option<DefaultUser>,
    /// Ontology library configuration (Phase 4 Stage 0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ontology: Option<OntologyConfig>,
}

impl ProjectConfig {
    /// Load .ckproject from specified path
    ///
    /// # Arguments
    /// * `path` - Path to .ckproject file
    ///
    /// # Example
    /// ```
    /// let config = ProjectConfig::load(".ckproject")?;
    /// assert_eq!(config.metadata.name, "my-project");
    /// ```
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, CkpError> {
        let path = path.as_ref();

        if !path.exists() {
            return Err(CkpError::FileNotFound(
                path.to_string_lossy().to_string(),
            ));
        }

        let content = fs::read_to_string(path).map_err(|e| {
            CkpError::IoError(format!("Failed to read .ckproject: {}", e))
        })?;

        let config: ProjectConfig = serde_yaml::from_str(&content).map_err(|e| {
            CkpError::ParseError(format!("Invalid .ckproject YAML: {}", e))
        })?;

        // Validate required fields
        config.validate()?;

        Ok(config)
    }

    /// Load .ckproject from project root directory
    ///
    /// # Arguments
    /// * `project_root` - Path to project root containing .ckproject
    ///
    /// # Example
    /// ```
    /// let config = ProjectConfig::load_from_project(".")?;
    /// ```
    pub fn load_from_project<P: AsRef<Path>>(project_root: P) -> Result<Self, CkpError> {
        let ckproject_path = project_root.as_ref().join(".ckproject");
        Self::load(ckproject_path)
    }

    /// Validate .ckproject structure
    ///
    /// Ensures:
    /// - apiVersion is "conceptkernel/v1"
    /// - kind is "Project"
    /// - All required fields are present and non-empty
    pub fn validate(&self) -> Result<(), CkpError> {
        // Validate apiVersion
        if self.api_version != "conceptkernel/v1" {
            return Err(CkpError::ValidationError(format!(
                "Invalid apiVersion: expected 'conceptkernel/v1', got '{}'",
                self.api_version
            )));
        }

        // Validate kind
        if self.kind != "Project" {
            return Err(CkpError::ValidationError(format!(
                "Invalid kind: expected 'Project', got '{}'",
                self.kind
            )));
        }

        // Validate metadata.name
        if self.metadata.name.is_empty() {
            return Err(CkpError::ValidationError(
                "metadata.name cannot be empty".to_string(),
            ));
        }

        // Validate metadata.id
        if self.metadata.id.is_empty() {
            return Err(CkpError::ValidationError(
                "metadata.id cannot be empty".to_string(),
            ));
        }

        // Validate spec.domain
        if self.spec.domain.is_empty() {
            return Err(CkpError::ValidationError(
                "spec.domain cannot be empty".to_string(),
            ));
        }

        // Validate spec.version
        if self.spec.version.is_empty() {
            return Err(CkpError::ValidationError(
                "spec.version cannot be empty".to_string(),
            ));
        }

        Ok(())
    }

    /// Save .ckproject to file
    ///
    /// # Arguments
    /// * `path` - Path where to save .ckproject
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), CkpError> {
        let yaml = serde_yaml::to_string(self).map_err(|e| {
            CkpError::SerializationError(format!("Failed to serialize .ckproject: {}", e))
        })?;

        fs::write(path.as_ref(), yaml).map_err(|e| {
            CkpError::IoError(format!("Failed to write .ckproject: {}", e))
        })?;

        Ok(())
    }

    /// Create a new ProjectConfig with given values
    ///
    /// # Example
    /// ```
    /// let config = ProjectConfig::new(
    ///     "my-project".to_string(),
    ///     "proj-my-project-20250125".to_string(),
    ///     "Org.MyDomain".to_string(),
    ///     "1.3.14".to_string(),
    /// );
    /// ```
    pub fn new(name: String, id: String, domain: String, version: String) -> Self {
        ProjectConfig {
            api_version: "conceptkernel/v1".to_string(),
            kind: "Project".to_string(),
            metadata: Metadata { name, id },
            spec: Spec {
                domain,
                version,
                ports: None,
                features: None,
                protocol: None,
                default_user: None,
                ontology: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_valid_ckproject() {
        let temp_dir = TempDir::new().unwrap();
        let ckproject_path = temp_dir.path().join(".ckproject");

        let yaml_content = r#"
apiVersion: conceptkernel/v1
kind: Project
metadata:
  name: test-project
  id: proj-test-20250125
spec:
  domain: Org.Test
  version: 1.3.14
"#;

        fs::write(&ckproject_path, yaml_content).unwrap();

        let config = ProjectConfig::load(&ckproject_path).unwrap();

        assert_eq!(config.api_version, "conceptkernel/v1");
        assert_eq!(config.kind, "Project");
        assert_eq!(config.metadata.name, "test-project");
        assert_eq!(config.metadata.id, "proj-test-20250125");
        assert_eq!(config.spec.domain, "Org.Test");
        assert_eq!(config.spec.version, "1.3.14");
    }

    #[test]
    fn test_parse_missing_file() {
        let result = ProjectConfig::load("/nonexistent/.ckproject");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_api_version() {
        let mut config = ProjectConfig::new(
            "test".to_string(),
            "proj-test".to_string(),
            "Org.Test".to_string(),
            "1.3.14".to_string(),
        );

        config.api_version = "invalid/v1".to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid apiVersion"));
    }

    #[test]
    fn test_validate_invalid_kind() {
        let mut config = ProjectConfig::new(
            "test".to_string(),
            "proj-test".to_string(),
            "Org.Test".to_string(),
            "1.3.14".to_string(),
        );

        config.kind = "InvalidKind".to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid kind"));
    }

    #[test]
    fn test_validate_empty_name() {
        let config = ProjectConfig::new(
            "".to_string(), // Empty name
            "proj-test".to_string(),
            "Org.Test".to_string(),
            "1.3.14".to_string(),
        );

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("metadata.name cannot be empty"));
    }

    #[test]
    fn test_load_from_project() {
        let temp_dir = TempDir::new().unwrap();
        let ckproject_path = temp_dir.path().join(".ckproject");

        let yaml_content = r#"
apiVersion: conceptkernel/v1
kind: Project
metadata:
  name: test-project
  id: proj-test-20250125
spec:
  domain: Org.Test
  version: 1.3.14
"#;

        fs::write(&ckproject_path, yaml_content).unwrap();

        let config = ProjectConfig::load_from_project(temp_dir.path()).unwrap();
        assert_eq!(config.metadata.name, "test-project");
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let ckproject_path = temp_dir.path().join(".ckproject");

        let config = ProjectConfig::new(
            "save-test".to_string(),
            "proj-save-20250125".to_string(),
            "Org.SaveTest".to_string(),
            "1.3.14".to_string(),
        );

        // Save
        config.save(&ckproject_path).unwrap();

        // Load
        let loaded_config = ProjectConfig::load(&ckproject_path).unwrap();

        assert_eq!(config, loaded_config);
    }

    #[test]
    fn test_new_creates_valid_config() {
        let config = ProjectConfig::new(
            "test-new".to_string(),
            "proj-new-20250125".to_string(),
            "Org.NewTest".to_string(),
            "1.3.14".to_string(),
        );

        assert!(config.validate().is_ok());
        assert_eq!(config.api_version, "conceptkernel/v1");
        assert_eq!(config.kind, "Project");
    }
}
