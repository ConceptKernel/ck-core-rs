//! Kernel metadata structures
//!
//! Defines data structures for kernel configuration that can be serialized
//! to both YAML (for file storage) and RDF (for Jena storage).
//!
//! Design: URNs are self-describing. We store minimal data in protocol-level
//! files (conceptkernel.yaml) and parse details from URN when needed.

use serde::{Deserialize, Serialize};

/// Default API version for KernelMetadata
fn default_api_version() -> String {
    "conceptkernel/v1".to_string()
}

/// Default kind for KernelMetadata
fn default_kind() -> String {
    "Ontology".to_string()
}

/// Kernel metadata representing a ConceptKernel
///
/// File format: conceptkernel.yaml (Kubernetes-style resource definition)
///
/// ## Protocol-Level Storage (conceptkernel.yaml)
///
/// Minimal YAML with only essential runtime configuration.
/// Kernel name, domain, version parsed from URN.
///
/// ## Example YAML (Protocol Level)
///
/// ```yaml
/// apiVersion: conceptkernel/v1
/// kind: Ontology
/// metadata:
///   name: Usecase.DataPipeline.Ingester
///   urn: "ckp://Usecase.DataPipeline.Ingester:v1.0.0"
///   domain: Usecase.DataPipeline
///   type: rust:cold
///   version: v1.0.0
///   template: false
///   forkable: false
/// spec:
///   runtime: batch
///   port: auto
///   description: "Ingests data from queue"
///   capabilities:
///     - "ckp:permission-read-queue"
///     - "ckp:permission-write-storage"
/// ```
///
/// ## Design Rationale
///
/// - URNs contain kernel identity (name:version)
/// - Type (rust:cold, python:hot, etc.) determines execution model
/// - Runtime (batch, daemon, serverless) determines lifecycle
/// - Capabilities are RBAC permissions required by kernel
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KernelMetadata {
    /// API version (Kubernetes-style)
    #[serde(default = "default_api_version")]
    pub api_version: String,

    /// Resource kind (always "Ontology" for kernels)
    #[serde(default = "default_kind")]
    pub kind: String,

    /// Kernel metadata section
    pub metadata: KernelMetadataInfo,

    /// Kernel specification section
    pub spec: KernelSpec,
}

/// Kernel metadata information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KernelMetadataInfo {
    /// Kernel name (e.g., "Usecase.DataPipeline.Ingester")
    pub name: String,

    /// Full kernel URN (e.g., "ckp://Usecase.DataPipeline.Ingester:v1.0.0")
    pub urn: String,

    /// Kernel domain (e.g., "Usecase.DataPipeline")
    pub domain: String,

    /// Kernel type (rust:cold, python:hot, nodejs:warm, etc.)
    #[serde(rename = "type")]
    pub kernel_type: String,

    /// Kernel version (e.g., "v1.0.0")
    pub version: String,

    /// Whether this kernel is a template
    #[serde(default)]
    pub template: bool,

    /// Whether this kernel can be forked
    #[serde(default)]
    pub forkable: bool,

    /// Optional: What template this was forked from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<String>,

    /// Optional: Description of fork purpose
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fork_description: Option<String>,

    /// Optional: Entrypoint for hot kernels (e.g., "tool/main.py", "tool/tool.js")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
}

/// Kernel specification section
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KernelSpec {
    /// Runtime mode: batch, daemon, serverless, passthrough
    pub runtime: String,

    /// Port allocation: auto, specific number, or "none"
    pub port: String,

    /// Human-readable description
    pub description: String,

    /// Required RBAC capabilities
    #[serde(default)]
    pub capabilities: Vec<String>,

    /// Optional: Notification contract for edges
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_contract: Option<Vec<NotificationTarget>>,

    /// Optional: Queue contract for edges
    #[serde(skip_serializing_if = "Option::is_none")]
    pub queue_contract: Option<QueueContract>,
}

/// Notification target in notification contract
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationTarget {
    /// Target kernel name or URN
    pub target_kernel: String,

    /// Target queue (inbox, outbox, etc.)
    pub queue: String,

    /// Edge predicate (PRODUCES, NOTIFIES, etc.)
    pub predicate: String,
}

/// Queue contract with edge definitions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueueContract {
    /// Edge URNs this kernel participates in
    pub edges: Vec<String>,
}

impl KernelMetadata {
    /// Create new kernel metadata
    ///
    /// # Arguments
    /// * `name` - Kernel name (e.g., "Usecase.DataPipeline.Ingester")
    /// * `kernel_type` - Type (rust:cold, python:hot, etc.)
    /// * `version` - Version string (e.g., "v1.0.0")
    /// * `runtime` - Runtime mode (batch, daemon, serverless, passthrough)
    /// * `description` - Human-readable description
    ///
    /// # Returns
    /// KernelMetadata with generated URN and default values
    pub fn new(
        name: &str,
        kernel_type: &str,
        version: &str,
        runtime: &str,
        description: &str,
    ) -> Self {
        let urn = format!("ckp://{}:{}", name, version);
        let domain = name.rsplitn(2, '.').last().unwrap_or(name).to_string();

        KernelMetadata {
            api_version: default_api_version(),
            kind: default_kind(),
            metadata: KernelMetadataInfo {
                name: name.to_string(),
                urn,
                domain,
                kernel_type: kernel_type.to_string(),
                version: version.to_string(),
                template: false,
                forkable: false,
                forked_from: None,
                fork_description: None,
                entrypoint: None,
            },
            spec: KernelSpec {
                runtime: runtime.to_string(),
                port: "auto".to_string(),
                description: description.to_string(),
                capabilities: Vec::new(),
                notification_contract: None,
                queue_contract: None,
            },
        }
    }

    /// Create kernel metadata by forking from a template
    ///
    /// # Arguments
    /// * `name` - New kernel name
    /// * `version` - New kernel version
    /// * `template_urn` - URN of template to fork from
    /// * `runtime` - Runtime mode override (or use template's runtime)
    /// * `description` - Description of new kernel
    ///
    /// # Returns
    /// KernelMetadata forked from template
    pub fn fork_from(
        name: &str,
        version: &str,
        template_urn: &str,
        runtime: Option<&str>,
        description: &str,
    ) -> Self {
        let urn = format!("ckp://{}:{}", name, version);
        let domain = name.rsplitn(2, '.').last().unwrap_or(name).to_string();

        // Parse template type from URN if available
        // For now, default to rust:cold for Template.Basic
        let kernel_type = if template_urn.contains("Template.Basic") {
            "rust:cold"
        } else {
            "python:hot" // fallback
        };

        KernelMetadata {
            api_version: default_api_version(),
            kind: default_kind(),
            metadata: KernelMetadataInfo {
                name: name.to_string(),
                urn,
                domain,
                kernel_type: kernel_type.to_string(),
                version: version.to_string(),
                template: false,
                forkable: true, // Forked kernels can be forked again
                forked_from: Some(template_urn.to_string()),
                fork_description: Some(format!("Forked from {}", template_urn)),
                entrypoint: None,
            },
            spec: KernelSpec {
                runtime: runtime.unwrap_or("batch").to_string(),
                port: "auto".to_string(),
                description: description.to_string(),
                capabilities: Vec::new(),
                notification_contract: None,
                queue_contract: None,
            },
        }
    }

    /// Add a capability to the kernel
    pub fn add_capability(&mut self, capability: &str) {
        if !self.spec.capabilities.contains(&capability.to_string()) {
            self.spec.capabilities.push(capability.to_string());
        }
    }

    /// Set capabilities from a list
    pub fn set_capabilities(&mut self, capabilities: Vec<String>) {
        self.spec.capabilities = capabilities;
    }

    /// Serialize to YAML string
    ///
    /// # Returns
    /// Result containing YAML string or serialization error
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Deserialize from YAML string
    ///
    /// # Arguments
    /// * `yaml` - YAML string to parse
    ///
    /// # Returns
    /// Result containing KernelMetadata or deserialization error
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Convert to RDF triples format (Turtle)
    ///
    /// # Returns
    /// String containing RDF Turtle representation
    pub fn to_rdf(&self) -> String {
        format!(
            r#"@prefix ckp: <https://conceptkernel.org/ontology#> .
@prefix bfo: <http://purl.obolibrary.org/obo/BFO_> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .

<{urn}>
    a bfo:BFO_0000040 ;
    rdfs:label "{name}" ;
    ckp:domain "{domain}" ;
    ckp:type "{kernel_type}" ;
    ckp:version "{version}" ;
    ckp:runtime "{runtime}" ;
    ckp:description "{description}" .
"#,
            urn = self.metadata.urn,
            name = self.metadata.name,
            domain = self.metadata.domain,
            kernel_type = self.metadata.kernel_type,
            version = self.metadata.version,
            runtime = self.spec.runtime,
            description = self.spec.description,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_kernel_metadata() {
        let metadata = KernelMetadata::new(
            "Usecase.DataPipeline.Ingester",
            "rust:cold",
            "v1.0.0",
            "batch",
            "Ingests data from queue",
        );

        assert_eq!(metadata.metadata.name, "Usecase.DataPipeline.Ingester");
        assert_eq!(metadata.metadata.kernel_type, "rust:cold");
        assert_eq!(metadata.spec.runtime, "batch");
    }

    #[test]
    fn test_fork_from_template() {
        let metadata = KernelMetadata::fork_from(
            "Usecase.DataPipeline.Validator",
            "v1.0.0",
            "ckp://ConceptKernel.Template.Basic:1.1.0",
            Some("batch"),
            "Validates processed data",
        );

        assert_eq!(metadata.metadata.kernel_type, "rust:cold");
        assert_eq!(
            metadata.metadata.forked_from,
            Some("ckp://ConceptKernel.Template.Basic:1.1.0".to_string())
        );
    }

    #[test]
    fn test_yaml_serialization() {
        let metadata = KernelMetadata::new(
            "Test.Kernel",
            "rust:cold",
            "v1.0.0",
            "batch",
            "Test kernel",
        );

        let yaml = metadata.to_yaml().unwrap();
        assert!(yaml.contains("apiVersion: conceptkernel/v1"));
        assert!(yaml.contains("kind: Ontology"));
        assert!(yaml.contains("name: Test.Kernel"));

        // Round-trip test
        let parsed = KernelMetadata::from_yaml(&yaml).unwrap();
        assert_eq!(parsed.metadata.name, "Test.Kernel");
    }
}
