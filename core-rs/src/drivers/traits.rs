//! Storage driver trait for ConceptKernel
//!
//! Defines the abstract interface for all storage backends.
//! Implementations include:
//! - FileSystemDriver (local filesystem)
//! - HttpDriver (remote HTTP)
//! - Future: S3Driver, RedisDriver, PostgresDriver, IpfsDriver

use crate::errors::Result;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

/// Storage location abstraction
///
/// Represents where something is stored without exposing physical details
#[derive(Debug, Clone)]
pub enum StorageLocation {
    /// Local filesystem path (for FileSystemDriver)
    Local(PathBuf),

    /// Remote URL (for HttpDriver, S3Driver, etc.)
    Remote(String),

    /// Abstract URN reference (resolved later)
    Urn(String),
}

/// Job file structure
///
/// Standard format for jobs written to inbox queues
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JobFile {
    /// Target kernel name or URN
    pub target: String,

    /// Job payload data
    pub payload: JsonValue,

    /// ISO 8601 timestamp
    pub timestamp: String,

    /// Transaction ID (format: {timestamp}-{shortId})
    #[serde(rename = "txId")]
    pub tx_id: String,

    /// Source kernel name or 'external'
    pub source: String,
}

/// Job handle returned when reading jobs
///
/// Abstracts how jobs are stored/retrieved
#[derive(Debug)]
pub struct JobHandle {
    /// Transaction ID
    pub tx_id: String,

    /// Job content
    pub content: JobFile,

    /// Storage-specific identifier (opaque to caller)
    pub(crate) storage_id: String,
}

impl JobHandle {
    /// Get transaction ID
    pub fn tx_id(&self) -> &str {
        &self.tx_id
    }

    /// Get job payload
    pub fn payload(&self) -> &JsonValue {
        &self.content.payload
    }

    /// Get source kernel
    pub fn source(&self) -> &str {
        &self.content.source
    }

    /// Get full job content
    pub fn content(&self) -> &JobFile {
        &self.content
    }
}

/// Storage driver trait
///
/// All storage backends must implement this interface.
/// The driver abstracts physical storage details from the protocol layer.
///
/// # Protocol Compliance
///
/// Drivers must:
/// - Accept URNs as identifiers (e.g., `ckp://Kernel#inbox`)
/// - Resolve URNs to appropriate storage locations
/// - Maintain filesystem-like semantics (queues, storage, archives)
/// - Support atomic operations where possible
///
/// # Example Implementation
///
/// ```rust,ignore
/// pub struct MyDriver { ... }
///
/// impl StorageDriver for MyDriver {
///     fn write_job(&self, target_urn: &str, job: JobFile) -> Result<String> {
///         // Parse URN
///         // Resolve to storage location
///         // Write job atomically
///         // Return transaction ID
///     }
///
///     // ... other methods
/// }
/// ```
pub trait StorageDriver: Send + Sync {
    /// Write job to target inbox
    ///
    /// # Arguments
    ///
    /// * `target_urn` - Target kernel URN (e.g., "ckp://Recipes.BakeCake#inbox" or just "Recipes.BakeCake")
    /// * `job` - Job content to write
    ///
    /// # Returns
    ///
    /// Transaction ID of the written job
    ///
    /// # Protocol Semantics
    ///
    /// - Job appears atomically in target's inbox
    /// - Governor watching inbox will detect immediately (if event-driven)
    /// - Job format must be protocol-compliant JSON
    fn write_job(&self, target_urn: &str, job: JobFile) -> Result<String>;

    /// Read all jobs from kernel's inbox
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name (e.g., "Recipes.BakeCake")
    ///
    /// # Returns
    ///
    /// Vector of job handles (may be empty)
    ///
    /// # Protocol Semantics
    ///
    /// - Returns all .job files in inbox
    /// - Jobs are NOT removed (use archive_job after processing)
    /// - Order is implementation-defined (filesystem: mtime, others: arbitrary)
    fn read_jobs(&self, kernel_name: &str) -> Result<Vec<JobHandle>>;

    /// Archive job after processing
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel that owns the job
    /// * `job` - Job handle returned from read_jobs
    ///
    /// # Protocol Semantics
    ///
    /// - Move job from inbox → archive
    /// - Should be atomic (rename if filesystem, transactional if DB)
    /// - After archiving, job no longer appears in read_jobs
    fn archive_job(&self, kernel_name: &str, job: &JobHandle) -> Result<()>;

    /// Mint storage artifact
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel creating the artifact
    /// * `instance_id` - Unique instance identifier
    /// * `data` - Artifact data (JSON)
    ///
    /// # Returns
    ///
    /// URN of the created artifact (e.g., "ckp://Kernel#storage/instance-123")
    ///
    /// # Protocol Semantics
    ///
    /// - Create immutable storage artifact
    /// - Artifacts are evidence in the event sourcing chain
    /// - Should include metadata (timestamp, tx_id, process URN)
    fn mint_storage_artifact(
        &self,
        kernel_name: &str,
        instance_id: &str,
        data: JsonValue,
    ) -> Result<String>;

    /// Record transaction in JSONL log
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel recording the transaction
    /// * `transaction` - Transaction data
    ///
    /// # Protocol Semantics
    ///
    /// - Append to JSONL transaction log
    /// - Each line is valid JSON (one transaction per line)
    /// - Log is append-only (never modify existing lines)
    /// - Used for temporal queries and audit trail
    fn record_transaction(&self, kernel_name: &str, transaction: JsonValue) -> Result<()>;

    /// Resolve URN to storage location
    ///
    /// # Arguments
    ///
    /// * `urn` - URN to resolve (e.g., "ckp://Kernel#inbox")
    ///
    /// # Returns
    ///
    /// Abstract storage location (driver-specific interpretation)
    ///
    /// # Protocol Semantics
    ///
    /// - URN is the protocol's addressing mechanism
    /// - Driver maps URN → physical storage
    /// - Same URN must always resolve to same logical location
    fn resolve_urn(&self, urn: &str) -> Result<StorageLocation>;

    /// Check if kernel exists
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel to check
    ///
    /// # Returns
    ///
    /// true if kernel has storage structure, false otherwise
    fn kernel_exists(&self, kernel_name: &str) -> Result<bool>;

    /// Get queue path for edge
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name
    /// * `source_kernel` - Source kernel for edge queue
    ///
    /// # Returns
    ///
    /// Storage location for edge queue (e.g., edges/SourceKernel/)
    ///
    /// # Protocol Semantics
    ///
    /// - Edge queues are per-source-kernel
    /// - Used for kernel-to-kernel communication
    /// - Same semantics as inbox but namespaced by source
    fn get_edge_queue(&self, kernel_name: &str, source_kernel: &str) -> Result<StorageLocation>;
}

/// Helper trait for driver construction
///
/// Allows drivers to be created from configuration
pub trait StorageDriverFactory {
    /// Create driver from configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Driver-specific configuration (JSON)
    ///
    /// # Example Config
    ///
    /// ```json
    /// {
    ///   "type": "filesystem",
    ///   "root": "/path/to/concepts"
    /// }
    /// ```
    ///
    /// ```json
    /// {
    ///   "type": "s3",
    ///   "bucket": "my-concepts",
    ///   "region": "us-east-1"
    /// }
    /// ```
    fn from_config(config: JsonValue) -> Result<Box<dyn StorageDriver>>
    where
        Self: Sized;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drivers::FileSystemDriver;
    use tempfile::TempDir;
    use serde_json::json;

    #[test]
    fn test_filesystem_driver_conforms_to_trait() {
        let temp = TempDir::new().unwrap();
        let driver = FileSystemDriver::new(temp.path().to_path_buf(), "Test".to_string());

        // Verify trait implementation - will fail to compile if not implemented
        let _driver_boxed: Box<dyn StorageDriver> = Box::new(driver);

        // This test validates that FileSystemDriver implements all required methods
        // Compilation success means trait conformance
    }

    #[test]
    fn test_trait_is_object_safe() {
        // This test verifies that StorageDriver is object-safe (can be boxed)
        let temp = TempDir::new().unwrap();
        let driver = FileSystemDriver::new(temp.path().to_path_buf(), "Test".to_string());

        // If this compiles, trait is object-safe
        let _boxed: Box<dyn StorageDriver> = Box::new(driver);

        // Can also create trait objects from references
        let driver2 = FileSystemDriver::new(temp.path().to_path_buf(), "Test".to_string());
        let _reference: &dyn StorageDriver = &driver2;
    }

    #[test]
    fn test_trait_is_send_and_sync() {
        // Verify trait requires Send + Sync (for thread safety)
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<Box<dyn StorageDriver>>();
        assert_sync::<Box<dyn StorageDriver>>();

        // This ensures drivers can be shared across threads
    }

    #[test]
    fn test_storage_location_local() {
        let path = PathBuf::from("/tmp/test");
        let location = StorageLocation::Local(path.clone());

        match location {
            StorageLocation::Local(p) => assert_eq!(p, path),
            _ => panic!("Expected Local variant"),
        }
    }

    #[test]
    fn test_storage_location_remote() {
        let url = "https://example.com/storage".to_string();
        let location = StorageLocation::Remote(url.clone());

        match location {
            StorageLocation::Remote(u) => assert_eq!(u, url),
            _ => panic!("Expected Remote variant"),
        }
    }

    #[test]
    fn test_storage_location_urn() {
        let urn = "ckp://Test/storage".to_string();
        let location = StorageLocation::Urn(urn.clone());

        match location {
            StorageLocation::Urn(u) => assert_eq!(u, urn),
            _ => panic!("Expected Urn variant"),
        }
    }

    #[test]
    fn test_job_file_serialization() {
        let job = JobFile {
            target: "Test.Target".to_string(),
            payload: json!({"test": "data"}),
            timestamp: "2025-11-29T10:00:00Z".to_string(),
            tx_id: "tx_20251129_100000_abc".to_string(),
            source: "Test.Source".to_string(),
        };

        // Serialize to JSON
        let json_str = serde_json::to_string(&job).unwrap();
        assert!(json_str.contains("Test.Target"));
        assert!(json_str.contains("txId")); // Verify rename works

        // Deserialize back
        let job2: JobFile = serde_json::from_str(&json_str).unwrap();
        assert_eq!(job2.target, "Test.Target");
        assert_eq!(job2.tx_id, "tx_20251129_100000_abc");
        assert_eq!(job2.source, "Test.Source");
    }

    #[test]
    fn test_job_handle_getters() {
        let job_content = JobFile {
            target: "Test.Target".to_string(),
            payload: json!({"key": "value"}),
            timestamp: "2025-11-29T10:00:00Z".to_string(),
            tx_id: "tx_test_123".to_string(),
            source: "Test.Source".to_string(),
        };

        let handle = JobHandle {
            tx_id: "tx_test_123".to_string(),
            content: job_content.clone(),
            storage_id: "internal_id_456".to_string(),
        };

        // Test all getter methods
        assert_eq!(handle.tx_id(), "tx_test_123");
        assert_eq!(handle.source(), "Test.Source");
        assert_eq!(handle.payload(), &json!({"key": "value"}));

        let content = handle.content();
        assert_eq!(content.target, "Test.Target");
        assert_eq!(content.tx_id, "tx_test_123");
    }

    #[test]
    fn test_multi_driver_instances() {
        // Test that we can create multiple driver instances via trait objects
        let temp = TempDir::new().unwrap();

        let drivers: Vec<Box<dyn StorageDriver>> = vec![
            Box::new(FileSystemDriver::new(temp.path().to_path_buf(), "Test1".to_string())),
            Box::new(FileSystemDriver::new(temp.path().to_path_buf(), "Test2".to_string())),
            Box::new(FileSystemDriver::new(temp.path().to_path_buf(), "Test3".to_string())),
        ];

        // Verify we can store multiple driver instances in same collection
        assert_eq!(drivers.len(), 3);

        // All drivers implement the same interface
        for driver in drivers {
            // Can call resolve_urn on any driver instance
            let result = driver.resolve_urn("ckp://Test/storage");
            // We don't check the result, just that the method exists and compiles
            let _ = result;
        }
    }

    #[test]
    fn test_storage_location_is_cloneable() {
        let location = StorageLocation::Local(PathBuf::from("/tmp/test"));
        let cloned = location.clone();

        match (location, cloned) {
            (StorageLocation::Local(p1), StorageLocation::Local(p2)) => {
                assert_eq!(p1, p2);
            }
            _ => panic!("Clone should preserve variant and value"),
        }
    }

    #[test]
    fn test_job_file_is_cloneable() {
        let job = JobFile {
            target: "Test".to_string(),
            payload: json!({"test": "data"}),
            timestamp: "2025-11-29T10:00:00Z".to_string(),
            tx_id: "tx_123".to_string(),
            source: "Source".to_string(),
        };

        let cloned = job.clone();
        assert_eq!(job.target, cloned.target);
        assert_eq!(job.tx_id, cloned.tx_id);
        assert_eq!(job.source, cloned.source);
    }
}
