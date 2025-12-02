//! Edge metadata structures
//!
//! Defines data structures for edge relationships between kernels
//!
//! Reference: Node.js v1.3.14 - EdgeKernel.js

use serde::{Deserialize, Serialize};

/// Default API version for EdgeMetadata
fn default_api_version() -> String {
    "conceptkernel/v1".to_string()
}

/// Default kind for EdgeMetadata
fn default_kind() -> String {
    "Edge".to_string()
}

/// Edge metadata representing a relationship between two kernels
///
/// File format: edgekernel.yaml (Kubernetes-style resource definition)
///
/// ## Protocol-Level Storage (edgekernel.yaml)
///
/// Kubernetes-style resource definition with minimal data.
/// Only `apiVersion`, `kind`, `urn` and `created_at` are serialized.
/// All other fields (predicate, source, target, version) are parsed from URN.
///
/// ## Example YAML (Protocol Level)
///
/// ```yaml
/// apiVersion: conceptkernel/v1
/// kind: Edge
/// urn: "ckp://Edge.PRODUCES.System.Consensus-to-System.Proof:v1.3.16"
/// createdAt: "2025-11-29T21:27:59.255335+00:00"
/// ```
///
/// ## Design Rationale
///
/// URNs are self-describing - we don't duplicate parseable information.
/// However, we keep fields in the struct for performance (avoid repeated parsing).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EdgeMetadata {
    /// API version (Kubernetes-style)
    #[serde(default = "default_api_version")]
    pub api_version: String,

    /// Resource kind (always "Edge")
    #[serde(default = "default_kind")]
    pub kind: String,

    /// Full edge URN (e.g., "ckp://Edge.PRODUCES.Source-to-Target:v1.3.16")
    pub urn: String,

    /// Creation timestamp (ISO 8601)
    pub created_at: String,

    /// Edge predicate (PRODUCES, NOTIFIES, VALIDATES, TRIGGERS, etc.)
    /// Parsed from URN, not serialized to protocol level
    #[serde(skip_serializing, skip_deserializing)]
    pub predicate: String,

    /// Source kernel name (e.g., "MixIngredients")
    /// Parsed from URN, not serialized to protocol level
    #[serde(skip_serializing, skip_deserializing)]
    pub source: String,

    /// Target kernel name (e.g., "BakeCake")
    /// Parsed from URN, not serialized to protocol level
    #[serde(skip_serializing, skip_deserializing)]
    pub target: String,

    /// Edge version
    /// Parsed from URN, not serialized to protocol level
    #[serde(skip_serializing, skip_deserializing)]
    pub version: String,
}

impl EdgeMetadata {
    /// Create new edge metadata
    ///
    /// # Arguments
    /// * `predicate` - Edge predicate (PRODUCES, NOTIFIES, etc.)
    /// * `source` - Source kernel name
    /// * `target` - Target kernel name
    /// * `version` - Version string (e.g., "v1.3.14")
    ///
    /// # Returns
    /// EdgeMetadata with generated URN and timestamp
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::edge::EdgeMetadata;
    ///
    /// let metadata = EdgeMetadata::new(
    ///     "PRODUCES",
    ///     "MixIngredients",
    ///     "BakeCake",
    ///     "v1.3.14"
    /// );
    ///
    /// assert_eq!(metadata.predicate, "PRODUCES");
    /// assert_eq!(metadata.source, "MixIngredients");
    /// assert_eq!(metadata.target, "BakeCake");
    /// ```
    pub fn new(predicate: &str, source: &str, target: &str, version: &str) -> Self {
        let urn = Self::generate_urn(predicate, source, target, version);
        let created_at = chrono::Utc::now().to_rfc3339();

        EdgeMetadata {
            api_version: default_api_version(),
            kind: default_kind(),
            urn,
            created_at,
            predicate: predicate.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            version: version.to_string(),
        }
    }

    /// Parse URN and populate fields
    ///
    /// Parses a URN like "ckp://Edge.PREDICATE.Source-to-Target:version"
    /// into its components.
    ///
    /// # Arguments
    /// * `urn` - Full edge URN
    ///
    /// # Returns
    /// Result containing (predicate, source, target, version) tuple
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::edge::EdgeMetadata;
    ///
    /// let (pred, src, tgt, ver) = EdgeMetadata::parse_urn(
    ///     "ckp://Edge.PRODUCES.System.Consensus-to-System.Proof:v1.3.14"
    /// ).unwrap();
    ///
    /// assert_eq!(pred, "PRODUCES");
    /// assert_eq!(src, "System.Consensus");
    /// assert_eq!(tgt, "System.Proof");
    /// assert_eq!(ver, "v1.3.14");
    /// ```
    pub fn parse_urn(urn: &str) -> Result<(String, String, String, String), String> {
        // Format: ckp://Edge.PREDICATE.Source-to-Target:version

        // Remove protocol prefix
        let without_protocol = urn.strip_prefix("ckp://Edge.")
            .ok_or("Invalid URN: missing 'ckp://Edge.' prefix")?;

        // Split on first dot to get predicate
        let parts: Vec<&str> = without_protocol.splitn(2, '.').collect();
        if parts.len() != 2 {
            return Err("Invalid URN: missing predicate".to_string());
        }

        let predicate = parts[0].to_string();
        let rest = parts[1];

        // Split on ':' to separate source-to-target from version
        let parts: Vec<&str> = rest.rsplitn(2, ':').collect();
        if parts.len() != 2 {
            return Err("Invalid URN: missing version".to_string());
        }

        let version = parts[0].to_string();
        let source_to_target = parts[1];

        // Split on '-to-' to get source and target
        let parts: Vec<&str> = source_to_target.split("-to-").collect();
        if parts.len() != 2 {
            return Err("Invalid URN: missing '-to-' separator".to_string());
        }

        let source = parts[0].to_string();
        let target = parts[1].to_string();

        Ok((predicate, source, target, version))
    }

    /// Create metadata from URN and timestamp
    ///
    /// Parses URN to populate all fields.
    ///
    /// # Arguments
    /// * `urn` - Full edge URN
    /// * `created_at` - ISO 8601 timestamp
    ///
    /// # Returns
    /// EdgeMetadata with parsed fields
    pub fn from_urn(urn: String, created_at: String) -> Result<Self, String> {
        let (predicate, source, target, version) = Self::parse_urn(&urn)?;

        Ok(EdgeMetadata {
            api_version: default_api_version(),
            kind: default_kind(),
            urn,
            created_at,
            predicate,
            source,
            target,
            version,
        })
    }

    /// Generate edge URN
    ///
    /// Format: ckp://Edge.{PREDICATE}.{Source}-to-{Target}:{version}
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::edge::EdgeMetadata;
    ///
    /// let urn = EdgeMetadata::generate_urn(
    ///     "PRODUCES",
    ///     "MixIngredients",
    ///     "BakeCake",
    ///     "v1.3.14"
    /// );
    ///
    /// assert_eq!(urn, "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.14");
    /// ```
    pub fn generate_urn(predicate: &str, source: &str, target: &str, version: &str) -> String {
        format!("ckp://Edge.{}.{}-to-{}:{}", predicate, source, target, version)
    }

    /// Get edge name (predicate.source)
    ///
    /// Used for directory naming in per-edge queues
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::edge::EdgeMetadata;
    ///
    /// let metadata = EdgeMetadata::new("PRODUCES", "MixIngredients", "BakeCake", "v1.3.14");
    /// assert_eq!(metadata.get_edge_name(), "PRODUCES.MixIngredients");
    /// ```
    pub fn get_edge_name(&self) -> String {
        format!("{}.{}", self.predicate, self.source)
    }

    /// Load EdgeMetadata from YAML string
    ///
    /// Deserializes minimal YAML (apiVersion, kind, urn + createdAt) and parses URN to populate fields.
    ///
    /// # Arguments
    /// * `yaml_str` - YAML string containing edge metadata
    ///
    /// # Returns
    /// Parsed EdgeMetadata with all fields populated
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::edge::EdgeMetadata;
    ///
    /// let yaml = r#"
    /// apiVersion: conceptkernel/v1
    /// kind: Edge
    /// urn: "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.14"
    /// createdAt: "2025-11-29T21:27:59Z"
    /// "#;
    ///
    /// let metadata = EdgeMetadata::from_yaml(yaml).unwrap();
    /// assert_eq!(metadata.predicate, "PRODUCES");
    /// assert_eq!(metadata.source, "MixIngredients");
    /// assert_eq!(metadata.target, "BakeCake");
    /// ```
    pub fn from_yaml(yaml_str: &str) -> Result<Self, String> {
        // Deserialize minimal YAML
        let minimal: Self = serde_yaml::from_str(yaml_str)
            .map_err(|e| format!("Failed to parse YAML: {}", e))?;

        // Parse URN to populate other fields
        Self::from_urn(minimal.urn, minimal.created_at)
    }


    /// Convert EdgeMetadata to YAML string
    ///
    /// Serializes to minimal YAML (apiVersion, kind, urn, createdAt only)
    ///
    /// # Returns
    /// YAML string representation
    pub fn to_yaml(&self) -> Result<String, String> {
        serde_yaml::to_string(self)
            .map_err(|e| format!("Failed to serialize to YAML: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_metadata_creation() {
        let metadata = EdgeMetadata::new("PRODUCES", "MixIngredients", "BakeCake", "v1.3.14");

        assert_eq!(metadata.predicate, "PRODUCES");
        assert_eq!(metadata.source, "MixIngredients");
        assert_eq!(metadata.target, "BakeCake");
        assert_eq!(metadata.version, "v1.3.14");
        assert!(!metadata.created_at.is_empty());
        assert!(metadata.urn.contains("Edge.PRODUCES.MixIngredients-to-BakeCake"));
    }

    #[test]
    fn test_edge_urn_generation() {
        let urn = EdgeMetadata::generate_urn("PRODUCES", "MixIngredients", "BakeCake", "v1.3.14");
        assert_eq!(urn, "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.14");
    }

    #[test]
    fn test_edge_urn_generation_different_predicates() {
        let urn1 = EdgeMetadata::generate_urn("NOTIFIES", "Source", "Target", "v1.0.0");
        assert_eq!(urn1, "ckp://Edge.NOTIFIES.Source-to-Target:v1.0.0");

        let urn2 = EdgeMetadata::generate_urn("VALIDATES", "Proof", "Claim", "v1.0.0");
        assert_eq!(urn2, "ckp://Edge.VALIDATES.Proof-to-Claim:v1.0.0");
    }

    #[test]
    fn test_edge_metadata_serialization_yaml() {
        let metadata = EdgeMetadata::new("PRODUCES", "Source", "Target", "v1.0.0");

        // Serialize to YAML - should only contain apiVersion, kind, URN and createdAt
        let yaml = metadata.to_yaml().unwrap();
        assert!(yaml.contains("apiVersion:"));
        assert!(yaml.contains("conceptkernel/v1"));
        assert!(yaml.contains("kind:"));
        assert!(yaml.contains("Edge"));
        assert!(yaml.contains("urn:"));
        assert!(yaml.contains("createdAt:"));

        // Should NOT contain redundant fields
        assert!(!yaml.contains("predicate:"));
        assert!(!yaml.contains("source:"));
        assert!(!yaml.contains("target:"));
        assert!(!yaml.contains("version:"));

        // Deserialize using from_yaml - should parse URN and populate fields
        let deserialized = EdgeMetadata::from_yaml(&yaml).unwrap();
        assert_eq!(deserialized.api_version, "conceptkernel/v1");
        assert_eq!(deserialized.kind, "Edge");
        assert_eq!(deserialized.predicate, metadata.predicate);
        assert_eq!(deserialized.source, metadata.source);
        assert_eq!(deserialized.target, metadata.target);
        assert_eq!(deserialized.version, metadata.version);
    }


    #[test]
    fn test_urn_parsing() {
        let urn = "ckp://Edge.PRODUCES.System.Consensus-to-System.Proof:v1.3.14";
        let (pred, src, tgt, ver) = EdgeMetadata::parse_urn(urn).unwrap();

        assert_eq!(pred, "PRODUCES");
        assert_eq!(src, "System.Consensus");
        assert_eq!(tgt, "System.Proof");
        assert_eq!(ver, "v1.3.14");
    }

    #[test]
    fn test_from_yaml_minimal() {
        let yaml = r#"
apiVersion: conceptkernel/v1
kind: Edge
urn: "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.14"
createdAt: "2025-11-29T21:27:59Z"
"#;

        let metadata = EdgeMetadata::from_yaml(yaml).unwrap();
        assert_eq!(metadata.api_version, "conceptkernel/v1");
        assert_eq!(metadata.kind, "Edge");
        assert_eq!(metadata.urn, "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.14");
        assert_eq!(metadata.predicate, "PRODUCES");
        assert_eq!(metadata.source, "MixIngredients");
        assert_eq!(metadata.target, "BakeCake");
        assert_eq!(metadata.version, "v1.3.14");
        assert_eq!(metadata.created_at, "2025-11-29T21:27:59Z");
    }


    #[test]
    fn test_get_edge_name() {
        let metadata = EdgeMetadata::new("PRODUCES", "MixIngredients", "BakeCake", "v1.3.14");
        assert_eq!(metadata.get_edge_name(), "PRODUCES.MixIngredients");
    }
}
