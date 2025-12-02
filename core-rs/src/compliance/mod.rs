//! Compliance module for audit logging and data governance
//!
//! Provides:
//! - Audit log generation with sensitive data redaction
//! - GDPR compliance checks (consent, access, erasure, portability)
//! - Data retention policies with archival
//! - Privacy controls
//!
//! # Example
//!
//! ```rust
//! use ckp_core::compliance::{AuditLogger, GdprChecker, RetentionPolicy};
//! use std::path::PathBuf;
//!
//! // Audit logging
//! let logger = AuditLogger::new(PathBuf::from("/concepts/audit"));
//! logger.log_operation("kernel.emit", "user123", serde_json::json!({"kernel": "Test.Kernel"}));
//!
//! // GDPR compliance
//! let mut gdpr = GdprChecker::new();
//! gdpr.record_consent("user123", true);
//! assert!(gdpr.check_consent("user123").unwrap());
//!
//! // Retention policy
//! let policy = RetentionPolicy::new(90, PathBuf::from("/concepts/archive"));
//! policy.check_expired_data(PathBuf::from("/concepts"));
//! ```

use crate::errors::{CkpError, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Audit logger for recording kernel operations
pub struct AuditLogger {
    log_path: PathBuf,
    max_log_size: u64, // bytes
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub operation: String,
    pub user_id: Option<String>,
    pub data: JsonValue,
    pub redacted: bool,
}

/// GDPR compliance checker
pub struct GdprChecker {
    consent_records: HashMap<String, ConsentRecord>,
}

/// Consent record for GDPR compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRecord {
    pub user_id: String,
    pub consented: bool,
    pub timestamp: DateTime<Utc>,
    pub expiry: Option<DateTime<Utc>>,
}

/// Data retention policy manager
pub struct RetentionPolicy {
    retention_days: i64,
    archive_path: PathBuf,
    exceptions: Vec<String>, // Kernel names exempt from retention
}

/// Retention check result
#[derive(Debug, Clone)]
pub struct RetentionCheckResult {
    pub expired_files: Vec<PathBuf>,
    pub total_size: u64,
}

/// GDPR data access request result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataAccessResult {
    pub user_id: String,
    pub data: Vec<JsonValue>,
    pub timestamp: DateTime<Utc>,
}

/// GDPR data portability export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPortabilityExport {
    pub user_id: String,
    pub export_format: String,
    pub data: JsonValue,
    pub timestamp: DateTime<Utc>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(log_path: PathBuf) -> Self {
        Self {
            log_path,
            max_log_size: 10_000_000, // 10MB default
        }
    }

    /// Log a kernel operation
    pub fn log_operation(&self, operation: &str, user_id: Option<&str>, data: JsonValue) -> Result<()> {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            operation: operation.to_string(),
            user_id: user_id.map(|s| s.to_string()),
            data,
            redacted: false,
        };

        self.write_entry(entry)
    }

    /// Log with user context
    pub fn log_with_context(&self, operation: &str, user_id: &str, data: JsonValue) -> Result<()> {
        self.log_operation(operation, Some(user_id), data)
    }

    /// Log with sensitive data redaction
    pub fn log_with_redaction(&self, operation: &str, user_id: Option<&str>, mut data: JsonValue) -> Result<()> {
        // Redact sensitive fields
        if let Some(obj) = data.as_object_mut() {
            let sensitive_fields = ["password", "token", "secret", "api_key", "credit_card"];
            for field in &sensitive_fields {
                if obj.contains_key(*field) {
                    obj.insert(field.to_string(), JsonValue::String("[REDACTED]".to_string()));
                }
            }
        }

        let entry = AuditEntry {
            timestamp: Utc::now(),
            operation: operation.to_string(),
            user_id: user_id.map(|s| s.to_string()),
            data,
            redacted: true,
        };

        self.write_entry(entry)
    }

    /// Rotate log file if it exceeds max size
    pub fn rotate_if_needed(&self) -> Result<bool> {
        if !self.log_path.exists() {
            return Ok(false);
        }

        let metadata = fs::metadata(&self.log_path)
            .map_err(|e| CkpError::IoError(format!("Failed to read log metadata: {}", e)))?;

        if metadata.len() > self.max_log_size {
            let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
            let rotated_path = self.log_path.with_extension(format!("log.{}", timestamp));

            fs::rename(&self.log_path, &rotated_path)
                .map_err(|e| CkpError::IoError(format!("Failed to rotate log: {}", e)))?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn write_entry(&self, entry: AuditEntry) -> Result<()> {
        // Create parent directory if needed
        if let Some(parent) = self.log_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| CkpError::IoError(format!("Failed to create log directory: {}", e)))?;
        }

        // Serialize entry
        let json = serde_json::to_string(&entry)
            .map_err(|e| CkpError::SerializationError(format!("Failed to serialize audit entry: {}", e)))?;

        // Append to log file (JSONL format)
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|e| CkpError::IoError(format!("Failed to open log file: {}", e)))?;

        writeln!(file, "{}", json)
            .map_err(|e| CkpError::IoError(format!("Failed to write log entry: {}", e)))?;

        Ok(())
    }
}

impl GdprChecker {
    /// Create a new GDPR checker
    pub fn new() -> Self {
        Self {
            consent_records: HashMap::new(),
        }
    }

    /// Record user consent
    pub fn record_consent(&mut self, user_id: &str, consented: bool) {
        let record = ConsentRecord {
            user_id: user_id.to_string(),
            consented,
            timestamp: Utc::now(),
            expiry: None,
        };
        self.consent_records.insert(user_id.to_string(), record);
    }

    /// Record consent with expiry
    pub fn record_consent_with_expiry(&mut self, user_id: &str, consented: bool, expiry_days: i64) {
        let record = ConsentRecord {
            user_id: user_id.to_string(),
            consented,
            timestamp: Utc::now(),
            expiry: Some(Utc::now() + Duration::days(expiry_days)),
        };
        self.consent_records.insert(user_id.to_string(), record);
    }

    /// Check if user has given consent
    pub fn check_consent(&self, user_id: &str) -> Result<bool> {
        match self.consent_records.get(user_id) {
            Some(record) => {
                // Check if consent has expired
                if let Some(expiry) = record.expiry {
                    if Utc::now() > expiry {
                        return Ok(false);
                    }
                }
                Ok(record.consented)
            }
            None => Err(CkpError::ValidationError(format!("No consent record for user: {}", user_id))),
        }
    }

    /// GDPR Right of Access - retrieve all user data
    pub fn data_access_request(&self, user_id: &str, data_sources: Vec<JsonValue>) -> Result<DataAccessResult> {
        // Verify consent
        if !self.check_consent(user_id)? {
            return Err(CkpError::ValidationError(format!("User {} has not consented to data processing", user_id)));
        }

        Ok(DataAccessResult {
            user_id: user_id.to_string(),
            data: data_sources,
            timestamp: Utc::now(),
        })
    }

    /// GDPR Right to Erasure - mark user data for deletion
    pub fn right_to_erasure(&mut self, user_id: &str) -> Result<()> {
        // Remove consent record
        self.consent_records.remove(user_id);

        // In production, this would trigger cascading deletion
        // across all kernels that store user data
        Ok(())
    }

    /// GDPR Data Portability - export user data in portable format
    pub fn data_portability(&self, user_id: &str, data: JsonValue, format: &str) -> Result<DataPortabilityExport> {
        // Verify consent
        if !self.check_consent(user_id)? {
            return Err(CkpError::ValidationError(format!("User {} has not consented to data processing", user_id)));
        }

        Ok(DataPortabilityExport {
            user_id: user_id.to_string(),
            export_format: format.to_string(),
            data,
            timestamp: Utc::now(),
        })
    }
}

impl Default for GdprChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl RetentionPolicy {
    /// Create a new retention policy
    pub fn new(retention_days: i64, archive_path: PathBuf) -> Self {
        Self {
            retention_days,
            archive_path,
            exceptions: Vec::new(),
        }
    }

    /// Add kernel exception (exempt from retention)
    pub fn add_exception(&mut self, kernel_name: String) {
        self.exceptions.push(kernel_name);
    }

    /// Check for expired data
    pub fn check_expired_data(&self, concepts_path: PathBuf) -> Result<RetentionCheckResult> {
        let cutoff_date = Utc::now() - Duration::days(self.retention_days);
        let mut expired_files = Vec::new();
        let mut total_size = 0u64;

        if !concepts_path.exists() {
            return Ok(RetentionCheckResult {
                expired_files,
                total_size,
            });
        }

        // Walk through concepts directory
        for entry in fs::read_dir(&concepts_path)
            .map_err(|e| CkpError::IoError(format!("Failed to read concepts directory: {}", e)))?
        {
            let entry = entry.map_err(|e| CkpError::IoError(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();

            // Skip exceptions
            if let Some(name) = path.file_name() {
                if self.exceptions.iter().any(|e| name.to_string_lossy().contains(e)) {
                    continue;
                }
            }

            // Check storage directory
            let storage_path = path.join("storage");
            if storage_path.exists() {
                self.scan_directory(&storage_path, cutoff_date, &mut expired_files, &mut total_size)?;
            }

            // Check archive directory
            let archive_path = path.join("queue/archive");
            if archive_path.exists() {
                self.scan_directory(&archive_path, cutoff_date, &mut expired_files, &mut total_size)?;
            }
        }

        Ok(RetentionCheckResult {
            expired_files,
            total_size,
        })
    }

    /// Archive old data
    pub fn archive_data(&self, source_path: PathBuf) -> Result<PathBuf> {
        if !source_path.exists() {
            return Err(CkpError::FileNotFound(format!("Source path does not exist: {:?}", source_path)));
        }

        // Create archive directory
        fs::create_dir_all(&self.archive_path)
            .map_err(|e| CkpError::IoError(format!("Failed to create archive directory: {}", e)))?;

        // Generate archive filename with timestamp
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
        let filename = source_path.file_name()
            .ok_or_else(|| CkpError::Path("Invalid source path".to_string()))?;
        let archive_file = self.archive_path.join(format!("{}.{}", filename.to_string_lossy(), timestamp));

        // Copy file to archive
        fs::copy(&source_path, &archive_file)
            .map_err(|e| CkpError::IoError(format!("Failed to archive file: {}", e)))?;

        Ok(archive_file)
    }

    /// Delete after successful archive
    pub fn delete_after_archive(&self, source_path: PathBuf, archive_path: PathBuf) -> Result<()> {
        // Verify archive exists
        if !archive_path.exists() {
            return Err(CkpError::FileNotFound(format!("Archive does not exist: {:?}", archive_path)));
        }

        // Delete source
        fs::remove_file(&source_path)
            .map_err(|e| CkpError::IoError(format!("Failed to delete source file: {}", e)))?;

        Ok(())
    }

    fn scan_directory(
        &self,
        dir_path: &PathBuf,
        cutoff_date: DateTime<Utc>,
        expired_files: &mut Vec<PathBuf>,
        total_size: &mut u64,
    ) -> Result<()> {
        if !dir_path.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(dir_path)
            .map_err(|e| CkpError::IoError(format!("Failed to read directory: {}", e)))?
        {
            let entry = entry.map_err(|e| CkpError::IoError(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();

            if path.is_file() {
                let metadata = fs::metadata(&path)
                    .map_err(|e| CkpError::IoError(format!("Failed to read metadata: {}", e)))?;

                if let Ok(modified) = metadata.modified() {
                    let modified_dt: DateTime<Utc> = modified.into();
                    if modified_dt < cutoff_date {
                        expired_files.push(path);
                        *total_size += metadata.len();
                    }
                }
            } else if path.is_dir() {
                // Recursively scan subdirectories
                self.scan_directory(&path, cutoff_date, expired_files, total_size)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ========================================
    // Audit Log Generation Tests (5 tests)
    // ========================================

    #[test]
    fn test_audit_logger_new() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit.log");

        let logger = AuditLogger::new(log_path.clone());

        assert_eq!(logger.log_path, log_path);
        assert_eq!(logger.max_log_size, 10_000_000);
    }

    #[test]
    fn test_log_kernel_operation() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit.log");
        let logger = AuditLogger::new(log_path.clone());

        let data = serde_json::json!({
            "kernel": "Test.Kernel",
            "operation": "emit",
            "target": "ckp://Target.Kernel:v1"
        });

        let result = logger.log_operation("kernel.emit", None, data);
        assert!(result.is_ok());

        // Verify log file was created
        assert!(log_path.exists());

        // Verify log content
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("kernel.emit"));
        assert!(content.contains("Test.Kernel"));
    }

    #[test]
    fn test_log_with_user_context() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit.log");
        let logger = AuditLogger::new(log_path.clone());

        let data = serde_json::json!({
            "action": "data_access_request"
        });

        let result = logger.log_with_context("gdpr.access", "user123", data);
        assert!(result.is_ok());

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("user123"));
        assert!(content.contains("gdpr.access"));
    }

    #[test]
    fn test_log_sensitive_data_redaction() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit.log");
        let logger = AuditLogger::new(log_path.clone());

        let data = serde_json::json!({
            "username": "alice",
            "password": "secret123",
            "api_key": "sk-1234567890",
            "email": "alice@example.com"
        });

        let result = logger.log_with_redaction("auth.login", Some("alice"), data);
        assert!(result.is_ok());

        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("[REDACTED]"));
        assert!(!content.contains("secret123"));
        assert!(!content.contains("sk-1234567890"));
        assert!(content.contains("alice@example.com")); // email not redacted
        assert!(content.contains("\"redacted\":true"));
    }

    #[test]
    fn test_audit_log_rotation() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("audit.log");
        let mut logger = AuditLogger::new(log_path.clone());

        // Set small max size for testing
        logger.max_log_size = 100;

        // Write enough data to trigger rotation
        for i in 0..10 {
            let data = serde_json::json!({"iteration": i, "data": "x".repeat(50)});
            logger.log_operation("test", None, data).unwrap();
        }

        // Verify log file exists and has size
        assert!(log_path.exists());
        let metadata = fs::metadata(&log_path).unwrap();
        assert!(metadata.len() > logger.max_log_size);

        // Attempt rotation
        let rotated = logger.rotate_if_needed().unwrap();
        assert!(rotated);

        // Verify rotated file exists (log file should be renamed)
        let parent = log_path.parent().unwrap();
        let entries: Vec<_> = fs::read_dir(parent).unwrap().collect();
        assert!(entries.len() >= 1); // At least the rotated file

        // Verify original log no longer exists or is smaller
        if log_path.exists() {
            let new_metadata = fs::metadata(&log_path).unwrap();
            assert!(new_metadata.len() < metadata.len());
        }
    }

    // ========================================
    // GDPR Compliance Tests (5 tests)
    // ========================================

    #[test]
    fn test_gdpr_consent_verification() {
        let mut checker = GdprChecker::new();

        checker.record_consent("user123", true);
        assert!(checker.check_consent("user123").unwrap());

        checker.record_consent("user456", false);
        assert!(!checker.check_consent("user456").unwrap());

        // No consent record
        let result = checker.check_consent("user789");
        assert!(result.is_err());
    }

    #[test]
    fn test_gdpr_data_access_request() {
        let mut checker = GdprChecker::new();
        checker.record_consent("alice", true);

        let data_sources = vec![
            serde_json::json!({"kernel": "Profile", "data": {"name": "Alice"}}),
            serde_json::json!({"kernel": "Orders", "data": {"order_id": "123"}}),
        ];

        let result = checker.data_access_request("alice", data_sources.clone());
        assert!(result.is_ok());

        let access_result = result.unwrap();
        assert_eq!(access_result.user_id, "alice");
        assert_eq!(access_result.data.len(), 2);
    }

    #[test]
    fn test_gdpr_right_to_erasure() {
        let mut checker = GdprChecker::new();
        checker.record_consent("user123", true);

        assert!(checker.check_consent("user123").is_ok());

        // Exercise right to erasure
        let result = checker.right_to_erasure("user123");
        assert!(result.is_ok());

        // Consent record should be removed
        let consent_check = checker.check_consent("user123");
        assert!(consent_check.is_err());
    }

    #[test]
    fn test_gdpr_data_portability() {
        let mut checker = GdprChecker::new();
        checker.record_consent("bob", true);

        let user_data = serde_json::json!({
            "profile": {"name": "Bob", "email": "bob@example.com"},
            "orders": [{"id": "123", "total": 99.99}]
        });

        let result = checker.data_portability("bob", user_data.clone(), "json");
        assert!(result.is_ok());

        let export = result.unwrap();
        assert_eq!(export.user_id, "bob");
        assert_eq!(export.export_format, "json");
        assert_eq!(export.data, user_data);
    }

    #[test]
    fn test_gdpr_consent_expiry() {
        let mut checker = GdprChecker::new();

        // Record consent with short expiry
        checker.record_consent_with_expiry("temp_user", true, -1); // Expired yesterday

        // Consent should be expired
        let result = checker.check_consent("temp_user");
        assert!(result.is_ok());
        assert!(!result.unwrap()); // Should be false due to expiry

        // Record valid consent
        checker.record_consent_with_expiry("valid_user", true, 30); // Expires in 30 days
        let result = checker.check_consent("valid_user");
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    // ========================================
    // Data Retention Policy Tests (5 tests)
    // ========================================

    #[test]
    fn test_retention_policy_setup() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("archive");

        let mut policy = RetentionPolicy::new(90, archive_path.clone());

        assert_eq!(policy.retention_days, 90);
        assert_eq!(policy.archive_path, archive_path);
        assert_eq!(policy.exceptions.len(), 0);

        policy.add_exception("System.Audit".to_string());
        assert_eq!(policy.exceptions.len(), 1);
    }

    #[test]
    fn test_retention_check_expired_data() {
        let temp_dir = TempDir::new().unwrap();
        let concepts_path = temp_dir.path().join("concepts");
        let archive_path = temp_dir.path().join("archive");

        // Create test kernel structure
        let kernel_path = concepts_path.join("Test.Kernel");
        let storage_path = kernel_path.join("storage");
        fs::create_dir_all(&storage_path).unwrap();

        // Create some test files
        let test_file = storage_path.join("old-data.json");
        fs::write(&test_file, "{}").unwrap();

        let policy = RetentionPolicy::new(0, archive_path); // 0 days = everything is expired

        let result = policy.check_expired_data(concepts_path);
        assert!(result.is_ok());

        let check_result = result.unwrap();
        assert!(check_result.expired_files.len() > 0);
        assert!(check_result.total_size > 0);
    }

    #[test]
    fn test_retention_archive_old_data() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("archive");
        let policy = RetentionPolicy::new(90, archive_path.clone());

        // Create source file
        let source_path = temp_dir.path().join("data.json");
        fs::write(&source_path, r#"{"test": "data"}"#).unwrap();

        let result = policy.archive_data(source_path.clone());
        assert!(result.is_ok());

        let archived = result.unwrap();
        assert!(archived.exists());
        assert!(archived.parent().unwrap() == archive_path);
    }

    #[test]
    fn test_retention_delete_after_archive() {
        let temp_dir = TempDir::new().unwrap();
        let archive_path = temp_dir.path().join("archive");
        let policy = RetentionPolicy::new(90, archive_path.clone());

        // Create and archive source file
        let source_path = temp_dir.path().join("data.json");
        fs::write(&source_path, "test").unwrap();

        let archived = policy.archive_data(source_path.clone()).unwrap();
        assert!(source_path.exists());
        assert!(archived.exists());

        // Delete after archive
        let result = policy.delete_after_archive(source_path.clone(), archived.clone());
        assert!(result.is_ok());
        assert!(!source_path.exists());
        assert!(archived.exists());
    }

    #[test]
    fn test_retention_policy_exceptions() {
        let temp_dir = TempDir::new().unwrap();
        let concepts_path = temp_dir.path().join("concepts");
        let archive_path = temp_dir.path().join("archive");

        // Create test kernel structures
        let normal_kernel = concepts_path.join("Normal.Kernel");
        let exempt_kernel = concepts_path.join("System.Audit");

        fs::create_dir_all(normal_kernel.join("storage")).unwrap();
        fs::create_dir_all(exempt_kernel.join("storage")).unwrap();

        // Create test files
        fs::write(normal_kernel.join("storage/data.json"), "{}").unwrap();
        fs::write(exempt_kernel.join("storage/data.json"), "{}").unwrap();

        let mut policy = RetentionPolicy::new(0, archive_path); // Everything expired
        policy.add_exception("System.Audit".to_string());

        let result = policy.check_expired_data(concepts_path).unwrap();

        // Should only find files from Normal.Kernel, not System.Audit
        let expired_paths: Vec<String> = result.expired_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        assert!(expired_paths.iter().any(|p| p.contains("Normal.Kernel")));
        assert!(!expired_paths.iter().any(|p| p.contains("System.Audit")));
    }
}
