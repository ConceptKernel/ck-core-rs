//! Kernel lifecycle management
//!
//! Provides high-level API for creating, starting, stopping, and monitoring kernels.
//! Maintains 100% compatibility with Node.js KernelManager.js
//!
//! Reference: Node.js v1.3.14 - KernelManager.js

use crate::errors::{CkpError, Result};
use crate::ontology::{OntologyReader, Ontology};
use crate::continuant_tracker::{ContinuantTracker, Function};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use sysinfo::{Pid, System};

/// High-level kernel lifecycle manager
pub struct KernelManager {
    /// Root directory for project
    root: PathBuf,

    /// Concepts directory ({root}/concepts)
    concepts_dir: PathBuf,
}

/// Status information for a kernel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelStatus {
    /// Kernel name
    pub name: String,

    /// Kernel type from ontology (e.g., "node:cold", "python:hot")
    #[serde(rename = "type")]
    pub kernel_type: String,

    /// Tool process ID (hot kernels or cold kernel currently processing)
    pub pid: Option<u32>,

    /// Watcher process ID (cold kernels)
    #[serde(rename = "watcherPid")]
    pub watcher_pid: Option<u32>,

    /// Current mode
    pub mode: String,

    /// Queue statistics
    #[serde(rename = "queueStats")]
    pub queue_stats: QueueStats,

    /// Port number (if specified in ontology)
    pub port: Option<u16>,
}

/// Queue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    /// Count of jobs in inbox
    pub inbox: usize,

    /// Count of jobs in staging
    pub staging: usize,

    /// Count of jobs in ready
    pub ready: usize,
}

/// Running process IDs
#[derive(Debug, Clone)]
pub struct RunningPids {
    /// Tool process ID
    pub pid: Option<u32>,

    /// Watcher process ID
    pub watcher_pid: Option<u32>,
}

/// Result of starting a kernel
#[derive(Debug, Clone)]
pub struct StartResult {
    /// Tool process ID
    pub pid: Option<u32>,

    /// Watcher process ID
    pub watcher_pid: Option<u32>,

    /// Kernel type
    pub kernel_type: String,

    /// Whether kernel was already running
    pub already_running: bool,
}

impl KernelManager {
    /// Create a new KernelManager
    ///
    /// # Arguments
    ///
    /// * `root` - Root directory for project (contains /concepts/)
    ///
    /// # Returns
    ///
    /// KernelManager instance with /concepts/ directory ensured
    ///
    /// # Example
    ///
    /// ```
    /// use ckp_core::kernel::KernelManager;
    /// use std::path::PathBuf;
    ///
    /// let manager = KernelManager::new(PathBuf::from("/my-project")).unwrap();
    /// ```
    pub fn new(root: PathBuf) -> Result<Self> {
        let concepts_dir = root.join("concepts");

        // Create concepts directory if it doesn't exist
        if !concepts_dir.exists() {
            fs::create_dir_all(&concepts_dir)
                .map_err(|e| CkpError::IoError(format!("Failed to create concepts directory: {}", e)))?;
        }

        Ok(Self { root, concepts_dir })
    }

    /// List all valid kernels in /concepts/
    ///
    /// Filters out:
    /// - Hidden directories (starting with '.')
    /// - Special directories ('bus')
    /// - Directories without conceptkernel.yaml
    ///
    /// # Returns
    ///
    /// Vector of kernel names (simple names, not URNs)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ckp_core::kernel::KernelManager;
    /// # use std::path::PathBuf;
    /// # fn example() -> ckp_core::errors::Result<()> {
    /// let manager = KernelManager::new(PathBuf::from("/my-project"))?;
    /// let kernels = manager.list_kernels()?;
    /// println!("Kernels: {:?}", kernels);
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_kernels(&self) -> Result<Vec<String>> {
        if !self.concepts_dir.exists() {
            return Ok(Vec::new());
        }

        let mut kernels = Vec::new();

        for entry in fs::read_dir(&self.concepts_dir)
            .map_err(|e| CkpError::IoError(format!("Failed to read concepts directory: {}", e)))?
        {
            let entry = entry.map_err(|e| CkpError::IoError(e.to_string()))?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();

            // Filter out hidden directories
            if name.starts_with('.') {
                continue;
            }

            // Filter out special directories
            if name == "bus" {
                continue;
            }

            // Filter out instance directories (e.g., System.Oidc.User.1, System.Oidc.User.2)
            // Instance pattern: ends with .N where N is a number
            if let Some(last_segment) = name.split('.').last() {
                if last_segment.parse::<u32>().is_ok() {
                    continue;
                }
            }

            // Only include if conceptkernel.yaml exists
            if path.join("conceptkernel.yaml").exists() {
                kernels.push(name);
            }
        }

        kernels.sort();
        Ok(kernels)
    }

    /// Get full path to kernel directory
    ///
    /// # Arguments
    ///
    /// * `name` - Kernel name
    ///
    /// # Returns
    ///
    /// PathBuf to {root}/concepts/{name}
    pub fn get_kernel_dir(&self, name: &str) -> PathBuf {
        self.concepts_dir.join(name)
    }

    /// Check if kernel exists with valid ontology
    ///
    /// # Arguments
    ///
    /// * `name` - Kernel name
    ///
    /// # Returns
    ///
    /// true if kernel directory and conceptkernel.yaml exist
    pub fn exists(&self, name: &str) -> bool {
        let kernel_dir = self.get_kernel_dir(name);
        kernel_dir.exists() && kernel_dir.join("conceptkernel.yaml").exists()
    }

    /// Create a new kernel from template
    ///
    /// # Arguments
    ///
    /// * `name` - Kernel name (e.g., "MyDomain.MyKernel")
    /// * `template` - Template type (node:cold, node:hot, python:cold, python:hot)
    /// * `version` - Version tag (e.g., "v0.1")
    ///
    /// # Returns
    ///
    /// Ok(()) if kernel was created successfully
    ///
    /// # Errors
    ///
    /// Returns error if kernel already exists or creation fails
    pub fn create_kernel(&self, name: &str, template: &str, version: &str) -> Result<()> {
        let kernel_dir = self.get_kernel_dir(name);

        // Check if already exists
        if kernel_dir.exists() {
            return Err(CkpError::IoError(format!(
                "Kernel already exists: {}",
                name
            )));
        }

        // Create directory structure
        fs::create_dir_all(&kernel_dir)
            .map_err(|e| CkpError::IoError(format!("Failed to create kernel directory: {}", e)))?;

        // Create conceptkernel.yaml
        let ontology_content = format!(
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:{}
  type: {}
  version: {}

spec:
  queue_contract:
    edges: []
  storage_contract:
    strategy: file
"#,
            name, version, template, version
        );

        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content)
            .map_err(|e| CkpError::IoError(format!("Failed to write conceptkernel.yaml: {}", e)))?;

        // Create queue directories
        fs::create_dir_all(kernel_dir.join("queue/inbox"))
            .map_err(|e| CkpError::IoError(format!("Failed to create queue/inbox: {}", e)))?;
        fs::create_dir_all(kernel_dir.join("queue/staging"))
            .map_err(|e| CkpError::IoError(format!("Failed to create queue/staging: {}", e)))?;
        fs::create_dir_all(kernel_dir.join("queue/ready"))
            .map_err(|e| CkpError::IoError(format!("Failed to create queue/ready: {}", e)))?;
        fs::create_dir_all(kernel_dir.join("queue/archive"))
            .map_err(|e| CkpError::IoError(format!("Failed to create queue/archive: {}", e)))?;

        // Create other directories
        fs::create_dir_all(kernel_dir.join("storage"))
            .map_err(|e| CkpError::IoError(format!("Failed to create storage: {}", e)))?;
        fs::create_dir_all(kernel_dir.join("logs"))
            .map_err(|e| CkpError::IoError(format!("Failed to create logs: {}", e)))?;
        fs::create_dir_all(kernel_dir.join("tool"))
            .map_err(|e| CkpError::IoError(format!("Failed to create tool: {}", e)))?;

        // Create minimal tool script based on template
        let tool_content = if template.starts_with("python") {
            // Python tool
            r#"#!/usr/bin/env python3
"""
Minimal Python tool for ConceptKernel
"""

import sys
import json

def main():
    print(f"Processing job in {sys.argv[0]}")
    # Add your processing logic here
    sys.exit(0)

if __name__ == "__main__":
    main()
"#
        } else {
            // Node.js tool
            r#"#!/usr/bin/env node
/**
 * Minimal Node.js tool for ConceptKernel
 */

console.log('Processing job');
// Add your processing logic here
process.exit(0);
"#
        };

        let tool_filename = if template.starts_with("python") {
            "tool/main.py"
        } else {
            "tool/tool.js"
        };

        fs::write(kernel_dir.join(tool_filename), tool_content)
            .map_err(|e| CkpError::IoError(format!("Failed to write tool script: {}", e)))?;

        // Create empty tx.jsonl
        fs::write(kernel_dir.join("tx.jsonl"), "")
            .map_err(|e| CkpError::IoError(format!("Failed to create tx.jsonl: {}", e)))?;

        Ok(())
    }

    /// Get comprehensive status of a kernel
    ///
    /// # Arguments
    ///
    /// * `name` - Kernel name
    ///
    /// # Returns
    ///
    /// KernelStatus with all details (PIDs, mode, queue stats, port)
    ///
    /// # Errors
    ///
    /// Returns error if kernel does not exist
    pub async fn get_kernel_status(&self, name: &str) -> Result<KernelStatus> {
        if !self.exists(name) {
            return Err(CkpError::FileNotFound(format!("Kernel not found: {}", name)));
        }

        let kernel_dir = self.get_kernel_dir(name);

        // Load ontology
        let ontology_reader = OntologyReader::new(self.root.clone());
        let ontology = ontology_reader.read_by_kernel_name(name)?;

        // Get running PIDs
        let pids = self.find_running_pids(name)?;

        // Calculate mode
        let mode = self.calculate_mode(&ontology.metadata.kernel_type, &pids, &ontology);

        // Get queue statistics
        let queue_stats = self.get_queue_stats(&kernel_dir)?;

        // Port extraction not supported yet (ontology structure doesn't have annotations map)
        let port = None;

        Ok(KernelStatus {
            name: name.to_string(),
            kernel_type: ontology.metadata.kernel_type.clone(),
            pid: pids.pid,
            watcher_pid: pids.watcher_pid,
            mode,
            queue_stats,
            port,
        })
    }

    /// Find running process IDs for kernel
    ///
    /// Reads PID files and validates processes are actually running.
    /// Cleans up stale PID files automatically.
    ///
    /// # Arguments
    ///
    /// * `name` - Kernel name
    ///
    /// # Returns
    ///
    /// RunningPids with validated PIDs (or None if not running)
    pub fn find_running_pids(&self, name: &str) -> Result<RunningPids> {
        let kernel_dir = self.get_kernel_dir(name);

        let tool_pid_file = kernel_dir.join(".tool.pid");
        let watcher_pid_file = kernel_dir.join(".watcher.pid");
        let governor_pid_file = kernel_dir.join("tool/.governor.pid");

        let pid = self.read_and_validate_pid(&tool_pid_file)?;

        // Try .watcher.pid first, then fall back to tool/.governor.pid
        let watcher_pid = match self.read_and_validate_pid(&watcher_pid_file)? {
            Some(pid) => Some(pid),
            None => self.read_and_validate_pid(&governor_pid_file)?,
        };

        Ok(RunningPids { pid, watcher_pid })
    }

    /// Check if process with given PID is running
    ///
    /// # Arguments
    ///
    /// * `pid` - Process ID to check
    ///
    /// # Returns
    ///
    /// true if process is running, false otherwise
    pub fn is_process_running(&self, pid: u32) -> bool {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
        sys.process(Pid::from_u32(pid)).is_some()
    }

    /// Start a kernel (hot or cold)
    ///
    /// # Arguments
    ///
    /// * `name` - Kernel name
    /// * `options` - Start options (currently unused)
    ///
    /// # Returns
    ///
    /// StartResult with PIDs and kernel type
    ///
    /// # Errors
    ///
    /// Returns error if kernel does not exist or type is unsupported
    pub async fn start_kernel(
        &self,
        name: &str,
        _options: &HashMap<String, String>,
    ) -> Result<StartResult> {
        if !self.exists(name) {
            return Err(CkpError::FileNotFound(format!("Kernel not found: {}", name)));
        }

        // CRITICAL: Validate ontology.ttl exists (required for BFO alignment and unified queries)
        let kernel_dir = self.get_kernel_dir(name);
        let ontology_ttl_path = kernel_dir.join("ontology.ttl");
        if !ontology_ttl_path.exists() {
            return Err(CkpError::Ontology(format!(
                "Kernel '{}' missing required ontology.ttl file. All kernels must have ontology.ttl for BFO alignment and unified SPARQL queries. Expected at: {}",
                name,
                ontology_ttl_path.display()
            )));
        }

        // Load ontology to determine type
        let ontology_reader = OntologyReader::new(self.root.clone());
        let ontology = ontology_reader.read_by_kernel_name(name)?;
        let kernel_type = &ontology.metadata.kernel_type;

        // Check if already running
        let pids = self.find_running_pids(name)?;
        if pids.pid.is_some() || pids.watcher_pid.is_some() {
            return Ok(StartResult {
                pid: pids.pid,
                watcher_pid: pids.watcher_pid,
                kernel_type: kernel_type.clone(),
                already_running: true,
            });
        }

        let is_hot = kernel_type.contains("hot");

        // Step 1: ALWAYS spawn governor daemon watcher for ALL kernels
        let watcher_pid = self.spawn_watcher(name)?;

        // Write watcher PID file
        let watcher_pid_file = kernel_dir.join(".watcher.pid");
        self.write_pid_file(&watcher_pid_file, watcher_pid)?;

        // Step 2: For hot kernels, also spawn the tool process
        let tool_pid = if is_hot {
            let pid = self.spawn_hot_tool(name, &kernel_type, &ontology)?;

            // Write tool PID file
            let tool_pid_file = kernel_dir.join(".tool.pid");
            self.write_pid_file(&tool_pid_file, pid)?;

            Some(pid)
        } else {
            None
        };

        // Phase 4 Stage 2: Create KernelEntity (BFO Material Entity)
        self.ensure_kernel_entity(name, &ontology).ok(); // Non-blocking

        Ok(StartResult {
            pid: tool_pid,
            watcher_pid: Some(watcher_pid),
            kernel_type: kernel_type.clone(),
            already_running: false,
        })
    }

    /// Ensure KernelEntity exists in Continuant tracker (Phase 4 Stage 2)
    ///
    /// Creates or updates a BFO Material Entity representing this kernel
    fn ensure_kernel_entity(&self, kernel_name: &str, ontology: &Ontology) -> Result<()> {
        let tracker = ContinuantTracker::new(self.root.clone());

        let version = ontology.metadata.version.clone()
            .unwrap_or_else(|| "unknown".to_string());
        let kernel_type = ontology.metadata.kernel_type.clone();

        // Metadata from ontology
        let mut metadata = HashMap::new();
        if let Some(description) = &ontology.metadata.description {
            metadata.insert("description".to_string(), serde_json::json!(description));
        }
        if let Some(port) = ontology.metadata.port {
            metadata.insert("port".to_string(), serde_json::json!(port));
        }

        // Create kernel entity
        let entity = tracker.create_kernel_entity(
            kernel_name,
            &version,
            &kernel_type,
            metadata,
        )?;

        // Derive function from kernel name
        if kernel_name.starts_with("System.Gateway") {
            let function = Function {
                name: "gateway".to_string(),
                description: "HTTP API gateway for kernel network".to_string(),
                assigned_at: chrono::Utc::now().to_rfc3339(),
                metadata: HashMap::new(),
            };
            tracker.assign_function(&entity.urn, function)?;
        } else if kernel_name.starts_with("System.Consensus") {
            let function = Function {
                name: "consensus".to_string(),
                description: "Governance and consensus voting".to_string(),
                assigned_at: chrono::Utc::now().to_rfc3339(),
                metadata: HashMap::new(),
            };
            tracker.assign_function(&entity.urn, function)?;
        } else if kernel_name.starts_with("System.Wss") {
            let function = Function {
                name: "websocket-hub".to_string(),
                description: "WebSocket collaboration hub".to_string(),
                assigned_at: chrono::Utc::now().to_rfc3339(),
                metadata: HashMap::new(),
            };
            tracker.assign_function(&entity.urn, function)?;
        } else if kernel_name.starts_with("System.Oidc") {
            let function = Function {
                name: "authentication".to_string(),
                description: "OIDC authentication and authorization".to_string(),
                assigned_at: chrono::Utc::now().to_rfc3339(),
                metadata: HashMap::new(),
            };
            tracker.assign_function(&entity.urn, function)?;
        }

        Ok(())
    }

    /// Stop a running kernel
    ///
    /// Sends SIGTERM to all running processes (tool and watcher).
    /// Cleans up PID files after stopping processes.
    ///
    /// # Arguments
    ///
    /// * `name` - Kernel name
    ///
    /// # Returns
    ///
    /// true if any process was stopped, false otherwise
    pub async fn stop_kernel(&self, name: &str) -> Result<bool> {
        let kernel_dir = self.get_kernel_dir(name);
        let tool_pid_file = kernel_dir.join(".tool.pid");
        let watcher_pid_file = kernel_dir.join(".watcher.pid");

        let mut stopped = false;

        // Try validated PID reading first
        let pids = self.find_running_pids(name)?;

        // Stop tool process
        if let Some(pid) = pids.pid {
            if self.send_sigterm(pid) {
                stopped = true;
            }
        } else if tool_pid_file.exists() {
            // Fallback: Try reading PID without validation
            if let Ok(content) = fs::read_to_string(&tool_pid_file) {
                if let Some(pid_str) = content.split(':').next() {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        if self.send_sigterm(pid) {
                            stopped = true;
                        }
                    }
                }
            }
        }

        // Stop watcher process
        if let Some(watcher_pid) = pids.watcher_pid {
            if self.send_sigterm(watcher_pid) {
                stopped = true;
            }
        } else if watcher_pid_file.exists() {
            // Fallback: Try reading PID without validation
            if let Ok(content) = fs::read_to_string(&watcher_pid_file) {
                if let Some(pid_str) = content.split(':').next() {
                    if let Ok(pid) = pid_str.parse::<u32>() {
                        if self.send_sigterm(pid) {
                            stopped = true;
                        }
                    }
                }
            }
        }

        // Clean up PID files
        let _ = fs::remove_file(&tool_pid_file);
        let _ = fs::remove_file(&watcher_pid_file);

        Ok(stopped)
    }

    /// Get status of all kernels
    ///
    /// Returns a comprehensive status report for every kernel in /concepts/
    ///
    /// # Returns
    ///
    /// Vector of KernelStatus for all valid kernels
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ckp_core::kernel::KernelManager;
    /// # use std::path::PathBuf;
    /// # async fn example() -> ckp_core::errors::Result<()> {
    /// let manager = KernelManager::new(PathBuf::from("/project"))?;
    /// let statuses = manager.status().await?;
    /// for status in statuses {
    ///     println!("{}: {}", status.name, status.mode);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn status(&self) -> Result<Vec<KernelStatus>> {
        let kernel_names = self.list_kernels()?;
        let mut statuses = Vec::new();

        for name in kernel_names {
            match self.get_kernel_status(&name).await {
                Ok(status) => statuses.push(status),
                Err(_) => continue, // Skip kernels with errors
            }
        }

        Ok(statuses)
    }

    /// Start all kernels in the project
    ///
    /// Attempts to start every kernel found in /concepts/.
    /// Skips kernels that are already running.
    ///
    /// # Returns
    ///
    /// Vector of StartResult for each kernel (successful or not)
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ckp_core::kernel::KernelManager;
    /// # use std::path::PathBuf;
    /// # async fn example() -> ckp_core::errors::Result<()> {
    /// let manager = KernelManager::new(PathBuf::from("/project"))?;
    /// let results = manager.start_all().await?;
    /// println!("Started {} kernel(s)", results.len());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start_all(&self) -> Result<Vec<StartResult>> {
        let kernel_names = self.list_kernels()?;
        let mut results = Vec::new();

        for name in kernel_names {
            match self.start_kernel(&name, &std::collections::HashMap::new()).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    eprintln!("Failed to start {}: {}", name, e);
                }
            }
        }

        Ok(results)
    }

    /// Stop all running kernels in the project
    ///
    /// Sends SIGTERM to all running kernel processes.
    ///
    /// # Returns
    ///
    /// Vector of tuples (kernel_name, was_stopped) for each kernel
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ckp_core::kernel::KernelManager;
    /// # use std::path::PathBuf;
    /// # async fn example() -> ckp_core::errors::Result<()> {
    /// let manager = KernelManager::new(PathBuf::from("/project"))?;
    /// let results = manager.stop_all().await?;
    /// let stopped = results.iter().filter(|(_, stopped)| *stopped).count();
    /// println!("Stopped {} kernel(s)", stopped);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stop_all(&self) -> Result<Vec<(String, bool)>> {
        let kernel_names = self.list_kernels()?;
        let mut results = Vec::new();

        for name in kernel_names {
            match self.stop_kernel(&name).await {
                Ok(stopped) => results.push((name.clone(), stopped)),
                Err(e) => {
                    eprintln!("Failed to stop {}: {}", name, e);
                    results.push((name.clone(), false));
                }
            }
        }

        Ok(results)
    }

    // ===== PRIVATE HELPER METHODS =====

    /// Write PID file with format: PID:START_TIME
    ///
    /// This format prevents PID reuse issues by validating start time on read
    fn write_pid_file(&self, pid_file: &Path, pid: u32) -> Result<()> {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All);

        let start_time = if let Some(process) = sys.process(Pid::from_u32(pid)) {
            process.start_time()
        } else {
            return Err(CkpError::Process(format!("Process {} not found", pid)));
        };

        let content = format!("{}:{}", pid, start_time);
        fs::write(pid_file, content)
            .map_err(|e| CkpError::IoError(format!("Failed to write PID file: {}", e)))?;

        Ok(())
    }

    /// Read and validate PID file with start_time verification
    ///
    /// Format: PID:START_TIME
    /// - Validates process exists
    /// - Validates start time matches (prevents PID reuse)
    /// - Auto-cleans stale PID files
    fn read_and_validate_pid(&self, pid_file: &Path) -> Result<Option<u32>> {
        if !pid_file.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(pid_file)
            .map_err(|e| CkpError::IoError(format!("Failed to read PID file: {}", e)))?;

        let parts: Vec<&str> = content.trim().split(':').collect();

        // Validate format
        if parts.len() != 2 {
            // Stale file with old format - clean up
            let _ = fs::remove_file(pid_file);
            return Ok(None);
        }

        let pid: u32 = parts[0]
            .parse()
            .map_err(|e| CkpError::ParseError(format!("Invalid PID: {}", e)))?;

        let expected_start_time: u64 = parts[1]
            .parse()
            .map_err(|e| CkpError::ParseError(format!("Invalid start time: {}", e)))?;

        // Validate process exists AND has matching start time
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All);

        if let Some(process) = sys.process(Pid::from_u32(pid)) {
            if process.start_time() == expected_start_time {
                // Valid! Process running with correct start time
                return Ok(Some(pid));
            }
        }

        // Process dead or PID reused - clean up stale file
        let _ = fs::remove_file(pid_file);
        Ok(None)
    }

    /// Calculate kernel mode based on type and PIDs
    fn calculate_mode(
        &self,
        kernel_type: &str,
        pids: &RunningPids,
        _ontology: &Ontology,
    ) -> String {
        // Note: Ontology annotations not yet supported in current structure
        // Would check for "conceptkernel.io/stopped" annotation here

        // Calculate mode based on type and running processes
        if kernel_type.contains("hot") {
            // Hot services: long-running processes (websockets, APIs, etc.)
            if pids.pid.is_some() {
                "ONLINE".to_string()
            } else {
                // Hot service not running = DOWN (regardless of watcher state)
                "DOWN".to_string()
            }
        } else if kernel_type.contains("cold") {
            // Cold services: job processors with governors
            match (pids.watcher_pid, pids.pid) {
                (Some(_), Some(_)) => "PROCESSING".to_string(), // Processing a job
                (Some(_), None) => "IDLE".to_string(),          // Ready, waiting for jobs
                _ => "DOWN".to_string(),                         // Governor not running
            }
        } else {
            // Unknown type
            "DOWN".to_string()
        }
    }

    /// Get queue statistics for kernel
    fn get_queue_stats(&self, kernel_dir: &Path) -> Result<QueueStats> {
        let count_queue = |subdir: &str| -> Result<usize> {
            let dir = kernel_dir.join("queue").join(subdir);
            if !dir.exists() {
                return Ok(0);
            }

            let count = fs::read_dir(&dir)
                .map_err(|e| CkpError::IoError(format!("Failed to read queue: {}", e)))?
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    name != ".gitkeep" && e.path().is_file()
                })
                .count();

            Ok(count)
        };

        Ok(QueueStats {
            inbox: count_queue("inbox")?,
            staging: count_queue("staging")?,
            ready: count_queue("ready")?,
        })
    }

    /// Spawn governor daemon watcher process for a kernel
    ///
    /// This is called for ALL kernels (both hot and cold).
    /// Uses the unified `ckr daemon governor` command.
    fn spawn_watcher(&self, name: &str) -> Result<u32> {
        use std::process::{Command, Stdio};

        // Find ckr binary - resolve symlinks to get the actual binary
        let current_exe = std::env::current_exe()
            .map_err(|_| CkpError::Process("Failed to get current executable path".to_string()))?;

        // Canonicalize to resolve symlinks (e.g., /usr/local/bin/ck -> .../target/release/ckr)
        let ckr = std::fs::canonicalize(&current_exe)
            .unwrap_or_else(|_| current_exe.clone());

        // Verify the binary exists and is executable
        if !ckr.exists() {
            return Err(CkpError::FileNotFound(format!(
                "ckr binary not found at: {}",
                ckr.display()
            )));
        }

        // Spawn ckr daemon governor with proper detachment
        // Redirect stderr to a debug log file for visibility
        let kernel_dir = self.get_kernel_dir(name);
        let debug_log = kernel_dir.join("logs").join("governor-debug.log");
        std::fs::create_dir_all(kernel_dir.join("logs")).ok();
        let stderr_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&debug_log)
            .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap());

        let child = Command::new(&ckr)
            .arg("daemon")
            .arg("governor")
            .arg("--kernel")
            .arg(name)
            .arg("--project")
            .arg(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr_file))
            .spawn()
            .map_err(|e| CkpError::Process(format!("Failed to spawn ckr daemon governor: {}", e)))?;

        Ok(child.id())
    }

    /// Spawn hot kernel tool process
    ///
    /// Only called for hot kernels. Spawns the actual tool binary/script.
    fn spawn_hot_tool(&self, name: &str, kernel_type: &str, ontology: &Ontology) -> Result<u32> {
        use std::process::{Command, Stdio};

        let kernel_dir = self.get_kernel_dir(name);

        let entrypoint = ontology.metadata.entrypoint
            .as_ref()
            .ok_or_else(|| CkpError::FileNotFound(format!(
                "No entrypoint specified in ontology for {}",
                name
            )))?;

        // Get or allocate port from .ckports file via PortManager
        // Hot kernels get their ports dynamically allocated, not from ontology
        let mut port_manager = crate::port::manager::PortManager::new(&self.root)
            .map_err(|e| CkpError::Process(format!("Failed to create PortManager: {}", e)))?;

        // Get existing port or allocate a new one
        let port = if let Some(port) = port_manager.get(name) {
            port
        } else {
            port_manager.allocate(name, None)
                .map_err(|e| CkpError::Process(format!(
                    "Failed to allocate port for hot kernel {}: {}",
                    name, e
                )))?
        };

        // Determine command and arguments based on kernel type
        let mut command = if kernel_type.starts_with("rust") {
            // For rust:hot, entrypoint is the binary path (e.g., "tool/rs/system-consensus")
            let binary_path = kernel_dir.join(entrypoint);
            if !binary_path.exists() {
                return Err(CkpError::FileNotFound(format!(
                    "Rust binary not found: {}",
                    binary_path.display()
                )));
            }
            Command::new(binary_path)
        } else if kernel_type.starts_with("python") {
            // For python:hot, entrypoint is typically "tool/main.py"
            let script_path = kernel_dir.join(entrypoint);
            if !script_path.exists() {
                return Err(CkpError::FileNotFound(format!(
                    "Python script not found: {}",
                    script_path.display()
                )));
            }
            let mut cmd = Command::new("python3");
            cmd.arg(script_path);
            cmd
        } else if kernel_type.starts_with("node") {
            // For node:hot, entrypoint is typically "tool/tool.js"
            let script_path = kernel_dir.join(entrypoint);
            if !script_path.exists() {
                return Err(CkpError::FileNotFound(format!(
                    "Node script not found: {}",
                    script_path.display()
                )));
            }
            let mut cmd = Command::new("node");
            cmd.arg(script_path);
            cmd
        } else {
            return Err(CkpError::Process(format!(
                "Unsupported kernel type: {}",
                kernel_type
            )));
        };

        // Set CK_PORT environment variable
        command.env("CK_PORT", port.to_string());

        // Set working directory for Python kernels (they expect to run from tool/)
        if kernel_type.starts_with("python") {
            command.current_dir(kernel_dir.join("tool"));
        }

        // Spawn with proper detachment
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| CkpError::Process(format!("Failed to spawn hot tool: {}", e)))?;

        Ok(child.id())
    }

    /// Send SIGTERM signal to process
    #[cfg(unix)]
    fn send_sigterm(&self, pid: u32) -> bool {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;

        match kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
            Ok(_) => true,
            Err(_) => false,
        }
    }

    #[cfg(not(unix))]
    fn send_sigterm(&self, _pid: u32) -> bool {
        // Windows support would go here
        eprintln!("[KernelManager] SIGTERM not supported on this platform");
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_manager() -> (TempDir, KernelManager) {
        let temp_dir = TempDir::new().unwrap();
        let manager = KernelManager::new(temp_dir.path().to_path_buf()).unwrap();
        (temp_dir, manager)
    }

    fn create_test_kernel(root: &Path, name: &str, kernel_type: &str) {
        let kernel_dir = root.join("concepts").join(name);
        fs::create_dir_all(&kernel_dir).unwrap();

        // Create ontology
        let ontology = format!(
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: {}
  version: v0.1
"#,
            name, kernel_type
        );
        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();

        // Create ontology.ttl (required for BFO alignment)
        let ontology_ttl = format!(
            r#"@prefix ckp: <ckp://{}:v0.1#> .
@prefix bfo: <http://purl.obolibrary.org/obo/BFO_> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

ckp: a bfo:0000029 ;  # Site (BFO)
    rdf:label "{}" .
"#,
            name, name
        );
        fs::write(kernel_dir.join("ontology.ttl"), ontology_ttl).unwrap();

        // Create queue directories
        fs::create_dir_all(kernel_dir.join("queue/inbox")).unwrap();
        fs::create_dir_all(kernel_dir.join("queue/staging")).unwrap();
        fs::create_dir_all(kernel_dir.join("queue/ready")).unwrap();
    }

    #[test]
    fn test_new() {
        let temp_dir = TempDir::new().unwrap();
        let manager = KernelManager::new(temp_dir.path().to_path_buf()).unwrap();

        assert_eq!(manager.root, temp_dir.path());
        assert!(manager.concepts_dir.exists());
    }

    #[test]
    fn test_list_kernels_empty() {
        let (_temp, manager) = setup_test_manager();
        let kernels = manager.list_kernels().unwrap();
        assert_eq!(kernels.len(), 0);
    }

    #[test]
    fn test_list_kernels_with_kernels() {
        let (temp, manager) = setup_test_manager();

        create_test_kernel(temp.path(), "Kernel1", "node:cold");
        create_test_kernel(temp.path(), "Kernel2", "node:hot");

        let kernels = manager.list_kernels().unwrap();
        assert_eq!(kernels, vec!["Kernel1", "Kernel2"]);
    }

    #[test]
    fn test_list_kernels_filters_hidden() {
        let (temp, manager) = setup_test_manager();

        create_test_kernel(temp.path(), "Kernel1", "node:cold");

        // Create hidden directory
        let hidden_dir = temp.path().join("concepts/.hidden");
        fs::create_dir_all(&hidden_dir).unwrap();
        fs::write(hidden_dir.join("conceptkernel.yaml"), "dummy").unwrap();

        let kernels = manager.list_kernels().unwrap();
        assert_eq!(kernels, vec!["Kernel1"]);
    }

    #[test]
    fn test_list_kernels_filters_without_ontology() {
        let (temp, manager) = setup_test_manager();

        create_test_kernel(temp.path(), "ValidKernel", "node:cold");

        // Create directory without ontology
        fs::create_dir_all(temp.path().join("concepts/InvalidKernel")).unwrap();

        let kernels = manager.list_kernels().unwrap();
        assert_eq!(kernels, vec!["ValidKernel"]);
    }

    #[test]
    fn test_get_kernel_dir() {
        let (temp, manager) = setup_test_manager();
        let kernel_dir = manager.get_kernel_dir("TestKernel");

        assert_eq!(kernel_dir, temp.path().join("concepts/TestKernel"));
    }

    #[test]
    fn test_exists() {
        let (temp, manager) = setup_test_manager();

        assert!(!manager.exists("NonExistent"));

        create_test_kernel(temp.path(), "ExistingKernel", "node:cold");
        assert!(manager.exists("ExistingKernel"));
    }

    #[tokio::test]
    async fn test_get_kernel_status() {
        let (temp, manager) = setup_test_manager();
        create_test_kernel(temp.path(), "TestKernel", "node:cold");

        let status = manager.get_kernel_status("TestKernel").await.unwrap();

        assert_eq!(status.name, "TestKernel");
        assert_eq!(status.kernel_type, "node:cold");
        assert_eq!(status.mode, "SLEEP");
        assert_eq!(status.pid, None);
        assert_eq!(status.watcher_pid, None);
    }

    #[tokio::test]
    async fn test_get_kernel_status_not_found() {
        let (_temp, manager) = setup_test_manager();

        let result = manager.get_kernel_status("NonExistent").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_find_running_pids_none() {
        let (temp, manager) = setup_test_manager();
        create_test_kernel(temp.path(), "TestKernel", "node:cold");

        let pids = manager.find_running_pids("TestKernel").unwrap();
        assert_eq!(pids.pid, None);
        assert_eq!(pids.watcher_pid, None);
    }

    #[test]
    fn test_is_process_running() {
        let (_temp, manager) = setup_test_manager();

        // Current process should be running
        let current_pid = std::process::id();
        assert!(manager.is_process_running(current_pid));

        // PID 999999 should not exist
        assert!(!manager.is_process_running(999999));
    }

    #[test]
    fn test_queue_stats() {
        let (temp, manager) = setup_test_manager();
        create_test_kernel(temp.path(), "TestKernel", "node:cold");

        let kernel_dir = manager.get_kernel_dir("TestKernel");

        // Create some job files
        fs::write(kernel_dir.join("queue/inbox/job1.job"), "{}").unwrap();
        fs::write(kernel_dir.join("queue/inbox/job2.job"), "{}").unwrap();
        fs::write(kernel_dir.join("queue/staging/job3.job"), "{}").unwrap();

        let stats = manager.get_queue_stats(&kernel_dir).unwrap();
        assert_eq!(stats.inbox, 2);
        assert_eq!(stats.staging, 1);
        assert_eq!(stats.ready, 0);
    }

    // ===== LIFECYCLE EDGE CASES =====

    #[tokio::test]
    async fn test_start_already_running() {
        let (temp, manager) = setup_test_manager();
        create_test_kernel(temp.path(), "TestKernel", "node:cold");

        let kernel_dir = manager.get_kernel_dir("TestKernel");

        // Write PID files to simulate running kernel
        let current_pid = std::process::id();
        let start_time = {
            let mut sys = sysinfo::System::new();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
            sys.process(sysinfo::Pid::from_u32(current_pid))
                .map(|p| p.start_time())
                .unwrap_or(0)
        };
        fs::write(kernel_dir.join(".watcher.pid"), format!("{}:{}", current_pid, start_time)).unwrap();

        let options = HashMap::new();
        let result = manager.start_kernel("TestKernel", &options).await.unwrap();

        assert!(result.already_running);
        assert_eq!(result.watcher_pid, Some(current_pid));
    }

    #[tokio::test]
    async fn test_stop_not_running() {
        let (temp, manager) = setup_test_manager();
        create_test_kernel(temp.path(), "TestKernel", "node:cold");

        let result = manager.stop_kernel("TestKernel").await.unwrap();

        // Should return false because nothing was running
        assert!(!result);
    }

    #[tokio::test]
    async fn test_restart_kernel() {
        let (temp, manager) = setup_test_manager();
        create_test_kernel(temp.path(), "TestKernel", "node:cold");

        let kernel_dir = manager.get_kernel_dir("TestKernel");

        // Create PID file with non-existent PID
        // Using a high PID number that won't exist
        let fake_pid = 999999;
        fs::write(kernel_dir.join(".watcher.pid"), fake_pid.to_string()).unwrap();

        // Verify kernel appears as running initially
        let pids = manager.find_running_pids("TestKernel").unwrap();
        // Should be None because fake PID doesn't exist (auto-cleanup)
        assert_eq!(pids.watcher_pid, None);

        // Start kernel (should not be "already running" since PID was cleaned up)
        let options = HashMap::new();
        let result = manager.start_kernel("TestKernel", &options).await.unwrap();

        // Should successfully start (mock implementation)
        assert_eq!(result.kernel_type, "node:cold");
        assert!(!result.already_running);
    }

    #[tokio::test]
    async fn test_concurrent_start_stop() {
        use std::sync::Arc;
        use tokio::task;

        let (temp, manager) = setup_test_manager();
        create_test_kernel(temp.path(), "TestKernel", "node:cold");

        let manager = Arc::new(manager);
        let options = HashMap::new();

        // Spawn concurrent start operations
        let manager1 = Arc::clone(&manager);
        let opts1 = options.clone();
        let handle1 = task::spawn(async move {
            manager1.start_kernel("TestKernel", &opts1).await
        });

        let manager2 = Arc::clone(&manager);
        let opts2 = options.clone();
        let handle2 = task::spawn(async move {
            manager2.start_kernel("TestKernel", &opts2).await
        });

        // Both should complete without panicking
        let result1 = handle1.await.unwrap();
        let result2 = handle2.await.unwrap();

        assert!(result1.is_ok());
        assert!(result2.is_ok());

        // Clean up: ensure temp dir is kept alive
        drop(temp);
    }

    // ===== STATUS MONITORING =====

    #[tokio::test]
    async fn test_get_status_not_found() {
        let (_temp, manager) = setup_test_manager();

        let result = manager.get_kernel_status("NonExistent").await;

        assert!(result.is_err());
        match result {
            Err(CkpError::FileNotFound(msg)) => {
                assert!(msg.contains("NonExistent"));
            }
            _ => panic!("Expected FileNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_get_status_running() {
        let (temp, manager) = setup_test_manager();
        create_test_kernel(temp.path(), "TestKernel", "node:hot");

        let kernel_dir = manager.get_kernel_dir("TestKernel");

        // Write PID file to simulate running hot kernel
        let current_pid = std::process::id();
        let start_time = {
            let mut sys = sysinfo::System::new();
            sys.refresh_processes(sysinfo::ProcessesToUpdate::All);
            sys.process(sysinfo::Pid::from_u32(current_pid))
                .map(|p| p.start_time())
                .unwrap_or(0)
        };
        fs::write(kernel_dir.join(".tool.pid"), format!("{}:{}", current_pid, start_time)).unwrap();

        let status = manager.get_kernel_status("TestKernel").await.unwrap();

        assert_eq!(status.name, "TestKernel");
        assert_eq!(status.kernel_type, "node:hot");
        assert_eq!(status.mode, "ONLINE");
        assert_eq!(status.pid, Some(current_pid));
        assert_eq!(status.watcher_pid, None);
    }

    #[tokio::test]
    async fn test_get_status_stopped() {
        let (temp, manager) = setup_test_manager();
        create_test_kernel(temp.path(), "TestKernel", "node:cold");

        let status = manager.get_kernel_status("TestKernel").await.unwrap();

        assert_eq!(status.name, "TestKernel");
        assert_eq!(status.kernel_type, "node:cold");
        assert_eq!(status.mode, "SLEEP");
        assert_eq!(status.pid, None);
        assert_eq!(status.watcher_pid, None);
    }

    #[test]
    fn test_list_all_kernels_empty() {
        let (_temp, manager) = setup_test_manager();

        let kernels = manager.list_kernels().unwrap();

        assert!(kernels.is_empty());
        assert_eq!(kernels.len(), 0);
    }
}
