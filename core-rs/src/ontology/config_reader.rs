//! Ontology reader for parsing kernel conceptkernel.yaml files
//!
//! Provides methods to extract contracts, edges, and metadata from ontologies

use crate::errors::{CkpError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Complete ontology document structure
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Ontology {
    pub api_version: String,
    pub kind: String,
    pub metadata: Metadata,
    pub spec: Option<Spec>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contracts: Option<OldContracts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interfaces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundaries: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub governance: Option<serde_json::Value>,
}

/// Old contracts format (deprecated, for compatibility)
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OldContracts {
    pub invocation: Option<serde_json::Value>,
}

/// Kernel metadata
///
/// Supports two formats:
/// 1. URN-based (preferred): `urn: ckp://System.Registry:v0.1`
/// 2. Legacy: `name: UI.Bakery` + `version: v0.1`
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    /// Full URN (preferred format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urn: Option<String>,

    /// Legacy name field (falls back if urn not present)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Kernel type: node:hot, node:cold, python:hot, python:cold, node:manual
    #[serde(rename = "type")]
    pub kernel_type: String,

    /// Version (used with name for legacy format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Port number (for hot kernels)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,

    /// Entry point script
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Authors list
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,

    /// Tags for categorization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
}

impl Metadata {
    /// Get the canonical URN for this kernel
    ///
    /// Returns metadata.urn if present, otherwise constructs from name + version
    pub fn get_urn(&self) -> String {
        if let Some(urn) = &self.urn {
            urn.clone()
        } else if let (Some(name), Some(version)) = (&self.name, &self.version) {
            format!("ckp://{}:{}", name, version)
        } else if let Some(name) = &self.name {
            name.clone()
        } else {
            "unknown".to_string()
        }
    }

    /// Get the kernel name (without version)
    pub fn get_name(&self) -> String {
        if let Some(urn) = &self.urn {
            // Extract name from URN: ckp://System.Registry:v0.1 -> System.Registry
            urn.strip_prefix("ckp://")
                .and_then(|s| s.split(':').next())
                .unwrap_or(urn)
                .to_string()
        } else if let Some(name) = &self.name {
            name.clone()
        } else {
            "unknown".to_string()
        }
    }
}

/// Specification section
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Spec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_contract: Option<QueueContract>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_contract: Option<StorageContract>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_contract: Option<Vec<NotificationContract>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rbac: Option<Rbac>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deploy_contract: Option<DeployContract>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cli: Option<CliContract>,
}

/// CLI contract for dynamic command registration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CliContract {
    /// Whether to expose this kernel as a CLI command
    pub expose: bool,
    /// Primary command name (e.g., "role" for "ck role")
    pub primary: String,
    /// Optional alias commands (e.g., ["roles"] for "ck roles")
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Description shown in help text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Subcommands (e.g., list, create, delete)
    #[serde(default)]
    pub subcommands: Vec<CliSubcommand>,
}

/// CLI subcommand mapping
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct CliSubcommand {
    /// Subcommand name (e.g., "list", "create")
    pub name: String,
    /// Action name from contracts.invocation.methods
    pub action: String,
}

/// Deploy contract
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DeployContract {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui_endpoint: Option<String>,
}

/// Queue contract
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct QueueContract {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edges: Option<Vec<EdgeEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<Vec<serde_json::Value>>,
}

/// Edge entry - can be string or object
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum EdgeEntry {
    Urn(String),
    Object(EdgeObject),
}

/// Edge object format
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct EdgeObject {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_urn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub urn: Option<String>,
}

/// Storage contract
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct StorageContract {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

/// Notification contract entry
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct NotificationContract {
    pub target_kernel: String,
    pub queue: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_urn: Option<String>,
}

/// RBAC configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Rbac {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub communication: Option<Communication>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub consensus: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_improvement: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git: Option<serde_json::Value>,
}

/// Communication RBAC rules
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Communication {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub denied: Option<Vec<String>>,
}

/// Ontology reader
pub struct OntologyReader {
    root: PathBuf,
}

impl OntologyReader {
    /// Create new ontology reader
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::ontology::OntologyReader;
    /// use std::path::PathBuf;
    ///
    /// let reader = OntologyReader::new(PathBuf::from("/concepts"));
    /// ```
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Read and parse conceptkernel.yaml file
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::ontology::OntologyReader;
    /// use std::path::{Path, PathBuf};
    ///
    /// let reader = OntologyReader::new(PathBuf::from("/concepts"));
    /// let ontology = reader.read(Path::new("/concepts/Recipes.BakeCake/conceptkernel.yaml")).unwrap();
    /// println!("Kernel: {}", ontology.metadata.name);
    /// ```
    pub fn read(&self, ontology_path: &Path) -> Result<Ontology> {
        if !ontology_path.exists() {
            return Err(CkpError::Ontology(format!(
                "Ontology file not found: {}",
                ontology_path.display()
            )));
        }

        let content = fs::read_to_string(ontology_path).map_err(|e| {
            CkpError::Ontology(format!("Failed to read ontology file: {}", e))
        })?;

        let ontology: Ontology = serde_yaml::from_str(&content).map_err(|e| {
            CkpError::Ontology(format!("Failed to parse ontology YAML: {}", e))
        })?;

        // Validate required fields - must have either urn or name
        if ontology.metadata.urn.is_none() && ontology.metadata.name.is_none() {
            return Err(CkpError::Ontology(
                "Invalid ontology format: missing both metadata.urn and metadata.name".to_string(),
            ));
        }

        Ok(ontology)
    }

    /// Read ontology for a kernel by name
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::ontology::OntologyReader;
    /// use std::path::PathBuf;
    ///
    /// let reader = OntologyReader::new(PathBuf::from("/concepts"));
    /// let ontology = reader.read_by_kernel_name("Recipes.BakeCake").unwrap();
    /// ```
    pub fn read_by_kernel_name(&self, kernel_name: &str) -> Result<Ontology> {
        let ontology_path = self
            .root
            .join("concepts")
            .join(kernel_name)
            .join("conceptkernel.yaml");

        self.read(&ontology_path)
    }

    /// Read authorized incoming edges from ontology
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::ontology::OntologyReader;
    /// use std::path::PathBuf;
    ///
    /// let reader = OntologyReader::new(PathBuf::from("/concepts"));
    /// let edges = reader.read_edges("Recipes.BakeCake").unwrap();
    /// for edge in edges {
    ///     println!("Authorized edge: {}", edge);
    /// }
    /// ```
    pub fn read_edges(&self, kernel_name: &str) -> Result<Vec<String>> {
        let ontology = self.read_by_kernel_name(kernel_name)?;

        let edges = ontology
            .spec
            .and_then(|s| s.queue_contract)
            .and_then(|qc| qc.edges)
            .unwrap_or_default();

        // Normalize edges to URN strings
        let edge_urns: Vec<String> = edges
            .into_iter()
            .filter_map(|edge| match edge {
                EdgeEntry::Urn(urn) => Some(urn),
                EdgeEntry::Object(obj) => obj.edge_urn.or(obj.urn),
            })
            .collect();

        Ok(edge_urns)
    }

    /// Check if kernel has specific edge authorized
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::ontology::OntologyReader;
    /// use std::path::PathBuf;
    ///
    /// let reader = OntologyReader::new(PathBuf::from("/concepts"));
    /// let authorized = reader.is_edge_authorized(
    ///     "Recipes.BakeCake",
    ///     "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12"
    /// ).unwrap();
    /// ```
    pub fn is_edge_authorized(&self, kernel_name: &str, edge_urn: &str) -> Result<bool> {
        // Check RBAC communication rules first
        if let Some(rbac) = self.read_rbac(kernel_name)? {
            if let Some(communication) = rbac.communication {
                // 1. Blacklist has priority - check denied list first
                if let Some(denied) = communication.denied {
                    // Support wildcard and exact match
                    for pattern in &denied {
                        if pattern == "*" || pattern == edge_urn {
                            return Ok(false);
                        }
                        // Support partial pattern matching (e.g., "ckp://External.*")
                        if pattern.ends_with("*") {
                            let prefix = pattern.trim_end_matches('*');
                            if edge_urn.starts_with(prefix) {
                                return Ok(false);
                            }
                        }
                    }
                }

                // 2. Check whitelist (allowed)
                if let Some(allowed) = communication.allowed {
                    for pattern in &allowed {
                        // Wildcard allows everything
                        if pattern == "*" {
                            return Ok(true);
                        }
                        // Exact match
                        if pattern == edge_urn {
                            return Ok(true);
                        }
                        // Partial pattern matching
                        if pattern.ends_with("*") {
                            let prefix = pattern.trim_end_matches('*');
                            if edge_urn.starts_with(prefix) {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
        }

        // 3. Fall back to queue_contract.edges list
        let authorized_edges = self.read_edges(kernel_name)?;

        // Check for exact match or wildcard patterns in edges list
        for edge in &authorized_edges {
            if edge == edge_urn {
                return Ok(true);
            }
            // Support wildcard in edge URN patterns (e.g., "ckp://Edge.NOTIFIES.*-to-Target:v1.3.14")
            if edge.contains('*') {
                let pattern_parts: Vec<&str> = edge.split('*').collect();
                if pattern_parts.len() == 2 {
                    let prefix = pattern_parts[0];
                    let suffix = pattern_parts[1];
                    if edge_urn.starts_with(prefix) && edge_urn.ends_with(suffix) {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    /// Read RBAC configuration from ontology
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::ontology::OntologyReader;
    /// use std::path::PathBuf;
    ///
    /// let reader = OntologyReader::new(PathBuf::from("/concepts"));
    /// let rbac = reader.read_rbac("Recipes.BakeCake").unwrap();
    /// ```
    pub fn read_rbac(&self, kernel_name: &str) -> Result<Option<Rbac>> {
        let ontology = self.read_by_kernel_name(kernel_name)?;
        Ok(ontology.spec.and_then(|s| s.rbac))
    }

    /// Read notification contract from ontology
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::ontology::OntologyReader;
    /// use std::path::PathBuf;
    ///
    /// let reader = OntologyReader::new(PathBuf::from("/concepts"));
    /// let notifications = reader.read_notification_contract("Recipes.BakeCake").unwrap();
    /// ```
    pub fn read_notification_contract(
        &self,
        kernel_name: &str,
    ) -> Result<Vec<NotificationContract>> {
        let ontology = self.read_by_kernel_name(kernel_name)?;
        Ok(ontology
            .spec
            .and_then(|s| s.notification_contract)
            .unwrap_or_default())
    }

    /// Get kernel metadata
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::ontology::OntologyReader;
    /// use std::path::PathBuf;
    ///
    /// let reader = OntologyReader::new(PathBuf::from("/concepts"));
    /// let metadata = reader.read_metadata("Recipes.BakeCake").unwrap();
    /// println!("URN: {}", metadata.name);
    /// println!("Type: {}", metadata.kernel_type);
    /// ```
    pub fn read_metadata(&self, kernel_name: &str) -> Result<Metadata> {
        let ontology = self.read_by_kernel_name(kernel_name)?;
        Ok(ontology.metadata)
    }

    /// Get kernel capabilities
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::ontology::OntologyReader;
    /// use std::path::PathBuf;
    ///
    /// let reader = OntologyReader::new(PathBuf::from("/concepts"));
    /// let capabilities = reader.read_capabilities("Recipes.BakeCake").unwrap();
    /// ```
    pub fn read_capabilities(&self, kernel_name: &str) -> Result<Vec<String>> {
        let ontology = self.read_by_kernel_name(kernel_name)?;
        Ok(ontology.capabilities)
    }

    /// List all kernels in concepts directory
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::ontology::OntologyReader;
    /// use std::path::PathBuf;
    ///
    /// let reader = OntologyReader::new(PathBuf::from("/concepts"));
    /// let kernels = reader.list_all().unwrap();
    /// for kernel in kernels {
    ///     println!("Found kernel: {} ({})", kernel.name, kernel.kernel_type);
    /// }
    /// ```
    pub fn list_all(&self) -> Result<Vec<KernelInfo>> {
        let concepts_dir = self.root.join("concepts");

        if !concepts_dir.exists() {
            return Ok(Vec::new());
        }

        let mut kernels = Vec::new();

        for entry in fs::read_dir(&concepts_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }

            let kernel_name = entry.file_name().to_string_lossy().to_string();
            let ontology_path = entry.path().join("conceptkernel.yaml");

            if !ontology_path.exists() {
                continue;
            }

            match self.read(&ontology_path) {
                Ok(ontology) => {
                    kernels.push(KernelInfo {
                        name: kernel_name.clone(),
                        urn: ontology.metadata.get_urn(),
                        kernel_type: ontology.metadata.kernel_type.clone(),
                        version: ontology.metadata.version.clone().unwrap_or_default(),
                        description: ontology.metadata.description.clone().unwrap_or_default(),
                        path: entry.path(),
                    });
                }
                Err(e) => {
                    eprintln!(
                        "[OntologyReader] Failed to read ontology for {}: {}",
                        kernel_name, e
                    );
                }
            }
        }

        Ok(kernels)
    }

    /// Get queue contract from ontology
    pub fn read_queue_contract(&self, kernel_name: &str) -> Result<Option<QueueContract>> {
        let ontology = self.read_by_kernel_name(kernel_name)?;
        Ok(ontology.spec.and_then(|s| s.queue_contract))
    }

    /// Get storage contract from ontology
    pub fn read_storage_contract(&self, kernel_name: &str) -> Result<Option<StorageContract>> {
        let ontology = self.read_by_kernel_name(kernel_name)?;
        Ok(ontology.spec.and_then(|s| s.storage_contract))
    }
}

/// Kernel information summary
#[derive(Debug, Clone, PartialEq)]
pub struct KernelInfo {
    pub name: String,
    pub urn: String,
    pub kernel_type: String,
    pub version: String,
    pub description: String,
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_ontology(temp_dir: &TempDir, kernel_name: &str, content: &str) {
        let kernel_dir = temp_dir.path().join("concepts").join(kernel_name);
        fs::create_dir_all(&kernel_dir).unwrap();
        let ontology_path = kernel_dir.join("conceptkernel.yaml");
        fs::write(ontology_path, content).unwrap();
    }

    #[test]
    fn test_read_valid_ontology() {
        let temp_dir = TempDir::new().unwrap();
        let ontology_yaml = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Recipes.BakeCake:v0.1
  type: node:cold
  version: v0.1
  description: Bakes a cake
spec:
  queue_contract:
    edges:
      - ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12
capabilities:
  - baking
        "#;

        create_test_ontology(&temp_dir, "Recipes.BakeCake", ontology_yaml);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let ontology = reader.read_by_kernel_name("Recipes.BakeCake").unwrap();

        assert_eq!(ontology.metadata.name, Some("ckp://Recipes.BakeCake:v0.1".to_string()));
        assert_eq!(ontology.metadata.kernel_type, "node:cold");
        assert_eq!(ontology.metadata.version, Some("v0.1".to_string()));
        assert_eq!(
            ontology.metadata.description,
            Some("Bakes a cake".to_string())
        );
        assert_eq!(ontology.capabilities, vec!["baking"]);
    }

    #[test]
    fn test_read_edges() {
        let temp_dir = TempDir::new().unwrap();
        let ontology_yaml = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Recipes.BakeCake:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12
      - ckp://Edge.VALIDATES.System.Proof-to-BakeCake:v1.3.12
        "#;

        create_test_ontology(&temp_dir, "Recipes.BakeCake", ontology_yaml);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let edges = reader.read_edges("Recipes.BakeCake").unwrap();

        assert_eq!(edges.len(), 2);
        assert!(edges.contains(&"ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12".to_string()));
        assert!(edges.contains(&"ckp://Edge.VALIDATES.System.Proof-to-BakeCake:v1.3.12".to_string()));
    }

    #[test]
    fn test_is_edge_authorized() {
        let temp_dir = TempDir::new().unwrap();
        let ontology_yaml = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Recipes.BakeCake:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12
        "#;

        create_test_ontology(&temp_dir, "Recipes.BakeCake", ontology_yaml);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());

        assert!(reader
            .is_edge_authorized(
                "Recipes.BakeCake",
                "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12"
            )
            .unwrap());

        assert!(!reader
            .is_edge_authorized(
                "Recipes.BakeCake",
                "ckp://Edge.PRODUCES.Evil-to-BakeCake:v1.3.12"
            )
            .unwrap());
    }

    #[test]
    fn test_read_rbac() {
        let temp_dir = TempDir::new().unwrap();
        let ontology_yaml = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Recipes.BakeCake:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - ckp://System.Proof:v0.1
      denied:
        - ckp://External.*
        "#;

        create_test_ontology(&temp_dir, "Recipes.BakeCake", ontology_yaml);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let rbac = reader.read_rbac("Recipes.BakeCake").unwrap().unwrap();

        let comm = rbac.communication.unwrap();
        assert_eq!(comm.allowed, Some(vec!["ckp://System.Proof:v0.1".to_string()]));
        assert_eq!(comm.denied, Some(vec!["ckp://External.*".to_string()]));
    }

    #[test]
    fn test_list_all() {
        let temp_dir = TempDir::new().unwrap();

        let ontology1 = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Kernel1:v0.1
  type: node:cold
  version: v0.1
capabilities: []
        "#;

        let ontology2 = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Kernel2:v0.1
  type: python:cold
  version: v0.2
capabilities: []
        "#;

        create_test_ontology(&temp_dir, "Kernel1", ontology1);
        create_test_ontology(&temp_dir, "Kernel2", ontology2);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let kernels = reader.list_all().unwrap();

        assert_eq!(kernels.len(), 2);
        assert!(kernels.iter().any(|k| k.name == "Kernel1"));
        assert!(kernels.iter().any(|k| k.name == "Kernel2"));
    }

    // NEW TESTS - Test Parity with Node.js

    /// Test: read() - missing file returns error
    /// Node.js equivalent: OntologyReader.test.js:98
    #[test]
    fn test_read_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let reader = OntologyReader::new(temp_dir.path().to_path_buf());

        let nonexistent = temp_dir.path().join("nonexistent.yaml");
        let result = reader.read(&nonexistent);
        assert!(result.is_err());
    }

    /// Test: readEdges() - no edges returns empty array
    /// Node.js equivalent: OntologyReader.test.js:173
    #[test]
    fn test_read_edges_empty() {
        let temp_dir = TempDir::new().unwrap();

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://NoEdges.Kernel:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    manifest: []
capabilities: []
        "#;

        create_test_ontology(&temp_dir, "NoEdges.Kernel", ontology);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let edges = reader.read_edges("NoEdges.Kernel").unwrap();

        assert!(edges.is_empty());
    }

    /// Test: readEdges() - object format with urn field
    /// Node.js equivalent: OntologyReader.test.js:128
    #[test]
    fn test_read_edges_object_format() {
        let temp_dir = TempDir::new().unwrap();

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://ObjectEdges.Kernel:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - urn: ckp://Edge.PRODUCES.Source-to-ObjectEdges.Kernel:v1.3.12
        predicate: PRODUCES
        source: Source
        target: ObjectEdges.Kernel
      - urn: ckp://Edge.REQUIRES.ObjectEdges.Kernel-to-Target:v1.3.12
        predicate: REQUIRES
        source: ObjectEdges.Kernel
        target: Target
capabilities: []
        "#;

        create_test_ontology(&temp_dir, "ObjectEdges.Kernel", ontology);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let edges = reader.read_edges("ObjectEdges.Kernel").unwrap();

        assert_eq!(edges.len(), 2);
        assert!(edges.contains(&"ckp://Edge.PRODUCES.Source-to-ObjectEdges.Kernel:v1.3.12".to_string()));
        assert!(edges.contains(&"ckp://Edge.REQUIRES.ObjectEdges.Kernel-to-Target:v1.3.12".to_string()));
    }

    /// Test: isEdgeAuthorized() - unauthorized edge
    /// Node.js equivalent: OntologyReader.test.js:226
    #[test]
    fn test_is_edge_unauthorized() {
        let temp_dir = TempDir::new().unwrap();

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://AuthTest.Kernel:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - ckp://Edge.PRODUCES.Authorized-to-AuthTest.Kernel:v1.3.12
capabilities: []
        "#;

        create_test_ontology(&temp_dir, "AuthTest.Kernel", ontology);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let authorized = reader.is_edge_authorized(
            "AuthTest.Kernel",
            "ckp://Edge.PRODUCES.Unauthorized-to-AuthTest.Kernel:v1.3.12"
        ).unwrap();

        assert!(!authorized);
    }

    /// Test: read_metadata() - extracts metadata fields
    /// Node.js equivalent: OntologyReader.test.js:250
    #[test]
    fn test_read_metadata_extraction() {
        let temp_dir = TempDir::new().unwrap();

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Metadata.Test:v0.1
  type: node:cold
  version: v0.1
  description: Test kernel for metadata
capabilities: []
        "#;

        create_test_ontology(&temp_dir, "Metadata.Test", ontology);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let metadata = reader.read_metadata("Metadata.Test").unwrap();

        assert_eq!(metadata.name, Some("ckp://Metadata.Test:v0.1".to_string()));
        assert_eq!(metadata.kernel_type, "node:cold");
        assert_eq!(metadata.version, Some("v0.1".to_string()));
    }

    /// Test: read_queue_contract() - extracts queue contract
    /// Node.js equivalent: OntologyReader.test.js:258 (extractContracts)
    #[test]
    fn test_read_queue_contract() {
        let temp_dir = TempDir::new().unwrap();

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Contract.Test:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    manifest:
      - name: input
        payload_type_link: string
        rule: required_one
capabilities: []
        "#;

        create_test_ontology(&temp_dir, "Contract.Test", ontology);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let contract = reader.read_queue_contract("Contract.Test").unwrap();

        assert!(contract.is_some());
        let queue = contract.unwrap();
        assert!(queue.manifest.is_some());

        let manifest = queue.manifest.unwrap();
        assert_eq!(manifest.len(), 1);
        assert_eq!(manifest[0]["name"], "input");
    }

    /// Test: read_storage_contract() - extracts storage schema
    /// Node.js equivalent: OntologyReader.test.js:303 (getStorageContract)
    #[test]
    fn test_read_storage_contract() {
        let temp_dir = TempDir::new().unwrap();

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Storage.Test:v0.1
  type: node:cold
  version: v0.1
spec:
  storage_contract:
    result:
      type: string
    success:
      type: bool
capabilities: []
        "#;

        create_test_ontology(&temp_dir, "Storage.Test", ontology);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let contract = reader.read_storage_contract("Storage.Test").unwrap();

        assert!(contract.is_some());
    }

    /// Test: read_rbac() - no RBAC returns None
    /// Node.js equivalent: OntologyReader.test.js:279 (extractRbacConfig empty)
    #[test]
    fn test_read_rbac_empty() {
        let temp_dir = TempDir::new().unwrap();

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://NoRbac.Test:v0.1
  type: node:cold
  version: v0.1
spec: {}
capabilities: []
        "#;

        create_test_ontology(&temp_dir, "NoRbac.Test", ontology);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let rbac = reader.read_rbac("NoRbac.Test").unwrap();

        assert!(rbac.is_none());
    }

    /// Test: read_rbac() - full RBAC extraction
    /// Node.js equivalent: OntologyReader.test.js:268 (extractRbacConfig)
    #[test]
    fn test_read_rbac_full() {
        let temp_dir = TempDir::new().unwrap();

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Rbac.Test:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - ckp://System.*
        - ckp://Test.Target:v0.1
      denied:
        - ckp://External.Malicious:v0.1
    git:
      can_commit: true
      can_tag: false
capabilities: []
        "#;

        create_test_ontology(&temp_dir, "Rbac.Test", ontology);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let rbac = reader.read_rbac("Rbac.Test").unwrap();

        assert!(rbac.is_some());
        let rbac_config = rbac.unwrap();

        assert!(rbac_config.communication.is_some());
        let comm = rbac_config.communication.unwrap();
        assert_eq!(comm.allowed.unwrap().len(), 2);
        assert_eq!(comm.denied.unwrap().len(), 1);
    }

    /// Test: read() - invalid YAML error handling
    /// Node.js equivalent: Not explicitly tested but important
    #[test]
    fn test_read_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();

        // Create invalid YAML
        let concepts_dir = temp_dir.path().join("concepts").join("Invalid.Kernel");
        fs::create_dir_all(&concepts_dir).unwrap();

        let invalid_yaml = "this is: [not: valid: yaml: content";
        fs::write(concepts_dir.join("conceptkernel.yaml"), invalid_yaml).unwrap();

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let result = reader.read_by_kernel_name("Invalid.Kernel");

        assert!(result.is_err());
    }

    /// Test: read_capabilities() - extracts capabilities array
    /// Node.js equivalent: Part of metadata extraction
    #[test]
    fn test_read_capabilities() {
        let temp_dir = TempDir::new().unwrap();

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Capabilities.Test:v0.1
  type: node:cold
  version: v0.1
capabilities:
  - validation
  - transformation
  - llm-assistance
        "#;

        create_test_ontology(&temp_dir, "Capabilities.Test", ontology);

        let reader = OntologyReader::new(temp_dir.path().to_path_buf());
        let capabilities = reader.read_capabilities("Capabilities.Test").unwrap();

        assert_eq!(capabilities.len(), 3);
        assert!(capabilities.contains(&"validation".to_string()));
        assert!(capabilities.contains(&"transformation".to_string()));
        assert!(capabilities.contains(&"llm-assistance".to_string()));
    }
}
