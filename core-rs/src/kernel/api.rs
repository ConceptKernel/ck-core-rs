//! High-level API for Concept Kernel tool implementations
//!
//! This module provides a convenient, protocol-compliant API for kernel tools to:
//! - Read jobs from inbox and edge queues
//! - Mint evidence to storage
//! - Archive processed jobs
//! - Send edge messages to other kernels
//! - Adopt context from source kernels
//!
//! ## Usage Example
//!
//! ```rust,no_run
//! use ckp_core::kernel::api::{KernelContext, Job};
//!
//! fn main() -> anyhow::Result<()> {
//!     // Initialize kernel context
//!     let ctx = KernelContext::init("ConceptKernel.LLM.Claude")?;
//!
//!     // Read jobs from inbox
//!     let jobs = ctx.read_jobs(None)?;
//!
//!     for job in jobs {
//!         // Process job...
//!
//!         // Mint evidence
//!         let evidence = create_evidence(&job);
//!         let tx_id = generate_tx_id();
//!         ctx.mint_evidence(&evidence, &tx_id)?;
//!
//!         // Archive job
//!         ctx.archive_job(&job)?;
//!     }
//!
//!     Ok(())
//! }
//! ```

use crate::errors::{CkpError, Result};
use serde::{Serialize, Deserialize};
use std::path::{Path, PathBuf};
use std::fs;
use std::env;
use chrono::Utc;

/// High-level API for kernel tool implementations
pub struct KernelContext {
    kernel_name: String,
    project_root: PathBuf,
    kernel_root: PathBuf,
}

impl KernelContext {
    /// Initialize kernel context (call this first in tool main())
    ///
    /// This will detect the project root and kernel root based on the current
    /// working directory or environment variables.
    pub fn init(kernel_name: &str) -> Result<Self> {
        let project_root = detect_project_root()?;
        let kernel_root = project_root.join("concepts").join(kernel_name);

        if !kernel_root.exists() {
            return Err(CkpError::KernelNotFound(kernel_name.to_string()));
        }

        Ok(Self {
            kernel_name: kernel_name.to_string(),
            project_root,
            kernel_root,
        })
    }

    /// Get the kernel name
    pub fn kernel_name(&self) -> &str {
        &self.kernel_name
    }

    /// Get the project root directory
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// Get the kernel root directory
    pub fn kernel_root(&self) -> &Path {
        &self.kernel_root
    }

    /// Read jobs from inbox or edge queue
    ///
    /// If `source_queue` is None, reads from `queue/inbox/`
    /// If `source_queue` is Some("edges/KernelName"), reads from that edge queue
    ///
    /// Returns a vector of Job structs with the file path and parsed payload.
    pub fn read_jobs(&self, source_queue: Option<&str>) -> Result<Vec<Job>> {
        let queue_dir = if let Some(queue) = source_queue {
            self.kernel_root.join("queue").join(queue)
        } else {
            self.kernel_root.join("queue").join("inbox")
        };

        if !queue_dir.exists() {
            return Ok(Vec::new());
        }

        scan_jobs(&queue_dir)
    }

    /// Mint evidence to storage in BFO-compliant format
    ///
    /// Creates a directory: `storage/{tx_id}-{suffix}.inst/payload.json`
    /// The suffix is extracted from the tx_id or defaults to "analysis"
    pub fn mint_evidence<T: Serialize>(&self, evidence: &T, tx_id: &str) -> Result<PathBuf> {
        let storage_dir = self.kernel_root.join("storage");
        write_evidence_bfo_compliant(&storage_dir, evidence, tx_id)
    }

    /// Archive processed job to queue/archive/
    ///
    /// Moves the job file from inbox or edge queue to the archive directory.
    pub fn archive_job(&self, job: &Job) -> Result<()> {
        let archive_dir = self.kernel_root.join("queue").join("archive");
        archive_job_file(&job.path, &archive_dir)
    }

    /// Move failed job to queue/failed/
    ///
    /// Moves the job file from inbox or edge queue to the failed directory.
    pub fn move_to_failed(&self, job: &Job) -> Result<()> {
        let failed_dir = self.kernel_root.join("queue").join("failed");
        move_job_file(&job.path, &failed_dir)
    }

    /// Send edge message to another kernel
    ///
    /// Writes a message to the target kernel's edge queue at:
    /// `{target_kernel}/queue/edges/{this_kernel}/message-{timestamp}.json`
    pub fn send_edge_message<T: Serialize>(
        &self,
        target_kernel: &str,
        message: &T
    ) -> Result<()> {
        let edge_dir = self.project_root
            .join("concepts")
            .join(target_kernel)
            .join("queue")
            .join("edges")
            .join(&self.kernel_name);

        send_edge_message_protocol_compliant(&edge_dir, message)
    }

    /// Read edge responses from a source kernel
    ///
    /// Reads response files from:
    /// `queue/edges/{source_kernel}/response-*.json`
    pub fn read_edge_responses(&self, source_kernel: &str) -> Result<Vec<EdgeResponse>> {
        let response_dir = self.kernel_root
            .join("queue")
            .join("edges")
            .join(source_kernel);

        read_edge_responses_protocol_compliant(&response_dir)
    }

    /// Adopt context from another kernel (for edge jobs)
    ///
    /// Returns an AdoptedContext with:
    /// - kernel_name: The source kernel name
    /// - working_directory: {source_kernel}/tool/
    /// - llm_instructions: Concatenated *.md files from {source_kernel}/llm/
    pub fn adopt_context(&self, source_kernel: &str) -> Result<AdoptedContext> {
        let source_dir = self.project_root
            .join("concepts")
            .join(source_kernel);

        if !source_dir.exists() {
            return Err(CkpError::KernelNotFound(source_kernel.to_string()));
        }

        AdoptedContext::new(source_dir, source_kernel)
    }
}

/// Adopted context from another kernel
#[derive(Debug, Clone)]
pub struct AdoptedContext {
    pub kernel_name: String,
    pub working_directory: PathBuf,
    pub llm_instructions: String,
}

impl AdoptedContext {
    fn new(kernel_dir: PathBuf, kernel_name: &str) -> Result<Self> {
        let working_directory = kernel_dir.join("tool");
        let llm_instructions = load_llm_instructions(&kernel_dir)?;

        Ok(Self {
            kernel_name: kernel_name.to_string(),
            working_directory,
            llm_instructions,
        })
    }
}

/// Job representation with file path and parsed payload
#[derive(Debug, Clone)]
pub struct Job {
    /// Original file path
    pub path: PathBuf,

    /// Parsed job payload
    pub task: String,
    pub mode: String,
    pub source_kernel: Option<String>,
    pub context: Option<serde_json::Value>,
    pub consensus_mode: Option<String>,
    pub proposal_id: Option<String>,
}

/// Internal job payload format for deserialization
#[derive(Debug, Deserialize)]
struct JobPayload {
    task: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(rename = "sourceKernel")]
    source_kernel: Option<String>,
    context: Option<serde_json::Value>,
    #[serde(rename = "consensusMode")]
    consensus_mode: Option<String>,
    #[serde(rename = "proposalId")]
    proposal_id: Option<String>,
}

fn default_mode() -> String {
    "analyze".to_string()
}

/// Edge response representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeResponse {
    pub task: String,
    pub response: String,
    pub timestamp: String,
    #[serde(rename = "processUrn")]
    pub process_urn: String,
}

// ============================================================================
// Internal Implementation Functions
// ============================================================================

/// Detect project root by looking for .ckproject file
fn detect_project_root() -> Result<PathBuf> {
    // Try current directory first
    let cwd = env::current_dir()
        .map_err(|e| CkpError::IoError(e.to_string()))?;

    if cwd.join(".ckproject").exists() {
        return Ok(cwd);
    }

    // Check if we're inside a concepts/ directory
    if let Some(parent) = cwd.parent() {
        if parent.join(".ckproject").exists() {
            return Ok(parent.to_path_buf());
        }
    }

    // Check environment variable
    if let Ok(project_root) = env::var("CK_PROJECT_ROOT") {
        let path = PathBuf::from(project_root);
        if path.join(".ckproject").exists() {
            return Ok(path);
        }
    }

    Err(CkpError::ProjectNotFound)
}

/// Scan job files from a queue directory
fn scan_jobs(queue_dir: &Path) -> Result<Vec<Job>> {
    let entries = fs::read_dir(queue_dir)
        .map_err(|e| CkpError::IoError(e.to_string()))?;

    let mut jobs = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Skip non-job files
        if !filename.ends_with(".job") && !filename.ends_with(".inst") {
            continue;
        }
        if filename == ".gitkeep" {
            continue;
        }

        // Read and parse job file
        let content = fs::read_to_string(&path)
            .map_err(|e| CkpError::IoError(format!("Failed to read {}: {}", path.display(), e)))?;

        let payload: JobPayload = serde_json::from_str(&content)
            .map_err(|e| CkpError::InvalidJson(format!("Failed to parse {}: {}", path.display(), e)))?;

        jobs.push(Job {
            path,
            task: payload.task,
            mode: payload.mode,
            source_kernel: payload.source_kernel,
            context: payload.context,
            consensus_mode: payload.consensus_mode,
            proposal_id: payload.proposal_id,
        });
    }

    Ok(jobs)
}

/// Write evidence in BFO-compliant format
fn write_evidence_bfo_compliant<T: Serialize>(
    storage_dir: &Path,
    evidence: &T,
    tx_id: &str
) -> Result<PathBuf> {
    fs::create_dir_all(storage_dir)
        .map_err(|e| CkpError::IoError(format!("Failed to create storage dir: {}", e)))?;

    // Extract suffix from tx_id (e.g., "tx_1234567890_analysis" -> "analysis")
    let suffix = tx_id.split('_').last().unwrap_or("analysis");
    let inst_dir = storage_dir.join(format!("{}-{}.inst", tx_id, suffix));

    fs::create_dir_all(&inst_dir)
        .map_err(|e| CkpError::IoError(format!("Failed to create instance dir: {}", e)))?;

    let payload_file = inst_dir.join("payload.json");
    let evidence_json = serde_json::to_string_pretty(evidence)
        .map_err(|e| CkpError::InvalidJson(format!("Failed to serialize evidence: {}", e)))?;

    fs::write(&payload_file, evidence_json)
        .map_err(|e| CkpError::IoError(format!("Failed to write evidence: {}", e)))?;

    Ok(payload_file)
}

/// Archive job file to archive directory
fn archive_job_file(job_path: &Path, archive_dir: &Path) -> Result<()> {
    fs::create_dir_all(archive_dir)
        .map_err(|e| CkpError::IoError(format!("Failed to create archive dir: {}", e)))?;

    let filename = job_path.file_name()
        .ok_or_else(|| CkpError::InvalidPath(job_path.display().to_string()))?;

    let dest = archive_dir.join(filename);

    fs::rename(job_path, &dest)
        .map_err(|e| CkpError::IoError(format!("Failed to archive job: {}", e)))?;

    Ok(())
}

/// Move job file to a destination directory
fn move_job_file(job_path: &Path, dest_dir: &Path) -> Result<()> {
    fs::create_dir_all(dest_dir)
        .map_err(|e| CkpError::IoError(format!("Failed to create destination dir: {}", e)))?;

    let filename = job_path.file_name()
        .ok_or_else(|| CkpError::InvalidPath(job_path.display().to_string()))?;

    let dest = dest_dir.join(filename);

    fs::rename(job_path, &dest)
        .map_err(|e| CkpError::IoError(format!("Failed to move job: {}", e)))?;

    Ok(())
}

/// Send edge message in protocol-compliant format
fn send_edge_message_protocol_compliant<T: Serialize>(
    edge_dir: &Path,
    message: &T
) -> Result<()> {
    fs::create_dir_all(edge_dir)
        .map_err(|e| CkpError::IoError(format!("Failed to create edge dir: {}", e)))?;

    let timestamp = Utc::now().timestamp_millis();
    let message_file = edge_dir.join(format!("message-{}.json", timestamp));

    let message_json = serde_json::to_string_pretty(message)
        .map_err(|e| CkpError::InvalidJson(format!("Failed to serialize message: {}", e)))?;

    fs::write(&message_file, message_json)
        .map_err(|e| CkpError::IoError(format!("Failed to write edge message: {}", e)))?;

    Ok(())
}

/// Read edge responses from a response directory
fn read_edge_responses_protocol_compliant(response_dir: &Path) -> Result<Vec<EdgeResponse>> {
    if !response_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(response_dir)
        .map_err(|e| CkpError::IoError(e.to_string()))?;

    let mut responses = Vec::new();

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if !filename.starts_with("response-") || !filename.ends_with(".json") {
            continue;
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| CkpError::IoError(format!("Failed to read response: {}", e)))?;

        let response: EdgeResponse = serde_json::from_str(&content)
            .map_err(|e| CkpError::InvalidJson(format!("Failed to parse response: {}", e)))?;

        responses.push(response);
    }

    Ok(responses)
}

/// Load LLM instructions from kernel's /llm/ directory
fn load_llm_instructions(kernel_dir: &Path) -> Result<String> {
    let llm_dir = kernel_dir.join("llm");

    if !llm_dir.exists() {
        return Ok(String::new());
    }

    let mut instructions = String::new();

    let entries = fs::read_dir(&llm_dir)
        .map_err(|e| CkpError::IoError(e.to_string()))?;

    let mut md_files: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext == "md")
                .unwrap_or(false)
        })
        .collect();

    // Sort by filename for consistent ordering
    md_files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    for entry in md_files {
        let file_path = entry.path();
        let content = fs::read_to_string(&file_path)
            .map_err(|e| CkpError::IoError(format!("Failed to read instruction file: {}", e)))?;

        let filename = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        instructions.push_str(&format!("## From {}\n\n", filename));
        instructions.push_str(&content);
        instructions.push_str("\n\n");
    }

    Ok(instructions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode() {
        assert_eq!(default_mode(), "analyze");
    }

    #[test]
    fn test_job_payload_deserialization() {
        let json = r#"{"task": "test", "mode": "analyze"}"#;
        let payload: JobPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.task, "test");
        assert_eq!(payload.mode, "analyze");
    }

    #[test]
    fn test_job_payload_default_mode() {
        let json = r#"{"task": "test"}"#;
        let payload: JobPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.task, "test");
        assert_eq!(payload.mode, "analyze");
    }
}
