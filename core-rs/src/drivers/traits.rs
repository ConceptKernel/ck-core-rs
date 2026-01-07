//! Storage and Transport driver traits for ConceptKernel v1.3.20
//!
//! Defines the abstract interfaces for all storage and transport backends.
//!
//! ## Storage Drivers
//! - FileSystemDriver (local filesystem)
//! - JenaStorage (RDF triple store)
//! - AgeStorage (graph database)
//! - SeaweedFSStorage (distributed object store)
//!
//! ## Transport Drivers
//! - LocalTransport (filesystem + notify)
//! - NatsTransport (NATS JetStream)
//! - WebSocketTransport (browser communication)
//! - GrpcTransport (agent-to-agent)

use crate::errors::{CkpError, Result};
use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;

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
#[derive(Debug, Clone)]
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

// ============================================================================
// NEW v1.3.20 TYPES
// ============================================================================

/// Execution mode for tool execution
///
/// Determines how a kernel's tool is executed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Local binary or script execution (default for v1.3.19 compatibility)
    Binary,

    /// Kubernetes Job execution
    Job,

    /// HTTP/gRPC service call
    Service,

    /// External API wrapper (Anthropic, OpenAI, etc.)
    ApiWrapper,
}

impl ExecutionMode {
    /// Parse execution mode from kernel_type in ontology
    ///
    /// # Examples
    ///
    /// - "node:cold" → Binary
    /// - "python:cold" → Binary
    /// - "rust:cold" → Binary
    /// - "k8s:job" → Job
    /// - "http:service" → Service
    /// - "api:anthropic" → ApiWrapper
    pub fn from_kernel_type(kernel_type: &str) -> Result<Self> {
        use crate::errors::CkpError;

        match kernel_type {
            t if t.starts_with("node:") || t.starts_with("python:") || t.starts_with("rust:") => {
                Ok(ExecutionMode::Binary)
            }
            t if t.starts_with("k8s:") || t.starts_with("job:") => Ok(ExecutionMode::Job),
            t if t.starts_with("http:") || t.starts_with("grpc:") => Ok(ExecutionMode::Service),
            t if t.starts_with("api:") => Ok(ExecutionMode::ApiWrapper),
            _ => Err(CkpError::Config(format!(
                "Unknown execution mode for kernel_type: {}",
                kernel_type
            ))),
        }
    }
}

/// Resource requirements for execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// CPU request (e.g., "500m")
    pub cpu: Option<String>,

    /// Memory request (e.g., "512Mi")
    pub memory: Option<String>,

    /// GPU count
    pub gpu: Option<u32>,
}

/// Tool definition from ontology
///
/// Configuration for how to execute a kernel's tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,

    /// Execution mode
    pub execution_mode: ExecutionMode,

    /// Container image (for Job/Service modes)
    pub container_image: Option<String>,

    /// Command to execute
    pub command: Vec<String>,

    /// Command arguments
    pub args: Vec<String>,

    /// Environment variables
    pub env_vars: HashMap<String, String>,

    /// Resource requirements
    pub resources: Option<ResourceRequirements>,

    /// Timeout in seconds
    pub timeout_seconds: u64,
}

/// Job message for transport
///
/// Standard format for jobs sent through transport layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMessage {
    /// Job ID
    pub job_id: String,

    /// Tool name
    pub tool: String,

    /// Job arguments
    pub args: JsonValue,

    /// ISO 8601 timestamp
    pub timestamp: String,

    /// Source kernel or client
    pub source: String,
}

/// Tool execution response
///
/// Result of tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    /// Job ID this response is for
    pub job_id: String,

    /// Status: "success", "failed", "timeout", etc.
    pub status: String,

    /// Output data (structure depends on tool)
    pub output: JsonValue,

    /// ISO 8601 timestamp
    pub timestamp: String,

    /// Execution duration in milliseconds
    pub duration_ms: u64,

    /// Error message (if status != "success")
    pub error: Option<String>,
}

/// Edge message for edge routing
///
/// Represents a message routed through an edge connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeMessage {
    /// Source kernel name
    pub source_kernel: String,

    /// Target kernel name
    pub target_kernel: String,

    /// Edge predicate (e.g., "PRODUCES", "CONSUMES")
    pub predicate: String,

    /// Instance URN being routed
    pub instance_urn: String,

    /// ISO 8601 timestamp
    pub timestamp: String,

    /// Optional metadata
    pub metadata: Option<JsonValue>,
}

/// Edge metadata for RDF storage
///
/// Represents edge routing metadata stored in RDF triple stores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeMetadata {
    /// API version (e.g., "conceptkernel/v1")
    pub api_version: String,

    /// Kind (always "Edge")
    pub kind: String,

    /// Edge URN (e.g., "ckp://Edge#Connection-Source-to-Target-PRODUCES:v1.3.20")
    pub urn: String,

    /// ISO 8601 timestamp when edge was created
    pub created_at: String,

    /// Edge predicate (e.g., "PRODUCES", "REQUIRES")
    pub predicate: String,

    /// Source kernel name
    pub source: String,

    /// Target kernel name
    pub target: String,

    /// Version (e.g., "v1.3.20")
    pub version: String,
}

/// Storage events for event-driven operations
///
/// Events emitted by storage drivers for monitoring and reactivity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageEvent {
    /// Instance created event
    InstanceCreated {
        /// Kernel name
        kernel_name: String,

        /// Instance URN
        instance_urn: String,
    },

    /// Job archived event
    JobArchived {
        /// Kernel name
        kernel_name: String,

        /// Job ID
        job_id: String,
    },

    /// Transaction recorded event
    TransactionRecorded {
        /// Kernel name
        kernel_name: String,

        /// Transaction ID
        tx_id: String,
    },
}

/// Stream type aliases for transport
pub type JobStream = Pin<Box<dyn Stream<Item = JobMessage> + Send>>;
pub type ResultStream = Pin<Box<dyn Stream<Item = ToolResponse> + Send>>;
pub type EdgeStream = Pin<Box<dyn Stream<Item = EdgeMessage> + Send>>;
pub type StorageEventStream = Pin<Box<dyn Stream<Item = StorageEvent> + Send>>;

/// Storage driver trait (v1.3.20 - async)
///
/// All storage backends must implement this interface.
/// The driver abstracts physical storage details from the protocol layer.
///
/// # Version History
///
/// - v1.3.19: 8 synchronous methods
/// - v1.3.20: 13 async methods (8 existing + 5 new)
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
/// use async_trait::async_trait;
///
/// pub struct MyDriver { ... }
///
/// #[async_trait]
/// impl StorageDriver for MyDriver {
///     async fn write_job(&self, target_urn: &str, job: JobFile) -> Result<String> {
///         // Parse URN
///         // Resolve to storage location
///         // Write job atomically
///         // Return transaction ID
///     }
///
///     // ... other methods
/// }
/// ```
#[async_trait]
pub trait StorageDriver: Send + Sync + std::fmt::Debug {
    // ========================================================================
    // EXISTING METHODS (v1.3.19 - now async)
    // ========================================================================

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
    async fn write_job(&self, target_urn: &str, job: JobFile) -> Result<String>;

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
    async fn read_jobs(&self, kernel_name: &str) -> Result<Vec<JobHandle>>;

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
    async fn archive_job(&self, kernel_name: &str, job: &JobHandle) -> Result<()>;

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
    async fn mint_storage_artifact(
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
    async fn record_transaction(&self, kernel_name: &str, transaction: JsonValue) -> Result<()>;

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
    async fn resolve_urn(&self, urn: &str) -> Result<StorageLocation>;

    /// Check if kernel exists
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel to check
    ///
    /// # Returns
    ///
    /// true if kernel has storage structure, false otherwise
    async fn kernel_exists(&self, kernel_name: &str) -> Result<bool>;

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
    async fn get_edge_queue(&self, kernel_name: &str, source_kernel: &str) -> Result<StorageLocation>;

    // ========================================================================
    // NEW METHODS (v1.3.20)
    // ========================================================================

    /// Load kernel ontology (RDF or YAML)
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name
    ///
    /// # Returns
    ///
    /// Ontology content as string (Turtle RDF or YAML)
    ///
    /// # Implementation Notes
    ///
    /// - Try RDF ontology first (ontology.ttl)
    /// - Fallback to YAML ontology (conceptkernel.yaml)
    /// - Return error if neither exists
    async fn load_ontology(&self, kernel_name: &str) -> Result<String>;

    /// Save kernel ontology (RDF format)
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name
    /// * `ttl` - Turtle RDF content
    ///
    /// # Implementation Notes
    ///
    /// - Write to ontology.ttl
    /// - Atomic write if possible
    async fn save_ontology(&self, kernel_name: &str, ttl: &str) -> Result<()>;

    /// Load tool definition from ontology
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name
    /// * `tool_name` - Tool name
    ///
    /// # Returns
    ///
    /// Parsed tool definition with execution configuration
    ///
    /// # Implementation Notes
    ///
    /// - Parse metadata.type to determine ExecutionMode
    /// - Parse spec.execution for tool configuration
    /// - Return ToolDefinition with all parameters
    async fn load_tool_definition(&self, kernel_name: &str, tool_name: &str) -> Result<ToolDefinition>;

    /// Save tool execution result
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name
    /// * `job_id` - Job ID
    /// * `result` - Tool execution result
    ///
    /// # Implementation Notes
    ///
    /// - Write to queue/results/{job_id}.result
    /// - JSON format
    /// - Used for result retrieval and audit
    async fn save_result(&self, kernel_name: &str, job_id: &str, result: &ToolResponse) -> Result<()>;

    /// Load tool execution result
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name
    /// * `job_id` - Job ID
    ///
    /// # Returns
    ///
    /// Tool execution result
    ///
    /// # Errors
    ///
    /// - Returns NotFound error if result doesn't exist
    async fn load_result(&self, kernel_name: &str, job_id: &str) -> Result<ToolResponse>;

    /// List edge queues for a kernel
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name
    ///
    /// # Returns
    ///
    /// Vector of tuples: (source_kernel, pending_count)
    ///
    /// # Implementation Notes
    ///
    /// - Returns all edge queues targeting this kernel
    /// - Includes count of pending instances in each queue
    /// - Used for monitoring and edge routing
    async fn list_edge_queues(&self, kernel_name: &str) -> Result<Vec<(String, usize)>>;

    /// List instances for a kernel
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name
    ///
    /// # Returns
    ///
    /// Vector of instance URNs
    ///
    /// # Implementation Notes
    ///
    /// - Returns all instances in storage
    /// - Used for instance discovery and querying
    async fn list_instances(&self, kernel_name: &str) -> Result<Vec<String>>;

    /// Subscribe to storage events
    ///
    /// # Returns
    ///
    /// Event stream for monitoring storage operations
    ///
    /// # Implementation Notes
    ///
    /// - For event-driven storage backends (e.g., Jena with polling)
    /// - LocalStorage: uses notify crate for filesystem events
    /// - JenaStorage: polls for new instances
    /// - Stream continues until driver is dropped
    async fn subscribe_storage_events(&self) -> Result<StorageEventStream>;

    /// Get root path (for drivers that use filesystem)
    ///
    /// # Returns
    ///
    /// Root path if driver uses filesystem, error otherwise
    ///
    /// # Implementation Notes
    ///
    /// - LocalStorage: returns root path
    /// - JenaStorage: returns error (no filesystem)
    /// - Used for compatibility with existing code
    fn root_path(&self) -> Result<PathBuf> {
        Err(CkpError::NotImplemented(
            "root_path not supported by this storage driver".to_string(),
        ))
    }

    /// Execute SPARQL UPDATE query (for RDF-based storage backends)
    ///
    /// # Arguments
    ///
    /// * `sparql_update` - SPARQL UPDATE query (INSERT DATA, DELETE DATA, etc.)
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    ///
    /// # Implementation Notes
    ///
    /// - JenaStorage: Executes SPARQL UPDATE via Fuseki HTTP endpoint
    /// - LocalStorage: Returns NotImplemented error (no RDF support)
    /// - Used by OccurrentTracker to persist workflow lifecycle events
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let sparql = r#"
    /// PREFIX ckp: <urn:ckp:>
    /// INSERT DATA {
    ///     <urn:ckp:occurrent:test> a ckp:WorkflowStart .
    /// }
    /// "#;
    /// storage.execute_sparql_update(sparql).await?;
    /// ```
    async fn execute_sparql_update(&self, _sparql_update: &str) -> Result<()> {
        Err(CkpError::NotImplemented(
            "SPARQL UPDATE not supported by this storage driver".to_string(),
        ))
    }

    /// Downcast to concrete type (for trait object downcasting)
    ///
    /// # Returns
    ///
    /// Reference to Any for downcasting
    ///
    /// # Implementation Notes
    ///
    /// Enables downcasting from `Arc<dyn StorageDriver>` to concrete types
    fn as_any(&self) -> &dyn std::any::Any;
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

// ============================================================================
// TRANSPORT DRIVER TRAIT (v1.3.20 - NEW)
// ============================================================================

/// Transport driver trait (v1.3.20 - NEW)
///
/// Handles job and result message transport between kernels and clients.
///
/// # Implementations
///
/// - LocalTransport: Filesystem + notify crate (v1.3.19 compatibility)
/// - NatsTransport: NATS JetStream (small footprint)
/// - WebSocketTransport: Browser communication (A2UI)
/// - GrpcTransport: Agent-to-agent communication (A2A)
///
/// # Design Principles
///
/// - Event-driven: subscribe_inbox returns async stream
/// - Non-blocking: all methods are async
/// - Reliable: delivery guarantees depend on implementation
/// - Stateless: drivers can be created/destroyed without data loss
///
/// # Example Implementation
///
/// ```rust,ignore
/// use async_trait::async_trait;
///
/// pub struct MyTransport { ... }
///
/// #[async_trait]
/// impl TransportDriver for MyTransport {
///     async fn subscribe_inbox(&self, kernel_name: &str) -> Result<JobStream> {
///         // Create async stream of incoming jobs
///         // Return stream that yields JobMessage items
///     }
///
///     // ... other methods
/// }
/// ```
#[async_trait]
pub trait TransportDriver: Send + Sync + std::fmt::Debug {
    /// Subscribe to kernel's inbox for incoming jobs
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel to subscribe to
    ///
    /// # Returns
    ///
    /// Async stream of JobMessage items
    ///
    /// # Implementation Notes
    ///
    /// - Stream should be infinite (doesn't end until shutdown)
    /// - Stream yields jobs as they arrive (event-driven)
    /// - Error items in stream indicate transport errors (not job errors)
    /// - LocalTransport: watches filesystem with notify crate
    /// - NatsTransport: subscribes to NATS subject ckp.{kernel}.inbox
    /// - WebSocket: receives messages from browser clients
    /// - gRPC: receives RPC calls from other agents
    async fn subscribe_inbox(&self, kernel_name: &str) -> Result<JobStream>;

    /// Publish job to target kernel's inbox
    ///
    /// # Arguments
    ///
    /// * `target` - Target kernel name
    /// * `job` - Job message to send
    ///
    /// # Implementation Notes
    ///
    /// - Should be non-blocking (async)
    /// - LocalTransport: writes .job file to target's inbox
    /// - NatsTransport: publishes to ckp.{target}.inbox subject
    /// - WebSocket: sends JSON message to browser
    /// - gRPC: makes RPC call to target agent
    async fn publish_job(&self, target: &str, job: JobMessage) -> Result<()>;

    /// Publish tool execution result
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel that executed the job
    /// * `response` - Tool execution response
    ///
    /// # Implementation Notes
    ///
    /// - LocalTransport: writes to queue/results/{job_id}.result
    /// - NatsTransport: publishes to ckp.{kernel}.results subject
    /// - WebSocket: sends to waiting browser client
    /// - gRPC: responds to RPC call
    async fn publish_response(&self, kernel_name: &str, response: ToolResponse) -> Result<()>;

    /// Subscribe to kernel's results stream
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel to subscribe to
    ///
    /// # Returns
    ///
    /// Async stream of ToolResponse items
    ///
    /// # Use Cases
    ///
    /// - Client waiting for job results
    /// - Monitoring tool execution
    /// - Result aggregation
    async fn subscribe_results(&self, kernel_name: &str) -> Result<ResultStream>;

    /// Health check for transport connection
    ///
    /// # Returns
    ///
    /// true if transport is healthy, false otherwise
    ///
    /// # Implementation Notes
    ///
    /// - LocalTransport: checks if inbox directory exists
    /// - NatsTransport: checks NATS connection
    /// - WebSocket: checks WebSocket server status
    /// - gRPC: pings gRPC server
    async fn health_check(&self) -> Result<bool>;

    /// Shutdown transport and clean up resources
    ///
    /// # Implementation Notes
    ///
    /// - Close any open connections
    /// - Stop background tasks
    /// - Flush pending messages if possible
    /// - Should be idempotent (safe to call multiple times)
    async fn shutdown(&self) -> Result<()>;

    // ========================================================================
    // EDGE ROUTING SUPPORT (v1.3.20 - REQUIRED for spec compliance)
    // ========================================================================

    /// Subscribe to edge stream for source → target routing
    ///
    /// # Arguments
    ///
    /// * `source` - Source kernel name
    /// * `target` - Target kernel name
    ///
    /// # Returns
    ///
    /// Async stream of EdgeMessage items
    ///
    /// # Implementation Notes
    ///
    /// - NatsTransport: subscribes to ckp.{target}.edges.{source}
    /// - LocalTransport: watches queue/edges/{source} directory
    async fn subscribe_edge(&self, source: &str, target: &str) -> Result<EdgeStream>;

    /// Notify edge with new instance
    ///
    /// # Arguments
    ///
    /// * `edge` - Edge message to send
    ///
    /// # Implementation Notes
    ///
    /// - NatsTransport: publishes to ckp.{target}.edges.{source}
    /// - LocalTransport: writes to queue/edges/{source}/
    async fn notify_edge(&self, edge: &EdgeMessage) -> Result<()>;

    /// Subscribe to edge queue for routing messages between kernels (LEGACY)
    ///
    /// # Arguments
    ///
    /// * `target` - Target kernel name
    /// * `predicate` - Edge predicate (e.g., "PRODUCES")
    /// * `source` - Source kernel name
    ///
    /// # Returns
    ///
    /// Async stream of JobMessage items from the edge queue
    ///
    /// # Implementation Notes
    ///
    /// - NatsTransport: subscribes to {target}/edges/{predicate}/{source}
    /// - LocalTransport: watches queue/edges/{predicate}.{source} directory
    /// - Default: Returns NotImplemented error
    ///
    /// # Optional
    ///
    /// Transports that don't support edge routing can use default implementation
    async fn subscribe_edge_queue(
        &self,
        _target: &str,
        _predicate: &str,
        _source: &str,
    ) -> Result<JobStream> {
        Err(CkpError::NotImplemented(
            "Edge queue subscription not supported by this transport".to_string(),
        ))
    }

    /// Publish job to edge queue
    ///
    /// # Arguments
    ///
    /// * `target` - Target kernel name
    /// * `predicate` - Edge predicate
    /// * `source` - Source kernel name
    /// * `job` - Job message to route
    ///
    /// # Implementation Notes
    ///
    /// - NatsTransport: publishes to {target}/edges/{predicate}/{source}
    /// - LocalTransport: writes to queue/edges/{predicate}.{source}/{job_id}.job
    /// - Default: Returns NotImplemented error
    ///
    /// # Optional
    ///
    /// Transports that don't support edge routing can use default implementation
    async fn publish_to_edge(
        &self,
        _target: &str,
        _predicate: &str,
        _source: &str,
        _job: JobMessage,
    ) -> Result<()> {
        Err(CkpError::NotImplemented(
            "Edge queue publishing not supported by this transport".to_string(),
        ))
    }

    /// List all edge streams for a target kernel
    ///
    /// # Arguments
    ///
    /// * `target_kernel` - Target kernel name
    ///
    /// # Returns
    ///
    /// Vector of edge identifiers in format: `{predicate}.{source}`
    ///
    /// # Implementation Notes
    ///
    /// - NatsTransport: queries JetStream for edge subjects
    /// - LocalTransport: lists queue/edges/ directory
    /// - Default: Returns NotImplemented error
    ///
    /// # Optional
    ///
    /// Transports that don't support edge routing can use default implementation
    async fn list_edge_streams(&self, _target_kernel: &str) -> Result<Vec<String>> {
        Err(CkpError::NotImplemented(
            "Edge stream listing not supported by this transport".to_string(),
        ))
    }
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
