//! Kernel Event Publisher - Publishes kernel lifecycle events to NATS
//!
//! Enables real-time observability of kernel operations including:
//! - Startup/shutdown lifecycle
//! - Job processing pipeline
//! - Validation results
//!
//! ## Configuration
//!
//! Set NATS_URL environment variable to enable event publishing:
//! ```bash
//! export NATS_URL=nats://localhost:4222
//! ```
//!
//! If NATS_URL is not set, events are logged but not published.

use crate::errors::Result;
use async_nats::Client as NatsClient;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Kernel lifecycle event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KernelEventType {
    // Startup lifecycle - granular phases
    StartupInitiated,
    StartupProjectConfigLoaded,        // .ckproject loaded
    StartupPortsConfigLoaded,          // .ckports loaded
    StartupGitValidationStarted,       // Git repository validation started (v1.3.20)
    StartupGitValidationPassed,        // Git repository and v0.1 tag validated (v1.3.20)
    StartupUrnParsed,
    StartupOntologyLoaded,            // conceptkernel.yaml loaded
    StartupToolPathResolved,
    StartupToolValidated,
    StartupInboxValidated,
    StartupPidFileCreated,
    StartupLoggingConfigured,
    StartupStorageInitialized,
    StartupOntologyLibraryLoaded,     // RDF ontologies loaded
    StartupBasicQueriesValidated,     // Basic SPARQL queries validated
    StartupJenaConnected,
    StartupNatsConnected,
    StartupToollessMode,              // Kernel running in toolless passthrough mode
    StartupReady,
    StartupFailed,  // NEW: Startup validation failed

    // Shutdown lifecycle
    ShutdownInitiated,
    ShutdownComplete,

    // Job processing - granular phases
    JobReceived,
    JobQueued,
    JobValidationStarted,
    JobOntologyValidated,
    JobShaclValidated,
    JobEdgesValidated,
    JobValidationPassed,
    JobValidationFailed,
    JobToolSpawning,
    JobToolStarted,
    JobProcessing,
    JobToolCompleted,
    JobToollessPassthrough,           // Job processed in toolless passthrough mode
    InstanceCreated,                  // Instance created in Jena
    EdgesDiscovered,                  // Interested edges found
    OutputRouted,                     // Output routed to edges/NATS
    JobMovedToErrorQueue,             // Job moved to error queue
    JobProofMinting,
    JobProofMinted,
    JobSavingToJena,
    JobSavedToJena,
    JobEdgeConfirmation,
    JobEdgesConfirmed,
    JobCompleted,
    JobRejected,

    // Workflow phase transitions
    PhasePending,
    PhaseInProgress,
    PhaseCompleted,
    PhaseFailed,
    PhaseSkipped,
}

impl KernelEventType {
    /// Get subject suffix for this event type
    fn subject_suffix(&self) -> &str {
        match self {
            Self::StartupInitiated => "lifecycle.startup.initiated",
            Self::StartupProjectConfigLoaded => "lifecycle.startup.project_config_loaded",
            Self::StartupPortsConfigLoaded => "lifecycle.startup.ports_config_loaded",
            Self::StartupGitValidationStarted => "lifecycle.startup.git_validation_started",
            Self::StartupGitValidationPassed => "lifecycle.startup.git_validation_passed",
            Self::StartupUrnParsed => "lifecycle.startup.urn_parsed",
            Self::StartupOntologyLoaded => "lifecycle.startup.ontology_loaded",
            Self::StartupToolPathResolved => "lifecycle.startup.tool_path_resolved",
            Self::StartupToolValidated => "lifecycle.startup.tool_validated",
            Self::StartupInboxValidated => "lifecycle.startup.inbox_validated",
            Self::StartupPidFileCreated => "lifecycle.startup.pid_file_created",
            Self::StartupLoggingConfigured => "lifecycle.startup.logging_configured",
            Self::StartupStorageInitialized => "lifecycle.startup.storage_initialized",
            Self::StartupOntologyLibraryLoaded => "lifecycle.startup.ontology_library_loaded",
            Self::StartupBasicQueriesValidated => "lifecycle.startup.basic_queries_validated",
            Self::StartupJenaConnected => "lifecycle.startup.jena_connected",
            Self::StartupNatsConnected => "lifecycle.startup.nats_connected",
            Self::StartupToollessMode => "lifecycle.startup.toolless_mode",
            Self::StartupReady => "lifecycle.startup.ready",
            Self::StartupFailed => "lifecycle.startup.failed",
            Self::ShutdownInitiated => "lifecycle.shutdown.initiated",
            Self::ShutdownComplete => "lifecycle.shutdown.complete",
            Self::JobReceived => "job.received",
            Self::JobQueued => "job.queued",
            Self::JobValidationStarted => "job.validation.started",
            Self::JobOntologyValidated => "job.validation.ontology_validated",
            Self::JobShaclValidated => "job.validation.shacl_validated",
            Self::JobEdgesValidated => "job.validation.edges_validated",
            Self::JobValidationPassed => "job.validation.passed",
            Self::JobValidationFailed => "job.validation.failed",
            Self::JobToolSpawning => "job.tool.spawning",
            Self::JobToolStarted => "job.tool.started",
            Self::JobProcessing => "job.processing",
            Self::JobToolCompleted => "job.tool.completed",
            Self::JobToollessPassthrough => "job.toolless.passthrough",
            Self::InstanceCreated => "job.instance.created",
            Self::EdgesDiscovered => "job.edges.discovered",
            Self::OutputRouted => "job.output.routed",
            Self::JobMovedToErrorQueue => "job.error.moved_to_queue",
            Self::JobProofMinting => "job.proof.minting",
            Self::JobProofMinted => "job.proof.minted",
            Self::JobSavingToJena => "job.jena.saving",
            Self::JobSavedToJena => "job.jena.saved",
            Self::JobEdgeConfirmation => "job.edges.confirming",
            Self::JobEdgesConfirmed => "job.edges.confirmed",
            Self::JobCompleted => "job.completed",
            Self::JobRejected => "job.rejected",
            Self::PhasePending => "workflow.phase.pending",
            Self::PhaseInProgress => "workflow.phase.in_progress",
            Self::PhaseCompleted => "workflow.phase.completed",
            Self::PhaseFailed => "workflow.phase.failed",
            Self::PhaseSkipped => "workflow.phase.skipped",
        }
    }
}

/// BFO Occurrent Type - distinguishes process boundaries from temporal parts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum OccurrentType {
    /// Process boundary (start/end of process)
    ProcessBoundary,
    /// Temporal part (ongoing phase within process)
    TemporalPart,
}

/// Kernel lifecycle event (BFO-compliant)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelEvent {
    /// Event type (maps to NATS subject suffix)
    pub event_type: KernelEventType,

    /// Kernel name
    pub kernel_name: String,

    /// ISO 8601 timestamp
    pub timestamp: String,

    /// Process URN (ckp://Process#{Type}-{timestampMs}-{hash})
    pub process_urn: String,

    /// BFO Occurrent type (ProcessBoundary or TemporalPart)
    pub occurrent_type: OccurrentType,

    /// Phase name (e.g., "startup-initiated", "job-processing")
    pub phase: String,

    /// Progress percentage (0-100, None for non-progressive events)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,

    /// Event-specific payload
    pub payload: serde_json::Value,
}

/// Kernel event publisher
pub struct KernelEventPublisher {
    /// Kernel name
    kernel_name: String,

    /// Optional NATS client
    pub(crate) nats_client: Option<NatsClient>,

    /// Process URN for current governor startup
    startup_process_urn: String,

    /// Job counter for unique job process URNs
    job_counter: Arc<AtomicU64>,
}

impl KernelEventPublisher {
    /// Get the startup process URN for Jena cataloging
    pub fn get_startup_process_urn(&self) -> String {
        self.startup_process_urn.clone()
    }

    /// Generate a Process URN
    ///
    /// Format: ckp://Process#{Type}-{timestampMs}-{hash}
    fn generate_process_urn(process_type: &str, kernel_name: &str) -> String {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        // Generate hash from kernel_name + timestamp
        let mut hasher = DefaultHasher::new();
        kernel_name.hash(&mut hasher);
        timestamp_ms.hash(&mut hasher);
        let hash = format!("{:x}", hasher.finish());

        format!("ckp://Process#{}-{}-{}", process_type, timestamp_ms, &hash[0..8])
    }

    /// Create new event publisher
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name
    /// * `nats_url` - Optional NATS URL from environment
    ///
    /// # Returns
    /// Publisher instance (with or without NATS connection)
    pub async fn new(kernel_name: String, nats_url: Option<&str>) -> Result<Self> {
        let nats_client = if let Some(url) = nats_url {
            match async_nats::connect(url).await {
                Ok(client) => {
                    eprintln!("[EventPublisher] Connected to NATS at {}", url);
                    Some(client)
                }
                Err(e) => {
                    eprintln!("[EventPublisher] Failed to connect to NATS: {}", e);
                    eprintln!("[EventPublisher] Continuing without event publishing");
                    None
                }
            }
        } else {
            None
        };

        // Generate startup process URN
        let startup_process_urn = Self::generate_process_urn("GovernorStartup", &kernel_name);

        let publisher = Self {
            kernel_name,
            nats_client,
            startup_process_urn,
            job_counter: Arc::new(AtomicU64::new(0)),
        };

        // Start discovery listener if NATS connected
        publisher.start_discovery_listener().await;

        Ok(publisher)
    }

    /// Start listening for discovery requests and auto-respond
    async fn start_discovery_listener(&self) {
        if let Some(nats) = &self.nats_client {
            let kernel_name = self.kernel_name.clone();
            let nats_clone = nats.clone();

            tokio::spawn(async move {
                let sub_result = nats_clone.subscribe("kernel.discovery.request").await;

                if let Ok(mut sub) = sub_result {
                    eprintln!("[EventPublisher] [{}] Listening for discovery requests", kernel_name);

                    while let Some(msg) = sub.next().await {
                        // Respond with announcement
                        let announce_subject = format!("kernel.{}.lifecycle.announce", kernel_name);
                        let announce_payload = serde_json::json!({
                            "kernel_name": kernel_name,
                            "status": "active",
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                            "capabilities": ["message", "trigger", "status"],
                            "event_type": "discovery_response"
                        });

                        if let Ok(payload_bytes) = serde_json::to_vec(&announce_payload) {
                            let _ = nats_clone.publish(announce_subject.clone(), payload_bytes.into()).await;
                            eprintln!("[EventPublisher] [{}] Responded to discovery request", kernel_name);
                        }
                    }
                } else {
                    eprintln!("[EventPublisher] [{}] Failed to subscribe to discovery requests", kernel_name);
                }
            });
        }
    }

    /// Publish a kernel event
    ///
    /// Non-blocking: Errors are logged but not propagated
    pub async fn publish(&self, event: KernelEvent) {
        // Always log event to stderr for debugging
        eprintln!(
            "[EventPublisher] [{}] {:?}: {}",
            self.kernel_name,
            event.event_type,
            serde_json::to_string(&event.payload).unwrap_or_default()
        );

        // Publish to NATS if connected
        if let Some(nats) = &self.nats_client {
            let subject = format!("kernel.{}.{}", self.kernel_name, event.event_type.subject_suffix());

            match serde_json::to_vec(&event) {
                Ok(payload) => {
                    if let Err(e) = nats.publish(subject.clone(), payload.into()).await {
                        eprintln!("[EventPublisher] Failed to publish to {}: {}", subject, e);
                    }
                }
                Err(e) => {
                    eprintln!("[EventPublisher] Failed to serialize event: {}", e);
                }
            }
        }
    }

    /// Publish custom message to arbitrary NATS subject
    ///
    /// Used for edge announcements and other non-kernel-specific events
    pub async fn publish_custom(&self, subject: &str, payload: serde_json::Value) {
        eprintln!("[EventPublisher] Publishing to {}", subject);

        if let Some(nats) = &self.nats_client {
            match serde_json::to_vec(&payload) {
                Ok(payload_bytes) => {
                    if let Err(e) = nats.publish(subject.to_string(), payload_bytes.into()).await {
                        eprintln!("[EventPublisher] Failed to publish to {}: {}", subject, e);
                    }
                }
                Err(e) => {
                    eprintln!("[EventPublisher] Failed to serialize payload: {}", e);
                }
            }
        }
    }

    /// Publish startup.initiated event
    pub async fn publish_startup_initiated(&self, pid: u32) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupInitiated,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::ProcessBoundary,
            phase: "startup-initiated".to_string(),
            progress: Some(0),
            payload: serde_json::json!({
                "pid": pid,
                "mode": "governor"
            }),
        }).await;
    }

    /// Publish startup.project_config_loaded event
    pub async fn publish_project_config_loaded(&self, ckproject_path: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupProjectConfigLoaded,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-project-config-loaded".to_string(),
            progress: Some(5),
            payload: serde_json::json!({
                "ckproject_path": ckproject_path
            }),
        }).await;
    }

    /// Publish startup.ports_config_loaded event
    pub async fn publish_ports_config_loaded(&self, ckports_path: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupPortsConfigLoaded,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-ports-config-loaded".to_string(),
            progress: Some(15),
            payload: serde_json::json!({
                "ckports_path": ckports_path
            }),
        }).await;
    }

    /// Publish startup.git_validation_started event (v1.3.20)
    pub async fn publish_git_validation_started(&self, kernel_dir: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupGitValidationStarted,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-git-validation-started".to_string(),
            progress: Some(17),
            payload: serde_json::json!({
                "kernel_dir": kernel_dir
            }),
        }).await;
    }

    /// Publish startup.git_validation_passed event (v1.3.20)
    pub async fn publish_git_validation_passed(&self, kernel_dir: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupGitValidationPassed,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-git-validation-passed".to_string(),
            progress: Some(19),
            payload: serde_json::json!({
                "git_repo": true,
                "has_v0_1_tag": true,
                "kernel_dir": kernel_dir,
                "status": "passed"
            }),
        }).await;
    }

    /// Publish startup.urn_parsed event
    pub async fn publish_urn_parsed(&self, kernel_urn: &str, parsed_name: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupUrnParsed,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-urn-parsed".to_string(),
            progress: Some(20),
            payload: serde_json::json!({
                "kernel_urn": kernel_urn,
                "parsed_name": parsed_name
            }),
        }).await;
    }

    /// Publish startup.tool_path_resolved event
    pub async fn publish_tool_path_resolved(&self, tool_path: &str, tool_command: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupToolPathResolved,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-tool-path-resolved".to_string(),
            progress: Some(30),
            payload: serde_json::json!({
                "tool_path": tool_path,
                "tool_command": tool_command
            }),
        }).await;
    }

    /// Publish startup.tool_validated event
    pub async fn publish_tool_validated(&self, tool_exists: bool) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupToolValidated,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-tool-validated".to_string(),
            progress: Some(40),
            payload: serde_json::json!({
                "tool_exists": tool_exists
            }),
        }).await;
    }

    /// Publish startup.inbox_validated event
    pub async fn publish_inbox_validated(&self, inbox_path: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupInboxValidated,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-inbox-validated".to_string(),
            progress: Some(50),
            payload: serde_json::json!({
                "inbox_path": inbox_path
            }),
        }).await;
    }

    /// Publish startup.pid_file_created event
    pub async fn publish_pid_file_created(&self, pid_path: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupPidFileCreated,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-pid-file-created".to_string(),
            progress: Some(60),
            payload: serde_json::json!({
                "pid_path": pid_path,
                "locked": true
            }),
        }).await;
    }

    /// Publish startup.logging_configured event
    pub async fn publish_logging_configured(&self, log_path: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupLoggingConfigured,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-logging-configured".to_string(),
            progress: Some(65),
            payload: serde_json::json!({
                "log_path": log_path
            }),
        }).await;
    }

    /// Publish startup.storage_initialized event
    pub async fn publish_storage_initialized(&self, storage_driver: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupStorageInitialized,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-storage-initialized".to_string(),
            progress: Some(70),
            payload: serde_json::json!({
                "storage_driver": storage_driver
            }),
        }).await;
    }

    /// Publish startup.ontology_library_loaded event
    pub async fn publish_ontology_library_loaded(&self, triples_count: usize) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupOntologyLibraryLoaded,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-ontology-library-loaded".to_string(),
            progress: Some(75),
            payload: serde_json::json!({
                "triples_count": triples_count
            }),
        }).await;
    }

    /// Publish startup.jena_connected event
    pub async fn publish_jena_connected(&self, jena_url: &str, dataset: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupJenaConnected,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-jena-connected".to_string(),
            progress: Some(80),
            payload: serde_json::json!({
                "jena_url": jena_url,
                "dataset": dataset
            }),
        }).await;
    }

    /// Publish startup.nats_connected event
    pub async fn publish_nats_connected(&self, nats_url: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupNatsConnected,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-nats-connected".to_string(),
            progress: Some(10),
            payload: serde_json::json!({
                "nats_url": nats_url,
                "discovery_enabled": true
            }),
        }).await;
    }

    /// Publish startup.ontology_loaded event
    pub async fn publish_ontology_loaded(&self, ontology_exists: bool) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupOntologyLoaded,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-ontology-loaded".to_string(),
            progress: Some(85),
            payload: serde_json::json!({
                "ontology_loaded": ontology_exists
            }),
        }).await;
    }

    /// Publish startup.toolless_mode event (v1.3.20)
    /// Indicates kernel is running in toolless passthrough mode
    pub async fn publish_startup_toolless_mode(&self) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupToollessMode,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-toolless-mode".to_string(),
            progress: Some(90),
            payload: serde_json::json!({
                "toolless_mode": true,
                "expected_performance": "10-20ms per job",
                "flow": "input → validation → instance → output"
            }),
        }).await;
    }

    /// Publish startup.ready event
    pub async fn publish_startup_ready(&self) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupReady,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::ProcessBoundary,
            phase: "startup-ready".to_string(),
            progress: Some(100),
            payload: serde_json::json!({
                "status": "ready"
            }),
        }).await;
    }

    /// Publish startup.failed event with error details
    pub async fn publish_startup_failed(&self, error: &str, failed_check: &str) {
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupFailed,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: self.startup_process_urn.clone(),
            occurrent_type: OccurrentType::ProcessBoundary,
            phase: failed_check.to_string(),
            progress: None,
            payload: serde_json::json!({
                "status": "failed",
                "error": error,
                "failed_check": failed_check
            }),
        }).await;
    }

    /// Publish shutdown.initiated event
    pub async fn publish_shutdown_initiated(&self, reason: &str) {
        let shutdown_urn = Self::generate_process_urn("GovernorShutdown", &self.kernel_name);
        self.publish(KernelEvent {
            event_type: KernelEventType::ShutdownInitiated,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: shutdown_urn.clone(),
            occurrent_type: OccurrentType::ProcessBoundary,
            phase: "shutdown-initiated".to_string(),
            progress: Some(0),
            payload: serde_json::json!({
                "reason": reason,
                "processUrn": shutdown_urn
            }),
        }).await;
    }

    /// Publish shutdown.complete event
    pub async fn publish_shutdown_complete(&self) {
        let shutdown_urn = Self::generate_process_urn("GovernorShutdown", &self.kernel_name);
        self.publish(KernelEvent {
            event_type: KernelEventType::ShutdownComplete,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: shutdown_urn.clone(),
            occurrent_type: OccurrentType::ProcessBoundary,
            phase: "shutdown-complete".to_string(),
            progress: Some(100),
            payload: serde_json::json!({
                "status": "shutdown",
                "processUrn": shutdown_urn
            }),
        }).await;
    }

    /// Publish kernel announcement for discovery
    /// This allows kernels to announce themselves without a centralized registry
    pub async fn publish_kernel_announcement(&self, kernel_urn: &str, version: &str, runtime: &str, port: Option<u16>) {
        // Publish on kernel.{name}.lifecycle.announce subject
        // UI subscribes to kernel.> to receive all announcements
        if let Some(nats) = &self.nats_client {
            let subject = format!("kernel.{}.lifecycle.announce", self.kernel_name);

            let payload = serde_json::json!({
                "urn": kernel_urn,
                "name": self.kernel_name,
                "version": version,
                "type": runtime,
                "status": "ONLINE",
                "port": port,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });

            match serde_json::to_vec(&payload) {
                Ok(payload_bytes) => {
                    if let Err(e) = nats.publish(subject.clone(), payload_bytes.into()).await {
                        eprintln!("[EventPublisher] Failed to publish kernel announcement to {}: {}", subject, e);
                    } else {
                        eprintln!("[EventPublisher] Kernel announced: {} on {}", self.kernel_name, subject);
                    }
                }
                Err(e) => eprintln!("[EventPublisher] Failed to serialize announcement: {}", e),
            }
        }
    }

    /// Publish job.received event
    pub async fn publish_job_received(&self, tx_id: &str, source: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobReceived,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::ProcessBoundary,
            phase: "job-received".to_string(),
            progress: Some(0),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "source": source,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job.validation.passed event
    pub async fn publish_job_validated(&self, tx_id: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobValidationPassed,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-validation-passed".to_string(),
            progress: Some(20),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "all_checks_passed": true,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job.processing event
    pub async fn publish_job_processing(&self, tx_id: Option<&str>, tool_pid: u32) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id.unwrap_or("unknown"));
        self.publish(KernelEvent {
            event_type: KernelEventType::JobProcessing,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-processing".to_string(),
            progress: Some(75),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "tool_pid": tool_pid,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job.completed event
    pub async fn publish_job_completed(&self, tx_id: Option<&str>, exit_code: i32) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id.unwrap_or("unknown"));
        self.publish(KernelEvent {
            event_type: KernelEventType::JobCompleted,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::ProcessBoundary,
            phase: "job-completed".to_string(),
            progress: Some(100),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "exit_code": exit_code,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job.rejected event
    pub async fn publish_job_rejected(&self, tx_id: Option<&str>, violations: Vec<String>) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id.unwrap_or("unknown"));
        self.publish(KernelEvent {
            event_type: KernelEventType::JobRejected,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::ProcessBoundary,
            phase: "job-rejected".to_string(),
            progress: Some(100),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "violations": violations,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job.validation.started event
    pub async fn publish_job_validation_started(&self, tx_id: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobValidationStarted,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::ProcessBoundary,
            phase: "job-validation-started".to_string(),
            progress: Some(0),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job.tool.spawning event
    pub async fn publish_job_tool_spawning(&self, tx_id: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobToolSpawning,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-tool-spawning".to_string(),
            progress: Some(25),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job.tool.started event
    pub async fn publish_job_tool_started(&self, tx_id: &str, tool_pid: u32) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobToolStarted,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-tool-started".to_string(),
            progress: Some(50),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "tool_pid": tool_pid,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job.tool.completed event
    pub async fn publish_job_tool_completed(&self, tx_id: &str, exit_code: i32) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobToolCompleted,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::ProcessBoundary,
            phase: "job-tool-completed".to_string(),
            progress: Some(100),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "exit_code": exit_code,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job queued event (JB02)
    pub async fn publish_job_queued(&self, tx_id: &str, queue_name: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobQueued,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-queued".to_string(),
            progress: Some(11),  // 2/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "queue": queue_name,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job ontology validated event (JB04)
    pub async fn publish_job_ontology_validated(&self, tx_id: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobOntologyValidated,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-ontology-validated".to_string(),
            progress: Some(22),  // 4/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job SHACL validated event (JB05)
    pub async fn publish_job_shacl_validated(&self, tx_id: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobShaclValidated,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-shacl-validated".to_string(),
            progress: Some(28),  // 5/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job edges validated event (JB06)
    pub async fn publish_job_edges_validated(&self, tx_id: &str, edge_count: usize) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobEdgesValidated,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-edges-validated".to_string(),
            progress: Some(33),  // 6/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "edge_count": edge_count,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job validation passed event (JB07)
    pub async fn publish_job_validation_passed(&self, tx_id: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobValidationPassed,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-validation-passed".to_string(),
            progress: Some(39),  // 7/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job proof minting event (JB12)
    pub async fn publish_job_proof_minting(&self, tx_id: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobProofMinting,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-proof-minting".to_string(),
            progress: Some(67),  // 12/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job proof minted event (JB13)
    pub async fn publish_job_proof_minted(&self, tx_id: &str, proof_hash: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobProofMinted,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-proof-minted".to_string(),
            progress: Some(72),  // 13/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "proof_hash": proof_hash,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job saving to Jena event (JB14)
    pub async fn publish_job_saving_to_jena(&self, tx_id: &str, instance_urn: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobSavingToJena,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-saving-to-jena".to_string(),
            progress: Some(78),  // 14/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "instance_urn": instance_urn,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job saved to Jena event (JB15)
    pub async fn publish_job_saved_to_jena(&self, tx_id: &str, instance_urn: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobSavedToJena,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-saved-to-jena".to_string(),
            progress: Some(83),  // 15/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "instance_urn": instance_urn,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job edge confirmation event (JB16)
    pub async fn publish_job_edge_confirmation(&self, tx_id: &str, edge_count: usize) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobEdgeConfirmation,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-edge-confirmation".to_string(),
            progress: Some(89),  // 16/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "edge_count": edge_count,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job edges confirmed event (JB17)
    pub async fn publish_job_edges_confirmed(&self, tx_id: &str, confirmed_count: usize) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobEdgesConfirmed,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-edges-confirmed".to_string(),
            progress: Some(94),  // 17/18 * 100
            payload: serde_json::json!({
                "tx_id": tx_id,
                "confirmed_count": confirmed_count,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job toolless passthrough event (v1.3.20)
    pub async fn publish_job_toolless_passthrough(&self, tx_id: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobToollessPassthrough,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-toolless-passthrough".to_string(),
            progress: Some(10),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "mode": "toolless",
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish instance created event (v1.3.20)
    pub async fn publish_instance_created(&self, tx_id: &str, instance_id: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::InstanceCreated,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "instance-created".to_string(),
            progress: Some(40),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "instance_id": instance_id,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish edges discovered event (v1.3.20)
    pub async fn publish_edges_discovered(&self, tx_id: &str, edge_count: usize) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::EdgesDiscovered,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "edges-discovered".to_string(),
            progress: Some(60),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "edge_count": edge_count,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish output routed event (v1.3.20)
    pub async fn publish_output_routed(&self, tx_id: &str, destination: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::OutputRouted,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "output-routed".to_string(),
            progress: Some(80),
            payload: serde_json::json!({
                "tx_id": tx_id,
                "destination": destination,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Publish job moved to error queue event (v1.3.20)
    pub async fn publish_job_moved_to_error_queue(&self, tx_id: &str, reason: &str) {
        let job_urn = Self::generate_process_urn("JobInvocation", tx_id);
        self.publish(KernelEvent {
            event_type: KernelEventType::JobMovedToErrorQueue,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: job_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "job-error-queued".to_string(),
            progress: None,
            payload: serde_json::json!({
                "tx_id": tx_id,
                "reason": reason,
                "processUrn": job_urn
            }),
        }).await;
    }

    /// Get NATS client reference for direct publishing (v1.3.20)
    pub fn get_nats_client(&self) -> Option<&NatsClient> {
        self.nats_client.as_ref()
    }

    /// Publish basic queries validated event (SU13)
    pub async fn publish_basic_queries_validated(&self, queries_tested: usize) {
        let startup_urn = &self.startup_process_urn;
        self.publish(KernelEvent {
            event_type: KernelEventType::StartupBasicQueriesValidated,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: startup_urn.clone(),
            occurrent_type: OccurrentType::TemporalPart,
            phase: "startup-basic-queries-validated".to_string(),
            progress: Some(81),  // 13/16 * 100
            payload: serde_json::json!({
                "queries_tested": queries_tested,
                "processUrn": startup_urn
            }),
        }).await;
    }

    /// Publish workflow phase event
    pub async fn publish_phase_event(
        &self,
        event_type: KernelEventType,
        workflow_urn: &str,
        phase_name: &str,
        kernel_urn: &str,
        progress: u8,
    ) {
        let occurrent_type = if progress == 0 || progress == 100 {
            OccurrentType::ProcessBoundary
        } else {
            OccurrentType::TemporalPart
        };

        self.publish(KernelEvent {
            event_type,
            kernel_name: self.kernel_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: workflow_urn.to_string(),
            occurrent_type,
            phase: format!("workflow-{}", phase_name),
            progress: Some(progress),
            payload: serde_json::json!({
                "workflow_urn": workflow_urn,
                "phase_name": phase_name,
                "kernel_urn": kernel_urn
            }),
        }).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_publisher_without_nats() {
        let publisher = KernelEventPublisher::new("TestKernel".to_string(), None)
            .await
            .unwrap();

        // Should not panic when NATS not configured
        publisher.publish_startup_initiated(12345).await;
        publisher.publish_startup_ready().await;
    }

    #[test]
    fn test_event_type_subject_suffix() {
        assert_eq!(
            KernelEventType::StartupInitiated.subject_suffix(),
            "lifecycle.startup.initiated"
        );
        assert_eq!(
            KernelEventType::JobReceived.subject_suffix(),
            "job.received"
        );
    }
}
