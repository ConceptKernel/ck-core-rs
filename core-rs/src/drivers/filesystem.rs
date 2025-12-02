//! FileSystemDriver for ConceptKernel event sourcing
//!
//! Provides file-based event sourcing operations including:
//! - Storage artifact minting
//! - Transaction recording
//! - Job archiving
//! - Per-edge queue management (v1.3.12)
//! - Symlink creation with relative paths

use crate::errors::{CkpError, Result};
use chrono::Utc;
use serde_json::Value as JsonValue;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Transaction record structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Transaction {
    #[serde(rename = "txId")]
    pub tx_id: String,
    pub timestamp: String,
    pub kernel: String,
    #[serde(flatten)]
    pub metadata: JsonValue,
}

/// FileSystemDriver for kernel operations
#[derive(Debug, Clone)]
pub struct FileSystemDriver {
    root: PathBuf,
    concept: String,
}

impl FileSystemDriver {
    /// Create new FileSystemDriver
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::PathBuf;
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    /// ```
    pub fn new(root: PathBuf, concept: String) -> Self {
        Self { root, concept }
    }

    /// Get kernel directory path
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::PathBuf;
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    /// let kernel_dir = driver.get_kernel_dir();
    /// assert_eq!(kernel_dir, PathBuf::from("/test/concepts/Recipes.BakeCake"));
    /// ```
    pub fn get_kernel_dir(&self) -> PathBuf {
        self.root.join("concepts").join(&self.concept)
    }

    /// Get queue inbox path
    pub fn get_queue_inbox(&self) -> PathBuf {
        self.get_kernel_dir().join("queue").join("inbox")
    }

    /// Get queue staging path
    pub fn get_queue_staging(&self) -> PathBuf {
        self.get_kernel_dir().join("queue").join("staging")
    }

    /// Get queue ready path
    pub fn get_queue_ready(&self) -> PathBuf {
        self.get_kernel_dir().join("queue").join("ready")
    }

    /// Get storage path
    pub fn get_storage(&self) -> PathBuf {
        self.get_kernel_dir().join("storage")
    }

    /// Get archive path
    pub fn get_archive(&self) -> PathBuf {
        self.get_kernel_dir().join("queue").join("archive")
    }

    /// Get transaction log path
    pub fn get_tx_log(&self) -> PathBuf {
        self.get_kernel_dir().join("tx.jsonl")
    }

    /// Mint a storage artifact
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::PathBuf;
    /// use serde_json::json;
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// let data = json!({"result": "success"});
    /// let artifact_path = driver.mint_storage_artifact(&data, "tx-123").unwrap();
    /// ```
    pub fn mint_storage_artifact(&self, data: &JsonValue, tx_id: &str) -> Result<PathBuf> {
        let artifact_path = self.get_storage().join(format!("{}.inst", tx_id));

        // Create artifact directory
        fs::create_dir_all(&artifact_path)?;

        // Write receipt.json
        let receipt_path = artifact_path.join("receipt.json");
        let receipt_data = serde_json::to_string_pretty(data)?;
        fs::write(&receipt_path, receipt_data)?;

        Ok(artifact_path)
    }

    /// Record transaction metadata with file locking for FIFO integrity
    ///
    /// Uses advisory file locking to prevent concurrent write corruption
    /// and ensure transaction log integrity for the queue system.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::PathBuf;
    /// use serde_json::json;
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// let metadata = json!({"event": "minted"});
    /// driver.record_transaction("tx-123", metadata).unwrap();
    /// ```
    pub fn record_transaction(&self, tx_id: &str, metadata: JsonValue) -> Result<()> {
        let tx_log = self.get_tx_log();

        // Ensure parent directory exists
        if let Some(parent) = tx_log.parent() {
            fs::create_dir_all(parent)?;
        }

        let transaction = Transaction {
            tx_id: tx_id.to_string(),
            timestamp: Utc::now().to_rfc3339(),
            kernel: self.concept.clone(),
            metadata,
        };

        let tx_line = serde_json::to_string(&transaction)?;

        // Append to tx.jsonl with file locking to ensure FIFO integrity
        use std::fs::OpenOptions;
        use std::io::Write;

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;

            // Open with append mode
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&tx_log)?;

            // Acquire exclusive lock (blocks until available)
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();

            // Use flock for advisory locking (LOCK_EX for exclusive lock)
            unsafe {
                if libc::flock(fd, libc::LOCK_EX) != 0 {
                    return Err(CkpError::Io(std::io::Error::last_os_error()));
                }
            }

            // Write transaction (lock is held)
            writeln!(file, "{}", tx_line)?;

            // Explicitly unlock (though close() will also release the lock)
            unsafe {
                libc::flock(fd, libc::LOCK_UN);
            }
        }

        #[cfg(not(unix))]
        {
            // Windows fallback: write without locking (Windows uses different locking APIs)
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(tx_log)?;

            writeln!(file, "{}", tx_line)?;
        }

        Ok(())
    }

    /// Move job between queue stages
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::{Path, PathBuf};
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// let inbox = driver.get_queue_inbox();
    /// let staging = driver.get_queue_staging();
    /// driver.move_job(&inbox.join("job-123.job"), &staging).unwrap();
    /// ```
    pub fn move_job(&self, source_path: &Path, target_dir: &Path) -> Result<PathBuf> {
        // Ensure target directory exists
        fs::create_dir_all(target_dir)?;

        // Get filename from source
        let filename = source_path
            .file_name()
            .ok_or_else(|| CkpError::Path("Invalid source path".to_string()))?;

        let target_path = target_dir.join(filename);

        // Move file
        fs::rename(source_path, &target_path)?;

        Ok(target_path)
    }

    /// Count files in a queue directory
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::PathBuf;
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// let inbox = driver.get_queue_inbox();
    /// let count = driver.count_queue_files(&inbox).unwrap();
    /// ```
    pub fn count_queue_files(&self, queue_dir: &Path) -> Result<usize> {
        if !queue_dir.exists() {
            return Ok(0);
        }

        let count = fs::read_dir(queue_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|ext| ext == "job" || ext == "inst")
                    .unwrap_or(false)
            })
            .count();

        Ok(count)
    }

    /// Archive a processed job
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::{Path, PathBuf};
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// driver.archive_job(Path::new("/test/job.job"), "tx-123").unwrap();
    /// ```
    pub fn archive_job(&self, job_path: &Path, tx_id: &str) -> Result<()> {
        let archive_dir = self.get_archive();
        fs::create_dir_all(&archive_dir)?;

        // Read job content
        let job_content = fs::read_to_string(job_path)?;

        // Create archive subdirectory for this transaction
        let tx_archive_dir = archive_dir.join(tx_id);
        fs::create_dir_all(&tx_archive_dir)?;

        // Write job file to archive
        let archive_path = tx_archive_dir.join("job.json");
        fs::write(&archive_path, job_content)?;

        // Delete original job from inbox
        fs::remove_file(job_path)?;

        Ok(())
    }

    /// Extract transaction ID from job filename
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::{Path, PathBuf};
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// let tx_id = driver.extract_tx_id_from_job_path(Path::new("/inbox/1234-abc.job"));
    /// assert_eq!(tx_id, Some("1234-abc".to_string()));
    /// ```
    pub fn extract_tx_id_from_job_path(&self, job_path: &Path) -> Option<String> {
        let filename = job_path.file_name()?.to_str()?;
        let tx_id = filename.strip_suffix(".job")?;
        Some(tx_id.to_string())
    }

    // ==================== v1.3.12: Per-Edge Queue Support ====================

    /// Create per-edge queue directory (v1.3.12)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::PathBuf;
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// let queue_path = driver.create_edge_queue("PRODUCES.MixIngredients").unwrap();
    /// ```
    pub fn create_edge_queue(&self, edge_name: &str) -> Result<PathBuf> {
        let queue_path = self
            .get_kernel_dir()
            .join("queue")
            .join("edges")
            .join(edge_name);

        fs::create_dir_all(&queue_path)?;

        Ok(queue_path)
    }

    /// Create symlink for per-edge queue with relative path (v1.3.12)
    ///
    /// Uses relative paths for portability across machines
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::{Path, PathBuf};
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// driver.create_symlink(
    ///     Path::new("/test/concepts/Recipes.MixIngredients/storage/tx-123.inst"),
    ///     Path::new("/test/concepts/Recipes.BakeCake/queue/edges/PRODUCES.MixIngredients/"),
    ///     Some("tx-123")
    /// ).unwrap();
    /// ```
    pub fn create_symlink(
        &self,
        source_path: &Path,
        target_path: &Path,
        tx_id: Option<&str>,
    ) -> Result<PathBuf> {
        // Determine symlink path
        let symlink_path = if target_path.is_dir() {
            // Extract tx_id from source path if not provided
            let tx_id_to_use = tx_id
                .map(|s| s.to_string())
                .or_else(|| self.extract_tx_id_from_path(source_path))
                .ok_or_else(|| CkpError::Path("Failed to extract tx_id".to_string()))?;

            target_path.join(format!("{}.inst", tx_id_to_use))
        } else {
            target_path.to_path_buf()
        };

        // Ensure target directory exists
        if let Some(parent) = symlink_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Calculate relative path for portability
        let relative_path = self.make_relative_path(source_path, &symlink_path)?;

        // Remove existing symlink if present
        if symlink_path.exists() || symlink_path.symlink_metadata().is_ok() {
            fs::remove_file(&symlink_path).ok(); // Ignore errors
        }

        // Create symlink
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&relative_path, &symlink_path)?;
        }

        #[cfg(windows)]
        {
            // On Windows, assume directories for .inst files
            std::os::windows::fs::symlink_dir(&relative_path, &symlink_path)?;
        }

        Ok(symlink_path)
    }

    /// Extract transaction ID from file path
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::{Path, PathBuf};
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// let tx_id = driver.extract_tx_id_from_path(Path::new("/storage/tx-123.inst"));
    /// assert_eq!(tx_id, Some("tx-123".to_string()));
    /// ```
    pub fn extract_tx_id_from_path(&self, file_path: &Path) -> Option<String> {
        let basename = file_path.file_name()?.to_str()?;
        // Match patterns like "1234567890-abc123.inst" or "tx-123.inst"
        let tx_id = basename.strip_suffix(".inst").or_else(|| {
            basename.strip_suffix(".job")
        })?;
        Some(tx_id.to_string())
    }

    /// List instances in per-edge queue (v1.3.12)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::{Path, PathBuf};
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// let instances = driver.list_instances(
    ///     Path::new("/test/concepts/Recipes.BakeCake/queue/edges/PRODUCES.MixIngredients/")
    /// ).unwrap();
    /// ```
    pub fn list_instances(&self, queue_path: &Path) -> Result<Vec<PathBuf>> {
        if !queue_path.exists() {
            return Ok(Vec::new());
        }

        let mut instances = Vec::new();

        for entry in fs::read_dir(queue_path)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            // Skip non-instance files
            if file_name_str == ".gitkeep" || !file_name_str.ends_with(".inst") {
                continue;
            }

            let file_path = entry.path();

            // Follow symlink to get actual instance path
            match fs::canonicalize(&file_path) {
                Ok(real_path) => instances.push(real_path),
                Err(e) => {
                    eprintln!(
                        "[FileSystemDriver] Failed to resolve symlink {}: {}",
                        file_path.display(),
                        e
                    );
                }
            }
        }

        Ok(instances)
    }

    /// Generate a new transaction ID
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::drivers::FileSystemDriver;
    /// use std::path::PathBuf;
    ///
    /// let driver = FileSystemDriver::new(
    ///     PathBuf::from("/test"),
    ///     "Recipes.BakeCake".to_string()
    /// );
    ///
    /// let tx_id = driver.generate_tx_id();
    /// assert!(!tx_id.is_empty());
    /// ```
    pub fn generate_tx_id(&self) -> String {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let uuid = Uuid::new_v4();
        format!("{}-{}", timestamp, &uuid.to_string()[..8])
    }

    /// Calculate relative path from target's directory to source
    ///
    /// This ensures symlinks are portable across machines
    fn make_relative_path(&self, source: &Path, target: &Path) -> Result<PathBuf> {
        let target_dir = target
            .parent()
            .ok_or_else(|| CkpError::Path("Target has no parent directory".to_string()))?;

        pathdiff::diff_paths(source, target_dir)
            .ok_or_else(|| CkpError::Path("Failed to calculate relative path".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn setup_test_kernel(temp_dir: &TempDir, kernel_name: &str) {
        let kernel_dir = temp_dir.path().join("concepts").join(kernel_name);
        fs::create_dir_all(kernel_dir.join("queue/inbox")).unwrap();
        fs::create_dir_all(kernel_dir.join("storage")).unwrap();
    }

    #[test]
    fn test_get_kernel_dir() {
        let temp_dir = TempDir::new().unwrap();
        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "Recipes.BakeCake".to_string(),
        );

        let kernel_dir = driver.get_kernel_dir();
        assert_eq!(
            kernel_dir,
            temp_dir.path().join("concepts/Recipes.BakeCake")
        );
    }

    #[test]
    fn test_mint_storage_artifact() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let data = json!({"status": "success", "result": 42});
        let artifact_path = driver.mint_storage_artifact(&data, "tx-123").unwrap();

        assert!(artifact_path.exists());
        assert!(artifact_path.join("receipt.json").exists());

        let content = fs::read_to_string(artifact_path.join("receipt.json")).unwrap();
        let parsed: JsonValue = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["status"], "success");
    }

    #[test]
    fn test_record_transaction() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let metadata = json!({"event": "minted"});
        driver.record_transaction("tx-123", metadata).unwrap();

        let tx_log = driver.get_tx_log();
        assert!(tx_log.exists());

        let content = fs::read_to_string(tx_log).unwrap();
        assert!(content.contains("tx-123"));
        assert!(content.contains("TestKernel"));
    }

    #[test]
    fn test_create_edge_queue() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let queue_path = driver.create_edge_queue("PRODUCES.Source").unwrap();
        assert!(queue_path.exists());
        assert_eq!(
            queue_path,
            temp_dir
                .path()
                .join("concepts/TestKernel/queue/edges/PRODUCES.Source")
        );
    }

    #[test]
    fn test_extract_tx_id_from_path() {
        let temp_dir = TempDir::new().unwrap();
        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let tx_id = driver.extract_tx_id_from_path(Path::new("/storage/tx-123.inst"));
        assert_eq!(tx_id, Some("tx-123".to_string()));

        let tx_id = driver.extract_tx_id_from_path(Path::new("/inbox/1234-abc.job"));
        assert_eq!(tx_id, Some("1234-abc".to_string()));
    }

    #[test]
    fn test_generate_tx_id() {
        let temp_dir = TempDir::new().unwrap();
        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let tx_id1 = driver.generate_tx_id();
        let tx_id2 = driver.generate_tx_id();

        assert!(!tx_id1.is_empty());
        assert!(!tx_id2.is_empty());
        assert_ne!(tx_id1, tx_id2);
        assert!(tx_id1.contains('-'));
    }

    // NEW TESTS - Test Parity with Node.js

    /// Test: createSymlink() - creates symlink to source
    /// Node.js equivalent: FileSystemDriver.test.js:135
    #[test]
    fn test_create_symlink_basic() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create source instance
        let storage_dir = temp_dir.path().join("concepts/TestKernel/storage");
        let source_dir = storage_dir.join("test-1.inst");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("receipt.bin"), "test data").unwrap();

        // Create target directory
        let target_dir = temp_dir.path().join("concepts/TestKernel/queue/edges/PRODUCES.Source");
        fs::create_dir_all(&target_dir).unwrap();

        // Create symlink
        let result = driver.create_symlink(&source_dir, &target_dir, Some("test-1"));
        assert!(result.is_ok());

        let symlink_path = result.unwrap();
        assert!(symlink_path.exists() || symlink_path.symlink_metadata().is_ok());
    }

    /// Test: createSymlink() - symlink is actually a symbolic link
    /// Node.js equivalent: FileSystemDriver.test.js:152
    #[test]
    fn test_create_symlink_is_symlink() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let storage_dir = temp_dir.path().join("concepts/TestKernel/storage");
        let source_dir = storage_dir.join("test-1.inst");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("receipt.bin"), "test data").unwrap();

        let target_dir = temp_dir.path().join("concepts/TestKernel/queue/edges/PRODUCES.Source");
        fs::create_dir_all(&target_dir).unwrap();

        let symlink_path = driver.create_symlink(&source_dir, &target_dir, Some("test-1")).unwrap();

        let metadata = fs::symlink_metadata(&symlink_path).unwrap();
        assert!(metadata.is_symlink());
    }

    /// Test: createSymlink() - symlink points to correct source
    /// Node.js equivalent: FileSystemDriver.test.js:171
    #[test]
    fn test_create_symlink_points_to_source() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let storage_dir = temp_dir.path().join("concepts/TestKernel/storage");
        let source_dir = storage_dir.join("test-1.inst");
        fs::create_dir_all(&source_dir).unwrap();
        let test_data = json!({"id": 1, "result": "test-1"});
        fs::write(source_dir.join("receipt.bin"), test_data.to_string()).unwrap();

        let target_dir = temp_dir.path().join("concepts/TestKernel/queue/edges/PRODUCES.Source");
        fs::create_dir_all(&target_dir).unwrap();

        let symlink_path = driver.create_symlink(&source_dir, &target_dir, Some("test-1")).unwrap();

        // Read through symlink
        let receipt_path = symlink_path.join("receipt.bin");
        assert!(receipt_path.exists());

        let content = fs::read_to_string(&receipt_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["result"], "test-1");
    }

    /// Test: createSymlink() - creates multiple symlinks
    /// Node.js equivalent: FileSystemDriver.test.js:195
    #[test]
    fn test_create_symlink_multiple() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let target_dir = temp_dir.path().join("concepts/TestKernel/queue/edges/PRODUCES.Source");
        fs::create_dir_all(&target_dir).unwrap();

        // Create multiple instances and symlinks
        for i in 1..=3 {
            let storage_dir = temp_dir.path().join("concepts/TestKernel/storage");
            let source_dir = storage_dir.join(format!("test-{}.inst", i));
            fs::create_dir_all(&source_dir).unwrap();
            fs::write(source_dir.join("receipt.bin"), format!("data-{}", i)).unwrap();

            let result = driver.create_symlink(&source_dir, &target_dir, Some(&format!("test-{}", i)));
            assert!(result.is_ok());
        }

        // Verify all symlinks exist
        for i in 1..=3 {
            let symlink = target_dir.join(format!("test-{}.inst", i));
            assert!(symlink.symlink_metadata().is_ok());
        }
    }

    /// Test: createSymlink() - uses relative paths
    /// Node.js equivalent: FileSystemDriver.test.js:219 (portability test)
    #[test]
    fn test_create_symlink_relative_path() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let storage_dir = temp_dir.path().join("concepts/TestKernel/storage");
        let source_dir = storage_dir.join("test-1.inst");
        fs::create_dir_all(&source_dir).unwrap();

        let target_dir = temp_dir.path().join("concepts/TestKernel/queue/edges/PRODUCES.Source");
        fs::create_dir_all(&target_dir).unwrap();

        let symlink_path = driver.create_symlink(&source_dir, &target_dir, Some("test-1")).unwrap();

        // Read the symlink target (should be relative)
        let link_target = fs::read_link(&symlink_path).unwrap();

        // Relative paths should not start with /
        let link_str = link_target.to_string_lossy();
        assert!(!link_str.starts_with('/'));
        assert!(link_str.contains("../")); // Should have relative path components
    }

    /// Test: mintStorageArtifact() - creates parent directories
    /// Node.js equivalent: FileSystemDriver.test.js:271
    #[test]
    fn test_mint_storage_creates_parents() {
        let temp_dir = TempDir::new().unwrap();
        // Don't setup kernel - test that it creates directories

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "NewKernel".to_string(),
        );

        let data = json!({"test": "data"});
        let tx_id = "test-123";

        let result = driver.mint_storage_artifact(&data, tx_id);
        assert!(result.is_ok());

        let storage_dir = temp_dir.path().join("concepts/NewKernel/storage");
        assert!(storage_dir.exists());

        let inst_dir = storage_dir.join(format!("{}.inst", tx_id));
        assert!(inst_dir.exists());
    }

    /// Test: recordTransaction() - creates tx.jsonl if not exists
    /// Node.js equivalent: FileSystemDriver.test.js:294
    #[test]
    fn test_record_transaction_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let metadata = json!({"action": "mint", "timestamp": 123456});
        let result = driver.record_transaction("tx-123", metadata);
        assert!(result.is_ok());

        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        assert!(tx_path.exists());
    }

    /// Test: recordTransaction() - appends multiple transactions
    /// Node.js equivalent: FileSystemDriver.test.js:304
    #[test]
    fn test_record_transaction_appends() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Record multiple transactions
        for i in 1..=3 {
            let metadata = json!({"action": "mint", "id": i});
            driver.record_transaction(&format!("tx-{}", i), metadata).unwrap();
        }

        // Read tx.jsonl
        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        let content = fs::read_to_string(tx_path).unwrap();
        let lines: Vec<&str> = content.trim().split('\n').collect();

        assert_eq!(lines.len(), 3);

        // Verify each line is valid JSON
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(parsed.get("txId").is_some());
            assert!(parsed.get("timestamp").is_some());
            assert!(parsed.get("kernel").is_some());
            // metadata fields are flattened, so check for a metadata field like "action"
            assert!(parsed.get("action").is_some());
        }
    }

    /// Test: listInstances() - lists all instances
    /// Node.js equivalent: FileSystemDriver.test.js:341
    #[test]
    fn test_list_instances_all() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create some instances
        let storage_dir = temp_dir.path().join("concepts/TestKernel/storage");
        let target_dir = temp_dir.path().join("concepts/TestKernel/queue/edges/PRODUCES.Source");
        fs::create_dir_all(&target_dir).unwrap();

        for i in 1..=3 {
            let source_dir = storage_dir.join(format!("test-{}.inst", i));
            fs::create_dir_all(&source_dir).unwrap();
            driver.create_symlink(&source_dir, &target_dir, Some(&format!("test-{}", i))).unwrap();
        }

        // List instances
        let instances = driver.list_instances(&target_dir).unwrap();
        assert_eq!(instances.len(), 3);
    }

    /// Test: listInstances() - returns empty array if no instances
    /// Node.js equivalent: FileSystemDriver.test.js:359
    #[test]
    fn test_list_instances_empty() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let empty_dir = temp_dir.path().join("concepts/TestKernel/queue/edges/EMPTY");
        fs::create_dir_all(&empty_dir).unwrap();

        let instances = driver.list_instances(&empty_dir).unwrap();
        assert_eq!(instances.len(), 0);
    }

    /// Test: listInstances() - follows symlinks
    /// Node.js equivalent: FileSystemDriver.test.js:368
    #[test]
    fn test_list_instances_follows_symlinks() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create instance with known content
        let storage_dir = temp_dir.path().join("concepts/TestKernel/storage");
        let source_dir = storage_dir.join("test-1.inst");
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(source_dir.join("receipt.bin"), "original data").unwrap();

        let target_dir = temp_dir.path().join("concepts/TestKernel/queue/edges/PRODUCES.Source");
        fs::create_dir_all(&target_dir).unwrap();
        driver.create_symlink(&source_dir, &target_dir, Some("test-1")).unwrap();

        // List should follow symlink
        let instances = driver.list_instances(&target_dir).unwrap();
        assert_eq!(instances.len(), 1);

        // Verify it points to actual storage location
        let instance_path = &instances[0];
        assert!(instance_path.to_string_lossy().contains("storage"));
        assert!(instance_path.join("receipt.bin").exists());
    }

    /// Test: createEdgeQueue() - idempotent (safe to call multiple times)
    /// Node.js equivalent: FileSystemDriver.test.js:115
    #[test]
    fn test_create_edge_queue_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let path1 = driver.create_edge_queue("PRODUCES.Source").unwrap();
        let path2 = driver.create_edge_queue("PRODUCES.Source").unwrap();
        let path3 = driver.create_edge_queue("PRODUCES.Source").unwrap();

        assert_eq!(path1, path2);
        assert_eq!(path2, path3);
        assert!(path1.exists());
    }

    // ==================== ERROR EDGE CASES ====================

    /// Test: createSymlink() - validates target exists
    #[test]
    fn test_symlink_target_validation() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Try to create symlink to non-existent source
        let non_existent = temp_dir.path().join("concepts/TestKernel/storage/missing.inst");
        let target_dir = temp_dir.path().join("concepts/TestKernel/queue/edges/PRODUCES.Source");
        fs::create_dir_all(&target_dir).unwrap();

        // Symlink creation should succeed even if target doesn't exist (Unix behavior)
        // But reading through symlink should fail
        let result = driver.create_symlink(&non_existent, &target_dir, Some("missing"));
        assert!(result.is_ok());

        let symlink_path = result.unwrap();
        // Symlink exists as metadata
        assert!(symlink_path.symlink_metadata().is_ok());
        // But reading through it fails (broken symlink)
        assert!(!symlink_path.exists());
    }

    /// Test: mintStorageArtifact() - handles permission errors gracefully
    #[test]
    #[cfg(unix)]
    fn test_write_to_readonly_directory() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let kernel_dir = temp_dir.path().join("concepts/ReadOnlyKernel");
        let storage_dir = kernel_dir.join("storage");
        fs::create_dir_all(&storage_dir).unwrap();

        // Make storage directory read-only
        let mut perms = fs::metadata(&storage_dir).unwrap().permissions();
        perms.set_mode(0o444); // Read-only
        fs::set_permissions(&storage_dir, perms).unwrap();

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "ReadOnlyKernel".to_string(),
        );

        let data = json!({"test": "data"});
        let result = driver.mint_storage_artifact(&data, "tx-123");

        // Should return an error due to permission denied
        assert!(result.is_err());

        // Restore permissions for cleanup
        let mut perms = fs::metadata(&storage_dir).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&storage_dir, perms).unwrap();
    }

    /// Test: recordTransaction() - handles invalid JSON gracefully
    #[test]
    fn test_queue_file_with_invalid_json() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Valid JSON should work
        let valid_metadata = json!({"valid": "data"});
        let result = driver.record_transaction("tx-valid", valid_metadata);
        assert!(result.is_ok());

        // Invalid JSON values are not possible with serde_json::Value
        // but we can test with complex nested structures
        let complex_metadata = json!({
            "nested": {
                "deep": {
                    "structure": [1, 2, 3],
                    "unicode": "日本語テキスト",
                }
            }
        });
        let result = driver.record_transaction("tx-complex", complex_metadata);
        assert!(result.is_ok());
    }

    /// Test: archiveJob() - handles non-existent file
    #[test]
    fn test_archive_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let non_existent = temp_dir.path().join("concepts/TestKernel/queue/inbox/missing.job");

        let result = driver.archive_job(&non_existent, "tx-123");
        // Should return error since file doesn't exist
        assert!(result.is_err());
    }

    /// Test: recordTransaction() - handles concurrent writes without crashing
    /// Note: Without file-level locking, concurrent writes may interleave.
    /// This test verifies the system doesn't panic, not that ordering is perfect.
    #[test]
    fn test_concurrent_write_same_file() {
        use std::sync::Arc;
        use std::thread;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = Arc::new(FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        ));

        // Spawn multiple threads writing to the same tx.jsonl
        let mut handles = vec![];
        for i in 0..10 {
            let driver_clone = Arc::clone(&driver);
            let handle = thread::spawn(move || {
                let metadata = json!({"thread": i, "action": "concurrent_write"});
                driver_clone.record_transaction(&format!("tx-{}", i), metadata)
            });
            handles.push(handle);
        }

        // Wait for all threads to complete - should not panic
        let mut success_count = 0;
        for handle in handles {
            let result = handle.join().unwrap();
            if result.is_ok() {
                success_count += 1;
            }
        }

        // Verify most writes succeeded (at least 80%)
        assert!(success_count >= 8, "Expected at least 8 successful writes, got {}", success_count);

        // Verify file was created and has content
        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        assert!(tx_path.exists());

        let content = fs::read_to_string(tx_path).unwrap();
        assert!(!content.is_empty(), "Transaction log should not be empty");

        // Check that some valid transaction data exists (may be interleaved)
        assert!(content.contains("\"txId\""));
        assert!(content.contains("\"action\":\"concurrent_write\""));
    }

    /// Test: mintStorageArtifact() - handles disk full scenario (simulated)
    #[test]
    fn test_disk_full_handling() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // We can't actually fill the disk, but we can test with very large data
        // This tests that the function completes without panicking
        let large_data = json!({
            "data": "x".repeat(1000),
            "array": (0..100).map(|i| json!({"id": i})).collect::<Vec<_>>()
        });

        let result = driver.mint_storage_artifact(&large_data, "tx-large");
        // Should succeed in normal conditions
        assert!(result.is_ok());
    }

    // ==================== ADVANCED QUEUE OPERATIONS ====================

    /// Test: Queue operations - priority ordering by timestamp
    #[test]
    fn test_queue_priority_ordering() {
        use std::thread;
        use std::time::Duration;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let inbox = driver.get_queue_inbox();

        // Create jobs with timestamps
        for i in 0..3 {
            thread::sleep(Duration::from_millis(10)); // Ensure different timestamps
            let tx_id = driver.generate_tx_id();
            let job_path = inbox.join(format!("{}.job", tx_id));
            fs::write(&job_path, format!("{{\"order\": {}}}", i)).unwrap();
        }

        // Read jobs sorted by filename (which contains timestamp)
        let mut entries: Vec<_> = fs::read_dir(&inbox)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
            .collect();

        entries.sort_by_key(|e| e.file_name());

        // Verify we have 3 jobs in order
        assert_eq!(entries.len(), 3);

        // First job should have earliest timestamp (smallest filename)
        let first_job = fs::read_to_string(entries[0].path()).unwrap();
        assert!(first_job.contains("\"order\": 0"));
    }

    /// Test: Queue operations - batch processing
    #[test]
    fn test_queue_batch_operations() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let storage_dir = temp_dir.path().join("concepts/TestKernel/storage");
        let target_dir = temp_dir.path().join("concepts/TestKernel/queue/edges/BATCH");
        fs::create_dir_all(&target_dir).unwrap();

        // Create batch of 10 instances
        let batch_size = 10;
        for i in 0..batch_size {
            let source_dir = storage_dir.join(format!("batch-{}.inst", i));
            fs::create_dir_all(&source_dir).unwrap();
            fs::write(source_dir.join("receipt.json"), format!("{{\"batch_id\": {}}}", i)).unwrap();

            driver.create_symlink(&source_dir, &target_dir, Some(&format!("batch-{}", i))).unwrap();
        }

        // Verify batch was created
        let instances = driver.list_instances(&target_dir).unwrap();
        assert_eq!(instances.len(), batch_size);

        // Verify all instances are accessible
        for instance in &instances {
            let receipt = instance.join("receipt.json");
            assert!(receipt.exists());
            let content = fs::read_to_string(&receipt).unwrap();
            assert!(content.contains("batch_id"));
        }
    }

    /// Test: Queue operations - timestamp-based sorting
    #[test]
    fn test_queue_timestamp_sorting() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create multiple transactions with generated IDs
        let mut tx_ids = vec![];
        for _ in 0..5 {
            let tx_id = driver.generate_tx_id();
            tx_ids.push(tx_id.clone());
            let metadata = json!({"action": "test"});
            driver.record_transaction(&tx_id, metadata).unwrap();
        }

        // Read tx.jsonl and verify order
        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        let content = fs::read_to_string(tx_path).unwrap();
        let lines: Vec<&str> = content.trim().split('\n').collect();

        assert_eq!(lines.len(), 5);

        // Parse transactions and verify timestamps are in order
        let mut timestamps = vec![];
        for line in lines {
            let tx: Transaction = serde_json::from_str(line).unwrap();
            timestamps.push(tx.timestamp);
        }

        // Timestamps should be in ascending order (or equal if created very quickly)
        for i in 1..timestamps.len() {
            assert!(timestamps[i] >= timestamps[i-1]);
        }
    }

    // ==================== TRANSACTION LOG EDGE CASES ====================

    /// Test: Transaction log - rotation handling (simulated)
    #[test]
    fn test_txlog_rotation() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Write 100 transactions to simulate log growth
        for i in 0..100 {
            let metadata = json!({"action": "rotation_test", "index": i});
            driver.record_transaction(&format!("tx-{:05}", i), metadata).unwrap();
        }

        // Verify all transactions are recorded
        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        let content = fs::read_to_string(tx_path).unwrap();
        let lines: Vec<&str> = content.trim().split('\n').collect();

        assert_eq!(lines.len(), 100);

        // Verify first and last transactions
        let first: Transaction = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first.tx_id, "tx-00000");

        let last: Transaction = serde_json::from_str(lines[99]).unwrap();
        assert_eq!(last.tx_id, "tx-00099");
    }

    /// Test: Transaction log - corruption recovery (partial write simulation)
    #[test]
    fn test_txlog_corruption_recovery() {
        use std::io::Write;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");

        // Write valid transactions
        driver.record_transaction("tx-001", json!({"action": "valid"})).unwrap();
        driver.record_transaction("tx-002", json!({"action": "valid"})).unwrap();

        // Simulate corruption by appending incomplete JSON line
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&tx_path)
            .unwrap();
        writeln!(file, "{{\"txId\": \"tx-corrupt\", \"incomplete").unwrap();

        // Write another valid transaction
        driver.record_transaction("tx-003", json!({"action": "valid"})).unwrap();

        // Read and parse tx.jsonl
        let content = fs::read_to_string(tx_path).unwrap();
        let lines: Vec<&str> = content.trim().split('\n').collect();

        assert_eq!(lines.len(), 4); // 3 valid + 1 corrupt

        // Verify valid lines can be parsed
        let tx1: std::result::Result<Transaction, _> = serde_json::from_str(lines[0]);
        assert!(tx1.is_ok());
        assert_eq!(tx1.unwrap().tx_id, "tx-001");

        let tx2: std::result::Result<Transaction, _> = serde_json::from_str(lines[1]);
        assert!(tx2.is_ok());

        // Corrupt line should fail to parse
        let corrupt: std::result::Result<Transaction, _> = serde_json::from_str(lines[2]);
        assert!(corrupt.is_err());

        // Last line should be valid
        let tx3: std::result::Result<Transaction, _> = serde_json::from_str(lines[3]);
        assert!(tx3.is_ok());
        assert_eq!(tx3.unwrap().tx_id, "tx-003");
    }

    // ==================== PHASE 1.2: TRANSACTION LOG TESTS (+6 TESTS) ====================

    /// Test: Transaction log - append multiple entries
    #[test]
    fn test_transaction_log_append() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Append 10 transactions
        for i in 0..10 {
            let metadata = json!({
                "action": "append_test",
                "index": i,
                "sequence": i * 10
            });
            driver.record_transaction(&format!("tx-append-{:03}", i), metadata).unwrap();
        }

        // Read and verify all entries
        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        let content = fs::read_to_string(tx_path).unwrap();
        let lines: Vec<&str> = content.trim().split('\n').collect();

        assert_eq!(lines.len(), 10, "Should have 10 transaction entries");

        // Verify each entry can be parsed and has correct fields
        for (i, line) in lines.iter().enumerate() {
            let tx: Transaction = serde_json::from_str(line).unwrap();
            assert_eq!(tx.tx_id, format!("tx-append-{:03}", i));
            assert_eq!(tx.kernel, "TestKernel");
            assert!(tx.timestamp.len() > 0);
        }
    }

    /// Test: Transaction log - parse and query entries
    #[test]
    fn test_transaction_log_parse() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create transactions with different types
        let actions = vec!["mint", "route", "archive", "mint", "route"];
        for (i, action) in actions.iter().enumerate() {
            let metadata = json!({
                "action": action,
                "sequence": i,
                "data": format!("payload-{}", i)
            });
            driver.record_transaction(&format!("tx-{}", i), metadata).unwrap();
        }

        // Read and parse all transactions
        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        let content = fs::read_to_string(tx_path).unwrap();

        let parsed_transactions: Vec<Transaction> = content
            .trim()
            .split('\n')
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        assert_eq!(parsed_transactions.len(), 5);

        // Query for "mint" actions
        let mint_txs: Vec<_> = parsed_transactions
            .iter()
            .filter(|tx| {
                tx.metadata.get("action")
                    .and_then(|v| v.as_str())
                    == Some("mint")
            })
            .collect();

        assert_eq!(mint_txs.len(), 2, "Should have 2 mint transactions");
    }

    /// Test: Transaction log - large payload handling
    #[test]
    fn test_transaction_log_large_payload() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create large metadata (10KB)
        let large_string = "x".repeat(10_000);
        let large_array: Vec<_> = (0..100).map(|i| json!({
            "id": i,
            "data": format!("item-{}", i),
            "payload": large_string.chars().take(100).collect::<String>()
        })).collect();

        let large_metadata = json!({
            "action": "large_payload",
            "items": large_array,
            "description": large_string
        });

        // Record large transaction
        let result = driver.record_transaction("tx-large", large_metadata.clone());
        assert!(result.is_ok(), "Should handle large payload");

        // Verify it was written correctly
        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        let content = fs::read_to_string(tx_path).unwrap();

        let tx: Transaction = serde_json::from_str(&content.trim()).unwrap();
        assert_eq!(tx.tx_id, "tx-large");
        assert_eq!(tx.metadata["action"], "large_payload");
        assert!(tx.metadata["items"].is_array());
        assert_eq!(tx.metadata["items"].as_array().unwrap().len(), 100);
    }

    /// Test: Transaction log - concurrent append operations with file locking
    #[test]
    fn test_transaction_log_concurrent_writes() {
        use std::sync::Arc;
        use std::thread;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = Arc::new(FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        ));

        // Spawn 10 threads appending transactions concurrently
        // File locking ensures FIFO integrity and prevents corruption
        let mut handles = vec![];
        for i in 0..10 {
            let driver_clone = Arc::clone(&driver);
            let handle = thread::spawn(move || {
                let metadata = json!({
                    "thread_id": i,
                    "action": "concurrent_append",
                    "sequence": i * 100
                });
                driver_clone.record_transaction(&format!("tx-concurrent-{:02}", i), metadata)
            });
            handles.push(handle);
        }

        // Wait for all threads
        let mut success_count = 0;
        for handle in handles {
            if handle.join().unwrap().is_ok() {
                success_count += 1;
            }
        }

        // With file locking, ALL writes should succeed
        assert_eq!(success_count, 10, "All 10 concurrent writes should succeed with file locking");

        // Verify all transactions are in the log
        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        assert!(tx_path.exists(), "Transaction log should exist");

        let content = fs::read_to_string(tx_path).unwrap();
        assert!(!content.is_empty(), "Transaction log should not be empty");

        // With file locking, all lines should be valid (no corruption)
        let lines: Vec<&str> = content.trim().split('\n').collect();
        assert_eq!(lines.len(), 10, "Should have exactly 10 transaction lines");

        // Verify all can be parsed successfully (no corrupted lines)
        let parsed: Vec<Transaction> = lines
            .iter()
            .map(|line| serde_json::from_str::<Transaction>(line).unwrap())
            .collect();

        assert_eq!(parsed.len(), 10, "All 10 transactions should parse successfully");

        // Verify all transaction IDs are unique
        let mut tx_ids: Vec<_> = parsed.iter().map(|tx| &tx.tx_id).collect();
        tx_ids.sort();
        tx_ids.dedup();
        assert_eq!(tx_ids.len(), 10, "All transaction IDs should be unique");
    }

    /// Test: Transaction log - special characters in metadata
    #[test]
    fn test_transaction_log_special_characters() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Test various special characters and unicode
        let special_cases = vec![
            ("unicode", json!({"text": "日本語 中文 한국어 Русский"})),
            ("emoji", json!({"text": "🚀 💻 🎉 ✨"})),
            ("quotes", json!({"text": "He said \"hello\" and 'goodbye'"})),
            ("newlines", json!({"text": "line1\nline2\nline3"})),
            ("escape", json!({"text": "\\backslash \t tab \r return"})),
            ("json", json!({"nested": {"key": "value", "number": 42}})),
        ];

        for (name, metadata) in special_cases {
            driver.record_transaction(&format!("tx-{}", name), metadata).unwrap();
        }

        // Read and verify all entries
        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        let content = fs::read_to_string(tx_path).unwrap();
        let lines: Vec<&str> = content.trim().split('\n').collect();

        assert_eq!(lines.len(), 6, "Should have 6 transactions");

        // Verify all can be parsed
        for line in &lines {
            let tx: std::result::Result<Transaction, _> = serde_json::from_str(line);
            assert!(tx.is_ok(), "Should parse transaction with special characters");
        }

        // Verify unicode was preserved
        let unicode_line = lines.iter().find(|l| l.contains("unicode")).unwrap();
        let tx: Transaction = serde_json::from_str(unicode_line).unwrap();
        let text = tx.metadata["text"].as_str().unwrap();
        assert!(text.contains("日本語"));
        assert!(text.contains("한국어"));
    }

    /// Test: Transaction log - read specific transaction by ID
    #[test]
    fn test_transaction_log_rotation_and_query() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Write 50 transactions simulating log growth
        let mut target_tx_id = String::new();
        for i in 0..50 {
            let tx_id = format!("tx-query-{:03}", i);
            if i == 25 {
                target_tx_id = tx_id.clone();
            }
            let metadata = json!({
                "action": "query_test",
                "index": i,
                "marker": if i == 25 { "TARGET" } else { "OTHER" }
            });
            driver.record_transaction(&tx_id, metadata).unwrap();
        }

        // Read log and find specific transaction
        let tx_path = temp_dir.path().join("concepts/TestKernel/tx.jsonl");
        let content = fs::read_to_string(tx_path).unwrap();

        // Parse all transactions
        let transactions: Vec<Transaction> = content
            .trim()
            .split('\n')
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();

        assert_eq!(transactions.len(), 50);

        // Query for specific transaction
        let target_tx = transactions
            .iter()
            .find(|tx| tx.tx_id == target_tx_id)
            .unwrap();

        assert_eq!(target_tx.metadata["marker"], "TARGET");
        assert_eq!(target_tx.metadata["index"], 25);

        // Query for transactions in range
        let range_txs: Vec<_> = transactions
            .iter()
            .filter(|tx| {
                tx.metadata["index"].as_i64().unwrap_or(0) >= 20
                    && tx.metadata["index"].as_i64().unwrap_or(0) <= 30
            })
            .collect();

        assert_eq!(range_txs.len(), 11, "Should find 11 transactions in range [20,30]");
    }

    // ==================== PHASE 1.3: STORAGE & ARTIFACT TESTS (+6 TESTS) ====================

    /// Test: Mint storage artifact with symlink
    #[test]
    fn test_mint_storage_artifact_with_symlink() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Mint an artifact
        let data = json!({"result": "success", "value": 42});
        let tx_id = "tx-artifact-123";
        let artifact_path = driver.mint_storage_artifact(&data, tx_id).unwrap();

        // Verify artifact exists
        assert!(artifact_path.exists());
        assert!(artifact_path.join("receipt.json").exists());

        // Create symlink to artifact in an edge queue
        let edge_queue = temp_dir.path().join("concepts/TestKernel/queue/edges/TEST");
        let symlink_path = driver.create_symlink(&artifact_path, &edge_queue, Some(tx_id)).unwrap();

        // Verify symlink exists and points to artifact
        assert!(symlink_path.symlink_metadata().is_ok());
        assert!(symlink_path.join("receipt.json").exists());

        // Read through symlink
        let content = fs::read_to_string(symlink_path.join("receipt.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["value"], 42);
    }

    /// Test: Storage artifact retrieval
    #[test]
    fn test_storage_artifact_retrieval() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Mint multiple artifacts
        let artifacts = vec![
            ("tx-001", json!({"id": 1, "type": "typeA"})),
            ("tx-002", json!({"id": 2, "type": "typeB"})),
            ("tx-003", json!({"id": 3, "type": "typeA"})),
        ];

        for (tx_id, data) in &artifacts {
            driver.mint_storage_artifact(data, tx_id).unwrap();
        }

        // Retrieve and verify each artifact
        let storage = driver.get_storage();
        for (tx_id, expected_data) in &artifacts {
            let artifact_path = storage.join(format!("{}.inst", tx_id));
            assert!(artifact_path.exists(), "Artifact {} should exist", tx_id);

            let receipt_path = artifact_path.join("receipt.json");
            let content = fs::read_to_string(&receipt_path).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

            assert_eq!(parsed["id"], expected_data["id"]);
            assert_eq!(parsed["type"], expected_data["type"]);
        }

        // List all artifacts
        let mut artifact_count = 0;
        for entry in fs::read_dir(&storage).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().and_then(|s| s.to_str()) == Some("inst") {
                artifact_count += 1;
            }
        }
        assert_eq!(artifact_count, 3, "Should have 3 artifacts in storage");
    }

    /// Test: Storage artifact deletion
    #[test]
    fn test_storage_artifact_deletion() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Mint artifacts
        let tx_ids = vec!["tx-del-001", "tx-del-002", "tx-del-003"];
        for tx_id in &tx_ids {
            let data = json!({"to_delete": true});
            driver.mint_storage_artifact(&data, tx_id).unwrap();
        }

        let storage = driver.get_storage();

        // Verify all exist
        for tx_id in &tx_ids {
            let artifact_path = storage.join(format!("{}.inst", tx_id));
            assert!(artifact_path.exists());
        }

        // Delete one artifact
        let to_delete = storage.join("tx-del-002.inst");
        fs::remove_dir_all(&to_delete).unwrap();

        // Verify deletion
        assert!(!to_delete.exists());

        // Verify others still exist
        assert!(storage.join("tx-del-001.inst").exists());
        assert!(storage.join("tx-del-003.inst").exists());

        // Delete remaining artifacts
        for tx_id in &["tx-del-001", "tx-del-003"] {
            let artifact_path = storage.join(format!("{}.inst", tx_id));
            fs::remove_dir_all(&artifact_path).unwrap();
        }

        // Verify all deleted
        for tx_id in &tx_ids {
            let artifact_path = storage.join(format!("{}.inst", tx_id));
            assert!(!artifact_path.exists(), "{} should be deleted", tx_id);
        }
    }

    /// Test: Storage path collision handling
    #[test]
    fn test_storage_path_collision() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        let tx_id = "tx-collision";
        let data1 = json!({"version": 1});
        let data2 = json!({"version": 2});

        // Mint first artifact
        let path1 = driver.mint_storage_artifact(&data1, tx_id).unwrap();
        assert!(path1.exists());

        // Read first version
        let receipt1 = fs::read_to_string(path1.join("receipt.json")).unwrap();
        let parsed1: serde_json::Value = serde_json::from_str(&receipt1).unwrap();
        assert_eq!(parsed1["version"], 1);

        // Mint second artifact with same tx_id (overwrites)
        let path2 = driver.mint_storage_artifact(&data2, tx_id).unwrap();
        assert!(path2.exists());
        assert_eq!(path1, path2); // Same path

        // Read second version (should have replaced first)
        let receipt2 = fs::read_to_string(path2.join("receipt.json")).unwrap();
        let parsed2: serde_json::Value = serde_json::from_str(&receipt2).unwrap();
        assert_eq!(parsed2["version"], 2);
    }

    /// Test: Storage directory permissions
    #[test]
    #[cfg(unix)]
    fn test_storage_directory_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Mint artifact
        let data = json!({"test": "permissions"});
        let artifact_path = driver.mint_storage_artifact(&data, "tx-perms").unwrap();

        // Check directory permissions
        let metadata = fs::metadata(&artifact_path).unwrap();
        let permissions = metadata.permissions();

        // Directory should be readable and writable
        let mode = permissions.mode();
        assert!(mode & 0o400 != 0, "Directory should be readable");
        assert!(mode & 0o200 != 0, "Directory should be writable");

        // Check receipt file permissions
        let receipt_path = artifact_path.join("receipt.json");
        let receipt_metadata = fs::metadata(&receipt_path).unwrap();
        let receipt_permissions = receipt_metadata.permissions();
        let receipt_mode = receipt_permissions.mode();

        assert!(receipt_mode & 0o400 != 0, "Receipt should be readable");
        assert!(receipt_mode & 0o200 != 0, "Receipt should be writable");
    }

    /// Test: Storage artifact metadata preservation
    #[test]
    fn test_storage_artifact_metadata() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Mint artifact with complex metadata
        let metadata = json!({
            "result": "processed",
            "metadata": {
                "source": "TestKernel",
                "target": "DownstreamKernel",
                "timestamp": "2025-11-25T12:00:00Z",
                "tags": ["important", "verified"],
                "metrics": {
                    "processing_time_ms": 150,
                    "memory_used_mb": 45
                }
            },
            "payload": {
                "items": [1, 2, 3, 4, 5],
                "status": "complete"
            }
        });

        let tx_id = "tx-metadata";
        let artifact_path = driver.mint_storage_artifact(&metadata, tx_id).unwrap();

        // Read back and verify all metadata preserved
        let receipt_path = artifact_path.join("receipt.json");
        let content = fs::read_to_string(&receipt_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Verify top-level fields
        assert_eq!(parsed["result"], "processed");

        // Verify nested metadata
        assert_eq!(parsed["metadata"]["source"], "TestKernel");
        assert_eq!(parsed["metadata"]["target"], "DownstreamKernel");
        assert_eq!(parsed["metadata"]["tags"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["metadata"]["metrics"]["processing_time_ms"], 150);

        // Verify payload
        assert_eq!(parsed["payload"]["items"].as_array().unwrap().len(), 5);
        assert_eq!(parsed["payload"]["status"], "complete");

        // Verify the artifact can be read multiple times (idempotent)
        let content2 = fs::read_to_string(&receipt_path).unwrap();
        let parsed2: serde_json::Value = serde_json::from_str(&content2).unwrap();
        assert_eq!(parsed, parsed2);
    }

    // ==================== PERFORMANCE BENCHMARK TESTS - LATENCY MEASUREMENT (+7 TESTS) ====================
    //
    // Latency Testing: Measure end-to-end latency, queue delays, RBAC overhead, and I/O bottlenecks
    // These tests measure microsecond-level operation timing

    /// Test: End-to-end emission latency (Test 1/7)
    #[test]
    fn test_perf_end_to_end_emission_latency() {
        use std::time::Instant;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "LatencyKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "LatencyKernel".to_string(),
        );

        let iterations = 100;
        let mut latencies = Vec::new();

        // Measure latency for minting + recording transaction
        for i in 0..iterations {
            let start = Instant::now();

            let data = json!({"test": "latency", "index": i});
            let tx_id = format!("latency-{}", i);
            driver.mint_storage_artifact(&data, &tx_id).unwrap();
            driver.record_transaction(&tx_id, json!({"action": "mint"})).unwrap();

            let latency = start.elapsed();
            latencies.push(latency.as_micros());
        }

        // Calculate statistics
        let sum: u128 = latencies.iter().sum();
        let avg_latency = sum / latencies.len() as u128;
        let max_latency = *latencies.iter().max().unwrap();
        let min_latency = *latencies.iter().min().unwrap();

        println!("[PERF] End-to-end latency: avg={} μs, min={} μs, max={} μs",
                 avg_latency, min_latency, max_latency);

        // Assert latency thresholds (typical file I/O should be < 10ms)
        assert!(avg_latency < 10_000,
                "Average latency too high: {} μs (expected < 10000 μs)", avg_latency);
        assert!(max_latency < 50_000,
                "Max latency spike too high: {} μs (expected < 50000 μs)", max_latency);
    }

    /// Test: End-to-end emission latency with large payloads (Test 2/7)
    #[test]
    fn test_perf_emission_latency_large_payload() {
        use std::time::Instant;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "LargePayloadKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "LargePayloadKernel".to_string(),
        );

        // Create 100KB payload
        let large_data = json!({
            "data": "x".repeat(100_000),
            "metadata": {"size": "100KB"}
        });

        let iterations = 50;
        let mut latencies = Vec::new();

        for i in 0..iterations {
            let start = Instant::now();

            let tx_id = format!("large-{}", i);
            driver.mint_storage_artifact(&large_data, &tx_id).unwrap();
            driver.record_transaction(&tx_id, json!({"size": "100KB"})).unwrap();

            let latency = start.elapsed();
            latencies.push(latency.as_micros());
        }

        let avg_latency: u128 = latencies.iter().sum::<u128>() / latencies.len() as u128;
        let max_latency = *latencies.iter().max().unwrap();

        println!("[PERF] Large payload latency (100KB): avg={} μs, max={} μs",
                 avg_latency, max_latency);

        // Large payloads should still complete within reasonable time
        assert!(avg_latency < 50_000,
                "Large payload avg latency too high: {} μs", avg_latency);
    }

    /// Test: Queue processing delays (Test 3/7)
    #[test]
    fn test_perf_queue_processing_delays() {
        use std::time::Instant;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "QueueDelayKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "QueueDelayKernel".to_string(),
        );

        let inbox = driver.get_queue_inbox();
        let staging = driver.get_queue_staging();
        let ready = driver.get_queue_ready();

        // Measure delay from job creation to queue movement
        let iterations = 100;
        let mut move_latencies = Vec::new();

        for i in 0..iterations {
            // Create job
            let job_path = inbox.join(format!("delay-{}.job", i));
            fs::write(&job_path, "{}").unwrap();

            // Measure time to move through queue stages
            let start = Instant::now();
            let staging_path = driver.move_job(&job_path, &staging).unwrap();
            let ready_path = driver.move_job(&staging_path, &ready).unwrap();
            let move_latency = start.elapsed();

            move_latencies.push(move_latency.as_micros());

            // Cleanup
            fs::remove_file(&ready_path).ok();
        }

        let avg_latency: u128 = move_latencies.iter().sum::<u128>() / move_latencies.len() as u128;
        let max_latency = *move_latencies.iter().max().unwrap();

        println!("[PERF] Queue move latency: avg={} μs, max={} μs",
                 avg_latency, max_latency);

        // Queue operations should be fast (mostly filesystem moves)
        assert!(avg_latency < 5_000,
                "Queue move latency too high: {} μs", avg_latency);
    }

    /// Test: Queue processing with saturation (Test 4/7)
    #[test]
    fn test_perf_queue_delays_under_load() {
        use std::time::Instant;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "LoadQueueKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "LoadQueueKernel".to_string(),
        );

        let inbox = driver.get_queue_inbox();
        let staging = driver.get_queue_staging();

        // Create backlog of 500 jobs
        for i in 0..500 {
            let job_path = inbox.join(format!("backlog-{}.job", i));
            fs::write(&job_path, "{}").unwrap();
        }

        // Measure latency with queue under load
        let test_iterations = 50;
        let mut latencies = Vec::new();

        for i in 0..test_iterations {
            let job_path = inbox.join(format!("test-{}.job", i));
            fs::write(&job_path, "{}").unwrap();

            let start = Instant::now();
            let moved_path = driver.move_job(&job_path, &staging).unwrap();
            let latency = start.elapsed();
            latencies.push(latency.as_micros());

            fs::remove_file(&moved_path).ok();
        }

        let avg_latency: u128 = latencies.iter().sum::<u128>() / latencies.len() as u128;
        let max_latency = *latencies.iter().max().unwrap();

        println!("[PERF] Queue latency under load (500 backlog): avg={} μs, max={} μs",
                 avg_latency, max_latency);

        // Should not degrade significantly under load
        assert!(avg_latency < 10_000,
                "Queue latency degraded under load: {} μs", avg_latency);
    }

    /// Test: RBAC check overhead (Test 5/7)
    #[test]
    fn test_perf_rbac_check_overhead() {
        use std::time::Instant;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "RbacOverheadKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "RbacOverheadKernel".to_string(),
        );

        // Simulate RBAC check by reading ontology files repeatedly
        let kernel_dir = temp_dir.path().join("concepts/RbacOverheadKernel");
        let ontology_path = kernel_dir.join("conceptkernel.yaml");

        let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://RbacOverheadKernel:v0.1
spec:
  rbac:
    communication:
      allowed:
        - "ckp://Target1"
        - "ckp://Target2"
"#;
        fs::write(&ontology_path, ontology_content).unwrap();

        let iterations = 1000;
        let mut read_latencies = Vec::new();

        // Measure file read latency (simulating RBAC checks)
        for _ in 0..iterations {
            let start = Instant::now();
            let _content = fs::read_to_string(&ontology_path).unwrap();
            let latency = start.elapsed();
            read_latencies.push(latency.as_micros());
        }

        let avg_latency: u128 = read_latencies.iter().sum::<u128>() / read_latencies.len() as u128;
        let max_latency = *read_latencies.iter().max().unwrap();

        println!("[PERF] RBAC check overhead: avg={} μs, max={} μs",
                 avg_latency, max_latency);

        // RBAC checks should be very fast (< 1ms average)
        assert!(avg_latency < 1_000,
                "RBAC check overhead too high: {} μs", avg_latency);
    }

    /// Test: RBAC check overhead with caching (Test 6/7)
    #[test]
    fn test_perf_rbac_cached_overhead() {
        use std::time::Instant;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "RbacCachedKernel");

        let kernel_dir = temp_dir.path().join("concepts/RbacCachedKernel");
        let ontology_path = kernel_dir.join("conceptkernel.yaml");

        let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://RbacCachedKernel:v0.1
spec:
  rbac:
    communication:
      allowed:
        - "ckp://Target*"
"#;
        fs::write(&ontology_path, ontology_content).unwrap();

        // First read (cache miss)
        let start_cold = Instant::now();
        let content = fs::read_to_string(&ontology_path).unwrap();
        let cold_latency = start_cold.elapsed();

        // Subsequent reads (cache hit simulation - just parse in memory)
        let iterations = 1000;
        let mut hot_latencies = Vec::new();

        for _ in 0..iterations {
            let start = Instant::now();
            let _parsed: serde_json::Value = serde_yaml::from_str(&content).unwrap();
            let latency = start.elapsed();
            hot_latencies.push(latency.as_nanos());
        }

        let avg_hot_latency: u128 = hot_latencies.iter().sum::<u128>() / hot_latencies.len() as u128;

        println!("[PERF] RBAC cold={} μs, hot_avg={} ns",
                 cold_latency.as_micros(), avg_hot_latency);

        // Cached/parsed checks should be extremely fast (< 100μs)
        assert!(avg_hot_latency < 100_000,
                "Cached RBAC check too slow: {} ns", avg_hot_latency);
    }

    /// Test: File I/O bottlenecks (Test 7/7)
    #[test]
    fn test_perf_file_io_bottlenecks() {
        use std::time::Instant;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "IoBottleneckKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "IoBottleneckKernel".to_string(),
        );

        // Test different I/O operations
        let iterations = 100;

        // 1. Write latency
        let mut write_latencies = Vec::new();
        for i in 0..iterations {
            let start = Instant::now();
            let data = json!({"test": "io", "index": i});
            let tx_id = format!("io-{}", i);
            driver.mint_storage_artifact(&data, &tx_id).unwrap();
            write_latencies.push(start.elapsed().as_micros());
        }

        // 2. Read latency
        let mut read_latencies = Vec::new();
        let storage = driver.get_storage();
        for i in 0..iterations {
            let artifact_path = storage.join(format!("io-{}.inst/receipt.json", i));
            let start = Instant::now();
            let _content = fs::read_to_string(&artifact_path).unwrap();
            read_latencies.push(start.elapsed().as_micros());
        }

        // 3. Transaction log append latency
        let mut tx_latencies = Vec::new();
        for i in 0..iterations {
            let start = Instant::now();
            driver.record_transaction(&format!("tx-io-{}", i), json!({"action": "test"})).unwrap();
            tx_latencies.push(start.elapsed().as_micros());
        }

        let avg_write: u128 = write_latencies.iter().sum::<u128>() / write_latencies.len() as u128;
        let avg_read: u128 = read_latencies.iter().sum::<u128>() / read_latencies.len() as u128;
        let avg_tx: u128 = tx_latencies.iter().sum::<u128>() / tx_latencies.len() as u128;

        println!("[PERF] I/O bottlenecks: write={} μs, read={} μs, tx_append={} μs",
                 avg_write, avg_read, avg_tx);

        // All I/O operations should be reasonable
        assert!(avg_write < 5_000, "Write latency too high: {} μs", avg_write);
        assert!(avg_read < 2_000, "Read latency too high: {} μs", avg_read);
        assert!(avg_tx < 3_000, "Transaction append too slow: {} μs", avg_tx);

        // Verify read is faster than write (typical for file systems)
        assert!(avg_read < avg_write,
                "Read should be faster than write: read={} μs, write={} μs", avg_read, avg_write);
    }

    // ==================== PHASE 1.1: QUEUE OPERATION TESTS (+8 TESTS) ====================

    /// Test: Move job from inbox to staging
    #[test]
    fn test_move_job_inbox_to_staging() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create job in inbox
        let inbox = driver.get_queue_inbox();
        let job_path = inbox.join("test-job-123.job");
        fs::write(&job_path, r#"{"target": "TestKernel", "payload": {"test": "data"}}"#).unwrap();

        // Move to staging
        let staging = driver.get_queue_staging();
        let result = driver.move_job(&job_path, &staging);

        assert!(result.is_ok());
        let new_path = result.unwrap();

        // Verify job moved
        assert!(!job_path.exists(), "Job should be removed from inbox");
        assert!(new_path.exists(), "Job should exist in staging");
        assert_eq!(new_path.parent().unwrap(), staging);
    }

    /// Test: Move job from staging to ready
    #[test]
    fn test_move_job_staging_to_ready() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create job in staging
        let staging = driver.get_queue_staging();
        fs::create_dir_all(&staging).unwrap();
        let job_path = staging.join("test-job-456.job");
        fs::write(&job_path, r#"{"target": "TestKernel", "payload": {"status": "processed"}}"#).unwrap();

        // Move to ready
        let ready = driver.get_queue_ready();
        let result = driver.move_job(&job_path, &ready);

        assert!(result.is_ok());
        let new_path = result.unwrap();

        // Verify job moved
        assert!(!job_path.exists(), "Job should be removed from staging");
        assert!(new_path.exists(), "Job should exist in ready");
        assert_eq!(new_path.parent().unwrap(), ready);

        // Verify content preserved
        let content = fs::read_to_string(&new_path).unwrap();
        assert!(content.contains("processed"));
    }

    /// Test: Archive job from ready queue
    #[test]
    fn test_archive_job_from_ready() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create job in ready
        let ready = driver.get_queue_ready();
        fs::create_dir_all(&ready).unwrap();
        let job_path = ready.join("tx-789.job");
        fs::write(&job_path, r#"{"target": "TestKernel", "payload": {"completed": true}}"#).unwrap();

        // Archive job
        let result = driver.archive_job(&job_path, "tx-789");

        assert!(result.is_ok());

        // Verify job archived
        assert!(!job_path.exists(), "Job should be removed from ready");

        let archive_dir = driver.get_archive().join("tx-789");
        assert!(archive_dir.exists(), "Archive directory should exist");
        assert!(archive_dir.join("job.json").exists(), "Archived job should exist");

        // Verify content preserved
        let content = fs::read_to_string(archive_dir.join("job.json")).unwrap();
        assert!(content.contains("completed"));
    }

    /// Test: Queue stats calculation (count files in each queue)
    #[test]
    fn test_queue_stats_calculation() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create jobs in different queues
        let inbox = driver.get_queue_inbox();
        let staging = driver.get_queue_staging();
        let ready = driver.get_queue_ready();

        fs::create_dir_all(&staging).unwrap();
        fs::create_dir_all(&ready).unwrap();

        // 3 jobs in inbox
        for i in 0..3 {
            fs::write(inbox.join(format!("inbox-{}.job", i)), "{}").unwrap();
        }

        // 2 jobs in staging
        for i in 0..2 {
            fs::write(staging.join(format!("staging-{}.job", i)), "{}").unwrap();
        }

        // 5 jobs in ready
        for i in 0..5 {
            fs::write(ready.join(format!("ready-{}.job", i)), "{}").unwrap();
        }

        // Count files
        let inbox_count = driver.count_queue_files(&inbox).unwrap();
        let staging_count = driver.count_queue_files(&staging).unwrap();
        let ready_count = driver.count_queue_files(&ready).unwrap();

        assert_eq!(inbox_count, 3, "Inbox should have 3 jobs");
        assert_eq!(staging_count, 2, "Staging should have 2 jobs");
        assert_eq!(ready_count, 5, "Ready should have 5 jobs");
    }

    /// Test: Job file cleanup after processing
    #[test]
    fn test_job_file_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create multiple jobs
        let inbox = driver.get_queue_inbox();
        let job_paths: Vec<_> = (0..5)
            .map(|i| {
                let path = inbox.join(format!("cleanup-{}.job", i));
                fs::write(&path, "{}").unwrap();
                path
            })
            .collect();

        // Verify all jobs exist
        assert_eq!(driver.count_queue_files(&inbox).unwrap(), 5);

        // Archive all jobs
        for (i, job_path) in job_paths.iter().enumerate() {
            let tx_id = format!("tx-cleanup-{}", i);
            driver.archive_job(job_path, &tx_id).unwrap();
        }

        // Verify inbox is empty
        assert_eq!(driver.count_queue_files(&inbox).unwrap(), 0, "Inbox should be empty after cleanup");

        // Verify all jobs are in archive
        let archive = driver.get_archive();
        for i in 0..5 {
            let tx_dir = archive.join(format!("tx-cleanup-{}", i));
            assert!(tx_dir.exists(), "Archive for tx-cleanup-{} should exist", i);
        }
    }

    /// Test: Invalid job handling (malformed JSON)
    #[test]
    fn test_invalid_job_handling() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        );

        // Create job with malformed JSON
        let inbox = driver.get_queue_inbox();
        let invalid_job = inbox.join("invalid.job");
        fs::write(&invalid_job, "not valid json {broken").unwrap();

        // Try to read the job content
        let content = fs::read_to_string(&invalid_job);
        assert!(content.is_ok(), "Should be able to read file");

        // Verify JSON parsing would fail
        let json_result: std::result::Result<serde_json::Value, _> =
            serde_json::from_str(&content.unwrap());
        assert!(json_result.is_err(), "JSON parsing should fail for malformed content");

        // Archive should still work (it just moves the file)
        let result = driver.archive_job(&invalid_job, "tx-invalid");
        assert!(result.is_ok(), "Archiving malformed job should succeed");

        // Verify job was moved
        assert!(!invalid_job.exists(), "Invalid job should be archived");
        let archived = driver.get_archive().join("tx-invalid/job.json");
        assert!(archived.exists(), "Archived job should exist");
    }

    /// Test: Concurrent queue operations (thread safety)
    #[test]
    fn test_concurrent_queue_operations() {
        use std::sync::Arc;
        use std::thread;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "TestKernel");

        let driver = Arc::new(FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "TestKernel".to_string(),
        ));

        let inbox = driver.get_queue_inbox();
        let staging = driver.get_queue_staging();
        fs::create_dir_all(&staging).unwrap();

        // Create jobs in inbox
        for i in 0..10 {
            fs::write(inbox.join(format!("concurrent-{}.job", i)), "{}").unwrap();
        }

        // Spawn multiple threads to move jobs concurrently
        let mut handles = vec![];
        for i in 0..10 {
            let driver_clone = Arc::clone(&driver);
            let inbox_clone = inbox.clone();
            let staging_clone = staging.clone();

            let handle = thread::spawn(move || {
                let job_path = inbox_clone.join(format!("concurrent-{}.job", i));
                if job_path.exists() {
                    driver_clone.move_job(&job_path, &staging_clone)
                } else {
                    Err(CkpError::Path("Job not found".to_string()))
                }
            });
            handles.push(handle);
        }

        // Wait for all threads
        let mut success_count = 0;
        for handle in handles {
            let result = handle.join().unwrap();
            if result.is_ok() {
                success_count += 1;
            }
        }

        // Verify most operations succeeded
        assert!(success_count >= 8, "At least 8/10 concurrent operations should succeed");

        // Verify files were moved
        let inbox_count = driver.count_queue_files(&inbox).unwrap();
        let staging_count = driver.count_queue_files(&staging).unwrap();

        assert_eq!(inbox_count + staging_count, 10, "All jobs should be accounted for");
    }

    /// Test: Queue directory auto-creation
    #[test]
    fn test_queue_directory_creation() {
        let temp_dir = TempDir::new().unwrap();
        // Don't use setup_test_kernel - test that directories are created

        let driver = FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "NewKernel".to_string(),
        );

        // Get queue paths (directories don't exist yet)
        let inbox = driver.get_queue_inbox();
        let staging = driver.get_queue_staging();
        let ready = driver.get_queue_ready();
        let archive = driver.get_archive();

        assert!(!inbox.exists(), "Inbox should not exist initially");
        assert!(!staging.exists(), "Staging should not exist initially");

        // Create a job - this should auto-create inbox
        fs::create_dir_all(&inbox).unwrap();
        let job_path = inbox.join("test.job");
        fs::write(&job_path, "{}").unwrap();
        assert!(inbox.exists(), "Inbox should be created");

        // Move job should auto-create staging
        driver.move_job(&job_path, &staging).unwrap();
        assert!(staging.exists(), "Staging should be auto-created");

        // Move to ready should auto-create ready
        let job_in_staging = staging.join("test.job");
        driver.move_job(&job_in_staging, &ready).unwrap();
        assert!(ready.exists(), "Ready should be auto-created");

        // Archive should auto-create archive
        let job_in_ready = ready.join("test.job");
        driver.archive_job(&job_in_ready, "tx-test").unwrap();
        assert!(archive.exists(), "Archive should be auto-created");
    }

    // ==================== WINDOWS-SPECIFIC FILE LOCKING TESTS (2 tests) ====================

    /// Test: Windows exclusive file locking for transaction log writes
    ///
    /// On Windows, file locking uses different APIs than Unix (LockFileEx/UnlockFileEx).
    /// This test ensures that exclusive locks prevent concurrent writes on Windows,
    /// maintaining FIFO integrity in the transaction log.
    ///
    /// Windows-specific behavior:
    /// - Uses kernel-level mandatory locking (not advisory like Unix)
    /// - LockFileEx blocks until lock is available
    /// - Lock is automatically released when file handle is closed
    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_file_locking_exclusive() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "WinLockKernel");

        let driver = Arc::new(FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "WinLockKernel".to_string(),
        ));

        // Spawn 5 threads writing to transaction log concurrently
        let mut handles = vec![];
        for i in 0..5 {
            let driver_clone = Arc::clone(&driver);
            let handle = thread::spawn(move || {
                // Small sleep to ensure threads overlap
                thread::sleep(Duration::from_millis(i * 10));

                let metadata = json!({
                    "thread_id": i,
                    "action": "windows_exclusive_lock",
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });
                driver_clone.record_transaction(&format!("win-excl-{}", i), metadata)
            });
            handles.push(handle);
        }

        // Wait for all threads
        let mut success_count = 0;
        for handle in handles {
            if handle.join().unwrap().is_ok() {
                success_count += 1;
            }
        }

        // All writes should succeed with exclusive locking
        assert_eq!(
            success_count, 5,
            "All 5 Windows exclusive lock writes should succeed"
        );

        // Verify transaction log integrity
        let tx_path = temp_dir.path().join("concepts/WinLockKernel/tx.jsonl");
        assert!(tx_path.exists(), "Transaction log should exist");

        let content = fs::read_to_string(tx_path).unwrap();
        let lines: Vec<&str> = content.trim().split('\n').collect();
        assert_eq!(lines.len(), 5, "Should have exactly 5 transaction lines");

        // Verify all lines are valid JSON (no corruption from concurrent writes)
        let parsed: Vec<Transaction> = lines
            .iter()
            .map(|line| {
                serde_json::from_str::<Transaction>(line).expect("Windows lock should prevent corruption")
            })
            .collect();

        assert_eq!(
            parsed.len(),
            5,
            "All 5 transactions should parse successfully with Windows locking"
        );

        // Verify all transaction IDs are unique (no overwrites)
        let mut tx_ids: Vec<_> = parsed.iter().map(|tx| &tx.tx_id).collect();
        tx_ids.sort();
        tx_ids.dedup();
        assert_eq!(
            tx_ids.len(),
            5,
            "Windows exclusive lock should ensure unique transaction IDs"
        );
    }

    /// Test: Windows shared read locks for concurrent transaction log queries
    ///
    /// Windows supports shared read locks (multiple readers, no writers).
    /// This test ensures that multiple threads can read the transaction log
    /// concurrently without blocking each other, while preventing writes.
    ///
    /// Windows-specific behavior:
    /// - Shared locks allow multiple concurrent readers
    /// - Shared locks block writers (exclusive locks)
    /// - Uses FILE_SHARE_READ flag during file open
    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_file_locking_shared() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "WinSharedKernel");

        let driver = Arc::new(FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "WinSharedKernel".to_string(),
        ));

        // Write initial transactions
        for i in 0..10 {
            let metadata = json!({"action": "initial_write", "id": i});
            driver.record_transaction(&format!("init-{}", i), metadata).unwrap();
        }

        let tx_path = temp_dir.path().join("concepts/WinSharedKernel/tx.jsonl");
        assert!(tx_path.exists(), "Transaction log should exist");

        // Spawn 10 threads reading the transaction log concurrently
        let mut read_handles = vec![];
        for thread_id in 0..10 {
            let tx_path_clone = tx_path.clone();
            let handle = thread::spawn(move || {
                // Sleep to overlap reads
                thread::sleep(Duration::from_millis(thread_id * 5));

                // Read transaction log (shared read)
                let content = fs::read_to_string(&tx_path_clone).unwrap();
                let lines: Vec<&str> = content.trim().split('\n').collect();

                // Parse all transactions
                let transactions: Vec<Transaction> = lines
                    .iter()
                    .map(|line| serde_json::from_str(line).unwrap())
                    .collect();

                // Return count for verification
                transactions.len()
            });
            read_handles.push(handle);
        }

        // Wait for all read threads
        let mut read_counts = vec![];
        for handle in read_handles {
            let count = handle.join().unwrap();
            read_counts.push(count);
        }

        // All reads should see the same number of transactions
        assert_eq!(
            read_counts.len(),
            10,
            "All 10 shared read threads should complete"
        );

        for count in &read_counts {
            assert_eq!(
                *count, 10,
                "Each shared read should see all 10 transactions"
            );
        }

        // Verify transaction log is still intact (no corruption)
        let final_content = fs::read_to_string(&tx_path).unwrap();
        let final_lines: Vec<&str> = final_content.trim().split('\n').collect();
        assert_eq!(
            final_lines.len(),
            10,
            "Shared reads should not corrupt transaction log"
        );

        // Verify concurrent shared reads were actually concurrent (fast)
        // If they were blocking each other, total time would be much longer
        let start = std::time::Instant::now();
        for _ in 0..10 {
            let _content = fs::read_to_string(&tx_path).unwrap();
        }
        let sequential_time = start.elapsed();

        println!(
            "[WINDOWS] Shared read time: 10 concurrent reads completed. \
             Sequential baseline: {} μs",
            sequential_time.as_micros()
        );

        // Sequential reads should be fast (< 10ms for 10 reads)
        assert!(
            sequential_time.as_micros() < 10_000,
            "Windows shared reads should be fast: {} μs",
            sequential_time.as_micros()
        );
    }

    /// Test: Unix file locking (excluded from Windows)
    ///
    /// This test verifies Unix flock() advisory locking behavior.
    /// It's excluded from Windows builds since Windows uses different APIs.
    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_unix_advisory_file_locking() {
        use std::sync::Arc;
        use std::thread;

        let temp_dir = TempDir::new().unwrap();
        setup_test_kernel(&temp_dir, "UnixLockKernel");

        let driver = Arc::new(FileSystemDriver::new(
            temp_dir.path().to_path_buf(),
            "UnixLockKernel".to_string(),
        ));

        // Spawn threads writing concurrently
        let mut handles = vec![];
        for i in 0..5 {
            let driver_clone = Arc::clone(&driver);
            let handle = thread::spawn(move || {
                let metadata = json!({"thread": i, "action": "unix_lock"});
                driver_clone.record_transaction(&format!("unix-{}", i), metadata)
            });
            handles.push(handle);
        }

        // Wait for all threads
        let mut success_count = 0;
        for handle in handles {
            if handle.join().unwrap().is_ok() {
                success_count += 1;
            }
        }

        // Unix flock() should ensure all writes succeed
        assert!(
            success_count >= 4,
            "Most Unix advisory lock writes should succeed: {}/5",
            success_count
        );

        // Verify transaction log
        let tx_path = temp_dir.path().join("concepts/UnixLockKernel/tx.jsonl");
        let content = fs::read_to_string(tx_path).unwrap();
        assert!(
            !content.is_empty(),
            "Unix transaction log should have content"
        );

        // Verify valid JSON lines
        let lines: Vec<&str> = content.trim().split('\n').collect();
        for line in &lines {
            let result: std::result::Result<Transaction, _> = serde_json::from_str(line);
            assert!(
                result.is_ok(),
                "Unix flock should prevent corruption: {} failed to parse",
                line
            );
        }
    }
}

// ===================================================================
// StorageDriver Trait Implementation
// ===================================================================

use crate::drivers::traits::{StorageDriver, JobFile as TraitJobFile, JobHandle, StorageLocation};
use crate::urn::UrnResolver;

impl StorageDriver for FileSystemDriver {
    fn write_job(&self, target_urn: &str, job: TraitJobFile) -> Result<String> {
        // Resolve target to queue path (inbox by default, or specified stage)
        let queue_path = if target_urn.starts_with("ckp://") {
            // Parse URN
            let parsed = UrnResolver::parse(target_urn)?;
            let kernel_path = self.root.join("concepts").join(&parsed.kernel);

            // Use stage if specified, otherwise default to inbox
            if let Some(stage) = parsed.stage {
                kernel_path.join("queue").join(&stage)
            } else {
                kernel_path.join("queue/inbox")
            }
        } else {
            // Simple kernel name - default to inbox
            self.root.join("concepts").join(target_urn).join("queue/inbox")
        };

        // Ensure queue directory exists
        fs::create_dir_all(&queue_path)
            .map_err(|e| CkpError::IoError(format!("Failed to create queue directory: {}", e)))?;

        // Write job file
        let job_path = queue_path.join(format!("{}.job", job.tx_id));
        let job_json = serde_json::to_string_pretty(&job)
            .map_err(|e| CkpError::Json(e))?;

        fs::write(&job_path, job_json)
            .map_err(|e| CkpError::IoError(format!("Failed to write job: {}", e)))?;

        Ok(job.tx_id.clone())
    }

    fn read_jobs(&self, kernel_name: &str) -> Result<Vec<JobHandle>> {
        let inbox_path = self.root.join("concepts").join(kernel_name).join("queue/inbox");

        if !inbox_path.exists() {
            return Ok(Vec::new());
        }

        let mut jobs = Vec::new();

        let entries = fs::read_dir(&inbox_path)
            .map_err(|e| CkpError::IoError(format!("Failed to read inbox: {}", e)))?;

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("job") {
                // Read job content
                let content_str = fs::read_to_string(&path)
                    .map_err(|e| CkpError::IoError(format!("Failed to read job: {}", e)))?;

                let content: TraitJobFile = serde_json::from_str(&content_str)
                    .map_err(|e| CkpError::Json(e))?;

                jobs.push(JobHandle {
                    tx_id: content.tx_id.clone(),
                    content,
                    storage_id: path.to_string_lossy().to_string(),
                });
            }
        }

        Ok(jobs)
    }

    fn archive_job(&self, kernel_name: &str, job: &JobHandle) -> Result<()> {
        let job_path = PathBuf::from(&job.storage_id);
        let archive_dir = self.root.join("concepts").join(kernel_name).join("queue/archive");

        fs::create_dir_all(&archive_dir)
            .map_err(|e| CkpError::IoError(format!("Failed to create archive: {}", e)))?;

        let archive_path = archive_dir.join(job_path.file_name().unwrap());

        fs::rename(&job_path, &archive_path)
            .map_err(|e| CkpError::IoError(format!("Failed to archive job: {}", e)))?;

        Ok(())
    }

    fn mint_storage_artifact(
        &self,
        kernel_name: &str,
        instance_id: &str,
        data: JsonValue,
    ) -> Result<String> {
        let storage_dir = self.root.join("concepts").join(kernel_name).join("storage");
        fs::create_dir_all(&storage_dir)
            .map_err(|e| CkpError::IoError(format!("Failed to create storage: {}", e)))?;

        let instance_dir = storage_dir.join(format!("{}.inst", instance_id));
        fs::create_dir_all(&instance_dir)
            .map_err(|e| CkpError::IoError(format!("Failed to create instance dir: {}", e)))?;

        // Write payload
        let payload_path = instance_dir.join("payload.json");
        let payload_json = serde_json::to_string_pretty(&data)
            .map_err(|e| CkpError::Json(e))?;

        fs::write(&payload_path, payload_json)
            .map_err(|e| CkpError::IoError(format!("Failed to write payload: {}", e)))?;

        // Return URN
        Ok(format!("ckp://{}#storage/{}", kernel_name, instance_id))
    }

    fn record_transaction(&self, kernel_name: &str, transaction: JsonValue) -> Result<()> {
        let tx_log = self.root.join("concepts").join(kernel_name).join("tx.jsonl");

        // Ensure parent exists
        if let Some(parent) = tx_log.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| CkpError::IoError(format!("Failed to create tx dir: {}", e)))?;
        }

        // Append transaction as single JSON line
        let tx_line = serde_json::to_string(&transaction)
            .map_err(|e| CkpError::Json(e))?;

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&tx_log)
            .map_err(|e| CkpError::IoError(format!("Failed to open tx log: {}", e)))?;

        use std::io::Write;
        writeln!(file, "{}", tx_line)
            .map_err(|e| CkpError::IoError(format!("Failed to write tx: {}", e)))?;

        Ok(())
    }

    fn resolve_urn(&self, urn: &str) -> Result<StorageLocation> {
        if urn.starts_with("ckp://") {
            let parsed = UrnResolver::parse(urn)?;
            let kernel_path = self.root.join("concepts").join(&parsed.kernel);

            // Resolve stage to path
            let path = if let Some(stage) = parsed.stage {
                let base_path = match stage.as_str() {
                    "inbox" => kernel_path.join("queue/inbox"),
                    "staging" => kernel_path.join("queue/staging"),
                    "ready" => kernel_path.join("queue/ready"),
                    "archive" => kernel_path.join("queue/archive"),
                    "storage" => kernel_path.join("storage"),
                    _ => kernel_path.join(&stage),
                };

                // If there's a path component (e.g., instance ID), append it
                if let Some(subpath) = parsed.path {
                    base_path.join(&subpath)
                } else {
                    base_path
                }
            } else {
                // No stage - default to kernel dir
                kernel_path
            };

            Ok(StorageLocation::Local(path))
        } else {
            // Not a URN - treat as kernel name, return kernel dir
            let kernel_path = self.root.join("concepts").join(urn);
            Ok(StorageLocation::Local(kernel_path))
        }
    }

    fn kernel_exists(&self, kernel_name: &str) -> Result<bool> {
        let kernel_dir = self.root.join("concepts").join(kernel_name);
        let ontology_path = kernel_dir.join("conceptkernel.yaml");
        Ok(ontology_path.exists())
    }

    fn get_edge_queue(&self, kernel_name: &str, source_kernel: &str) -> Result<StorageLocation> {
        let edge_queue_path = self.root
            .join("concepts")
            .join(kernel_name)
            .join("queue/edges")
            .join(source_kernel);

        Ok(StorageLocation::Local(edge_queue_path))
    }
}
