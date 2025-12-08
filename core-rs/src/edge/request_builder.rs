//! Edge request builder for routing notifications through edge kernels
//!
//! Creates `.edgereq` files that EdgeKernel processes to route instances
//! to downstream kernels via per-edge queues.
//!
//! Reference: Node.js v1.3.14 - EdgeRequestBuilder.js

use crate::errors::{CkpError, Result};
use chrono::{Datelike, Local, Timelike};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Edge request builder for creating routing requests
pub struct EdgeRequestBuilder {
    /// Project root directory
    root: PathBuf,

    /// Domain name from config
    _domain: String,
}

/// Edge routing request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeRequest {
    /// Request ID (transaction ID with .edgereq extension)
    #[serde(rename = "requestId")]
    pub request_id: String,

    /// Source information
    pub source: EdgeSource,

    /// Target information
    pub target: EdgeTarget,

    /// Relationship type predicate
    #[serde(rename = "type")]
    pub relationship_type: String,

    /// Additional properties
    pub properties: HashMap<String, Value>,
}

/// Source kernel and instance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSource {
    /// Source kernel URN
    pub kernel: String,

    /// Source instance URN (full path)
    pub instance: String,
}

/// Target kernel and queue information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeTarget {
    /// Target kernel URN
    pub kernel: String,

    /// Target queue URN (always inbox)
    pub queue: String,
}

/// Notification contract entry from ontology
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEntry {
    /// Target kernel URN
    pub target: String,

    /// Relationship type (default: PRODUCES)
    #[serde(rename = "type")]
    pub relationship_type: Option<String>,

    /// Custom properties
    pub properties: Option<HashMap<String, Value>>,
}

impl EdgeRequestBuilder {
    /// Create a new EdgeRequestBuilder
    ///
    /// # Arguments
    ///
    /// * `root` - Project root directory
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::edge::EdgeRequestBuilder;
    /// use std::path::PathBuf;
    ///
    /// let builder = EdgeRequestBuilder::new(PathBuf::from("/my-project"));
    /// ```
    pub fn new(root: PathBuf) -> Self {
        // Load domain from .ckproject (v1.3.16+)
        let domain = Self::load_domain_from_project(&root);

        Self { root, _domain: domain }
    }

    /// Load domain from .ckproject
    fn load_domain_from_project(root: &Path) -> String {
        use crate::project::ProjectConfig;

        let ckproject_path = root.join(".ckproject");

        if let Ok(config) = ProjectConfig::load(&ckproject_path) {
            config.spec.domain
        } else {
            // Fallback to default domain
            "Org.ConceptKernel".to_string()
        }
    }

    /// Build a CKP URN
    ///
    /// # Arguments
    ///
    /// * `kernel` - Kernel name
    /// * `version` - Version (default: "v0.1")
    /// * `stage` - Stage shortcut (inbox, storage, etc.)
    /// * `path` - Path component after stage
    ///
    /// # Returns
    ///
    /// URN string in format `ckp://{kernel}:{version}[#{stage}[/{path}]]`
    ///
    /// # Example
    ///
    /// ```
    /// # use ckp_core::edge::EdgeRequestBuilder;
    /// # use std::path::PathBuf;
    /// let builder = EdgeRequestBuilder::new(PathBuf::from("/tmp"));
    ///
    /// assert_eq!(
    ///     builder.build_ckp_uri("BakeCake", None, None, None),
    ///     "ckp://BakeCake:v0.1"
    /// );
    ///
    /// assert_eq!(
    ///     builder.build_ckp_uri("BakeCake", Some("v1.0"), Some("storage"), None),
    ///     "ckp://BakeCake:v1.0#storage"
    /// );
    /// ```
    pub fn build_ckp_uri(
        &self,
        kernel: &str,
        version: Option<&str>,
        stage: Option<&str>,
        path: Option<&str>,
    ) -> String {
        let version = version.unwrap_or("v0.1");
        let mut uri = format!("ckp://{}:{}", kernel, version);

        if let Some(stage) = stage {
            uri.push('#');
            uri.push_str(stage);

            if let Some(path) = path {
                uri.push('/');
                uri.push_str(path);
            }
        }

        uri
    }

    /// Generate transaction ID in format yyMMddHHmmssf-shortId
    ///
    /// # Returns
    ///
    /// Transaction ID string (e.g., "2511211530456-c8788f41")
    ///
    /// # Format
    ///
    /// - yy: 2-digit year
    /// - MM: 2-digit month
    /// - dd: 2-digit day
    /// - HH: 2-digit hour
    /// - mm: 2-digit minute
    /// - ss: 2-digit second
    /// - f: 1-digit decisecond (milliseconds / 100)
    /// - shortId: 8-character hex string
    pub fn generate_tx_id(&self) -> String {
        let now = Local::now();

        let yy = format!("{:02}", now.year() % 100);
        let mm = format!("{:02}", now.month());
        let dd = format!("{:02}", now.day());
        let hh = format!("{:02}", now.hour());
        let min = format!("{:02}", now.minute());
        let ss = format!("{:02}", now.second());
        let f = format!("{}", now.timestamp_subsec_millis() / 100);

        // Generate 8-character hex short ID
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let random_bytes: [u8; 4] = rng.gen();
        let short_id = hex::encode(random_bytes);

        format!("{}{}{}{}{}{}{}-{}", yy, mm, dd, hh, min, ss, f, short_id)
    }

    /// Create edge requests for all notifications in contract
    ///
    /// # Arguments
    ///
    /// * `source_kernel` - Source kernel name
    /// * `source_instance_path` - Path to minted instance
    /// * `notification_contract` - Array of notification entries
    ///
    /// # Returns
    ///
    /// Result indicating success or error
    ///
    /// # Behavior
    ///
    /// - Returns early if notification_contract is empty
    /// - Logs warnings for invalid URIs or missing edge kernels
    /// - Continues processing on errors (does not throw)
    pub async fn create_edge_requests(
        &self,
        source_kernel: &str,
        source_instance_path: &Path,
        notification_contract: &[NotificationEntry],
    ) -> Result<()> {
        if notification_contract.is_empty() {
            return Ok(());
        }

        for notif in notification_contract {
            if let Err(e) = self
                .create_single_edge_request(source_kernel, source_instance_path, notif)
                .await
            {
                eprintln!("[EdgeRequestBuilder] Error creating edge request: {}", e);
                // Continue processing remaining notifications
            }
        }

        Ok(())
    }

    // ===== PRIVATE HELPER METHODS =====

    /// Create a single edge request
    async fn create_single_edge_request(
        &self,
        source_kernel: &str,
        source_instance_path: &Path,
        notif: &NotificationEntry,
    ) -> Result<()> {
        // Extract relationship type (default: PRODUCES)
        let relationship_type = notif
            .relationship_type
            .as_deref()
            .unwrap_or("PRODUCES")
            .to_string();

        // Parse target URN to extract kernel name
        let target_kernel = self.extract_target_kernel(&notif.target)?;

        // Construct edge kernel name: {type}.{source}-to-{target}
        let edge_kernel_name = format!("{}.{}-to-{}", relationship_type, source_kernel, target_kernel);

        // Validate edge kernel inbox exists
        let edge_inbox = self
            .root
            .join("concepts/.edges")
            .join(&edge_kernel_name)
            .join("queue/inbox");

        if !edge_inbox.exists() {
            eprintln!(
                "[EdgeRequestBuilder] Warning: Edge kernel inbox not found: {}",
                edge_inbox.display()
            );
            return Ok(()); // Skip this notification
        }

        // Generate transaction ID
        let tx_id = self.generate_tx_id();
        let request_id = format!("{}.edgereq", tx_id);

        // Build URNs
        let source_kernel_urn = self.build_ckp_uri(source_kernel, None, None, None);

        // Extract instance filename from path
        let instance_filename = source_instance_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| CkpError::ParseError("Invalid instance path".to_string()))?;

        let source_instance_urn = self.build_ckp_uri(
            source_kernel,
            None,
            Some("storage"),
            Some(instance_filename),
        );

        let target_queue_urn = self.build_ckp_uri(&target_kernel, None, Some("inbox"), None);

        // Build properties
        let mut properties = notif.properties.clone().unwrap_or_default();
        properties.insert(
            "timestamp".to_string(),
            Value::String(chrono::Utc::now().to_rfc3339()),
        );

        // Build edge request
        let request = EdgeRequest {
            request_id: request_id.clone(),
            source: EdgeSource {
                kernel: source_kernel_urn,
                instance: source_instance_urn,
            },
            target: EdgeTarget {
                kernel: notif.target.clone(),
                queue: target_queue_urn,
            },
            relationship_type,
            properties,
        };

        // Write .edgereq file
        let request_path = edge_inbox.join(&request_id);
        let request_json = serde_json::to_string_pretty(&request)
            .map_err(|e| CkpError::Json(e))?;

        fs::write(&request_path, request_json)
            .map_err(|e| CkpError::IoError(format!("Failed to write edge request: {}", e)))?;

        println!(
            "[EdgeRequestBuilder] Created edge request: {}",
            request_path.display()
        );

        Ok(())
    }

    /// Extract target kernel name from CKP URI
    ///
    /// Handles both `ckp://Kernel:v0.1` and `ckp://Domain:Kernel:v0.1`
    fn extract_target_kernel(&self, target_ckp_uri: &str) -> Result<String> {
        let re = Regex::new(r"ckp://(?:[^:]+:)?([^:]+):").unwrap();

        re.captures(target_ckp_uri)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| {
                CkpError::UrnParse(format!("Invalid target ckp:// URI: {}", target_ckp_uri))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        assert_eq!(builder._domain, "Org.ConceptKernel");
    }

    #[test]
    fn test_build_ckp_uri_basic() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        let uri = builder.build_ckp_uri("BakeCake", None, None, None);
        assert_eq!(uri, "ckp://BakeCake:v0.1");
    }

    #[test]
    fn test_build_ckp_uri_with_version() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        let uri = builder.build_ckp_uri("BakeCake", Some("v1.0"), None, None);
        assert_eq!(uri, "ckp://BakeCake:v1.0");
    }

    #[test]
    fn test_build_ckp_uri_with_stage() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        let uri = builder.build_ckp_uri("BakeCake", Some("v0.1"), Some("storage"), None);
        assert_eq!(uri, "ckp://BakeCake:v0.1#storage");
    }

    #[test]
    fn test_build_ckp_uri_with_path() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        let uri = builder.build_ckp_uri(
            "BakeCake",
            Some("v0.1"),
            Some("storage"),
            Some("tx-123.inst"),
        );
        assert_eq!(uri, "ckp://BakeCake:v0.1#storage/tx-123.inst");
    }

    #[test]
    fn test_generate_tx_id_format() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        let tx_id = builder.generate_tx_id();

        // Format: yyMMddHHmmssf-shortId
        assert!(tx_id.len() >= 21); // 13 digits + dash + 8 hex chars
        assert!(tx_id.contains('-'));

        let parts: Vec<&str> = tx_id.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].len(), 13); // yyMMddHHmmssf
        assert_eq!(parts[1].len(), 8); // hex shortId
    }

    #[test]
    fn test_extract_target_kernel_simple() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        let result = builder.extract_target_kernel("ckp://BakeCake:v0.1");
        assert_eq!(result.unwrap(), "BakeCake");
    }

    #[test]
    fn test_extract_target_kernel_with_domain() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        let result = builder.extract_target_kernel("ckp://Org:BakeCake:v0.1");
        assert_eq!(result.unwrap(), "BakeCake");
    }

    #[test]
    fn test_extract_target_kernel_invalid() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        let result = builder.extract_target_kernel("invalid");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_edge_requests_empty() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        let instance_path = temp_dir.path().join("concepts/Source/storage/tx-123.inst");
        let result = builder
            .create_edge_requests("Source", &instance_path, &[])
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_single_edge_request() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Setup edge kernel inbox
        let edge_inbox = root.join("concepts/.edges/PRODUCES.Source-to-Target/queue/inbox");
        fs::create_dir_all(&edge_inbox).unwrap();

        // Create builder
        let builder = EdgeRequestBuilder::new(root.to_path_buf());

        // Create notification
        let notif = NotificationEntry {
            target: "ckp://Target:v0.1".to_string(),
            relationship_type: Some("PRODUCES".to_string()),
            properties: None,
        };

        // Create instance file
        let instance_path = root.join("concepts/Source/storage/tx-123.inst");
        fs::create_dir_all(instance_path.parent().unwrap()).unwrap();
        fs::write(&instance_path, "dummy").unwrap();

        // Execute
        let result = builder
            .create_edge_requests("Source", &instance_path, &[notif])
            .await;

        assert!(result.is_ok());

        // Verify .edgereq file created
        let edgereq_files: Vec<_> = fs::read_dir(&edge_inbox)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    == Some("edgereq")
            })
            .collect();

        assert_eq!(edgereq_files.len(), 1);

        // Verify JSON content
        let content = fs::read_to_string(edgereq_files[0].path()).unwrap();
        let request: EdgeRequest = serde_json::from_str(&content).unwrap();

        assert_eq!(request.relationship_type, "PRODUCES");
        assert_eq!(request.source.kernel, "ckp://Source:v0.1");
        assert_eq!(request.target.kernel, "ckp://Target:v0.1");
        assert_eq!(request.source.instance, "ckp://Source:v0.1#storage/tx-123.inst");
        assert!(request.properties.contains_key("timestamp"));
    }

    #[tokio::test]
    async fn test_build_notification_request() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Setup edge kernel inbox for NOTIFIES relationship
        let edge_inbox = root.join("concepts/.edges/NOTIFIES.Source-to-Target/queue/inbox");
        fs::create_dir_all(&edge_inbox).unwrap();

        // Create builder
        let builder = EdgeRequestBuilder::new(root.to_path_buf());

        // Create notification with NOTIFIES type
        let notif = NotificationEntry {
            target: "ckp://Target:v0.1".to_string(),
            relationship_type: Some("NOTIFIES".to_string()),
            properties: None,
        };

        // Create instance file
        let instance_path = root.join("concepts/Source/storage/tx-456.inst");
        fs::create_dir_all(instance_path.parent().unwrap()).unwrap();
        fs::write(&instance_path, "notification data").unwrap();

        // Execute
        let result = builder
            .create_edge_requests("Source", &instance_path, &[notif])
            .await;

        assert!(result.is_ok());

        // Verify .edgereq file created
        let edgereq_files: Vec<_> = fs::read_dir(&edge_inbox)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    == Some("edgereq")
            })
            .collect();

        assert_eq!(edgereq_files.len(), 1);

        // Verify JSON content
        let content = fs::read_to_string(edgereq_files[0].path()).unwrap();
        let request: EdgeRequest = serde_json::from_str(&content).unwrap();

        assert_eq!(request.relationship_type, "NOTIFIES");
        assert_eq!(request.source.kernel, "ckp://Source:v0.1");
        assert_eq!(request.target.kernel, "ckp://Target:v0.1");
        assert_eq!(request.source.instance, "ckp://Source:v0.1#storage/tx-456.inst");
        assert_eq!(request.target.queue, "ckp://Target:v0.1#inbox");
    }

    #[tokio::test]
    async fn test_notification_with_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Setup edge kernel inbox
        let edge_inbox = root.join("concepts/.edges/PRODUCES.Source-to-Target/queue/inbox");
        fs::create_dir_all(&edge_inbox).unwrap();

        // Create builder
        let builder = EdgeRequestBuilder::new(root.to_path_buf());

        // Create notification with rich metadata
        let mut metadata = HashMap::new();
        metadata.insert("priority".to_string(), Value::String("high".to_string()));
        metadata.insert("retry_count".to_string(), Value::Number(3.into()));
        metadata.insert("urgent".to_string(), Value::Bool(true));

        let notif = NotificationEntry {
            target: "ckp://Target:v0.1".to_string(),
            relationship_type: Some("PRODUCES".to_string()),
            properties: Some(metadata),
        };

        // Create instance file
        let instance_path = root.join("concepts/Source/storage/tx-789.inst");
        fs::create_dir_all(instance_path.parent().unwrap()).unwrap();
        fs::write(&instance_path, "metadata test").unwrap();

        // Execute
        let result = builder
            .create_edge_requests("Source", &instance_path, &[notif])
            .await;

        assert!(result.is_ok());

        // Verify .edgereq file created
        let edgereq_files: Vec<_> = fs::read_dir(&edge_inbox)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    == Some("edgereq")
            })
            .collect();

        assert_eq!(edgereq_files.len(), 1);

        // Verify JSON content and metadata
        let content = fs::read_to_string(edgereq_files[0].path()).unwrap();
        let request: EdgeRequest = serde_json::from_str(&content).unwrap();

        assert_eq!(request.relationship_type, "PRODUCES");
        assert!(request.properties.contains_key("priority"));
        assert!(request.properties.contains_key("retry_count"));
        assert!(request.properties.contains_key("urgent"));
        assert!(request.properties.contains_key("timestamp"));

        assert_eq!(request.properties["priority"], Value::String("high".to_string()));
        assert_eq!(request.properties["retry_count"], Value::Number(3.into()));
        assert_eq!(request.properties["urgent"], Value::Bool(true));
    }

    #[test]
    fn test_edge_urn_with_version_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let builder = EdgeRequestBuilder::new(temp_dir.path().to_path_buf());

        // Test various version patterns
        let result1 = builder.extract_target_kernel("ckp://Target:v1.0");
        assert_eq!(result1.unwrap(), "Target");

        let result2 = builder.extract_target_kernel("ckp://Target:v2.3.14");
        assert_eq!(result2.unwrap(), "Target");

        let result3 = builder.extract_target_kernel("ckp://Target:v0.1.0-alpha");
        assert_eq!(result3.unwrap(), "Target");

        // Test with domain prefix and version patterns
        let result4 = builder.extract_target_kernel("ckp://Org.Domain:Target:v1.0");
        assert_eq!(result4.unwrap(), "Target");

        // Test URN with stage and path (should still extract kernel)
        let result5 = builder.extract_target_kernel("ckp://Target:v0.1#storage/tx-123.inst");
        assert_eq!(result5.unwrap(), "Target");

        // Test with complex domain
        let result6 = builder.extract_target_kernel("ckp://Recipes.Baking:BakeCake:v1.3.12");
        assert_eq!(result6.unwrap(), "BakeCake");
    }
}
