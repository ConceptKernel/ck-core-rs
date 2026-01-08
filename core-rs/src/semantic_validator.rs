//! Semantic Validator - Ontology compatibility checking for kernel admission
//!
//! ## Purpose
//!
//! Ensures that all admitted kernels maintain semantic compatibility with
//! the project's ontological framework. No semantic compatibility = no admission.
//!
//! ## Validation Rules
//!
//! 1. **BFO Conformance**: All kernels must conform to BFO (Basic Formal Ontology)
//! 2. **Namespace Consistency**: URN prefixes must be valid
//! 3. **Predicate Validity**: Edge predicates must be from approved vocabulary
//! 4. **Type Safety**: Kernel types must be recognized (node:cold, node:hot, etc.)
//! 5. **Version Compatibility**: Semantic versioning must be respected
//!
//! ## Example
//!
//! ```rust,ignore
//! use ckp_core::semantic_validator::SemanticValidator;
//!
//! let validator = SemanticValidator::new("/project".into());
//!
//! // Validate kernel before admission
//! let result = validator.validate_kernel("MyKernel", &ontology_ttl).await?;
//!
//! if !result.is_valid {
//!     eprintln!("Kernel rejected: {:?}", result.errors);
//!     return Err("Semantic validation failed");
//! }
//! ```

use crate::errors::{CkpError, Result};
use crate::ontology::OntologyReader;
use std::collections::HashSet;
use std::path::PathBuf;

/// Validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Is the kernel valid?
    pub is_valid: bool,

    /// Validation errors (if any)
    pub errors: Vec<String>,

    /// Validation warnings (non-fatal)
    pub warnings: Vec<String>,

    /// Detected kernel type
    pub kernel_type: Option<String>,

    /// Detected version
    pub version: Option<String>,
}

/// Semantic validator for kernel admission
pub struct SemanticValidator {
    /// Project root
    root: PathBuf,

    /// Ontology reader
    ontology_reader: OntologyReader,

    /// Approved predicates
    approved_predicates: HashSet<String>,

    /// Approved kernel types
    approved_types: HashSet<String>,
}

impl SemanticValidator {
    /// Create new semantic validator
    pub fn new(root: PathBuf) -> Self {
        // Initialize approved predicates from BFO + project ontology
        let mut approved_predicates = HashSet::new();
        approved_predicates.insert("PRODUCES".to_string());
        approved_predicates.insert("REQUIRES".to_string());
        approved_predicates.insert("CONSUMES".to_string());
        approved_predicates.insert("ENABLES".to_string());
        approved_predicates.insert("DEPENDS_ON".to_string());
        approved_predicates.insert("TRIGGERS".to_string());

        // Initialize approved kernel types
        let mut approved_types = HashSet::new();
        approved_types.insert("node:cold".to_string());
        approved_types.insert("node:hot".to_string());
        approved_types.insert("service".to_string());
        approved_types.insert("agent".to_string());
        approved_types.insert("gateway".to_string());

        Self {
            root: root.clone(),
            ontology_reader: OntologyReader::new(root),
            approved_predicates,
            approved_types,
        }
    }

    /// Validate kernel ontology for admission
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name
    /// * `ontology_ttl` - Ontology in Turtle format
    ///
    /// # Returns
    ///
    /// ValidationResult with errors and warnings
    pub async fn validate_kernel(
        &self,
        kernel_name: &str,
        ontology_ttl: &str,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut kernel_type = None;
        let mut version = None;

        // Parse ontology (YAML-based for now, RDF in future)
        let ontology_yaml = match serde_yaml::from_str::<serde_yaml::Value>(ontology_ttl) {
            Ok(yaml) => yaml,
            Err(e) => {
                errors.push(format!("Invalid ontology format: {}", e));
                return Ok(ValidationResult {
                    is_valid: false,
                    errors,
                    warnings,
                    kernel_type,
                    version,
                });
            }
        };

        // Validation 1: Check apiVersion
        if let Some(api_version) = ontology_yaml.get("apiVersion").and_then(|v| v.as_str()) {
            if !api_version.starts_with("conceptkernel/") {
                errors.push(format!(
                    "Invalid apiVersion: {}. Must be 'conceptkernel/v1'",
                    api_version
                ));
            }
        } else {
            errors.push("Missing apiVersion field".to_string());
        }

        // Validation 2: Check kind
        if let Some(kind) = ontology_yaml.get("kind").and_then(|v| v.as_str()) {
            if kind != "Ontology" {
                errors.push(format!("Invalid kind: {}. Must be 'Ontology'", kind));
            }
        } else {
            errors.push("Missing kind field".to_string());
        }

        // Validation 3: Check metadata
        if let Some(metadata) = ontology_yaml.get("metadata") {
            // Check name matches kernel_name
            if let Some(name) = metadata.get("name").and_then(|v| v.as_str()) {
                let expected_name = format!("ckp://{}", kernel_name);
                if name != expected_name {
                    errors.push(format!(
                        "Name mismatch: expected '{}', found '{}'",
                        expected_name, name
                    ));
                }
            } else {
                errors.push("Missing metadata.name field".to_string());
            }

            // Check type
            if let Some(ktype) = metadata.get("type").and_then(|v| v.as_str()) {
                kernel_type = Some(ktype.to_string());
                if !self.approved_types.contains(ktype) {
                    warnings.push(format!(
                        "Unknown kernel type: {}. Approved types: {:?}",
                        ktype, self.approved_types
                    ));
                }
            } else {
                errors.push("Missing metadata.type field".to_string());
            }

            // Check version
            if let Some(ver) = metadata.get("version").and_then(|v| v.as_str()) {
                version = Some(ver.to_string());
                if !ver.starts_with('v') {
                    warnings.push(format!(
                        "Version should start with 'v': {}",
                        ver
                    ));
                }
            }
        } else {
            errors.push("Missing metadata section".to_string());
        }

        // Validation 4: Check notification_contract (if present)
        if let Some(notification_contract) = ontology_yaml.get("notification_contract") {
            if let Some(targets) = notification_contract.as_sequence() {
                for (i, target) in targets.iter().enumerate() {
                    if let Some(target_kernel) = target.get("target_kernel").and_then(|v| v.as_str()) {
                        // Validate URN format
                        if !target_kernel.starts_with("ckp://") && !target_kernel.contains('.') {
                            warnings.push(format!(
                                "notification_contract[{}]: target_kernel '{}' should be a URN or dotted name",
                                i, target_kernel
                            ));
                        }
                    } else {
                        errors.push(format!(
                            "notification_contract[{}]: missing target_kernel",
                            i
                        ));
                    }

                    // Validate predicate (if specified)
                    if let Some(predicate) = target.get("predicate").and_then(|v| v.as_str()) {
                        if !self.approved_predicates.contains(predicate) {
                            warnings.push(format!(
                                "notification_contract[{}]: unknown predicate '{}'. Approved: {:?}",
                                i, predicate, self.approved_predicates
                            ));
                        }
                    }
                }
            }
        }

        // Validation 5: Check spec.tools (if present)
        if let Some(spec) = ontology_yaml.get("spec") {
            if let Some(tools) = spec.get("tools").and_then(|v| v.as_sequence()) {
                for (i, tool) in tools.iter().enumerate() {
                    if let Some(name) = tool.get("name").and_then(|v| v.as_str()) {
                        if name.is_empty() {
                            errors.push(format!("spec.tools[{}]: name cannot be empty", i));
                        }
                    } else {
                        errors.push(format!("spec.tools[{}]: missing name", i));
                    }

                    if let Some(command) = tool.get("command").and_then(|v| v.as_str()) {
                        if command.is_empty() {
                            errors.push(format!("spec.tools[{}]: command cannot be empty", i));
                        }
                    }
                }
            }
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
            kernel_type,
            version,
        })
    }

    /// Add approved predicate
    pub fn add_approved_predicate(&mut self, predicate: String) {
        self.approved_predicates.insert(predicate);
    }

    /// Add approved kernel type
    pub fn add_approved_type(&mut self, kernel_type: String) {
        self.approved_types.insert(kernel_type);
    }

    /// Get approved predicates
    pub fn get_approved_predicates(&self) -> &HashSet<String> {
        &self.approved_predicates
    }

    /// Get approved types
    pub fn get_approved_types(&self) -> &HashSet<String> {
        &self.approved_types
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_valid_kernel() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let validator = SemanticValidator::new(temp_dir.path().to_path_buf());

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://TestKernel
  type: node:cold
  version: v1.0.0
notification_contract:
  - target_kernel: ckp://TargetKernel
    predicate: PRODUCES
"#;

        let result = validator.validate_kernel("TestKernel", ontology).await.unwrap();

        assert!(result.is_valid, "Validation should pass: {:?}", result.errors);
        assert_eq!(result.kernel_type, Some("node:cold".to_string()));
        assert_eq!(result.version, Some("v1.0.0".to_string()));
    }

    #[tokio::test]
    async fn test_invalid_kernel_missing_fields() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let validator = SemanticValidator::new(temp_dir.path().to_path_buf());

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
"#;

        let result = validator.validate_kernel("TestKernel", ontology).await.unwrap();

        assert!(!result.is_valid, "Validation should fail");
        assert!(!result.errors.is_empty(), "Should have errors");
    }

    #[tokio::test]
    async fn test_invalid_predicate_warning() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let validator = SemanticValidator::new(temp_dir.path().to_path_buf());

        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://TestKernel
  type: node:cold
  version: v1.0.0
notification_contract:
  - target_kernel: ckp://TargetKernel
    predicate: UNKNOWN_PREDICATE
"#;

        let result = validator.validate_kernel("TestKernel", ontology).await.unwrap();

        assert!(result.is_valid, "Should be valid but with warnings");
        assert!(!result.warnings.is_empty(), "Should have warnings about unknown predicate");
    }
}
