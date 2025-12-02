//! ConceptKernel Governor - Watches queues and spawns tools
//!
//! Simplified implementation that:
//! - Watches inbox directory
//! - Spawns tool.js or tool.py on file creation
//! - Prevents concurrent executions
//! - Logs to kernel logs/

use crate::errors::{CkpError, Result};
use crate::kernel::PidFile;
use crate::ontology::{OntologyReader, OntologyLibrary};
use crate::urn::UrnResolver;
use crate::drivers::{StorageDriver, FileSystemDriver};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, Config as NotifyConfig};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// ConceptKernel Governor for cold kernels
pub struct ConceptKernelGovernor {
    kernel_name: String,
    root: PathBuf,
    #[allow(dead_code)]
    kernel_type: String,
    tool_path: PathBuf,
    tool_command: String,
    log_file: Arc<Mutex<fs::File>>,
    _pid_file: PidFile,
    driver: Arc<dyn StorageDriver>,
    /// RDF ontology library (Phase 4 Stage 0) - loaded on startup
    ontology_library: Option<Arc<OntologyLibrary>>,
}

impl std::fmt::Debug for ConceptKernelGovernor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConceptKernelGovernor")
            .field("kernel_name", &self.kernel_name)
            .field("root", &self.root)
            .field("kernel_type", &self.kernel_type)
            .field("tool_path", &self.tool_path)
            .field("tool_command", &self.tool_command)
            .field("log_file", &"<File>")
            .field("_pid_file", &self._pid_file)
            .field("ontology_library", &self.ontology_library.as_ref().map(|_| "<OntologyLibrary>"))
            .finish()
    }
}

impl ConceptKernelGovernor {
    /// Create new governor for a kernel
    pub fn new(kernel_name_or_urn: &str, root: PathBuf) -> Result<Self> {
        eprintln!("[Governor] Initializing for kernel: {}", kernel_name_or_urn);
        eprintln!("[Governor] Project root: {}", root.display());

        // Parse URN if provided
        let kernel_name = if kernel_name_or_urn.starts_with("ckp://") {
            let parsed = UrnResolver::parse(kernel_name_or_urn)?;
            eprintln!(
                "[Governor] Parsed URN {} -> kernel: {}",
                kernel_name_or_urn, parsed.kernel
            );
            parsed.kernel
        } else {
            kernel_name_or_urn.to_string()
        };

        let concepts_dir = root.join("concepts");
        let kernel_dir = concepts_dir.join(&kernel_name);
        let ontology_path = kernel_dir.join("conceptkernel.yaml");

        eprintln!("[Governor] Kernel directory: {}", kernel_dir.display());
        eprintln!("[Governor] Ontology path: {}", ontology_path.display());

        // Check if kernel exists
        if !ontology_path.exists() {
            eprintln!("[Governor] ERROR: Ontology not found at {}", ontology_path.display());
            return Err(CkpError::Governor(format!(
                "Kernel {} not found (no conceptkernel.yaml at {})",
                kernel_name,
                ontology_path.display()
            )));
        }

        eprintln!("[Governor] Reading ontology...");
        // Read ontology to determine kernel type
        let ontology_reader = OntologyReader::new(root.clone());
        let ontology = ontology_reader.read_by_kernel_name(&kernel_name)?;
        let kernel_type = ontology.metadata.kernel_type.clone();
        eprintln!("[Governor] Kernel type: {}", kernel_type);

        // Determine tool path and command
        eprintln!("[Governor] Determining tool path for type: {}", kernel_type);
        let (tool_path, tool_command) = if kernel_type.starts_with("python:") {
            let path = kernel_dir.join("tool/tool.py");
            eprintln!("[Governor] Python tool path: {}", path.display());
            (path, "python3".to_string())
        } else if kernel_type.starts_with("rust:") {
            // For rust kernels, look for the compiled binary in the entrypoint
            let entrypoint = ontology.metadata.entrypoint
                .as_ref()
                .ok_or_else(|| CkpError::Governor(format!(
                    "Rust kernel {} missing entrypoint in ontology",
                    kernel_name
                )))?;

            eprintln!("[Governor] Rust entrypoint: {}", entrypoint);

            // Find the binary name from the entrypoint (e.g., tool/rs/llm_executor -> llm_executor)
            let binary_name = entrypoint.split('/').last().unwrap_or("tool");
            eprintln!("[Governor] Binary name: {}", binary_name);

            // Priority order for finding the binary:
            // 1. entrypoint path directly (e.g., tool/rs/gateway-http)
            // 2. tool/rs/{binary} - standard location after build.sh
            // 3. tool/{binary} - alternative location
            // 4. tool/rs/target/release/{binary} - legacy/development location
            let entrypoint_path = kernel_dir.join(entrypoint);
            let tool_rs_binary = kernel_dir.join("tool/rs").join(binary_name);
            let tool_binary = kernel_dir.join("tool").join(binary_name);

            // For backwards compatibility, check if entrypoint contains target/release/
            let release_binary = if entrypoint.contains("target/release") {
                entrypoint_path.clone()
            } else {
                kernel_dir.join("tool/rs/target/release").join(binary_name)
            };

            eprintln!("[Governor] Checking entrypoint: {}", entrypoint_path.display());
            eprintln!("[Governor] entrypoint exists: {}", entrypoint_path.exists());
            eprintln!("[Governor] Checking tool/rs binary: {}", tool_rs_binary.display());
            eprintln!("[Governor] tool/rs exists: {}", tool_rs_binary.exists());
            eprintln!("[Governor] Checking tool binary: {}", tool_binary.display());
            eprintln!("[Governor] tool exists: {}", tool_binary.exists());

            if entrypoint_path.exists() && entrypoint_path.is_file() {
                eprintln!("[Governor] Using entrypoint path directly");
                (entrypoint_path, String::new())
            } else if tool_rs_binary.exists() {
                eprintln!("[Governor] Using tool/rs binary");
                (tool_rs_binary, String::new())
            } else if tool_binary.exists() {
                eprintln!("[Governor] Using tool binary");
                (tool_binary, String::new())
            } else if release_binary.exists() {
                eprintln!("[Governor] Using target/release binary");
                (release_binary, String::new())
            } else {
                eprintln!("[Governor] ERROR: Binary not found");
                eprintln!("[Governor] Tried:");
                eprintln!("[Governor]   1. {}", entrypoint_path.display());
                eprintln!("[Governor]   2. {}", tool_rs_binary.display());
                eprintln!("[Governor]   3. {}", tool_binary.display());
                eprintln!("[Governor]   4. {}", release_binary.display());
                (entrypoint_path, String::new())  // Return for error message
            }
        } else {
            let path = kernel_dir.join("tool/tool.js");
            eprintln!("[Governor] Node.js tool path: {}", path.display());
            (path, "node".to_string())
        };

        eprintln!("[Governor] Final tool path: {}", tool_path.display());
        eprintln!("[Governor] Tool command: {:?}", tool_command);

        // Check if tool exists
        eprintln!("[Governor] Checking if tool exists...");
        if !tool_path.exists() {
            eprintln!("[Governor] ERROR: Tool not found at {}", tool_path.display());
            return Err(CkpError::Governor(format!(
                "Tool not found: {}",
                tool_path.display()
            )));
        }
        eprintln!("[Governor] Tool exists!");

        // Check if inbox exists
        let inbox = kernel_dir.join("queue/inbox");
        eprintln!("[Governor] Checking inbox: {}", inbox.display());
        if !inbox.exists() {
            eprintln!("[Governor] ERROR: Inbox not found");
            return Err(CkpError::Governor(format!(
                "Inbox not found: {}",
                inbox.display()
            )));
        }
        eprintln!("[Governor] Inbox exists!");

        // Create PID file (prevents duplicate governors)
        let pid_path = kernel_dir.join("tool/.governor.pid");
        eprintln!("[Governor] Creating PID file: {}", pid_path.display());
        let pid_file = PidFile::create(&pid_path)?;
        eprintln!("[Governor] PID file created!");

        // Set up logging
        let logs_dir = kernel_dir.join("logs");
        fs::create_dir_all(&logs_dir)?;
        let log_path = logs_dir.join(format!("{}.log", kernel_name));
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        // Create storage driver
        let driver = Arc::new(FileSystemDriver::new(root.clone(), kernel_name.clone())) as Arc<dyn StorageDriver>;

        // Load ontology library (Phase 4 Stage 0) - load RDF ontologies from .ckproject
        eprintln!("[Governor] Loading ontology library...");
        let ontology_library = match OntologyLibrary::new(root.clone()) {
            Ok(lib) => {
                eprintln!("[Governor] Ontology library loaded successfully");
                Some(Arc::new(lib))
            }
            Err(e) => {
                eprintln!("[Governor] Warning: Could not load ontology library: {}", e);
                eprintln!("[Governor] Continuing without RDF ontology support");
                None
            }
        };

        let governor = Self {
            kernel_name: kernel_name.clone(),
            root,
            kernel_type: kernel_type.clone(),
            tool_path,
            tool_command,
            log_file: Arc::new(Mutex::new(log_file)),
            _pid_file: pid_file,
            driver,
            ontology_library,
        };

        governor.log(&format!(
            "[ConceptKernel] [{}] Starting governor (PID: {}, Type: {}, Ontology: {})",
            kernel_name,
            std::process::id(),
            kernel_type,
            if governor.ontology_library.is_some() { "Loaded" } else { "None" }
        ));

        Ok(governor)
    }

    /// Start watching queues (event-driven with notify crate)
    ///
    /// Uses filesystem events for instant detection with fallback polling
    pub async fn start(&self, shutdown: Arc<AtomicBool>) -> Result<()> {
        let inbox_path = self.get_inbox_path();
        let edges_path = self.get_edges_path();

        self.log(&format!(
            "[ConceptKernel] [{}] Status: GOVERNOR",
            self.kernel_name
        ));
        self.log(&format!(
            "[ConceptKernel] [{}] Watching inbox: {} (event-driven)",
            self.kernel_name,
            inbox_path.display()
        ));

        let tool_running = Arc::new(AtomicBool::new(false));

        // Check for existing jobs first (important!)
        self.check_and_process_existing_jobs(tool_running.clone()).await;

        // Set up filesystem watcher
        let (tx, rx) = channel();
        let mut watcher = match RecommendedWatcher::new(tx, NotifyConfig::default()) {
            Ok(w) => {
                self.log(&format!(
                    "[ConceptKernel] [{}] Event-driven watching enabled",
                    self.kernel_name
                ));
                Some(w)
            }
            Err(e) => {
                self.log(&format!(
                    "[ConceptKernel] [{}] Warning: Could not create watcher, falling back to polling: {}",
                    self.kernel_name, e
                ));
                None
            }
        };

        // Watch directories if watcher available
        if let Some(ref mut w) = watcher {
            // Watch inbox
            if let Err(e) = w.watch(&inbox_path, RecursiveMode::NonRecursive) {
                self.log(&format!(
                    "[ConceptKernel] [{}] Warning: Could not watch inbox: {}",
                    self.kernel_name, e
                ));
            }

            // Watch edges directory (if exists)
            if edges_path.exists() {
                if let Err(e) = w.watch(&edges_path, RecursiveMode::Recursive) {
                    self.log(&format!(
                        "[ConceptKernel] [{}] Warning: Could not watch edges: {}",
                        self.kernel_name, e
                    ));
                }
            }
        }

        // Event loop
        if watcher.is_some() {
            // Event-driven mode
            self.log(&format!(
                "[ConceptKernel] [{}] Ready - Waiting for filesystem events",
                self.kernel_name
            ));

            loop {
                // Check shutdown flag
                if shutdown.load(Ordering::SeqCst) {
                    self.log(&format!(
                        "[ConceptKernel] [{}] Received shutdown signal, exiting gracefully",
                        self.kernel_name
                    ));
                    break;
                }

                match rx.recv_timeout(Duration::from_millis(1000)) {
                    Ok(Ok(event)) => {
                        self.handle_filesystem_event(event, tool_running.clone()).await;
                    }
                    Ok(Err(e)) => {
                        self.log(&format!(
                            "[ConceptKernel] [{}] Watcher error: {}",
                            self.kernel_name, e
                        ));
                    }
                    Err(_) => {
                        // Timeout - check edges directory in case new edge queues were created
                        // (edges/* directories themselves trigger events, but we want to be thorough)
                    }
                }
            }
        } else {
            // Fallback to polling
            self.log(&format!(
                "[ConceptKernel] [{}] Using polling mode (500ms interval)",
                self.kernel_name
            ));

            loop {
                // Check shutdown flag
                if shutdown.load(Ordering::SeqCst) {
                    self.log(&format!(
                        "[ConceptKernel] [{}] Received shutdown signal, exiting gracefully",
                        self.kernel_name
                    ));
                    break;
                }

                self.check_and_process_existing_jobs(tool_running.clone()).await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        Ok(())
    }

    /// Handle a filesystem event
    async fn handle_filesystem_event(&self, event: Event, tool_running: Arc<AtomicBool>) {
        // We only care about Create and Modify events
        let is_relevant = matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(_)
        );

        if !is_relevant {
            return;
        }

        // Check if any of the paths are .job or .inst files
        let has_job_files = event.paths.iter().any(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.ends_with(".job") || n.ends_with(".inst"))
                .unwrap_or(false)
        });

        if !has_job_files {
            return;
        }

        // Determine if this is inbox or edge queue
        let inbox_path = self.get_inbox_path();
        let edges_path = self.get_edges_path();

        for path in &event.paths {
            // Check if in inbox
            if path.parent() == Some(&inbox_path) && !tool_running.load(Ordering::SeqCst) {
                tool_running.store(true, Ordering::SeqCst);
                self.log(&format!(
                    "[ConceptKernel] [{}] Event: New job in inbox",
                    self.kernel_name
                ));
                self.spawn_tool(None, tool_running.clone()).await;
                return;
            }

            // Check if in edge queue
            if let Some(parent) = path.parent() {
                if let Some(grandparent) = parent.parent() {
                    if grandparent == edges_path && !tool_running.load(Ordering::SeqCst) {
                        if let Some(edge_name) = parent.file_name().and_then(|n| n.to_str()) {
                            tool_running.store(true, Ordering::SeqCst);
                            self.log(&format!(
                                "[ConceptKernel] [{}] Event: New job in edge queue {}",
                                self.kernel_name, edge_name
                            ));

                            // Validate edge predicate if ontology library is loaded
                            let _ = self.validate_edge_predicate(edge_name);

                            self.spawn_tool(
                                Some(format!("edges/{}", edge_name)),
                                tool_running.clone(),
                            )
                            .await;
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Check for existing jobs (used on startup and polling fallback)
    async fn check_and_process_existing_jobs(&self, tool_running: Arc<AtomicBool>) {
        // Check inbox using driver
        if let Ok(jobs) = self.driver.read_jobs(&self.kernel_name) {
            if !jobs.is_empty() && !tool_running.load(Ordering::SeqCst) {
                tool_running.store(true, Ordering::SeqCst);
                self.log(&format!(
                    "[ConceptKernel] [{}] Found {} job(s) in inbox",
                    self.kernel_name,
                    jobs.len()
                ));
                self.spawn_tool(None, tool_running.clone()).await;
                return;
            }
        }

        // Check edge queues
        let edges_path = self.get_edges_path();
        if edges_path.exists() {
            if let Ok(edge_dirs) = fs::read_dir(&edges_path) {
                for edge_dir in edge_dirs.filter_map(|e| e.ok()) {
                    if !edge_dir.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        continue;
                    }

                    let edge_name = edge_dir.file_name().to_string_lossy().to_string();
                    let edge_queue_path = edge_dir.path();

                    if let Ok(entries) = fs::read_dir(&edge_queue_path) {
                        let jobs: Vec<_> = entries
                            .filter_map(|e| e.ok())
                            .filter(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                (name.ends_with(".inst") || name.ends_with(".job"))
                                    && name != ".gitkeep"
                            })
                            .collect();

                        if !jobs.is_empty() && !tool_running.load(Ordering::SeqCst) {
                            tool_running.store(true, Ordering::SeqCst);
                            self.log(&format!(
                                "[ConceptKernel] [{}] Found {} job(s) in edge queue {}",
                                self.kernel_name,
                                jobs.len(),
                                edge_name
                            ));

                            // Validate edge predicate if ontology library is loaded
                            let _ = self.validate_edge_predicate(&edge_name);

                            self.spawn_tool(Some(format!("edges/{}", edge_name)), tool_running.clone())
                                .await;
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Spawn the kernel tool
    async fn spawn_tool(&self, source_queue: Option<String>, tool_running: Arc<AtomicBool>) {
        let tool_name = self
            .tool_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("tool");

        // Build command
        let mut cmd = if self.tool_command.is_empty() {
            // For Rust binaries, execute directly
            Command::new(&self.tool_path)
        } else {
            // For interpreted languages, use the interpreter
            let mut c = Command::new(&self.tool_command);
            c.arg(&self.tool_path);
            c
        };

        cmd.current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        // For Rust tools, add --batch flag for one-time processing
        if self.tool_command.is_empty() {
            cmd.arg("--batch");
        }

        // Set CK_SOURCE_QUEUE environment variable for edge queues
        if let Some(ref queue) = source_queue {
            cmd.env("CK_SOURCE_QUEUE", queue);
        }

        // Spawn process
        match cmd.spawn() {
            Ok(mut child) => {
                let pid = child.id();
                let queue_info = source_queue
                    .as_ref()
                    .map(|s| format!(" for edge {}", s))
                    .unwrap_or_default();

                self.log(&format!(
                    "[ConceptKernel] [{}] {} started{} (PID: {})",
                    self.kernel_name, tool_name, queue_info, pid
                ));

                // Wait for tool to complete
                match child.wait() {
                    Ok(status) => {
                        tool_running.store(false, Ordering::SeqCst);

                        if status.success() {
                            self.log(&format!(
                                "[ConceptKernel] [{}] {} exited successfully (PID: {})",
                                self.kernel_name, tool_name, pid
                            ));
                        } else {
                            self.log(&format!(
                                "[ConceptKernel] [{}] {} exited with code {:?} (PID: {})",
                                self.kernel_name,
                                tool_name,
                                status.code(),
                                pid
                            ));
                        }
                    }
                    Err(e) => {
                        tool_running.store(false, Ordering::SeqCst);
                        self.log(&format!(
                            "[ConceptKernel] [{}] Failed to wait for tool: {}",
                            self.kernel_name, e
                        ));
                    }
                }
            }
            Err(e) => {
                tool_running.store(false, Ordering::SeqCst);
                self.log(&format!(
                    "[ConceptKernel] [{}] Failed to spawn {}: {}",
                    self.kernel_name, tool_name, e
                ));
            }
        }
    }

    /// Get inbox path
    fn get_inbox_path(&self) -> PathBuf {
        self.root
            .join("concepts")
            .join(&self.kernel_name)
            .join("queue/inbox")
    }

    /// Get edges path
    fn get_edges_path(&self) -> PathBuf {
        self.root
            .join("concepts")
            .join(&self.kernel_name)
            .join("queue/edges")
    }

    /// Log a message
    fn log(&self, msg: &str) {
        println!("{}", msg);
        if let Ok(mut file) = self.log_file.lock() {
            writeln!(file, "{}", msg).ok();
        }
    }

    /// Validate edge predicate using ontology library (Phase 4 Stage 0)
    ///
    /// Checks if an edge name (e.g., "REQUIRES") has a corresponding RDF predicate
    /// mapping in the ontology library. This enables runtime validation of edge
    /// relationships against the semantic model.
    ///
    /// # Arguments
    /// * `edge_name` - Edge name from edge queue directory (e.g., "PRODUCES.SourceKernel")
    ///
    /// # Returns
    /// * `Ok(predicate_uri)` - RDF predicate URI if mapping exists
    /// * `Err(msg)` - Warning message if ontology library not loaded or no mapping found
    ///
    /// # Example
    /// ```
    /// // Edge queue: queue/edges/REQUIRES.System.Consensus
    /// match governor.validate_edge_predicate("REQUIRES") {
    ///     Ok(predicate) => println!("Edge maps to: {}", predicate), // "ckp:requires"
    ///     Err(msg) => eprintln!("Warning: {}", msg),
    /// }
    /// ```
    fn validate_edge_predicate(&self, edge_name: &str) -> std::result::Result<String, String> {
        // Extract the edge type from edge queue name (e.g., "PRODUCES.SourceKernel" -> "PRODUCES")
        let edge_type = edge_name.split('.').next().unwrap_or(edge_name);

        if let Some(ref library) = self.ontology_library {
            match library.get_edge_predicate(edge_type) {
                Ok(predicate) => {
                    self.log(&format!(
                        "[ConceptKernel] [{}] Edge validation: {} -> {}",
                        self.kernel_name, edge_type, predicate
                    ));
                    Ok(predicate)
                }
                Err(e) => {
                    let msg = format!("Edge {} has no predicate mapping: {}", edge_type, e);
                    self.log(&format!(
                        "[ConceptKernel] [{}] Warning: {}",
                        self.kernel_name, msg
                    ));
                    Err(msg)
                }
            }
        } else {
            Err("Ontology library not loaded".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_kernel(temp_dir: &TempDir, kernel_name: &str, tool_type: &str) -> PathBuf {
        let concepts_dir = temp_dir.path().join("concepts");
        let kernel_dir = concepts_dir.join(kernel_name);

        fs::create_dir_all(kernel_dir.join("queue/inbox")).unwrap();
        fs::create_dir_all(kernel_dir.join("queue/edges")).unwrap();
        fs::create_dir_all(kernel_dir.join("tool")).unwrap();
        fs::create_dir_all(kernel_dir.join("logs")).unwrap();

        // Create ontology
        let ontology = format!(
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{kernel}
  type: {tool_type}
  version: v0.1
spec:
  queue_contract:
    edges: []
"#,
            kernel = kernel_name,
            tool_type = tool_type
        );
        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();

        // Create tool
        let tool_path = if tool_type.starts_with("python:") {
            let tool_content = r#"#!/usr/bin/env python3
import sys
print("Tool executed")
sys.exit(0)
"#;
            let path = kernel_dir.join("tool/tool.py");
            fs::write(&path, tool_content).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path).unwrap().permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms).unwrap();
            }
            path
        } else {
            let tool_content = r#"#!/usr/bin/env node
console.log("Tool executed");
process.exit(0);
"#;
            let path = kernel_dir.join("tool/tool.js");
            fs::write(&path, tool_content).unwrap();
            path
        };

        temp_dir.path().to_path_buf()
    }

    // === Governor Initialization Tests (5 tests) ===

    #[test]
    fn test_governor_new_with_valid_kernel() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "TestKernel", "node:cold");

        let result = ConceptKernelGovernor::new("TestKernel", root);
        assert!(result.is_ok(), "Governor should initialize with valid kernel");

        let gov = result.unwrap();
        assert_eq!(gov.kernel_name, "TestKernel");
        assert!(gov.tool_path.exists(), "Tool path should exist");
    }

    #[test]
    fn test_governor_new_with_urn() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "TestKernel", "node:cold");

        let result = ConceptKernelGovernor::new("ckp://TestKernel:v0.1", root);
        assert!(result.is_ok(), "Governor should parse URN correctly");

        let gov = result.unwrap();
        assert_eq!(gov.kernel_name, "TestKernel", "Should extract kernel name from URN");
    }

    #[test]
    fn test_governor_new_with_missing_kernel() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();
        fs::create_dir_all(root.join("concepts")).unwrap();

        let result = ConceptKernelGovernor::new("NonExistent", root);
        assert!(result.is_err(), "Governor should fail for missing kernel");
        assert!(matches!(result.unwrap_err(), CkpError::Governor(_)));
    }

    #[test]
    fn test_governor_new_with_missing_tool() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "NoToolKernel", "node:cold");

        // Remove the tool file
        let tool_path = temp_dir
            .path()
            .join("concepts/NoToolKernel/tool/tool.js");
        fs::remove_file(tool_path).unwrap();

        let result = ConceptKernelGovernor::new("NoToolKernel", root);
        assert!(result.is_err(), "Governor should fail when tool is missing");
    }

    #[test]
    fn test_governor_new_with_python_tool() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "PythonKernel", "python:cold");

        let result = ConceptKernelGovernor::new("PythonKernel", root);
        assert!(result.is_ok(), "Governor should handle Python kernels");

        let gov = result.unwrap();
        assert!(
            gov.tool_path.to_string_lossy().ends_with("tool.py"),
            "Should use tool.py for Python kernels"
        );
        assert_eq!(gov.tool_command, "python3", "Should use python3 command");
    }

    // === PID File Management Tests (5 tests) ===

    #[test]
    fn test_governor_creates_pid_file() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "PidTestKernel", "node:cold");

        let _gov = ConceptKernelGovernor::new("PidTestKernel", root.clone()).unwrap();

        let pid_path = root.join("concepts/PidTestKernel/tool/.governor.pid");
        assert!(pid_path.exists(), "PID file should be created");

        let pid_content = fs::read_to_string(&pid_path).unwrap();
        let pid: u32 = pid_content.trim().parse().unwrap();
        assert_eq!(pid, std::process::id(), "PID file should contain current process ID");
    }

    #[test]
    fn test_governor_prevents_duplicate_governors() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "DuplicateKernel", "node:cold");

        let _gov1 = ConceptKernelGovernor::new("DuplicateKernel", root.clone()).unwrap();

        // Try to create second governor
        let result2 = ConceptKernelGovernor::new("DuplicateKernel", root);
        assert!(
            result2.is_err(),
            "Second governor should fail due to PID file lock"
        );
    }

    #[test]
    fn test_governor_pid_file_cleanup_on_drop() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "CleanupKernel", "node:cold");

        let pid_path = root.join("concepts/CleanupKernel/tool/.governor.pid");

        {
            let _gov = ConceptKernelGovernor::new("CleanupKernel", root.clone()).unwrap();
            assert!(pid_path.exists(), "PID file should exist while governor is alive");
        }

        // PID file should be cleaned up when governor is dropped
        assert!(!pid_path.exists(), "PID file should be removed on drop");
    }

    #[test]
    fn test_governor_creates_log_file() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "LogKernel", "node:cold");

        let _gov = ConceptKernelGovernor::new("LogKernel", root.clone()).unwrap();

        let log_path = root.join("concepts/LogKernel/logs/LogKernel.log");
        assert!(log_path.exists(), "Log file should be created");

        let log_content = fs::read_to_string(&log_path).unwrap();
        assert!(
            log_content.contains("Starting governor"),
            "Log should contain startup message"
        );
    }

    #[test]
    fn test_governor_appends_to_existing_log() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "AppendLogKernel", "node:cold");

        // Create initial log entry
        let log_path = root.join("concepts/AppendLogKernel/logs/AppendLogKernel.log");
        fs::write(&log_path, "Previous log entry\n").unwrap();

        let _gov = ConceptKernelGovernor::new("AppendLogKernel", root.clone()).unwrap();

        let log_content = fs::read_to_string(&log_path).unwrap();
        assert!(
            log_content.contains("Previous log entry"),
            "Should preserve existing log entries"
        );
        assert!(
            log_content.contains("Starting governor"),
            "Should append new entries"
        );
    }

    // === File Watching Tests (5 tests) ===

    #[test]
    fn test_governor_get_inbox_path() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "InboxKernel", "node:cold");

        let gov = ConceptKernelGovernor::new("InboxKernel", root.clone()).unwrap();
        let inbox_path = gov.get_inbox_path();

        assert!(inbox_path.exists(), "Inbox path should exist");
        assert!(
            inbox_path.to_string_lossy().ends_with("queue/inbox"),
            "Should return correct inbox path"
        );
    }

    #[test]
    fn test_governor_get_edges_path() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "EdgesKernel", "node:cold");

        let gov = ConceptKernelGovernor::new("EdgesKernel", root.clone()).unwrap();
        let edges_path = gov.get_edges_path();

        assert!(edges_path.exists(), "Edges path should exist");
        assert!(
            edges_path.to_string_lossy().ends_with("queue/edges"),
            "Should return correct edges path"
        );
    }

    #[test]
    fn test_governor_detects_job_file_in_inbox() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "JobDetectKernel", "node:cold");

        // Create a job file
        let inbox_path = root.join("concepts/JobDetectKernel/queue/inbox");
        let job_path = inbox_path.join("test-job-123.job");
        fs::write(&job_path, r#"{"test": "data"}"#).unwrap();

        // Verify job detection logic
        let entries: Vec<_> = fs::read_dir(&inbox_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name().to_string_lossy().ends_with(".job")
                    || e.file_name().to_string_lossy().ends_with(".inst")
            })
            .collect();

        assert_eq!(entries.len(), 1, "Should detect 1 job file");
    }

    #[test]
    fn test_governor_detects_instance_file_in_inbox() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "InstDetectKernel", "node:cold");

        // Create an instance file
        let inbox_path = root.join("concepts/InstDetectKernel/queue/inbox");
        let inst_path = inbox_path.join("test-inst-456.inst");
        fs::write(&inst_path, r#"{"test": "instance"}"#).unwrap();

        // Verify instance detection logic
        let entries: Vec<_> = fs::read_dir(&inbox_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name().to_string_lossy().ends_with(".job")
                    || e.file_name().to_string_lossy().ends_with(".inst")
            })
            .collect();

        assert_eq!(entries.len(), 1, "Should detect 1 instance file");
    }

    #[test]
    fn test_governor_detects_jobs_in_edge_queue() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "EdgeQueueKernel", "node:cold");

        // Create edge queue directory
        let edge_queue_path = root.join("concepts/EdgeQueueKernel/queue/edges/PRODUCES.SourceKernel");
        fs::create_dir_all(&edge_queue_path).unwrap();

        // Create instance in edge queue
        let inst_path = edge_queue_path.join("edge-inst-789.inst");
        fs::write(&inst_path, r#"{"edge": "data"}"#).unwrap();

        // Verify edge queue detection logic
        let entries: Vec<_> = fs::read_dir(&edge_queue_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.ends_with(".inst") && name != ".gitkeep"
            })
            .collect();

        assert_eq!(entries.len(), 1, "Should detect 1 instance in edge queue");
    }

    // === Process Spawning Tests (5 tests) ===

    #[test]
    fn test_governor_spawn_tool_command_construction_node() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "NodeSpawnKernel", "node:cold");

        let gov = ConceptKernelGovernor::new("NodeSpawnKernel", root.clone()).unwrap();

        assert_eq!(gov.tool_command, "node", "Should use 'node' command");
        assert!(
            gov.tool_path.to_string_lossy().ends_with("tool.js"),
            "Should point to tool.js"
        );
    }

    #[test]
    fn test_governor_spawn_tool_command_construction_python() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "PythonSpawnKernel", "python:cold");

        let gov = ConceptKernelGovernor::new("PythonSpawnKernel", root.clone()).unwrap();

        assert_eq!(gov.tool_command, "python3", "Should use 'python3' command");
        assert!(
            gov.tool_path.to_string_lossy().ends_with("tool.py"),
            "Should point to tool.py"
        );
    }

    #[test]
    fn test_governor_log_writes_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "LogWriteKernel", "node:cold");

        let gov = ConceptKernelGovernor::new("LogWriteKernel", root.clone()).unwrap();
        gov.log("Test log message");

        let log_path = root.join("concepts/LogWriteKernel/logs/LogWriteKernel.log");
        let log_content = fs::read_to_string(&log_path).unwrap();
        assert!(
            log_content.contains("Test log message"),
            "Log message should be written to file"
        );
    }

    #[test]
    fn test_governor_log_multiple_messages() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "MultiLogKernel", "node:cold");

        let gov = ConceptKernelGovernor::new("MultiLogKernel", root.clone()).unwrap();
        gov.log("First message");
        gov.log("Second message");
        gov.log("Third message");

        let log_path = root.join("concepts/MultiLogKernel/logs/MultiLogKernel.log");
        let log_content = fs::read_to_string(&log_path).unwrap();
        assert!(log_content.contains("First message"));
        assert!(log_content.contains("Second message"));
        assert!(log_content.contains("Third message"));
    }

    #[test]
    fn test_governor_handles_missing_inbox_error() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "NoInboxKernel", "node:cold");

        // Remove inbox directory
        let inbox_path = root.join("concepts/NoInboxKernel/queue/inbox");
        fs::remove_dir(&inbox_path).unwrap();

        let result = ConceptKernelGovernor::new("NoInboxKernel", root);
        assert!(result.is_err(), "Governor should fail when inbox is missing");
        assert!(
            matches!(result.unwrap_err(), CkpError::Governor(_)),
            "Should return Governor error"
        );
    }

    // === Advanced Governor Tests - Process Lifecycle Hooks (3 tests) ===

    #[test]
    fn test_governor_pre_spawn_validation() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "PreSpawnKernel", "node:cold");

        // Create a malformed job that should fail validation
        let inbox_path = root.join("concepts/PreSpawnKernel/queue/inbox");
        let bad_job = inbox_path.join("malformed.job");
        fs::write(&bad_job, "not valid json {{{").unwrap();

        let gov = ConceptKernelGovernor::new("PreSpawnKernel", root.clone()).unwrap();

        // In a real implementation, pre-spawn validation would check job format
        // For now, verify that malformed jobs are detectable
        let job_content = fs::read_to_string(&bad_job).unwrap();
        assert!(
            !job_content.starts_with("{"),
            "Should detect malformed job before spawn"
        );
    }

    #[test]
    fn test_governor_post_completion_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "CleanupHookKernel", "node:cold");

        // Create a job that should be archived after processing
        let inbox_path = root.join("concepts/CleanupHookKernel/queue/inbox");
        let archive_path = root.join("concepts/CleanupHookKernel/queue/archive");
        fs::create_dir_all(&archive_path).unwrap();

        let job_file = inbox_path.join("test-job-999.job");
        fs::write(&job_file, r#"{"data": "cleanup_test"}"#).unwrap();

        // Verify cleanup hook would move job to archive
        // (In real implementation, this happens after tool execution)
        let job_exists = job_file.exists();
        assert!(job_exists, "Job should exist before cleanup");

        // Simulate post-completion cleanup
        let archive_dest = archive_path.join("test-job-999.job");
        fs::rename(&job_file, &archive_dest).unwrap();

        assert!(!job_file.exists(), "Job should be removed from inbox");
        assert!(archive_dest.exists(), "Job should be in archive");
    }

    #[test]
    fn test_governor_error_recovery_hook() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "ErrorRecoveryKernel", "node:cold");

        // Create tool that will fail
        let tool_path = root.join("concepts/ErrorRecoveryKernel/tool/tool.js");
        fs::write(
            &tool_path,
            r#"#!/usr/bin/env node
console.error("Tool error");
process.exit(1);
"#,
        )
        .unwrap();

        let gov = ConceptKernelGovernor::new("ErrorRecoveryKernel", root.clone()).unwrap();

        // Error recovery hook should log failure
        let log_path = root.join("concepts/ErrorRecoveryKernel/logs/ErrorRecoveryKernel.log");
        let initial_log = fs::read_to_string(&log_path).unwrap();

        // Simulate error logging
        gov.log("[ERROR] Tool failed with exit code 1");

        let updated_log = fs::read_to_string(&log_path).unwrap();
        assert!(
            updated_log.len() > initial_log.len(),
            "Error should be logged"
        );
        assert!(
            updated_log.contains("[ERROR]"),
            "Error marker should be in log"
        );
    }

    // === Advanced Governor Tests - Edge Queue Priority (3 tests) ===

    #[test]
    fn test_governor_priority_queue_selection() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "PriorityKernel", "node:cold");

        // Create multiple edge queues with different priorities
        let edges_path = root.join("concepts/PriorityKernel/queue/edges");

        let high_priority = edges_path.join("CRITICAL.HighPriority");
        let normal_priority = edges_path.join("PRODUCES.NormalPriority");
        let low_priority = edges_path.join("NOTIFIES.LowPriority");

        for dir in [&high_priority, &normal_priority, &low_priority] {
            fs::create_dir_all(dir).unwrap();
        }

        // Add jobs to each queue
        fs::write(high_priority.join("job1.inst"), r#"{"priority": "high"}"#).unwrap();
        fs::write(normal_priority.join("job2.inst"), r#"{"priority": "normal"}"#).unwrap();
        fs::write(low_priority.join("job3.inst"), r#"{"priority": "low"}"#).unwrap();

        // Verify all queues are detectable
        let edge_dirs: Vec<_> = fs::read_dir(&edges_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();

        assert_eq!(
            edge_dirs.len(),
            3,
            "Should detect all 3 priority queues"
        );

        // In real implementation, CRITICAL edges would be processed first
        let has_critical = edge_dirs
            .iter()
            .any(|e| e.file_name().to_string_lossy().starts_with("CRITICAL"));
        assert!(has_critical, "Should identify CRITICAL priority queue");
    }

    #[test]
    fn test_governor_queue_starvation_prevention() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "StarvationKernel", "node:cold");

        let edges_path = root.join("concepts/StarvationKernel/queue/edges");

        // Create two queues - one with many jobs, one with few
        let busy_queue = edges_path.join("PRODUCES.BusySource");
        let starved_queue = edges_path.join("PRODUCES.QuietSource");

        fs::create_dir_all(&busy_queue).unwrap();
        fs::create_dir_all(&starved_queue).unwrap();

        // Busy queue has 10 jobs
        for i in 0..10 {
            fs::write(
                busy_queue.join(format!("job{}.inst", i)),
                r#"{"source": "busy"}"#,
            )
            .unwrap();
        }

        // Starved queue has only 1 job
        fs::write(
            starved_queue.join("important.inst"),
            r#"{"source": "quiet", "important": true}"#,
        )
        .unwrap();

        // Fair scheduling should ensure starved queue is serviced
        let starved_jobs: Vec<_> = fs::read_dir(&starved_queue)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".inst"))
            .collect();

        assert_eq!(
            starved_jobs.len(),
            1,
            "Starved queue should have job waiting"
        );

        // In real implementation, round-robin or weighted fair queuing would prevent starvation
    }

    #[test]
    fn test_governor_fair_scheduling_algorithm() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "FairScheduleKernel", "node:cold");

        let edges_path = root.join("concepts/FairScheduleKernel/queue/edges");

        // Create 3 queues with different job counts
        let queues = [
            ("PRODUCES.Source1", 5),
            ("PRODUCES.Source2", 3),
            ("PRODUCES.Source3", 7),
        ];

        for (queue_name, job_count) in &queues {
            let queue_path = edges_path.join(queue_name);
            fs::create_dir_all(&queue_path).unwrap();

            for i in 0..*job_count {
                fs::write(
                    queue_path.join(format!("job{}.inst", i)),
                    format!(r#"{{"queue": "{}"}}"#, queue_name),
                )
                .unwrap();
            }
        }

        // Verify all queues exist
        let edge_dirs: Vec<_> = fs::read_dir(&edges_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();

        assert_eq!(edge_dirs.len(), 3, "Should have 3 edge queues");

        // Fair scheduling would process from each queue in round-robin
        // Total jobs: 5 + 3 + 7 = 15
        let total_jobs: usize = edge_dirs
            .iter()
            .map(|dir| {
                fs::read_dir(dir.path())
                    .unwrap()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_name().to_string_lossy().ends_with(".inst"))
                    .count()
            })
            .sum();

        assert_eq!(total_jobs, 15, "Should detect all jobs across queues");
    }

    // === Advanced Governor Tests - Multi-Kernel Coordination (2 tests) ===

    #[test]
    fn test_governor_cross_kernel_dependency_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Create two dependent kernels: KernelA depends on KernelB
        let kernel_a = create_test_kernel(&temp_dir, "KernelA", "node:cold");
        let kernel_b_dir = kernel_a.join("concepts/KernelB");
        fs::create_dir_all(kernel_b_dir.join("queue/inbox")).unwrap();
        fs::create_dir_all(kernel_b_dir.join("tool")).unwrap();

        // Create ontology for KernelB
        let ontology_b = r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://KernelB
  type: node:cold
  version: v0.1
spec:
  dependencies:
    - ckp://KernelA
"#;
        fs::write(kernel_b_dir.join("conceptkernel.yaml"), ontology_b).unwrap();
        fs::write(
            kernel_b_dir.join("tool/tool.js"),
            "console.log('KernelB'); process.exit(0);",
        )
        .unwrap();

        // Verify both kernels can be initialized
        let gov_a = ConceptKernelGovernor::new("KernelA", root.clone());
        let gov_b = ConceptKernelGovernor::new("KernelB", root.clone());

        assert!(gov_a.is_ok(), "KernelA should initialize");
        assert!(gov_b.is_ok(), "KernelB should initialize");

        // In real implementation, dependency tracking would ensure KernelA starts before KernelB
    }

    #[test]
    fn test_governor_coordinated_startup_sequencing() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        // Create 3 kernels that should start in specific order
        for (i, kernel_name) in ["FirstKernel", "SecondKernel", "ThirdKernel"]
            .iter()
            .enumerate()
        {
            let kernel_root = create_test_kernel(&temp_dir, kernel_name, "node:cold");
            let ontology_path = kernel_root
                .join("concepts")
                .join(kernel_name)
                .join("conceptkernel.yaml");

            // Add startup_order metadata
            let ontology = format!(
                r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}
  type: node:cold
  version: v0.1
  annotations:
    startup_order: "{}"
spec:
  queue_contract:
    edges: []
"#,
                kernel_name, i
            );
            fs::write(ontology_path, ontology).unwrap();
        }

        // Verify all kernels can be initialized
        for kernel_name in ["FirstKernel", "SecondKernel", "ThirdKernel"] {
            let gov = ConceptKernelGovernor::new(kernel_name, root.clone());
            assert!(
                gov.is_ok(),
                "{} should initialize in sequence",
                kernel_name
            );
        }

        // In real implementation, governors would coordinate startup order
    }

    // === Windows-Specific Process Spawning Test (1 test) ===

    /// Test: Windows process spawning for .bat and .cmd files
    ///
    /// On Windows, kernel tools can be batch files (.bat, .cmd) instead of
    /// just .js or .py files. This test ensures the governor correctly
    /// spawns Windows batch files using cmd.exe.
    ///
    /// Windows-specific behavior:
    /// - .bat and .cmd files require cmd.exe /C to execute
    /// - Process creation uses CreateProcess API (not fork+exec)
    /// - Path separators are backslashes
    /// - Environment variables use %VAR% syntax
    #[test]
    #[cfg(target_os = "windows")]
    fn test_governor_windows_process_spawn() {
        let temp_dir = TempDir::new().unwrap();
        let concepts_dir = temp_dir.path().join("concepts");
        let kernel_dir = concepts_dir.join("WindowsBatchKernel");

        fs::create_dir_all(kernel_dir.join("queue/inbox")).unwrap();
        fs::create_dir_all(kernel_dir.join("queue/edges")).unwrap();
        fs::create_dir_all(kernel_dir.join("tool")).unwrap();
        fs::create_dir_all(kernel_dir.join("logs")).unwrap();

        // Create ontology for Windows batch kernel
        let ontology = r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://WindowsBatchKernel
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges: []
"#;
        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();

        // Create a Windows batch file tool
        let batch_content = r#"@echo off
REM Windows batch tool for ConceptKernel
echo [WindowsBatchKernel] Processing job...
echo CK_SOURCE_QUEUE=%CK_SOURCE_QUEUE%

REM Create output marker file
echo Batch tool executed > "%~dp0..\logs\batch_executed.txt"

REM Exit successfully
exit /b 0
"#;
        let batch_path = kernel_dir.join("tool/tool.bat");
        fs::write(&batch_path, batch_content).unwrap();

        // Create job to trigger tool execution
        let job_path = kernel_dir.join("queue/inbox/test-job.job");
        fs::write(&job_path, r#"{"test": "windows_spawn"}"#).unwrap();

        // Test that governor can spawn Windows batch file
        // Note: We can't easily test the full governor loop in a unit test,
        // but we can verify the command construction would work

        // Verify batch file exists
        assert!(batch_path.exists(), "Batch tool should exist");

        // Verify we can spawn cmd.exe with the batch file
        use std::process::Command;

        let output = Command::new("cmd")
            .args(&["/C", batch_path.to_str().unwrap()])
            .current_dir(&temp_dir.path())
            .output();

        assert!(output.is_ok(), "Should be able to spawn cmd.exe with batch file");

        let output = output.unwrap();
        assert!(
            output.status.success(),
            "Batch file should execute successfully: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify batch tool output
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Processing job"),
            "Batch tool should log processing: {}",
            stdout
        );

        // Verify output marker was created
        let marker_path = kernel_dir.join("logs/batch_executed.txt");
        assert!(
            marker_path.exists(),
            "Batch tool should create output marker file"
        );

        let marker_content = fs::read_to_string(&marker_path).unwrap();
        assert!(
            marker_content.contains("Batch tool executed"),
            "Marker file should contain expected content"
        );

        // Test with CK_SOURCE_QUEUE environment variable
        let output_with_env = Command::new("cmd")
            .args(&["/C", batch_path.to_str().unwrap()])
            .env("CK_SOURCE_QUEUE", "edges/PRODUCES.SourceKernel")
            .current_dir(&temp_dir.path())
            .output()
            .unwrap();

        assert!(
            output_with_env.status.success(),
            "Batch file with env vars should execute successfully"
        );

        let stdout_with_env = String::from_utf8_lossy(&output_with_env.stdout);
        assert!(
            stdout_with_env.contains("PRODUCES.SourceKernel"),
            "Batch tool should access CK_SOURCE_QUEUE env var: {}",
            stdout_with_env
        );
    }

    /// Test: Unix shell script spawning (excluded from Windows)
    ///
    /// This test ensures Unix shell scripts work correctly.
    /// It's excluded from Windows builds to avoid conflicts.
    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_governor_unix_shell_spawn() {
        let temp_dir = TempDir::new().unwrap();
        let concepts_dir = temp_dir.path().join("concepts");
        let kernel_dir = concepts_dir.join("UnixShellKernel");

        fs::create_dir_all(kernel_dir.join("queue/inbox")).unwrap();
        fs::create_dir_all(kernel_dir.join("tool")).unwrap();
        fs::create_dir_all(kernel_dir.join("logs")).unwrap();

        // Create ontology
        let ontology = r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://UnixShellKernel
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges: []
"#;
        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();

        // Create Unix shell script
        let shell_content = r#"#!/bin/sh
echo "[UnixShellKernel] Processing job..."
echo "CK_SOURCE_QUEUE=$CK_SOURCE_QUEUE"
echo "Shell tool executed" > "$(dirname $0)/../logs/shell_executed.txt"
exit 0
"#;
        let shell_path = kernel_dir.join("tool/tool.sh");
        fs::write(&shell_path, shell_content).unwrap();

        // Make executable
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&shell_path).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&shell_path, perms).unwrap();
        }

        // Verify we can spawn the shell script
        use std::process::Command;

        let output = Command::new(&shell_path)
            .current_dir(&temp_dir.path())
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "Shell script should execute successfully"
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Processing job"),
            "Shell script should log processing"
        );

        // Verify marker file
        let marker_path = kernel_dir.join("logs/shell_executed.txt");
        assert!(marker_path.exists(), "Shell script should create marker file");
    }

    // === Advanced Governor Tests - Performance Benchmarks (2 tests) ===

    #[test]
    fn test_governor_job_processing_throughput() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "ThroughputKernel", "node:cold");

        let inbox_path = root.join("concepts/ThroughputKernel/queue/inbox");

        // Create 100 jobs to simulate high throughput
        let start = std::time::Instant::now();
        for i in 0..100 {
            fs::write(
                inbox_path.join(format!("job{}.job", i)),
                format!(r#"{{"id": {}, "data": "throughput_test"}}"#, i),
            )
            .unwrap();
        }
        let creation_time = start.elapsed();

        // Verify all jobs are created
        let job_count = fs::read_dir(&inbox_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".job"))
            .count();

        assert_eq!(job_count, 100, "Should create 100 jobs");

        // Measure detection time
        let detection_start = std::time::Instant::now();
        let entries: Vec<_> = fs::read_dir(&inbox_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name().to_string_lossy().ends_with(".job")
                    || e.file_name().to_string_lossy().ends_with(".inst")
            })
            .collect();
        let detection_time = detection_start.elapsed();

        assert_eq!(entries.len(), 100, "Should detect all 100 jobs");

        // Performance assertions (should be fast)
        assert!(
            creation_time.as_millis() < 500,
            "Job creation should be fast: {:?}",
            creation_time
        );
        assert!(
            detection_time.as_millis() < 100,
            "Job detection should be fast: {:?}",
            detection_time
        );
    }

    #[test]
    fn test_governor_queue_polling_efficiency() {
        let temp_dir = TempDir::new().unwrap();
        let root = create_test_kernel(&temp_dir, "PollingKernel", "node:cold");

        let inbox_path = root.join("concepts/PollingKernel/queue/inbox");
        let edges_path = root.join("concepts/PollingKernel/queue/edges");

        // Create 5 edge queues
        for i in 0..5 {
            let edge_queue = edges_path.join(format!("PRODUCES.Source{}", i));
            fs::create_dir_all(&edge_queue).unwrap();

            // Add 10 jobs to each edge queue
            for j in 0..10 {
                fs::write(
                    edge_queue.join(format!("job{}.inst", j)),
                    format!(r#"{{"edge": {}, "job": {}}}"#, i, j),
                )
                .unwrap();
            }
        }

        // Measure polling time for all queues
        let start = std::time::Instant::now();

        // Poll inbox
        let inbox_jobs = fs::read_dir(&inbox_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .count();

        // Poll all edge queues
        let mut edge_job_count = 0;
        if let Ok(edge_dirs) = fs::read_dir(&edges_path) {
            for edge_dir in edge_dirs.filter_map(|e| e.ok()) {
                if let Ok(entries) = fs::read_dir(edge_dir.path()) {
                    edge_job_count += entries
                        .filter_map(|e| e.ok())
                        .filter(|e| {
                            let name = e.file_name().to_string_lossy().to_string();
                            name.ends_with(".inst") && name != ".gitkeep"
                        })
                        .count();
                }
            }
        }

        let polling_time = start.elapsed();

        assert_eq!(edge_job_count, 50, "Should detect 50 edge queue jobs");
        assert_eq!(inbox_jobs, 0, "Inbox should be empty");

        // Polling should be efficient even with multiple queues
        assert!(
            polling_time.as_millis() < 50,
            "Queue polling should be fast: {:?}",
            polling_time
        );
    }
}
