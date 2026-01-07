//! LocalTransport - Filesystem-based transport using notify crate
//!
//! Provides TransportDriver implementation that wraps the notify crate
//! filesystem watching pattern used in Governor (v1.3.19).
//!
//! This is the "continuant" implementation that preserves v1.3.19 behavior
//! while conforming to the async TransportDriver trait.

use crate::drivers::traits::{EdgeMessage, EdgeStream, JobMessage, JobStream, ResultStream, ToolResponse, TransportDriver};
use crate::errors::{CkpError, Result};
use async_trait::async_trait;
use futures::stream::Stream;
use notify::{
    Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use std::path::PathBuf;
use std::pin::Pin;
use tokio::sync::mpsc;

/// LocalTransport - Filesystem-based transport using notify crate
///
/// Wraps notify crate filesystem watching to provide event-driven
/// job and result streaming compatible with TransportDriver trait.
///
/// # Design Principles
///
/// - **Same Pattern**: Uses notify crate exactly like governor.rs (v1.3.19)
/// - **Event-Driven**: File creation triggers immediate job/result delivery
/// - **Async Streams**: Returns async streams of jobs and results
/// - **Cross-Platform**: Works on Unix, Windows, macOS
///
/// # Example
///
/// ```rust,ignore
/// use ckp_core::drivers::LocalTransport;
/// use std::path::PathBuf;
///
/// let transport = LocalTransport::new(
///     PathBuf::from("/project"),
///     "MyKernel".to_string()
/// );
///
/// // Subscribe to inbox
/// let mut job_stream = transport.subscribe_inbox("MyKernel").await?;
///
/// // Wait for jobs
/// while let Some(job) = job_stream.next().await {
///     println!("Received job: {:?}", job);
/// }
/// ```
#[derive(Debug)]
pub struct LocalTransport {
    /// Project root path
    root: PathBuf,

    /// Kernel name (for default paths)
    kernel_name: String,
}

impl LocalTransport {
    /// Create new LocalTransport
    ///
    /// # Arguments
    ///
    /// * `root` - Project root path
    /// * `kernel_name` - Kernel name
    pub fn new(root: PathBuf, kernel_name: String) -> Self {
        Self { root, kernel_name }
    }

    /// Get inbox path for kernel
    fn get_inbox_path(&self, kernel_name: &str) -> PathBuf {
        self.root
            .join("concepts")
            .join(kernel_name)
            .join("queue")
            .join("inbox")
    }

    /// Get results path for kernel
    fn get_results_path(&self, kernel_name: &str) -> PathBuf {
        self.root
            .join("concepts")
            .join(kernel_name)
            .join("queue")
            .join("results")
    }

    /// Background task: Watch inbox directory for job files
    async fn watch_inbox(inbox_path: PathBuf, tx: mpsc::Sender<JobMessage>) -> Result<()> {
        let (watch_tx, watch_rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(watch_tx, NotifyConfig::default())
            .map_err(|e| CkpError::IoError(format!("Failed to create watcher: {}", e)))?;

        watcher
            .watch(&inbox_path, RecursiveMode::NonRecursive)
            .map_err(|e| CkpError::IoError(format!("Failed to watch inbox: {}", e)))?;

        loop {
            match watch_rx.recv() {
                Ok(Ok(event)) => {
                    if let Some(job) = Self::handle_inbox_event(event, &inbox_path).await {
                        if tx.send(job).await.is_err() {
                            // Receiver dropped, exit
                            break;
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("[LocalTransport] Watch error: {}", e);
                }
                Err(e) => {
                    eprintln!("[LocalTransport] Channel error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle inbox filesystem event
    async fn handle_inbox_event(event: Event, inbox_path: &PathBuf) -> Option<JobMessage> {
        // Only care about Create events
        if !matches!(event.kind, EventKind::Create(_)) {
            return None;
        }

        for path in &event.paths {
            // Check if .job file
            if path.extension().and_then(|s| s.to_str()) != Some("job") {
                continue;
            }

            // Check if in inbox
            if path.parent() != Some(inbox_path) {
                continue;
            }

            // Read job file
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                if let Ok(job) = serde_json::from_str::<JobMessage>(&content) {
                    return Some(job);
                }
            }
        }

        None
    }

    /// Background task: Watch results directory for result files
    async fn watch_results(
        results_path: PathBuf,
        tx: mpsc::Sender<ToolResponse>,
    ) -> Result<()> {
        let (watch_tx, watch_rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(watch_tx, NotifyConfig::default())
            .map_err(|e| CkpError::IoError(format!("Failed to create watcher: {}", e)))?;

        watcher
            .watch(&results_path, RecursiveMode::NonRecursive)
            .map_err(|e| CkpError::IoError(format!("Failed to watch results: {}", e)))?;

        loop {
            match watch_rx.recv() {
                Ok(Ok(event)) => {
                    if let Some(result) = Self::handle_result_event(event, &results_path).await {
                        if tx.send(result).await.is_err() {
                            // Receiver dropped, exit
                            break;
                        }
                    }
                }
                Ok(Err(e)) => {
                    eprintln!("[LocalTransport] Results watch error: {}", e);
                }
                Err(e) => {
                    eprintln!("[LocalTransport] Results channel error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Handle result filesystem event
    async fn handle_result_event(
        event: Event,
        results_path: &PathBuf,
    ) -> Option<ToolResponse> {
        // Only care about Create events
        if !matches!(event.kind, EventKind::Create(_)) {
            return None;
        }

        for path in &event.paths {
            // Check if .result file
            if path.extension().and_then(|s| s.to_str()) != Some("result") {
                continue;
            }

            // Check if in results directory
            if path.parent() != Some(results_path) {
                continue;
            }

            // Read result file
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                if let Ok(result) = serde_json::from_str::<ToolResponse>(&content) {
                    return Some(result);
                }
            }
        }

        None
    }
}

#[async_trait]
impl TransportDriver for LocalTransport {
    async fn subscribe_inbox(&self, kernel_name: &str) -> Result<JobStream> {
        let inbox_path = self.get_inbox_path(kernel_name);

        // Ensure inbox exists
        if !inbox_path.exists() {
            tokio::fs::create_dir_all(&inbox_path)
                .await
                .map_err(|e| CkpError::IoError(format!("Failed to create inbox: {}", e)))?;
        }

        // Create channel for job stream
        let (tx, rx) = mpsc::channel(100);

        // Spawn watcher task (same pattern as governor.rs)
        let inbox_clone = inbox_path.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::watch_inbox(inbox_clone, tx).await {
                eprintln!("[LocalTransport] Inbox watcher error: {}", e);
            }
        });

        // Convert mpsc receiver to stream
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    async fn publish_job(&self, target: &str, job: JobMessage) -> Result<()> {
        let inbox_path = self.get_inbox_path(target);

        // Ensure inbox exists
        if !inbox_path.exists() {
            tokio::fs::create_dir_all(&inbox_path)
                .await
                .map_err(|e| CkpError::IoError(format!("Failed to create inbox: {}", e)))?;
        }

        // Write job file
        let job_path = inbox_path.join(format!("{}.job", job.job_id));
        let job_json = serde_json::to_string_pretty(&job)
            .map_err(|e| CkpError::Json(e))?;

        tokio::fs::write(&job_path, job_json)
            .await
            .map_err(|e| CkpError::IoError(format!("Failed to write job: {}", e)))?;

        Ok(())
    }

    async fn publish_response(
        &self,
        kernel_name: &str,
        response: ToolResponse,
    ) -> Result<()> {
        let job_id = &response.job_id;
        let results_path = self.get_results_path(kernel_name);

        // Ensure results directory exists
        if !results_path.exists() {
            tokio::fs::create_dir_all(&results_path)
                .await
                .map_err(|e| CkpError::IoError(format!("Failed to create results dir: {}", e)))?;
        }

        // Write result file
        let result_path = results_path.join(format!("{}.result", job_id));
        let result_json = serde_json::to_string_pretty(&response)
            .map_err(|e| CkpError::Json(e))?;

        tokio::fs::write(&result_path, result_json)
            .await
            .map_err(|e| CkpError::IoError(format!("Failed to write result: {}", e)))?;

        Ok(())
    }

    async fn subscribe_results(&self, kernel_name: &str) -> Result<ResultStream> {
        let results_path = self.get_results_path(kernel_name);

        // Ensure results directory exists
        if !results_path.exists() {
            tokio::fs::create_dir_all(&results_path)
                .await
                .map_err(|e| CkpError::IoError(format!("Failed to create results dir: {}", e)))?;
        }

        // Create channel for result stream
        let (tx, rx) = mpsc::channel(100);

        // Spawn watcher task
        let results_clone = results_path.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::watch_results(results_clone, tx).await {
                eprintln!("[LocalTransport] Results watcher error: {}", e);
            }
        });

        // Convert mpsc receiver to stream
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    async fn health_check(&self) -> Result<bool> {
        // Check if inbox directory is accessible
        let inbox_path = self.get_inbox_path(&self.kernel_name);

        Ok(tokio::fs::metadata(&inbox_path).await.is_ok())
    }

    async fn shutdown(&self) -> Result<()> {
        // No resources to clean up for filesystem watching
        // (watcher tasks will exit when stream is dropped)
        Ok(())
    }

    async fn subscribe_edge(&self, _source: &str, _target: &str) -> Result<EdgeStream> {
        // Edge routing not implemented for LocalTransport
        // (use NatsTransport for edge routing)
        Err(CkpError::NotImplemented(
            "Edge routing not supported by LocalTransport - use NatsTransport".to_string(),
        ))
    }

    async fn notify_edge(&self, _edge: &EdgeMessage) -> Result<()> {
        // Edge routing not implemented for LocalTransport
        Err(CkpError::NotImplemented(
            "Edge routing not supported by LocalTransport - use NatsTransport".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use tempfile::TempDir;
    use std::fs;

    fn setup_test_kernel(temp_dir: &TempDir, kernel_name: &str) -> PathBuf {
        let concepts_dir = temp_dir.path().join("concepts");
        let kernel_dir = concepts_dir.join(kernel_name);

        fs::create_dir_all(kernel_dir.join("queue/inbox")).unwrap();
        fs::create_dir_all(kernel_dir.join("queue/results")).unwrap();

        temp_dir.path().to_path_buf()
    }

    #[tokio::test]
    async fn test_local_transport_publish_job() {
        let temp_dir = TempDir::new().unwrap();
        let root = setup_test_kernel(&temp_dir, "TestKernel");

        let transport = LocalTransport::new(root.clone(), "TestKernel".to_string());

        let job = JobMessage {
            job_id: "test-job-123".to_string(),
            tool: "test-tool".to_string(),
            args: serde_json::json!({"test": "data"}),
            timestamp: chrono::Utc::now().to_rfc3339(),
            source: "test".to_string(),
        };

        transport.publish_job("TestKernel", job).await.unwrap();

        // Verify job file exists
        let job_path = root.join("concepts/TestKernel/queue/inbox/test-job-123.job");
        assert!(job_path.exists());
    }

    #[tokio::test]
    async fn test_local_transport_subscribe_inbox() {
        let temp_dir = TempDir::new().unwrap();
        let root = setup_test_kernel(&temp_dir, "StreamKernel");

        let transport = LocalTransport::new(root.clone(), "StreamKernel".to_string());

        let mut job_stream = transport.subscribe_inbox("StreamKernel").await.unwrap();

        // Spawn task to create job file
        let inbox_path = root.join("concepts/StreamKernel/queue/inbox");
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            let job = JobMessage {
                job_id: "stream-test-999".to_string(),
                tool: "stream-tool".to_string(),
                args: serde_json::json!({"stream": "test"}),
                timestamp: chrono::Utc::now().to_rfc3339(),
                source: "test".to_string(),
            };

            let job_json = serde_json::to_string_pretty(&job).unwrap();
            std::fs::write(inbox_path.join("stream-test-999.job"), job_json).unwrap();
        });

        // Wait for job from stream
        let job = tokio::time::timeout(tokio::time::Duration::from_secs(2), job_stream.next())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(job.job_id, "stream-test-999");
        assert_eq!(job.tool, "stream-tool");
    }

    #[tokio::test]
    async fn test_local_transport_health_check() {
        let temp_dir = TempDir::new().unwrap();
        let root = setup_test_kernel(&temp_dir, "HealthKernel");

        let transport = LocalTransport::new(root, "HealthKernel".to_string());

        assert!(transport.health_check().await.is_ok());
    }
}
