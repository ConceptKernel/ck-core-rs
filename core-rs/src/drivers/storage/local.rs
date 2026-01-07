//! LocalStorage - Async wrapper around FileSystemDriver
//!
//! Provides async interface for filesystem storage operations using
//! tokio::task::spawn_blocking to wrap synchronous filesystem calls.
//!
//! This is the "continuant" implementation that preserves v1.3.19 behavior
//! while conforming to the async StorageDriver trait.

use crate::drivers::traits::{
    ExecutionMode, JobFile, JobHandle, ResourceRequirements, StorageDriver, StorageLocation,
    ToolDefinition, ToolResponse,
};
use crate::drivers::FileSystemDriver;
use crate::errors::{CkpError, Result};
use async_trait::async_trait;
use futures::executor::block_on;
use serde_json::Value as JsonValue;
use std::path::PathBuf;
use std::sync::Arc;

/// LocalStorage - Async wrapper around FileSystemDriver
///
/// Uses tokio::task::spawn_blocking to provide async interface
/// while preserving all v1.3.19 filesystem semantics.
///
/// # Design Principles
///
/// - **Zero Rewrites**: Wraps existing FileSystemDriver (proven code)
/// - **Async-Safe**: Uses spawn_blocking for all filesystem operations
/// - **Backward Compatible**: Maintains exact v1.3.19 semantics
/// - **Minimal Overhead**: Direct delegation to FileSystemDriver
///
/// # Example
///
/// ```rust,ignore
/// use ckp_core::drivers::LocalStorage;
/// use std::path::PathBuf;
///
/// let storage = LocalStorage::new(
///     PathBuf::from("/project"),
///     "MyKernel".to_string()
/// );
///
/// // All methods are async
/// let ontology = storage.load_ontology("MyKernel").await?;
/// ```
#[derive(Debug)]
pub struct LocalStorage {
    /// Wrapped FileSystemDriver instance
    inner: Arc<FileSystemDriver>,
}

impl LocalStorage {
    /// Create new LocalStorage wrapper
    ///
    /// # Arguments
    ///
    /// * `root` - Project root path
    /// * `kernel_name` - Kernel name
    ///
    /// # Returns
    ///
    /// LocalStorage instance wrapping FileSystemDriver
    pub fn new(root: PathBuf, kernel_name: String) -> Self {
        let inner = Arc::new(FileSystemDriver::new(root, kernel_name));
        Self { inner }
    }

    /// Get reference to inner FileSystemDriver
    ///
    /// Useful for accessing FileSystemDriver-specific methods
    pub fn inner(&self) -> &Arc<FileSystemDriver> {
        &self.inner
    }
}

#[async_trait]
impl StorageDriver for LocalStorage {
    // ========================================================================
    // EXISTING METHODS (v1.3.19 - now async via spawn_blocking)
    // ========================================================================

    async fn write_job(&self, target_urn: &str, job: JobFile) -> Result<String> {
        self.inner.write_job(target_urn, job).await
    }

    async fn read_jobs(&self, kernel_name: &str) -> Result<Vec<JobHandle>> {
        self.inner.read_jobs(kernel_name).await
    }

    async fn archive_job(&self, kernel_name: &str, job: &JobHandle) -> Result<()> {
        let inner = self.inner.clone();
        let kernel = kernel_name.to_string();
        let job_clone = job.clone();

        tokio::task::spawn_blocking(move || {
            inner.archive_job_sync_pub(&kernel, &job_clone)
        })
        .await
        .map_err(|e| CkpError::TaskJoin(format!("spawn_blocking failed: {}", e)))?
    }

    async fn mint_storage_artifact(
        &self,
        kernel_name: &str,
        instance_id: &str,
        data: JsonValue,
    ) -> Result<String> {
        let inner = self.inner.clone();
        let kernel = kernel_name.to_string();
        let instance = instance_id.to_string();

        tokio::task::spawn_blocking(move || {
            inner.mint_storage_artifact_sync_pub(&kernel, &instance, data)
        })
        .await
        .map_err(|e| CkpError::TaskJoin(format!("spawn_blocking failed: {}", e)))?
    }

    async fn record_transaction(&self, kernel_name: &str, transaction: JsonValue) -> Result<()> {
        let inner = self.inner.clone();
        let kernel = kernel_name.to_string();

        tokio::task::spawn_blocking(move || {
            inner.record_transaction_sync_pub(&kernel, transaction)
        })
        .await
        .map_err(|e| CkpError::TaskJoin(format!("spawn_blocking failed: {}", e)))?
    }

    async fn resolve_urn(&self, urn: &str) -> Result<StorageLocation> {
        self.inner.resolve_urn(urn).await
    }

    async fn kernel_exists(&self, kernel_name: &str) -> Result<bool> {
        self.inner.kernel_exists(kernel_name).await
    }

    async fn get_edge_queue(
        &self,
        kernel_name: &str,
        source_kernel: &str,
    ) -> Result<StorageLocation> {
        self.inner.get_edge_queue(kernel_name, source_kernel).await
    }

    // ========================================================================
    // NEW METHODS (v1.3.20)
    // ========================================================================

    async fn load_ontology(&self, kernel_name: &str) -> Result<String> {
        self.inner.load_ontology(kernel_name).await
    }

    async fn save_ontology(&self, kernel_name: &str, ttl: &str) -> Result<()> {
        self.inner.save_ontology(kernel_name, ttl).await
    }

    async fn load_tool_definition(
        &self,
        kernel_name: &str,
        tool_name: &str,
    ) -> Result<ToolDefinition> {
        self.inner.load_tool_definition(kernel_name, tool_name).await
    }

    async fn save_result(
        &self,
        kernel_name: &str,
        job_id: &str,
        result: &ToolResponse,
    ) -> Result<()> {
        self.inner.save_result(kernel_name, job_id, result).await
    }

    async fn load_result(&self, kernel_name: &str, job_id: &str) -> Result<ToolResponse> {
        self.inner.load_result(kernel_name, job_id).await
    }

    async fn list_edge_queues(&self, kernel_name: &str) -> Result<Vec<(String, usize)>> {
        // Call trait method explicitly through StorageDriver trait
        <FileSystemDriver as StorageDriver>::list_edge_queues(&*self.inner, kernel_name).await
    }

    async fn list_instances(&self, kernel_name: &str) -> Result<Vec<String>> {
        // Call trait method explicitly through StorageDriver trait
        <FileSystemDriver as StorageDriver>::list_instances(&*self.inner, kernel_name).await
    }

    async fn subscribe_storage_events(&self) -> Result<crate::drivers::traits::StorageEventStream> {
        // Call trait method explicitly through StorageDriver trait
        <FileSystemDriver as StorageDriver>::subscribe_storage_events(&*self.inner).await
    }

    fn root_path(&self) -> Result<PathBuf> {
        <FileSystemDriver as StorageDriver>::root_path(&*self.inner)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn setup_test_kernel(temp_dir: &TempDir, kernel_name: &str) -> PathBuf {
        let concepts_dir = temp_dir.path().join("concepts");
        let kernel_dir = concepts_dir.join(kernel_name);

        fs::create_dir_all(kernel_dir.join("queue/inbox")).unwrap();
        fs::create_dir_all(kernel_dir.join("queue/archive")).unwrap();
        fs::create_dir_all(kernel_dir.join("storage")).unwrap();

        // Create ontology
        let ontology = format!(
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}
  type: node:cold
  version: v0.1
"#,
            kernel_name
        );
        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();

        temp_dir.path().to_path_buf()
    }

    #[tokio::test]
    async fn test_local_storage_load_ontology() {
        let temp_dir = TempDir::new().unwrap();
        let root = setup_test_kernel(&temp_dir, "TestKernel");

        let storage = LocalStorage::new(root, "TestKernel".to_string());

        let ontology = storage.load_ontology("TestKernel").await.unwrap();
        assert!(ontology.contains("conceptkernel/v1"));
        assert!(ontology.contains("TestKernel"));
    }

    #[tokio::test]
    async fn test_local_storage_kernel_exists() {
        let temp_dir = TempDir::new().unwrap();
        let root = setup_test_kernel(&temp_dir, "ExistingKernel");

        let storage = LocalStorage::new(root, "ExistingKernel".to_string());

        assert!(storage.kernel_exists("ExistingKernel").await.unwrap());
        assert!(!storage.kernel_exists("NonExistentKernel").await.unwrap());
    }

    #[tokio::test]
    async fn test_local_storage_save_load_result() {
        let temp_dir = TempDir::new().unwrap();
        let root = setup_test_kernel(&temp_dir, "ResultKernel");

        let storage = LocalStorage::new(root, "ResultKernel".to_string());

        let result = ToolResponse {
            job_id: "test-job-123".to_string(),
            status: "success".to_string(),
            output: serde_json::json!({"result": "ok"}),
            timestamp: "2025-12-18T10:00:00Z".to_string(),
            duration_ms: 1000,
            error: None,
        };

        // Save result
        storage.save_result("ResultKernel", "test-job-123", &result)
            .await
            .unwrap();

        // Load result
        let loaded = storage.load_result("ResultKernel", "test-job-123")
            .await
            .unwrap();

        assert_eq!(loaded.job_id, result.job_id);
        assert_eq!(loaded.status, result.status);
        assert_eq!(loaded.duration_ms, result.duration_ms);
    }
}
