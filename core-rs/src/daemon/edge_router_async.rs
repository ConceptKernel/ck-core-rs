//! EdgeRouterDaemonAsync - Async transport-based edge routing (v1.3.20)
//!
//! ## Responsibilities
//!
//! - Subscribe to result streams from source kernels
//! - Read notification_contract from ontology
//! - Auto-create edges (PRODUCES predicate by default)
//! - Route instances to target kernels via TransportDriver
//! - Track routing with Process URNs
//!
//! ## Architecture (v1.3.20)
//!
//! - **Transport Abstraction**: Uses `Arc<dyn TransportDriver>`
//! - **Result Monitoring**: Subscribes to result streams (not filesystem)
//! - **Edge Publishing**: Publishes to edge queues via transport
//! - **Async/Await**: Full tokio async runtime
//! - **Multi-Transport**: Works with LocalTransport, NatsTransport, etc.
//!
//! ## Migration from v1.3.19
//!
//! **Old (FileSystem-based)**:
//! - Watched filesystem with notify crate
//! - Created symlinks in `.edges/` directories
//! - Synchronous operation
//!
//! **New (Transport-based)**:
//! - Subscribes to result streams
//! - Publishes via transport driver
//! - Async operation
//! - Supports distributed deployments

use crate::drivers::{EdgeMetadata, JobMessage, JenaStorage, ResultStream, StorageDriver, TransportDriver, ToolResponse};
use crate::edge::EdgeKernel;
use crate::edge_event_publisher::EdgeEventPublisher;
use crate::ontology::{OntologyLibrary, OntologyReader};
use crate::process_tracker::ProcessTracker;
use crate::errors::{CkpError, Result};
use futures::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::sync::broadcast;

/// EdgeRouterDaemonAsync - Async transport-based edge routing
///
/// Monitors result streams from source kernels and routes instances
/// to target kernels based on notification contracts.
///
/// # Example
///
/// ```rust,ignore
/// use ckp_core::daemon::EdgeRouterDaemonAsync;
/// use ckp_core::drivers::{LocalTransport, LocalStorage};
/// use std::sync::Arc;
///
/// let transport = Arc::new(LocalTransport::new("/project".into(), "Router".into()));
/// let storage = Arc::new(LocalStorage::new("/project".into(), "Router".into()).await?);
/// let router = EdgeRouterDaemonAsync::new(
///     "/project".into(),
///     transport,
///     Some(storage),
///     true // verbose
/// ).await?;
///
/// // Start routing in background
/// let shutdown = tokio::sync::broadcast::channel(1).0;
/// router.start(shutdown).await?;
/// ```
pub struct EdgeRouterDaemonAsync {
    /// Project root path
    root: PathBuf,

    /// Transport driver (LocalTransport, NatsTransport, etc.)
    transport: Arc<dyn TransportDriver>,

    /// Optional storage driver for edge metadata persistence
    storage: Option<Arc<dyn StorageDriver>>,

    /// Edge kernel for edge lifecycle management
    edge_kernel: Arc<Mutex<EdgeKernel>>,

    /// Ontology reader for notification contracts
    ontology_reader: OntologyReader,

    /// Optional ontology library
    _ontology_library: Option<Arc<OntologyLibrary>>,

    /// Process tracker for BFO occurrent tracking
    _process_tracker: Arc<ProcessTracker>,

    /// Verbose logging
    verbose: bool,

    /// Cache: kernel_name -> Vec<(target, predicate)>
    notification_cache: Arc<RwLock<HashMap<String, Vec<(String, String)>>>>,

    /// Active kernel monitors
    active_monitors: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,

    /// Edge event publisher for NATS lifecycle events (v1.3.20)
    edge_event_publisher: Option<Arc<EdgeEventPublisher>>,
}

impl EdgeRouterDaemonAsync {
    /// Create new EdgeRouterDaemonAsync
    ///
    /// # Arguments
    ///
    /// * `root` - Project root path
    /// * `transport` - Transport driver (LocalTransport, NatsTransport, etc.)
    /// * `storage` - Optional storage driver for edge metadata persistence (e.g., JenaStorage)
    /// * `verbose` - Enable verbose logging
    ///
    /// # Returns
    ///
    /// EdgeRouterDaemonAsync instance
    pub async fn new(
        root: PathBuf,
        transport: Arc<dyn TransportDriver>,
        storage: Option<Arc<dyn StorageDriver>>,
        verbose: bool,
    ) -> Result<Self> {
        // Initialize EdgeKernel with OntologyLibrary and ProcessTracker
        let ontology_library = OntologyLibrary::new(root.clone()).ok().map(Arc::new);
        let process_tracker = Arc::new(
            ProcessTracker::new(root.clone())
                .map_err(|e| CkpError::EdgeRouting(format!("ProcessTracker init failed: {}", e)))?,
        );

        let edge_kernel = EdgeKernel::with_ontology(
            root.clone(),
            ontology_library.clone(),
            Some(process_tracker.clone()),
        )
        .map_err(|e| CkpError::EdgeRouting(format!("EdgeKernel init failed: {}", e)))?;

        // Initialize EdgeEventPublisher for NATS lifecycle events
        let nats_url = std::env::var("NATS_URL").ok();
        let edge_event_publisher = if let Some(url) = nats_url.as_deref() {
            match EdgeEventPublisher::new(Some(url)).await {
                Ok(publisher) => Some(Arc::new(publisher)),
                Err(e) => {
                    eprintln!("[EdgeRouterAsync] Failed to initialize EdgeEventPublisher: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            root: root.clone(),
            transport,
            storage,
            edge_kernel: Arc::new(Mutex::new(edge_kernel)),
            ontology_reader: OntologyReader::new(root.clone()),
            _ontology_library: ontology_library,
            _process_tracker: process_tracker,
            verbose,
            notification_cache: Arc::new(RwLock::new(HashMap::new())),
            active_monitors: Arc::new(RwLock::new(HashMap::new())),
            edge_event_publisher,
        })
    }

    /// Start the edge router daemon
    ///
    /// Discovers all kernels with notification contracts and starts
    /// monitoring their result streams for routing opportunities.
    ///
    /// # Arguments
    ///
    /// * `shutdown` - Broadcast channel for shutdown signal
    ///
    /// # Returns
    ///
    /// Result indicating success or error
    pub async fn start(&self, mut shutdown: broadcast::Receiver<()>) -> Result<()> {
        self.log("[EdgeRouterAsync] Starting daemon...").await;
        self.log(&format!("[EdgeRouterAsync] Project: {}", self.root.display()))
            .await;

        // Write daemon PID for tracking
        let pid = std::process::id();
        let pid_file = self.root.join(".edge-router.pid");
        tokio::fs::write(&pid_file, pid.to_string())
            .await
            .map_err(|e| CkpError::IoError(format!("Failed to write PID file: {}", e)))?;
        self.log(&format!("[EdgeRouterAsync] PID: {} (written to {})", pid, pid_file.display()))
            .await;

        // Discover kernels with notification contracts
        let kernels = self.discover_kernels_with_contracts().await?;

        self.log(&format!(
            "[EdgeRouterAsync] Found {} kernel(s) with notification contracts",
            kernels.len()
        ))
        .await;

        // Start monitoring each kernel
        for kernel_name in kernels {
            self.start_kernel_monitor(&kernel_name).await?;
        }

        self.log("[EdgeRouterAsync] Ready - Monitoring result streams")
            .await;

        // Wait for shutdown signal
        let _ = shutdown.recv().await;

        self.log("[EdgeRouterAsync] Shutdown signal received, stopping monitors...")
            .await;

        // Stop all monitors
        self.stop_all_monitors().await;

        // Cleanup PID file on shutdown
        let pid_file = self.root.join(".edge-router.pid");
        if tokio::fs::try_exists(&pid_file).await.unwrap_or(false) {
            let _ = tokio::fs::remove_file(&pid_file).await;
            self.log(&format!("[EdgeRouterAsync] Removed PID file: {}", pid_file.display()))
                .await;
        }

        self.log("[EdgeRouterAsync] Shutdown complete").await;

        Ok(())
    }

    /// Discover kernels with notification contracts
    ///
    /// First tries diskless discovery via Jena SPARQL query. If that fails or no
    /// JenaStorage is configured, falls back to filesystem discovery.
    async fn discover_kernels_with_contracts(&self) -> Result<Vec<String>> {
        // Try diskless discovery via Jena if JenaStorage is configured
        if let Some(storage) = &self.storage {
            if let Some(jena) = storage.as_any().downcast_ref::<JenaStorage>() {
                if self.verbose {
                    self.log("[EdgeRouterAsync] Using diskless kernel discovery via Jena SPARQL")
                        .await;
                }

                match self.discover_kernels_from_jena(jena).await {
                    Ok(kernels) => {
                        if self.verbose {
                            self.log(&format!(
                                "[EdgeRouterAsync] Jena discovery found {} kernels",
                                kernels.len()
                            ))
                            .await;
                        }
                        return Ok(kernels);
                    }
                    Err(e) => {
                        if self.verbose {
                            self.log(&format!(
                                "[EdgeRouterAsync] Jena discovery failed, falling back to filesystem: {}",
                                e
                            ))
                            .await;
                        }
                    }
                }
            }
        }

        // Fallback: Filesystem discovery (v1.3.19 behavior)
        if self.verbose {
            self.log("[EdgeRouterAsync] Using filesystem kernel discovery")
                .await;
        }

        let concepts_dir = self.root.join("concepts");

        if !concepts_dir.exists() {
            return Err(CkpError::FileNotFound(format!(
                "Concepts directory not found: {}",
                concepts_dir.display()
            )));
        }

        let mut kernels = Vec::new();

        // Read concepts directory
        let mut entries = tokio::fs::read_dir(&concepts_dir)
            .await
            .map_err(|e| CkpError::IoError(format!("Failed to read concepts dir: {}", e)))?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| CkpError::IoError(format!("Failed to read entry: {}", e)))? {
            let kernel_name = entry.file_name().to_string_lossy().to_string();

            // Check if kernel has notification_contract
            match self.get_notification_targets(&kernel_name).await {
                Ok(targets) if !targets.is_empty() => {
                    kernels.push(kernel_name);
                }
                Ok(_) => {
                    // No targets, skip
                }
                Err(e) => {
                    if self.verbose {
                        self.log(&format!(
                            "[EdgeRouterAsync] Warning: Failed to read contract for {}: {}",
                            kernel_name, e
                        ))
                        .await;
                    }
                }
            }
        }

        Ok(kernels)
    }

    /// Discover kernels from Jena using SPARQL (diskless)
    ///
    /// Queries the RDF store for all kernels that have notification contracts
    /// (ckp:natsPublication predicate).
    ///
    /// # Arguments
    ///
    /// * `jena` - JenaStorage instance
    ///
    /// # Returns
    ///
    /// Vec of kernel names
    ///
    /// # SPARQL Query
    ///
    /// ```sparql
    /// PREFIX ckp: <urn:ckp:>
    /// SELECT DISTINCT ?kernelName WHERE {
    ///   ?kernel a ckp:Kernel ;
    ///           ckp:kernelName ?kernelName ;
    ///           ckp:natsPublication ?pub .
    /// }
    /// ```
    async fn discover_kernels_from_jena(&self, jena: &JenaStorage) -> Result<Vec<String>> {
        let sparql = r#"PREFIX ckp: <urn:ckp:>
SELECT DISTINCT ?kernelName WHERE {
  ?kernel a ckp:Kernel ;
          ckp:kernelName ?kernelName ;
          ckp:natsPublication ?pub .
}
ORDER BY ?kernelName"#;

        let result = jena.execute_sparql_query(sparql).await
            .map_err(|e| CkpError::EdgeRouting(format!("Jena kernel discovery query failed: {}", e)))?;

        // Parse SPARQL results
        let bindings = result
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
            .ok_or_else(|| CkpError::EdgeRouting("Invalid SPARQL results format".to_string()))?;

        let mut kernels = Vec::new();

        for binding in bindings {
            if let Some(kernel_name) = binding
                .get("kernelName")
                .and_then(|k| k.get("value"))
                .and_then(|v| v.as_str())
            {
                kernels.push(kernel_name.to_string());
            }
        }

        if kernels.is_empty() {
            if self.verbose {
                self.log("[EdgeRouterAsync] No kernels with notification contracts found in Jena")
                    .await;
            }
        }

        Ok(kernels)
    }

    /// Start monitoring a specific kernel's result stream
    async fn start_kernel_monitor(&self, kernel_name: &str) -> Result<()> {
        self.log(&format!("[EdgeRouterAsync] Starting monitor for {}", kernel_name))
            .await;

        // Subscribe to result stream
        let result_stream = self
            .transport
            .subscribe_results(kernel_name)
            .await
            .map_err(|e| CkpError::EdgeRouting(format!("Failed to subscribe to results: {}", e)))?;

        // Get notification targets
        let targets = self.get_notification_targets(kernel_name).await?;

        if targets.is_empty() {
            return Ok(());
        }

        // Spawn monitoring task
        let kernel_name_owned = kernel_name.to_string();
        let router = self.clone_for_task();
        let targets_clone = targets.clone();

        let handle = tokio::spawn(async move {
            router
                .monitor_results(kernel_name_owned, result_stream, targets_clone)
                .await;
        });

        // Store handle
        let mut monitors = self.active_monitors.write().await;
        monitors.insert(kernel_name.to_string(), handle);

        Ok(())
    }

    /// Monitor results from a specific kernel
    async fn monitor_results(
        &self,
        source_kernel: String,
        mut result_stream: ResultStream,
        targets: Vec<(String, String)>,
    ) {
        self.log(&format!("[EdgeRouterAsync] Monitoring results from {}", source_kernel))
            .await;

        while let Some(tool_response) = result_stream.next().await {
            if self.verbose {
                self.log(&format!(
                    "[EdgeRouterAsync] Result received from {}: {}",
                    source_kernel, tool_response.job_id
                ))
                .await;
            }

            // Route to each target
            for (target, predicate) in &targets {
                if let Err(e) = self
                    .route_result(&source_kernel, target, predicate, &tool_response)
                    .await
                {
                    eprintln!(
                        "[EdgeRouterAsync] Failed to route to {}: {}",
                        target, e
                    );
                }
            }
        }

        self.log(&format!(
            "[EdgeRouterAsync] Monitor stopped for {}",
            source_kernel
        ))
        .await;
    }

    /// Route a tool result to a target kernel via edge
    async fn route_result(
        &self,
        source: &str,
        target: &str,
        predicate: &str,
        result: &ToolResponse,
    ) -> Result<()> {
        // Ensure edge exists
        let edge_urn = format!("ckp://Edge#Connection-{}-to-{}-{}:v1.3.20", source, target, predicate);

        {
            let mut edge_kernel = self.edge_kernel.lock().await;

            if edge_kernel
                .get_edge(&edge_urn)
                .map_err(|e| CkpError::EdgeRouting(format!("get_edge failed: {}", e)))?
                .is_none()
            {
                self.log(&format!(
                    "[EdgeRouterAsync] Creating edge: {} -> {} ({})",
                    source, target, predicate
                ))
                .await;

                edge_kernel
                    .create_edge(predicate, source, target)
                    .map_err(|e| CkpError::EdgeRouting(format!("create_edge failed: {}", e)))?;

                // Persist edge metadata if storage is available
                if let Some(storage) = &self.storage {
                    // Check if storage is JenaStorage and save metadata
                    let edge_metadata = EdgeMetadata {
                        api_version: "conceptkernel/v1".to_string(),
                        kind: "EdgeConnection".to_string(),
                        urn: edge_urn.clone(),
                        created_at: chrono::Utc::now().to_rfc3339(),
                        predicate: predicate.to_string(),
                        source: source.to_string(),
                        target: target.to_string(),
                        version: "v1.3.20".to_string(),
                    };

                    // Try to downcast to JenaStorage for edge metadata storage
                    if let Some(jena) = storage.as_any().downcast_ref::<JenaStorage>() {
                        if let Err(e) = jena.save_edge_metadata_structured(&edge_metadata).await {
                            eprintln!(
                                "[EdgeRouterAsync] Warning: Failed to persist edge metadata: {}",
                                e
                            );
                        } else {
                            self.log(&format!(
                                "[EdgeRouterAsync] Persisted edge metadata to Jena: {}",
                                edge_urn
                            ))
                            .await;
                        }
                    }
                }

                // Publish edge.created event to NATS (v1.3.20)
                if let Some(publisher) = &self.edge_event_publisher {
                    publisher.publish_edge_created(&edge_urn, source, target, predicate).await;
                }
            }
        }

        // Create job message from result
        let job = JobMessage {
            job_id: format!("{}-routed", result.job_id),
            tool: "edge_routed_instance".to_string(),
            args: result.output.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            source: source.to_string(),
        };

        // Publish to edge queue via transport (NATS, LocalTransport, etc.)
        self.transport
            .publish_to_edge(target, predicate, source, job)
            .await?;

        self.log(&format!(
            "[EdgeRouterAsync] Routed {} to {} via edge {}",
            result.job_id, target, predicate
        ))
        .await;

        // Publish edge routing event to NATS (v1.3.20)
        if let Some(publisher) = &self.edge_event_publisher {
            publisher.publish_edge_routed(&edge_urn, source, target, predicate, &result.job_id).await;
        }

        Ok(())
    }

    /// Get notification targets for a kernel (with caching)
    async fn get_notification_targets(&self, kernel_name: &str) -> Result<Vec<(String, String)>> {
        // Check cache first
        {
            let cache = self.notification_cache.read().await;
            if let Some(targets) = cache.get(kernel_name) {
                if self.verbose {
                    self.log(&format!("[EdgeRouterAsync] Cache hit for {}", kernel_name))
                        .await;
                }
                return Ok(targets.clone());
            }
        }

        if self.verbose {
            self.log(&format!(
                "[EdgeRouterAsync] Reading notification_contract for {}",
                kernel_name
            ))
            .await;
        }

        // Read from ontology
        let contract = self
            .ontology_reader
            .read_notification_contract(kernel_name)
            .map_err(|e| {
                CkpError::Ontology(format!("Failed to read notification contract: {}", e))
            })?;

        // Convert to (target, predicate) tuples
        // Default predicate: PRODUCES
        let targets: Vec<(String, String)> = contract
            .into_iter()
            .map(|notif| (notif.target_kernel, "PRODUCES".to_string()))
            .collect();

        // Update cache
        {
            let mut cache = self.notification_cache.write().await;
            cache.insert(kernel_name.to_string(), targets.clone());
        }

        Ok(targets)
    }

    /// Stop all active monitors
    async fn stop_all_monitors(&self) {
        let mut monitors = self.active_monitors.write().await;

        for (kernel_name, handle) in monitors.drain() {
            self.log(&format!("[EdgeRouterAsync] Stopping monitor for {}", kernel_name))
                .await;
            handle.abort();
        }
    }

    /// Clone self for spawning tasks
    ///
    /// Creates a lightweight clone suitable for passing to tokio::spawn
    fn clone_for_task(&self) -> Self {
        Self {
            root: self.root.clone(),
            transport: self.transport.clone(),
            storage: self.storage.clone(),
            edge_kernel: self.edge_kernel.clone(),
            ontology_reader: OntologyReader::new(self.root.clone()),
            _ontology_library: self._ontology_library.clone(),
            _process_tracker: self._process_tracker.clone(),
            verbose: self.verbose,
            notification_cache: self.notification_cache.clone(),
            active_monitors: self.active_monitors.clone(),
            edge_event_publisher: self.edge_event_publisher.clone(),
        }
    }

    /// Log message
    async fn log(&self, message: &str) {
        eprintln!("{}", message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires full setup
    async fn test_edge_router_daemon_creation() {
        use crate::drivers::LocalTransport;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Create concepts directory
        tokio::fs::create_dir_all(root.join("concepts"))
            .await
            .unwrap();

        let transport = Arc::new(LocalTransport::new(root.clone(), "TestKernel".to_string()));

        let router = EdgeRouterDaemonAsync::new(root, transport, None, true).await;
        assert!(router.is_ok());
    }
}
