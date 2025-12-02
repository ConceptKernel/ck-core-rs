// storage/scanner.rs - Generic instance storage scanner
//
// Provides generic listing and querying of Concept Kernel Instances (CKIs)
// from any kernel's storage directory. Works by reading receipt.bin files
// and extracting envelope fields (id, name, timestamp, kernel).

use crate::errors::CkpError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

/// Summary of a stored instance (envelope fields only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceSummary {
    /// Instance ID (usually txId)
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Kernel that created this instance
    pub kernel: String,
    /// Creation timestamp (ISO 8601 UTC)
    pub timestamp: DateTime<Utc>,
}

/// Full instance data (envelope + data payload)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceDetail {
    /// Instance ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Kernel that created this instance
    pub kernel: String,
    /// Creation timestamp
    pub timestamp: DateTime<Utc>,
    /// Action that created this instance
    pub action: Option<String>,
    /// Processing success status
    pub success: Option<bool>,
    /// Full data payload (kernel-specific)
    pub data: Value,
}

/// Scanner for kernel instance storage
pub struct InstanceScanner {
    /// Path to kernel root (e.g., /concepts/System.Oidc.User)
    kernel_root: PathBuf,
    /// Kernel name
    kernel_name: String,
}

impl InstanceScanner {
    /// Create a new scanner for a kernel
    pub fn new(kernel_root: PathBuf, kernel_name: String) -> Self {
        Self {
            kernel_root,
            kernel_name,
        }
    }

    /// List instances from storage (sorted by name)
    ///
    /// # Arguments
    /// * `limit` - Maximum number of instances to return (0 = unlimited)
    ///
    /// # Returns
    /// Vector of instance summaries, sorted alphabetically by name
    pub fn list_instances(&self, limit: usize) -> Result<Vec<InstanceSummary>, CkpError> {
        let storage_path = self.find_storage_dir()?;

        let mut instances = Vec::new();

        // Read all *.inst directories
        if let Ok(entries) = fs::read_dir(&storage_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.extension().and_then(|s| s.to_str()) == Some("inst") {
                    if let Ok(summary) = self.read_instance_summary(&path) {
                        instances.push(summary);
                    }
                }
            }
        }

        // Sort by name (alphabetically)
        instances.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        // Apply limit
        if limit > 0 && instances.len() > limit {
            instances.truncate(limit);
        }

        Ok(instances)
    }

    /// Get detailed view of a specific instance by name
    ///
    /// # Arguments
    /// * `name` - Name to search for (case-insensitive)
    ///
    /// # Returns
    /// Full instance detail including data payload
    pub fn describe_instance(&self, name: &str) -> Result<InstanceDetail, CkpError> {
        let storage_path = self.find_storage_dir()?;
        let name_lower = name.to_lowercase();

        // Find matching instance
        if let Ok(entries) = fs::read_dir(&storage_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.extension().and_then(|s| s.to_str()) == Some("inst") {
                    if let Ok(detail) = self.read_instance_detail(&path) {
                        if detail.name.to_lowercase() == name_lower {
                            return Ok(detail);
                        }
                    }
                }
            }
        }

        Err(CkpError::FileNotFound(format!(
            "Instance '{}' not found in {}",
            name, self.kernel_name
        )))
    }

    /// Count total instances in storage
    pub fn count_instances(&self) -> Result<usize, CkpError> {
        let storage_path = self.find_storage_dir()?;

        let mut count = 0;
        if let Ok(entries) = fs::read_dir(&storage_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.extension().and_then(|s| s.to_str()) == Some("inst") {
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Find the storage directory for this kernel
    ///
    /// Handles both old structure (/concepts/Kernel/storage/) and
    /// new structure (/concepts/Kernel/Kernel/storage/)
    fn find_storage_dir(&self) -> Result<PathBuf, CkpError> {
        // Try new structure first: /concepts/Kernel/Kernel/storage/
        let new_path = self
            .kernel_root
            .join(&self.kernel_name)
            .join("storage");

        if new_path.exists() {
            return Ok(new_path);
        }

        // Try old structure: /concepts/Kernel/storage/
        let old_path = self.kernel_root.join("storage");
        if old_path.exists() {
            return Ok(old_path);
        }

        Err(CkpError::FileNotFound(format!(
            "Storage directory not found for {}",
            self.kernel_name
        )))
    }

    /// Read instance summary from receipt.bin (envelope only)
    fn read_instance_summary(&self, inst_dir: &PathBuf) -> Result<InstanceSummary, CkpError> {
        let receipt_path = inst_dir.join("receipt.bin");
        let receipt_str = fs::read_to_string(&receipt_path).map_err(|e| {
            CkpError::IoError(format!("Failed to read receipt.bin: {}", e))
        })?;

        let receipt: Value = serde_json::from_str(&receipt_str).map_err(|e| {
            CkpError::ParseError(format!("Failed to parse receipt.bin: {}", e))
        })?;

        // Extract envelope fields
        let id = self.extract_id(&receipt, inst_dir)?;
        let name = self.extract_name(&receipt, inst_dir)?;
        let kernel = receipt
            .get("kernel")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.kernel_name)
            .to_string();
        let timestamp = self.extract_timestamp(&receipt)?;

        Ok(InstanceSummary {
            id,
            name,
            kernel,
            timestamp,
        })
    }

    /// Read full instance detail from receipt.bin (envelope + data)
    fn read_instance_detail(&self, inst_dir: &PathBuf) -> Result<InstanceDetail, CkpError> {
        let receipt_path = inst_dir.join("receipt.bin");
        let receipt_str = fs::read_to_string(&receipt_path).map_err(|e| {
            CkpError::IoError(format!("Failed to read receipt.bin: {}", e))
        })?;

        let receipt: Value = serde_json::from_str(&receipt_str).map_err(|e| {
            CkpError::ParseError(format!("Failed to parse receipt.bin: {}", e))
        })?;

        // Extract envelope fields
        let id = self.extract_id(&receipt, inst_dir)?;
        let name = self.extract_name(&receipt, inst_dir)?;
        let kernel = receipt
            .get("kernel")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.kernel_name)
            .to_string();
        let timestamp = self.extract_timestamp(&receipt)?;
        let action = receipt.get("action").and_then(|v| v.as_str()).map(|s| s.to_string());
        let success = receipt.get("success").and_then(|v| v.as_bool());

        // Extract data payload (kernel-specific fields)
        let data = if let Some(data_obj) = receipt.get("data") {
            data_obj.clone()
        } else {
            // Backward compatibility: if no data field, use entire receipt minus envelope
            let mut data_obj = receipt.clone();
            if let Some(obj) = data_obj.as_object_mut() {
                obj.remove("id");
                obj.remove("name");
                obj.remove("kernel");
                obj.remove("timestamp");
                obj.remove("action");
                obj.remove("success");
                obj.remove("txId");
                obj.remove("processed");
            }
            data_obj
        };

        Ok(InstanceDetail {
            id,
            name,
            kernel,
            timestamp,
            action,
            success,
            data,
        })
    }

    /// Extract ID from receipt (tries multiple fields for backward compatibility)
    fn extract_id(&self, receipt: &Value, inst_dir: &PathBuf) -> Result<String, CkpError> {
        // Try id field first
        if let Some(id) = receipt.get("id").and_then(|v| v.as_str()) {
            return Ok(id.to_string());
        }

        // Try txId field
        if let Some(tx_id) = receipt.get("txId").and_then(|v| v.as_str()) {
            return Ok(tx_id.to_string());
        }

        // Extract from filename as fallback
        if let Some(name) = inst_dir.file_name().and_then(|n| n.to_str()) {
            if let Some(id_part) = name.strip_suffix(".inst") {
                return Ok(id_part.to_string());
            }
        }

        Err(CkpError::ParseError("No id or txId found in receipt".to_string()))
    }

    /// Extract name from receipt (tries multiple sources for backward compatibility)
    fn extract_name(&self, receipt: &Value, inst_dir: &PathBuf) -> Result<String, CkpError> {
        // Try name field first (envelope)
        if let Some(name) = receipt.get("name").and_then(|v| v.as_str()) {
            return Ok(name.to_string());
        }

        // Try data.username
        if let Some(username) = receipt
            .get("data")
            .and_then(|d| d.get("username"))
            .and_then(|v| v.as_str())
        {
            return Ok(username.to_string());
        }

        // Try username (backward compat)
        if let Some(username) = receipt.get("username").and_then(|v| v.as_str()) {
            return Ok(username.to_string());
        }

        // Try data.roleName
        if let Some(role_name) = receipt
            .get("data")
            .and_then(|d| d.get("roleName"))
            .and_then(|v| v.as_str())
        {
            return Ok(role_name.to_string());
        }

        // Try roleName (backward compat)
        if let Some(role_name) = receipt.get("roleName").and_then(|v| v.as_str()) {
            return Ok(role_name.to_string());
        }

        // Extract from filename (e.g., "1234-txid-user-alice.inst" -> "alice")
        if let Some(filename) = inst_dir.file_name().and_then(|n| n.to_str()) {
            let parts: Vec<&str> = filename.strip_suffix(".inst").unwrap_or(filename).split('-').collect();
            if parts.len() >= 4 {
                // Format: timestamp-txid-type-name
                return Ok(parts[3..].join("-"));
            }
        }

        Err(CkpError::ParseError("No name found in receipt or filename".to_string()))
    }

    /// Extract timestamp from receipt
    fn extract_timestamp(&self, receipt: &Value) -> Result<DateTime<Utc>, CkpError> {
        // Try timestamp field
        if let Some(ts_str) = receipt.get("timestamp").and_then(|v| v.as_str()) {
            if let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) {
                return Ok(dt.with_timezone(&Utc));
            }
        }

        // Try createdAt field
        if let Some(ts_str) = receipt.get("createdAt").and_then(|v| v.as_str()) {
            if let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) {
                return Ok(dt.with_timezone(&Utc));
            }
        }

        Err(CkpError::ParseError("No valid timestamp found in receipt".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: Create a test instance directory with receipt.bin
    fn create_test_instance(storage_dir: &PathBuf, instance_name: &str, data: Value) -> PathBuf {
        let inst_dir = storage_dir.join(format!("{}.inst", instance_name));
        fs::create_dir_all(&inst_dir).unwrap();

        let receipt_path = inst_dir.join("receipt.bin");
        fs::write(&receipt_path, serde_json::to_string_pretty(&data).unwrap()).unwrap();

        inst_dir
    }

    /// Test: Scanner can be constructed
    #[test]
    fn test_scanner_new() {
        let temp = TempDir::new().unwrap();
        let scanner = InstanceScanner::new(
            temp.path().to_path_buf(),
            "Test.Kernel".to_string()
        );

        assert_eq!(scanner.kernel_name, "Test.Kernel");
    }

    /// Test: Scan empty directory returns empty list
    #[test]
    fn test_list_instances_empty_directory() {
        let temp = TempDir::new().unwrap();
        let kernel_root = temp.path().join("Test.Empty");
        fs::create_dir_all(&kernel_root).unwrap();

        // Create storage directory but no instances
        fs::create_dir_all(kernel_root.join("storage")).unwrap();

        let scanner = InstanceScanner::new(kernel_root, "Test.Empty".to_string());
        let instances = scanner.list_instances(0).unwrap();

        assert_eq!(instances.len(), 0);
    }

    /// Test: Scan with single instance
    #[test]
    fn test_list_instances_single() {
        let temp = TempDir::new().unwrap();
        let kernel_root = temp.path().join("Test.Single");
        let storage_dir = kernel_root.join("storage");
        fs::create_dir_all(&storage_dir).unwrap();

        // Create one instance
        let data = serde_json::json!({
            "id": "tx-123",
            "name": "test-instance",
            "kernel": "Test.Single",
            "timestamp": "2025-11-29T10:00:00Z"
        });
        create_test_instance(&storage_dir, "tx-123", data);

        let scanner = InstanceScanner::new(kernel_root, "Test.Single".to_string());
        let instances = scanner.list_instances(0).unwrap();

        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].id, "tx-123");
        assert_eq!(instances[0].name, "test-instance");
        assert_eq!(instances[0].kernel, "Test.Single");
    }

    /// Test: Scan with multiple instances
    #[test]
    fn test_list_instances_multiple() {
        let temp = TempDir::new().unwrap();
        let kernel_root = temp.path().join("Test.Multiple");
        let storage_dir = kernel_root.join("storage");
        fs::create_dir_all(&storage_dir).unwrap();

        // Create three instances
        for i in 1..=3 {
            let data = serde_json::json!({
                "id": format!("tx-{}", i),
                "name": format!("instance-{}", i),
                "kernel": "Test.Multiple",
                "timestamp": "2025-11-29T10:00:00Z"
            });
            create_test_instance(&storage_dir, &format!("tx-{}", i), data);
        }

        let scanner = InstanceScanner::new(kernel_root, "Test.Multiple".to_string());
        let instances = scanner.list_instances(0).unwrap();

        assert_eq!(instances.len(), 3);

        // Instances should be sorted by name
        assert_eq!(instances[0].name, "instance-1");
        assert_eq!(instances[1].name, "instance-2");
        assert_eq!(instances[2].name, "instance-3");
    }

    /// Test: Scan respects limit parameter
    #[test]
    fn test_list_instances_respects_limit() {
        let temp = TempDir::new().unwrap();
        let kernel_root = temp.path().join("Test.Limit");
        let storage_dir = kernel_root.join("storage");
        fs::create_dir_all(&storage_dir).unwrap();

        // Create 10 instances
        for i in 1..=10 {
            let data = serde_json::json!({
                "id": format!("tx-{}", i),
                "name": format!("instance-{:02}", i),
                "kernel": "Test.Limit",
                "timestamp": "2025-11-29T10:00:00Z"
            });
            create_test_instance(&storage_dir, &format!("tx-{}", i), data);
        }

        let scanner = InstanceScanner::new(kernel_root, "Test.Limit".to_string());

        // Test limit of 5
        let limited = scanner.list_instances(5).unwrap();
        assert_eq!(limited.len(), 5);

        // Test limit of 0 (unlimited)
        let unlimited = scanner.list_instances(0).unwrap();
        assert_eq!(unlimited.len(), 10);
    }

    /// Test: Scan handles invalid files gracefully
    #[test]
    fn test_list_instances_handles_invalid_files() {
        let temp = TempDir::new().unwrap();
        let kernel_root = temp.path().join("Test.Invalid");
        let storage_dir = kernel_root.join("storage");
        fs::create_dir_all(&storage_dir).unwrap();

        // Create valid instance
        let valid_data = serde_json::json!({
            "id": "tx-valid",
            "name": "valid-instance",
            "kernel": "Test.Invalid",
            "timestamp": "2025-11-29T10:00:00Z"
        });
        create_test_instance(&storage_dir, "tx-valid", valid_data);

        // Create invalid instance (malformed JSON)
        let invalid_inst = storage_dir.join("tx-invalid.inst");
        fs::create_dir_all(&invalid_inst).unwrap();
        fs::write(invalid_inst.join("receipt.bin"), "{ invalid json }").unwrap();

        // Create instance without receipt.bin
        let no_receipt_inst = storage_dir.join("tx-no-receipt.inst");
        fs::create_dir_all(&no_receipt_inst).unwrap();

        let scanner = InstanceScanner::new(kernel_root, "Test.Invalid".to_string());
        let instances = scanner.list_instances(0).unwrap();

        // Should only find the valid instance
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].id, "tx-valid");
    }

    /// Test: Count instances
    #[test]
    fn test_count_instances() {
        let temp = TempDir::new().unwrap();
        let kernel_root = temp.path().join("Test.Count");
        let storage_dir = kernel_root.join("storage");
        fs::create_dir_all(&storage_dir).unwrap();

        // Create 5 instances
        for i in 1..=5 {
            let data = serde_json::json!({
                "id": format!("tx-{}", i),
                "name": format!("instance-{}", i),
                "kernel": "Test.Count",
                "timestamp": "2025-11-29T10:00:00Z"
            });
            create_test_instance(&storage_dir, &format!("tx-{}", i), data);
        }

        let scanner = InstanceScanner::new(kernel_root, "Test.Count".to_string());
        let count = scanner.count_instances().unwrap();

        assert_eq!(count, 5);
    }

    /// Test: Describe instance by name
    #[test]
    fn test_describe_instance() {
        let temp = TempDir::new().unwrap();
        let kernel_root = temp.path().join("Test.Describe");
        let storage_dir = kernel_root.join("storage");
        fs::create_dir_all(&storage_dir).unwrap();

        // Create instance with full data
        let data = serde_json::json!({
            "id": "tx-describe",
            "name": "target-instance",
            "kernel": "Test.Describe",
            "timestamp": "2025-11-29T10:00:00Z",
            "action": "create",
            "success": true,
            "data": {
                "field1": "value1",
                "field2": 42
            }
        });
        create_test_instance(&storage_dir, "tx-describe", data);

        let scanner = InstanceScanner::new(kernel_root, "Test.Describe".to_string());
        let detail = scanner.describe_instance("target-instance").unwrap();

        assert_eq!(detail.id, "tx-describe");
        assert_eq!(detail.name, "target-instance");
        assert_eq!(detail.action, Some("create".to_string()));
        assert_eq!(detail.success, Some(true));
        assert!(detail.data.get("field1").is_some());
    }

    /// Test: Describe instance not found
    #[test]
    fn test_describe_instance_not_found() {
        let temp = TempDir::new().unwrap();
        let kernel_root = temp.path().join("Test.NotFound");
        let storage_dir = kernel_root.join("storage");
        fs::create_dir_all(&storage_dir).unwrap();

        let scanner = InstanceScanner::new(kernel_root, "Test.NotFound".to_string());
        let result = scanner.describe_instance("nonexistent");

        assert!(result.is_err());
        match result.unwrap_err() {
            CkpError::FileNotFound(msg) => {
                assert!(msg.contains("nonexistent"));
            }
            _ => panic!("Expected FileNotFound error"),
        }
    }

    /// Test: Storage directory not found
    #[test]
    fn test_storage_directory_not_found() {
        let temp = TempDir::new().unwrap();
        let kernel_root = temp.path().join("Test.NoStorage");
        fs::create_dir_all(&kernel_root).unwrap();
        // Don't create storage directory

        let scanner = InstanceScanner::new(kernel_root, "Test.NoStorage".to_string());
        let result = scanner.list_instances(0);

        assert!(result.is_err());
        match result.unwrap_err() {
            CkpError::FileNotFound(msg) => {
                assert!(msg.contains("Storage directory not found"));
            }
            _ => panic!("Expected FileNotFound error"),
        }
    }

    /// Test: Alphabetical sorting (case-insensitive)
    #[test]
    fn test_list_instances_alphabetical_sorting() {
        let temp = TempDir::new().unwrap();
        let kernel_root = temp.path().join("Test.Sorting");
        let storage_dir = kernel_root.join("storage");
        fs::create_dir_all(&storage_dir).unwrap();

        // Create instances in non-alphabetical order
        let names = vec!["Zebra", "apple", "banana", "Apple"];
        for (i, name) in names.iter().enumerate() {
            let data = serde_json::json!({
                "id": format!("tx-{}", i),
                "name": name,
                "kernel": "Test.Sorting",
                "timestamp": "2025-11-29T10:00:00Z"
            });
            create_test_instance(&storage_dir, &format!("tx-{}", i), data);
        }

        let scanner = InstanceScanner::new(kernel_root, "Test.Sorting".to_string());
        let instances = scanner.list_instances(0).unwrap();

        // Should be sorted case-insensitively: apple, Apple, banana, Zebra
        assert_eq!(instances.len(), 4);
        assert_eq!(instances[0].name.to_lowercase(), "apple");
        assert_eq!(instances[2].name.to_lowercase(), "banana");
        assert_eq!(instances[3].name.to_lowercase(), "zebra");
    }
}
