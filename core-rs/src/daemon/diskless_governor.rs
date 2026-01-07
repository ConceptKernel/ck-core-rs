//! DisklessGovernor - Diskless job processing daemon (v1.3.20)
//!
//! ## Responsibilities
//!
//! - Load kernel configuration from Jena via SPARQL
//! - Subscribe to NATS inbox for incoming jobs
//! - Process jobs through kernel's tool execution
//! - Publish results back to NATS
//! - Emit lifecycle events for monitoring
//!
//! ## Architecture (Diskless Design)
//!
//! **NO Disk Operations**:
//! - Configuration loaded from Jena RDF store (SPARQL queries)
//! - Jobs received via NATS JetStream (not filesystem)
//! - Results published via NATS (not filesystem)
//! - State tracked in memory only (stateless containers)
//!
//! **Key Design Principles**:
//! - **Stateless**: No local filesystem dependencies
//! - **Cloud-Native**: Runs in Kubernetes without persistent volumes
//! - **Event-Driven**: All I/O through NATS messaging
//! - **RDF-First**: Ontology as source of truth (no YAML files)
//!
//! ## Kernel Configuration via SPARQL
//!
//! The governor loads kernel configuration from Jena using SPARQL:
//!
//! ```sparql
//! PREFIX ckp: <urn:ckp:>
//! PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
//! SELECT ?kernel ?description ?subSubject ?pubSubject WHERE {
//!   BIND(<ckp://Usecase.SimplePassthrough.Source:v1.0.0> as ?kernel)
//!   ?kernel a ckp:Kernel ;
//!           ckp:description ?description .
//!   OPTIONAL { ?kernel ckp:natsSubscription/ckp:subject ?subSubject }
//!   OPTIONAL { ?kernel ckp:natsPublication/ckp:subject ?pubSubject }
//! }
//! ```
//!
//! ## NATS Messaging Pattern
//!
//! **Inbox Subscription**:
//! - Subject: `ckp.{kernel_name}.inbox`
//! - Consumer group: `{kernel_name}-governor`
//! - Receives JobMessage instances
//!
//! **Result Publication**:
//! - Subject: `ckp.{kernel_name}.results`
//! - Publishes ToolResponse instances
//!
//! ## Lifecycle Events
//!
//! The governor emits the following events to NATS:
//! - `governor.startup.ready` - Governor started and subscribed
//! - `governor.job.received` - Job received from inbox
//! - `governor.job.completed` - Job execution completed
//! - `governor.job.failed` - Job execution failed
//! - `governor.shutdown` - Governor shutting down
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use ckp_core::daemon::DisklessGovernor;
//! use ckp_core::drivers::{NatsTransport, JenaStorage};
//! use std::sync::Arc;
//!
//! // Create transport and storage drivers
//! let transport = Arc::new(NatsTransport::new("nats://localhost:4222").await?);
//! let storage = Arc::new(JenaStorage::new(
//!     "http://localhost:3030".to_string(),
//!     "ck".to_string()
//! ));
//!
//! // Create diskless governor
//! let governor = DisklessGovernor::new(
//!     "Usecase.SimplePassthrough.Source".to_string(),
//!     transport,
//!     storage,
//!     true // verbose
//! ).await?;
//!
//! // Start processing jobs
//! let shutdown = tokio::sync::broadcast::channel(1).0;
//! governor.start(shutdown).await?;
//! ```

use crate::drivers::{JobMessage, StorageDriver, ToolResponse, TransportDriver};
use crate::errors::{CkpError, Result};
use async_nats::Client as NatsClient;
use futures::StreamExt;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Kernel configuration loaded from Jena
///
/// Contains the runtime configuration needed to execute a kernel
/// without requiring filesystem access.
#[derive(Debug, Clone)]
pub struct KernelConfig {
    /// Kernel name (e.g., "Usecase.SimplePassthrough.Source")
    pub kernel_name: String,

    /// Human-readable description
    pub description: String,

    /// NATS subjects to subscribe to for incoming jobs
    pub nats_subscriptions: Vec<String>,

    /// NATS subjects to publish results to
    pub nats_publications: Vec<String>,

    /// Kernel capabilities (e.g., ["passthrough", "transform"])
    pub capabilities: Vec<String>,

    /// Kernel runtime (e.g., "node:cold", "python:hot", "rust:binary")
    pub runtime: String,
}

/// DisklessGovernor - Stateless job processing daemon
///
/// Processes jobs from NATS without any filesystem dependencies.
/// Configuration is loaded from Jena RDF store via SPARQL queries.
///
/// # Architecture
///
/// ```text
/// ┌─────────────────┐
/// │  Jena Fuseki    │ ──SPARQL──> [Load kernel config]
/// │  (RDF Store)    │
/// └─────────────────┘
///         ↓
/// ┌─────────────────┐
/// │ DisklessGovernor│
/// │  (In-Memory)    │
/// └─────────────────┘
///         ↓
/// ┌─────────────────┐
/// │  NATS JetStream │ ──subscribe──> [Receive jobs]
/// │  (Messaging)    │ <─publish──── [Send results]
/// └─────────────────┘
/// ```
pub struct DisklessGovernor {
    /// Kernel name this governor manages
    kernel_name: String,

    /// Transport driver (NATS JetStream)
    transport: Arc<dyn TransportDriver>,

    /// Storage driver (Jena RDF store)
    storage: Arc<dyn StorageDriver>,

    /// Kernel configuration loaded from Jena
    config: Option<KernelConfig>,

    /// Verbose logging
    verbose: bool,

    /// Optional NATS client for lifecycle events
    nats_client: Option<NatsClient>,
}

impl DisklessGovernor {
    /// Create new DisklessGovernor
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name to manage
    /// * `transport` - Transport driver (typically NatsTransport)
    /// * `storage` - Storage driver (typically JenaStorage)
    /// * `verbose` - Enable verbose logging
    ///
    /// # Returns
    ///
    /// DisklessGovernor instance (configuration not yet loaded)
    pub async fn new(
        kernel_name: String,
        transport: Arc<dyn TransportDriver>,
        storage: Arc<dyn StorageDriver>,
        verbose: bool,
    ) -> Result<Self> {
        // Initialize NATS client for lifecycle events
        let nats_url = std::env::var("NATS_URL").ok();
        let nats_client = if let Some(url) = nats_url.as_deref() {
            match async_nats::connect(url).await {
                Ok(client) => Some(client),
                Err(e) => {
                    eprintln!("[DisklessGovernor] Failed to connect to NATS for events: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            kernel_name,
            transport,
            storage,
            config: None,
            verbose,
            nats_client,
        })
    }

    /// Load kernel configuration from Jena via SPARQL
    ///
    /// Executes a SPARQL query to retrieve kernel metadata:
    /// - Description
    /// - NATS subscriptions (inbox subjects)
    /// - NATS publications (result subjects)
    /// - Capabilities
    /// - Runtime configuration
    ///
    /// # Returns
    ///
    /// KernelConfig loaded from RDF store
    async fn load_kernel_config(&self) -> Result<KernelConfig> {
        self.log(&format!(
            "[DisklessGovernor] Loading configuration for kernel: {}",
            self.kernel_name
        ))
        .await;

        // Build SPARQL query to load kernel configuration
        let sparql_query = format!(
            r#"
PREFIX ckp: <urn:ckp:>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
SELECT ?kernel ?description ?capability ?runtime ?subSubject ?pubSubject WHERE {{
  BIND(<ckp://{}> as ?kernel)
  ?kernel a ckp:Kernel ;
          ckp:description ?description .
  OPTIONAL {{ ?kernel ckp:capability ?capability }}
  OPTIONAL {{ ?kernel ckp:runtime ?runtime }}
  OPTIONAL {{ ?kernel ckp:natsSubscription/ckp:subject ?subSubject }}
  OPTIONAL {{ ?kernel ckp:natsPublication/ckp:subject ?pubSubject }}
}}
"#,
            self.kernel_name
        );

        // Execute SPARQL query via storage driver
        let results = self.execute_sparql_select(&sparql_query).await?;

        if results.is_empty() {
            return Err(CkpError::KernelNotFound(format!(
                "No configuration found for kernel: {}",
                self.kernel_name
            )));
        }

        // Parse SPARQL results into KernelConfig
        let mut description = String::new();
        let mut capabilities = Vec::new();
        let mut runtime = String::new();
        let mut nats_subscriptions = Vec::new();
        let mut nats_publications = Vec::new();

        for result in &results {
            // Extract description (same for all rows)
            if let Some(desc) = result.get("description") {
                if let Some(desc_str) = desc.as_str() {
                    description = desc_str.to_string();
                }
            }

            // Extract runtime
            if let Some(rt) = result.get("runtime") {
                if let Some(rt_str) = rt.as_str() {
                    runtime = rt_str.to_string();
                }
            }

            // Extract capability
            if let Some(cap) = result.get("capability") {
                if let Some(cap_str) = cap.as_str() {
                    if !capabilities.contains(&cap_str.to_string()) {
                        capabilities.push(cap_str.to_string());
                    }
                }
            }

            // Extract NATS subscription subject
            if let Some(sub) = result.get("subSubject") {
                if let Some(sub_str) = sub.as_str() {
                    if !nats_subscriptions.contains(&sub_str.to_string()) {
                        nats_subscriptions.push(sub_str.to_string());
                    }
                }
            }

            // Extract NATS publication subject
            if let Some(pub_sub) = result.get("pubSubject") {
                if let Some(pub_str) = pub_sub.as_str() {
                    if !nats_publications.contains(&pub_str.to_string()) {
                        nats_publications.push(pub_str.to_string());
                    }
                }
            }
        }

        // Default to standard NATS subjects if not configured
        if nats_subscriptions.is_empty() {
            nats_subscriptions.push(format!("ckp.{}.inbox", self.kernel_name));
        }

        if nats_publications.is_empty() {
            nats_publications.push(format!("ckp.{}.results", self.kernel_name));
        }

        let config = KernelConfig {
            kernel_name: self.kernel_name.clone(),
            description,
            nats_subscriptions,
            nats_publications,
            capabilities,
            runtime,
        };

        self.log(&format!(
            "[DisklessGovernor] Configuration loaded: {:?}",
            config
        ))
        .await;

        Ok(config)
    }

    /// Execute SPARQL SELECT query and parse results
    ///
    /// Helper method to execute SPARQL queries via storage driver
    /// and parse JSON results into structured data.
    ///
    /// # Arguments
    ///
    /// * `sparql_query` - SPARQL SELECT query
    ///
    /// # Returns
    ///
    /// Vector of result bindings (each row as HashMap)
    async fn execute_sparql_select(&self, sparql_query: &str) -> Result<Vec<HashMap<String, JsonValue>>> {
        // Try to downcast storage to JenaStorage for SPARQL support
        let jena_storage = self
            .storage
            .as_any()
            .downcast_ref::<crate::drivers::JenaStorage>()
            .ok_or_else(|| {
                CkpError::NotImplemented(
                    "SPARQL queries require JenaStorage backend".to_string(),
                )
            })?;

        // Execute SPARQL query
        let json_results = jena_storage.query_sparql_json(sparql_query).await?;

        // Parse JSON response from Jena
        let results_obj = json_results
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
            .ok_or_else(|| CkpError::SparqlError("Invalid SPARQL results format".to_string()))?;

        // Convert SPARQL bindings to HashMap for easier access
        let mut parsed_results = Vec::new();

        for binding in results_obj {
            let mut row = HashMap::new();

            if let Some(obj) = binding.as_object() {
                for (key, value) in obj {
                    // Extract value from SPARQL binding format
                    if let Some(binding_value) = value.get("value") {
                        row.insert(key.clone(), binding_value.clone());
                    }
                }
            }

            parsed_results.push(row);
        }

        Ok(parsed_results)
    }

    /// Start the diskless governor daemon
    ///
    /// Loads configuration from Jena, subscribes to NATS inbox,
    /// processes incoming jobs, and publishes results.
    ///
    /// # Arguments
    ///
    /// * `shutdown` - Broadcast channel for shutdown signal
    ///
    /// # Returns
    ///
    /// Result indicating success or error
    pub async fn start(&mut self, mut shutdown: broadcast::Receiver<()>) -> Result<()> {
        self.log("[DisklessGovernor] Starting diskless governor...")
            .await;
        self.log(&format!(
            "[DisklessGovernor] Kernel: {}",
            self.kernel_name
        ))
        .await;

        // Write daemon PID for tracking (ephemeral, not persisted)
        let pid = std::process::id();
        self.log(&format!("[DisklessGovernor] PID: {}", pid))
            .await;

        // Load kernel configuration from Jena
        let config = self.load_kernel_config().await?;
        self.config = Some(config.clone());

        self.log(&format!(
            "[DisklessGovernor] Subscriptions: {:?}",
            config.nats_subscriptions
        ))
        .await;
        self.log(&format!(
            "[DisklessGovernor] Publications: {:?}",
            config.nats_publications
        ))
        .await;

        // Subscribe to NATS inbox
        let mut job_stream = self
            .transport
            .subscribe_inbox(&self.kernel_name)
            .await
            .map_err(|e| {
                CkpError::Transport(format!("Failed to subscribe to inbox: {}", e))
            })?;

        self.log(&format!(
            "[DisklessGovernor] Subscribed to inbox: ckp.{}.inbox",
            self.kernel_name
        ))
        .await;

        // Emit startup ready event
        self.publish_event(
            "governor.startup.ready",
            serde_json::json!({
                "kernel": self.kernel_name,
                "pid": pid,
                "config": {
                    "subscriptions": config.nats_subscriptions,
                    "publications": config.nats_publications,
                    "capabilities": config.capabilities,
                    "runtime": config.runtime
                }
            }),
        )
        .await;

        self.log("[DisklessGovernor] Ready - Waiting for jobs").await;

        // Main event loop: process jobs from NATS
        loop {
            tokio::select! {
                // Handle shutdown signal
                _ = shutdown.recv() => {
                    self.log("[DisklessGovernor] Shutdown signal received").await;

                    // Emit shutdown event
                    self.publish_event(
                        "governor.shutdown",
                        serde_json::json!({
                            "kernel": self.kernel_name,
                            "pid": pid
                        })
                    ).await;

                    break;
                }

                // Process incoming jobs
                Some(job) = job_stream.next() => {
                    self.log(&format!(
                        "[DisklessGovernor] Job received: {}",
                        job.job_id
                    )).await;

                    // Emit job received event
                    self.publish_event(
                        "governor.job.received",
                        serde_json::json!({
                            "kernel": self.kernel_name,
                            "job_id": job.job_id,
                            "tool": job.tool
                        })
                    ).await;

                    // Process job
                    match self.process_job(job).await {
                        Ok(response) => {
                            // Publish result via NATS
                            if let Err(e) = self.transport.publish_response(&self.kernel_name, response.clone()).await {
                                eprintln!(
                                    "[DisklessGovernor] Failed to publish result: {}",
                                    e
                                );
                            } else {
                                self.log(&format!(
                                    "[DisklessGovernor] Published result: {}",
                                    response.job_id
                                )).await;

                                // Emit job completed event
                                self.publish_event(
                                    "governor.job.completed",
                                    serde_json::json!({
                                        "kernel": self.kernel_name,
                                        "job_id": response.job_id,
                                        "status": response.status,
                                        "duration_ms": response.duration_ms
                                    })
                                ).await;
                            }
                        }
                        Err(e) => {
                            eprintln!("[DisklessGovernor] Job processing failed: {}", e);

                            // Emit job failed event
                            self.publish_event(
                                "governor.job.failed",
                                serde_json::json!({
                                    "kernel": self.kernel_name,
                                    "error": e.to_string()
                                })
                            ).await;
                        }
                    }
                }
            }
        }

        self.log("[DisklessGovernor] Shutdown complete").await;

        Ok(())
    }

    /// Process a job by executing the kernel's tool
    ///
    /// This is a simplified implementation that demonstrates the pattern.
    /// In production, this would:
    /// - Load tool definition from Jena
    /// - Execute tool based on runtime (binary, container, API, etc.)
    /// - Handle timeouts and errors
    /// - Track execution in RDF store
    ///
    /// # Arguments
    ///
    /// * `job` - Job message from NATS
    ///
    /// # Returns
    ///
    /// Tool execution response
    async fn process_job(&self, job: JobMessage) -> Result<ToolResponse> {
        let start_time = std::time::Instant::now();

        self.log(&format!(
            "[DisklessGovernor] Processing job: {} (tool: {})",
            job.job_id, job.tool
        ))
        .await;

        // TODO: Load tool definition from Jena
        // let tool_def = self.storage.load_tool_definition(&self.kernel_name, &job.tool).await?;

        // TODO: Execute tool based on execution mode
        // For now, simulate successful execution (passthrough pattern)
        let output = serde_json::json!({
            "status": "success",
            "message": "Job processed successfully (diskless)",
            "kernel": self.kernel_name,
            "input": job.args
        });

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(ToolResponse {
            job_id: job.job_id,
            status: "success".to_string(),
            output,
            timestamp: chrono::Utc::now().to_rfc3339(),
            duration_ms,
            error: None,
        })
    }

    /// Log message
    async fn log(&self, message: &str) {
        if self.verbose {
            eprintln!("{}", message);
        }
    }

    /// Publish lifecycle event to NATS
    ///
    /// Publishes event to `ckp.events.governor.{event_type}` subject
    ///
    /// # Arguments
    ///
    /// * `event_type` - Event type (e.g., "startup.ready", "job.received")
    /// * `payload` - Event payload
    async fn publish_event(&self, event_type: &str, payload: JsonValue) {
        if let Some(client) = &self.nats_client {
            let subject = format!("ckp.events.{}", event_type);
            let payload_json = serde_json::to_vec(&payload).unwrap_or_default();

            if let Err(e) = client.publish(subject.clone(), payload_json.into()).await {
                eprintln!(
                    "[DisklessGovernor] Failed to publish event to {}: {}",
                    subject, e
                );
            } else if self.verbose {
                self.log(&format!("[DisklessGovernor] Published event: {}", subject))
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires NATS and Jena setup
    async fn test_diskless_governor_creation() {
        use crate::drivers::{JenaStorage, NatsTransport};

        let transport = Arc::new(NatsTransport::new("nats://localhost:4222").await.unwrap());
        let storage = Arc::new(JenaStorage::new(
            "http://localhost:3030".to_string(),
            "ck".to_string(),
        ));

        let governor = DisklessGovernor::new(
            "TestKernel".to_string(),
            transport,
            storage,
            true,
        )
        .await;

        assert!(governor.is_ok());
    }

    #[test]
    fn test_kernel_config_clone() {
        let config = KernelConfig {
            kernel_name: "Test".to_string(),
            description: "Test kernel".to_string(),
            nats_subscriptions: vec!["ckp.Test.inbox".to_string()],
            nats_publications: vec!["ckp.Test.results".to_string()],
            capabilities: vec!["test".to_string()],
            runtime: "node:cold".to_string(),
        };

        let cloned = config.clone();
        assert_eq!(config.kernel_name, cloned.kernel_name);
        assert_eq!(config.description, cloned.description);
    }
}
