//! CKR - ConceptKernel Rust CLI
//!
//! Command-line interface for the Rust runtime

use clap::{Parser, Subcommand};
use std::sync::Arc;
use ckp_core::drivers::{JenaStorage, NatsTransport};

#[derive(Parser)]
#[command(name = "ckp")]
#[command(version = "1.3.20-alpha.1")]
#[command(about = "ConceptKernel Rust Runtime", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage concepts (list, create, load, unload, start, stop, export, cache)
    Concept {
        #[command(subcommand)]
        command: ConceptCommands,
    },
    /// Manage projects (list, create, current, switch, remove)
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
    /// Manage edges (list, create)
    Edge {
        #[command(subcommand)]
        command: EdgeCommands,
    },
    /// Manage workflows (add, list, delete, apply, validate)
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommands,
    },
    /// Manage kernels (list)
    Kernel {
        #[command(subcommand)]
        command: KernelCommands,
    },
    /// Manage transactions (list, show, run workflows)
    Tx {
        #[command(subcommand)]
        command: TxCommands,
    },
    /// Manage packages (list, import, fork)
    Package {
        #[command(subcommand)]
        command: TopLevelPackageCommands,
    },
    /// Start all concepts in the project
    Up,
    /// Stop all running concepts in the project
    Down,
    /// Show status of all concepts
    Status {
        /// Show extended information including tool paths
        #[arg(long, short = 'w')]
        wide: bool,
    },
    /// Emit an event to a concept
    Emit {
        /// Target concept name or URN
        target: String,
        /// Payload (JSON string)
        payload: String,
    },
    /// Validate a URN
    ValidateUrn {
        /// URN to validate
        urn: String,
    },
    /// Fork a cached package to create new kernel
    Fork {
        /// Source package name (e.g., System.Gateway.HTTP)
        source: String,
        /// New kernel name
        #[arg(long)]
        name: String,
        /// Remove runtime data (queues/storage/tx/consensus/logs)
        #[arg(long)]
        clean: bool,
        /// Create git tag after fork
        #[arg(long)]
        tag: Option<String>,
        /// Don't auto-start after forking
        #[arg(long)]
        no_start: bool,
    },
    /// System daemons (governor, edge-router)
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },
    /// Query resources by URN
    Query {
        /// URN with query parameters (e.g., "ckp://Process?limit=10&order=desc")
        urn: String,
        /// Output format (table, json, yaml)
        #[arg(long, default_value = "table")]
        format: String,
    },
    /// Dynamic kernel command (e.g., role, user, provider)
    #[command(external_subcommand)]
    Dynamic(Vec<String>),
}

#[derive(Subcommand)]
enum DaemonCommands {
    /// Start governor daemon for a kernel
    Governor {
        /// Kernel name
        #[arg(long)]
        kernel: String,
        /// Project root directory
        #[arg(long, default_value = ".")]
        project: std::path::PathBuf,
        /// Storage mode: file (default) or nats (ephemeral)
        #[arg(long, default_value = "file")]
        storage: String,
        /// NATS URL (required if storage=nats)
        #[arg(long)]
        nats_url: Option<String>,
        /// Enable verbose logging
        #[arg(long, short = 'v')]
        verbose: bool,
    },
    /// Start edge router daemon
    EdgeRouter {
        /// Project root directory
        #[arg(long, default_value = ".")]
        project: std::path::PathBuf,
        /// Enable verbose logging
        #[arg(long, short = 'v')]
        verbose: bool,
    },
}

#[derive(Subcommand)]
enum ConceptCommands {
    /// List loaded concepts (in concepts/ directory)
    List,
    /// Create a new concept from template
    Create {
        /// Concept name (e.g., MyDomain.MyKernel)
        name: String,
        /// Template type (node:cold, node:hot, python:cold, python:hot)
        #[arg(long, short, default_value = "node:cold")]
        template: String,
        /// Version tag (e.g., v0.1)
        #[arg(long, short, default_value = "v0.1")]
        version: String,
    },
    /// Load a concept from cache to concepts/
    Load {
        /// Concept name (e.g., System.Gateway.HTTP)
        name: String,
        /// Version (e.g., v1.3.14) - optional if only one version exists
        #[arg(long, short)]
        version: Option<String>,
        /// Filter by architecture (e.g., aarch64-darwin, x86_64-linux, universal)
        #[arg(long)]
        arch: Option<String>,
        /// Filter by runtime (e.g., rs, py, js)
        #[arg(long)]
        runtime: Option<String>,
        /// Optional instance name (e.g., --as mykernel.custom)
        #[arg(long)]
        as_name: Option<String>,
    },
    /// Unload a concept from concepts/ (keeps in cache)
    Unload {
        /// Concept name
        name: String,
    },
    /// Start a concept instance
    Start {
        /// Concept name or URN
        name: String,
        /// Optional instance name (e.g., --as mykernel.custom)
        #[arg(long)]
        as_name: Option<String>,
        /// Run in foreground and watch (default: runs in background)
        #[arg(long)]
        watch: bool,
    },
    /// Stop a concept instance
    Stop {
        /// Concept name or URN
        name: String,
    },
    /// Export a concept to cache as tar.gz
    Export {
        /// Concept name
        name: String,
        /// Version to tag
        #[arg(long, short, default_value = "v0.1")]
        version: String,
    },
    /// Manage concept packages (list, import, unload)
    Package {
        #[command(subcommand)]
        command: PackageCommands,
    },
    /// Build Rust kernels using ontology metadata
    Build {
        /// Optional kernel name (builds all if omitted)
        #[arg(long)]
        kernel: Option<String>,
        /// Build in release mode
        #[arg(long)]
        release: bool,
        /// Check if rebuild needed before building
        #[arg(long)]
        incremental: bool,
    },
}

#[derive(Subcommand)]
enum PackageCommands {
    /// List all cached concept packages
    List,
    /// Import a tar.gz package to cache
    Import {
        /// Path to .tar.gz file
        file: String,
    },
    /// Unload a package from concepts/ directory (keeps in cache)
    Unload {
        /// Package name:version (e.g., System.Gateway.HTTP:v0.1)
        name_version: String,
    },
}

#[derive(Subcommand)]
enum ProjectCommands {
    /// List all registered projects
    List {
        /// Show detailed information
        #[arg(long, short)]
        verbose: bool,
    },
    /// Create/register a new project
    Create {
        /// Optional path (. or /path/to/folder). If omitted, creates in ~/.config/conceptkernel/projects/<name>
        path: Option<String>,
        /// Force re-registration if already registered
        #[arg(long)]
        force: bool,
    },
    /// Show current project
    Current,
    /// Switch current project (updates registry marker)
    Switch {
        /// Project name
        name: String,
    },
    /// Remove project from registry
    Remove {
        /// Project name
        name: String,
    },
}

#[derive(Subcommand)]
enum EdgeCommands {
    /// List edges (optionally for specific concept)
    List {
        /// Optional concept name
        concept: Option<String>,
    },
    /// Create an edge between concepts
    Create {
        /// Edge predicate (PRODUCES, VALIDATES, etc.)
        predicate: String,
        /// Source concept
        source: String,
        /// Target concept
        target: String,
    },
}

#[derive(Subcommand)]
enum KernelCommands {
    /// List all kernels from Jena
    List,
}

#[derive(Subcommand)]
enum WorkflowCommands {
    /// Add workflow from CKDL file
    Add {
        /// Path to CKDL file
        file: String,
    },
    /// List all workflows
    List,
    /// Delete a workflow
    Delete {
        /// Workflow URN
        urn: String,
    },
    /// Apply/execute a workflow
    Apply {
        /// Workflow URN
        urn: String,
    },
    /// Validate workflow structure
    Validate {
        /// Workflow URN
        urn: String,
    },
}

#[derive(Subcommand)]
enum TxCommands {
    /// Execute a workflow (creates a transaction)
    Run {
        /// Workflow URN
        workflow_urn: String,
        /// Input parameters as JSON
        #[arg(long)]
        input: String,
        /// Timeout in seconds (default: 30)
        #[arg(long, default_value = "30")]
        timeout: u64,
    },
    /// List all transactions
    List {
        /// Filter by workflow URN
        #[arg(long)]
        workflow: Option<String>,
        /// Limit number of results
        #[arg(long, default_value = "20")]
        limit: usize,
    },
    /// Show detailed occurrents for a specific transaction
    Show {
        /// Transaction ID
        tx_id: String,
        /// Output format (table, json, timeline)
        #[arg(long, default_value = "timeline")]
        format: String,
    },
}

#[derive(Subcommand)]
enum TopLevelPackageCommands {
    /// List all cached packages
    List,
    /// Import a tar.gz package to cache
    Import {
        /// Path to .tar.gz file
        file: String,
    },
    /// Fork a cached package to create new kernel
    Fork {
        /// Source package name (e.g., System.Gateway.HTTP)
        source: String,
        /// New kernel name
        #[arg(long)]
        name: String,
        /// Remove runtime data (queues/storage/tx/consensus/logs)
        #[arg(long)]
        clean: bool,
        /// Create git tag after fork
        #[arg(long)]
        tag: Option<String>,
        /// Don't auto-start after forking
        #[arg(long)]
        no_start: bool,
    },
}

/// Handle `ckr stop <kernel>` command
async fn handle_stop(kernel: &str) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::KernelManager;

    println!("Stopping kernel: {}", kernel);

    // Find project root using registry if not in a project directory
    let root = match resolve_project_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Run 'ckp project list' to see available projects or cd into a project directory");
            std::process::exit(1);
        }
    };
    let manager = KernelManager::new(root)?;

    let stopped = manager.stop_kernel(kernel).await?;

    if stopped {
        println!("✓ Kernel '{}' stopped successfully", kernel);
    } else {
        println!("Kernel '{}' was not running", kernel);
    }

    Ok(())
}

/// Handle `ckr status` command
async fn handle_status(wide: bool) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::{ContinuantTracker, PortManager, KernelManager, BfoAligned, ProjectRegistry};
    use std::path::PathBuf;

    // Get project root
    let mut root = std::env::current_dir()?;
    if !root.join(".ckproject").exists() {
        let mut registry = ProjectRegistry::new()?;
        if let Some(current_name) = registry.get_current_name()? {
            if let Some(project) = registry.get(&current_name)? {
                root = PathBuf::from(&project.path);
            }
        }
    }

    // Initialize Phase 4 components
    let continuant_tracker = ContinuantTracker::new(root.join("concepts"));
    let port_manager = PortManager::new(root.clone())?;
    let kernel_manager = KernelManager::new(root.clone())?;

    // Get all kernels from KernelManager
    let kernel_names = kernel_manager.list_kernels()?;

    if kernel_names.is_empty() {
        println!("No kernels found.");
        return Ok(());
    }

    let mut rows = Vec::new();

    for kernel_name in kernel_names {
        // Try to get BFO entity first
        let entity_opt = continuant_tracker.get_kernel_entity(&kernel_name).ok();

        // Get runtime status
        let status = match kernel_manager.get_kernel_status(&kernel_name).await {
            Ok(s) => s,
            Err(_) => continue, // Skip if can't get status
        };

        // Get port from PortManager
        let port = port_manager.get(&kernel_name)
            .map(|p| p.to_string())
            .unwrap_or_default();

        // Governor PID from status
        let gov_pid = status.watcher_pid
            .map(|p| p.to_string())
            .unwrap_or_default();

        // Tool PID from status (for hot kernels)
        let tool_pid = status.pid
            .map(|p| p.to_string())
            .unwrap_or_default();

        // BFO type and tool path from entity if available
        let (bfo_type, tool_path) = if let Some(ref entity) = entity_opt {
            let bfo = entity.bfo_label().to_string();
            let tool = if wide {
                Some(resolve_tool_path_from_entity(entity)?)
            } else {
                None
            };
            (bfo, tool)
        } else {
            let tool = if wide {
                Some(resolve_tool_path(&root, &kernel_name, &status.kernel_type)?)
            } else {
                None
            };
            ("Material Entity".to_string(), tool)
        };

        // Get version from git if repository exists (display version with hash)
        let version = {
            use ckp_core::GitDriver;
            let kernel_path = root.join("concepts").join(&kernel_name);
            if kernel_path.join(".git").exists() {
                let git_driver = GitDriver::new(kernel_path, kernel_name.clone());
                git_driver.get_current_version()
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "-".to_string())
            } else {
                "-".to_string()
            }
        };

        rows.push(StatusRow {
            name: kernel_name,
            kernel_type: status.kernel_type.clone(),
            gov_pid,
            mode: status.mode.clone(),
            port,
            tool_pid,
            version,
            tool_path,
            bfo_type,
        });
    }

    // Print table
    print_status_table(rows, wide);

    Ok(())
}

/// Status row data structure
#[derive(Debug)]
struct StatusRow {
    name: String,
    kernel_type: String,
    gov_pid: String,       // Governor/CK daemon PID
    mode: String,
    port: String,
    tool_pid: String,      // Hot tool process PID
    version: String,       // Display version (e.g., v0.1, v0.2.3-gab12cd)
    tool_path: Option<String>,
    bfo_type: String,
}

/// Resolve tool path from KernelEntity
fn resolve_tool_path_from_entity(entity: &ckp_core::KernelEntity) -> Result<String, Box<dyn std::error::Error>> {
    // Extract from metadata if available (should be absolute)
    if let Some(tool_path) = entity.metadata.get("tool_path") {
        if let Some(path_str) = tool_path.as_str() {
            return Ok(path_str.to_string());
        }
    }

    // Fall back to convention-based resolution
    // Note: This shouldn't happen in practice, but if it does, return relative path
    let tool_subdir = if entity.kernel_type.starts_with("rust:") {
        "rs"
    } else if entity.kernel_type.starts_with("node:") {
        "js"
    } else if entity.kernel_type.starts_with("python:") {
        "py"
    } else {
        "unknown"
    };

    Ok(format!("concepts/{}/tool/{}", entity.kernel_name, tool_subdir))
}

/// Resolve tool path from kernel name and type (filesystem fallback)
fn resolve_tool_path(
    root: &std::path::Path,
    kernel_name: &str,
    kernel_type: &str
) -> Result<String, Box<dyn std::error::Error>> {
    let tool_subdir = if kernel_type.contains("Rust") || kernel_type.contains("rust") {
        "rs"
    } else if kernel_type.contains("Node") || kernel_type.contains("node") {
        "js"
    } else if kernel_type.contains("Python") || kernel_type.contains("python") {
        "py"
    } else {
        "unknown"
    };

    let tool_path = root
        .join("concepts")
        .join(kernel_name)
        .join("tool")
        .join(tool_subdir);

    // Return absolute path so user can verify which binary is running
    Ok(tool_path.display().to_string())
}

/// Print status table
fn print_status_table(rows: Vec<StatusRow>, wide: bool) {
    if wide {
        println!("\n{:<32} {:<16} {:<8} {:<8} {:<8} {:<10} {:<18} {:<18} {:<50}",
            "NAME", "TYPE", "GOV_PID", "MODE", "PORT", "TOOL_PID", "VERSION", "BFO TYPE", "TOOL PATH");
        println!("{}", "-".repeat(170));

        for row in rows {
            println!("{:<32} {:<16} {:<8} {:<8} {:<8} {:<10} {:<18} {:<18} {:<50}",
                row.name,
                row.kernel_type,
                row.gov_pid,
                row.mode,
                row.port,
                row.tool_pid,
                row.version,
                row.bfo_type,
                row.tool_path.unwrap_or_default()
            );
        }
    } else {
        println!("\n{:<32} {:<16} {:<8} {:<8} {:<8} {:<10} {:<18} {:<18}",
            "NAME", "TYPE", "GOV_PID", "MODE", "PORT", "TOOL_PID", "VERSION", "BFO TYPE");
        println!("{}", "-".repeat(118));

        for row in rows {
            println!("{:<32} {:<16} {:<8} {:<8} {:<8} {:<10} {:<18} {:<18}",
                row.name,
                row.kernel_type,
                row.gov_pid,
                row.mode,
                row.port,
                row.tool_pid,
                row.version,
                row.bfo_type
            );
        }
    }

    println!();
}

/// Calculate kernel mode based on type and PID
///
/// Logic:
/// - node:hot/python:hot/rust:hot with PID = ONLINE
/// - node:hot/python:hot/rust:hot without PID = DOWN (service is not running)
/// - node:cold/python:cold/rust:cold with PID = IDLE (processing)
/// - node:cold/python:cold/rust:cold without PID = SLEEP (awaiting work)
/// - Unknown type = SLEEP
fn calculate_mode(kernel_type: &str, pid: Option<u32>) -> String {
    match (kernel_type, pid) {
        // Hot kernels (services that should be running)
        (t, Some(_)) if t.contains(":hot") => "ONLINE".to_string(),
        (t, None) if t.contains(":hot") => "DOWN".to_string(),

        // Cold kernels (on-demand processes)
        (t, Some(_)) if t.contains(":cold") => "IDLE".to_string(),
        (t, None) if t.contains(":cold") => "SLEEP".to_string(),

        // Manual or unknown
        (_, None) => "SLEEP".to_string(),
        (_, Some(_)) => "IDLE".to_string(),
    }
}

/// Handle `ckr emit <target> <payload>` command
async fn handle_emit(target: &str, payload_str: &str) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::{Kernel, ProjectRegistry};
    use serde_json::Value;
    use std::path::PathBuf;

    println!("Emitting to {}", target);

    // Try current directory first
    let mut root = std::env::current_dir()?;

    // If not in a project, use current project from registry
    if !root.join(".ckproject").exists() {
        let mut registry = ProjectRegistry::new()?;
        if let Some(current_name) = registry.get_current_name()? {
            if let Some(project) = registry.get(&current_name)? {
                root = PathBuf::from(&project.path);
            }
        } else {
            return Err("Error: Not in a ConceptKernel project and no current project set".into());
        }
    }

    // Parse payload JSON
    let payload: Value = serde_json::from_str(payload_str)?;

    // Create kernel instance (using project root as concepts root)
    // Don't bootstrap - emit anonymously from CLI (no source kernel)
    let mut kernel = Kernel::new(root.clone(), None, false);

    // Emit to target (without bootstrap, kernel.emit skips RBAC and emits anonymously)
    let tx_id = kernel.emit(target, payload).await?;

    println!("✓ Event emitted successfully");
    println!("  Transaction ID: {}", tx_id);
    println!("  Target: {}", target);

    Ok(())
}

/// Find project root by searching upward for .ckproject file
///
/// Starts from current directory and searches parent directories
/// until .ckproject is found or filesystem root is reached.
fn find_project_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let mut current = std::env::current_dir()?;

    loop {
        let ckproject_path = current.join(".ckproject");
        if ckproject_path.exists() {
            return Ok(current);
        }

        // Move to parent directory
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => {
                return Err(format!(
                    "Could not find .ckproject in current directory or any parent directory.\n\
                     Please run this command from within a ConceptKernel project."
                ).into());
            }
        }
    }
}

fn determine_current_kernel(root: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    // Try to find .ckproject file
    let project_file = root.join(".ckproject");
    if project_file.exists() {
        use ckp_core::ProjectConfig;
        let config = ProjectConfig::load(&project_file)?;
        return Ok(config.metadata.name);
    }

    // Otherwise, assume we're in concepts/<kernel>/ directory
    if let Some(file_name) = root.file_name() {
        return Ok(file_name.to_string_lossy().to_string());
    }

    Err("Could not determine current kernel".into())
}

/// Resolve project root using CK Core API conventions
fn resolve_project_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    use ckp_core::ProjectRegistry;
    use std::path::PathBuf;

    // Try current directory first
    let cwd = std::env::current_dir()?;

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
    if let Ok(project_root) = std::env::var("CK_PROJECT_ROOT") {
        let path = PathBuf::from(project_root);
        if path.join(".ckproject").exists() {
            return Ok(path);
        }
    }

    // Fall back to current project from registry
    let mut registry = ProjectRegistry::new()?;
    if let Some(current_name) = registry.get_current_name()? {
        if let Some(project) = registry.get(&current_name)? {
            return Ok(PathBuf::from(&project.path));
        }
    }

    Err("Error: Not in a ConceptKernel project and no current project set".into())
}

/// Handle `ckr create-edge <predicate> <source> <target>` command
async fn handle_create_edge(predicate: &str, source: &str, target: &str) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::EdgeKernel;
    use ckp_core::drivers::{JenaStorage, StorageDriver};
    use std::sync::Arc;

    println!("Creating edge: {} --{}--> {}", source, predicate, target);

    let root = std::env::current_dir()?;
    let mut edge_kernel = EdgeKernel::new(root.clone())?;

    let edge_metadata = edge_kernel.create_edge(predicate, source, target)?;

    println!("✓ Edge created in YAML");
    println!("  URN: {}", edge_metadata.urn);

    // DISKLESS MODE: Also save to Jena per .ckproject settings
    println!("Saving edge to Jena (diskless mode)...");

    // Read .ckproject for Jena settings
    let ckproject_path = root.join(".ckproject");
    if ckproject_path.exists() {
        let content = std::fs::read_to_string(&ckproject_path)?;
        let yaml: serde_yaml::Value = serde_yaml::from_str(&content)?;

        if let Some(backends) = yaml.get("spec").and_then(|s| s.get("backends")) {
            if let Some(jena_config) = backends.get("jena") {
                let endpoint = jena_config.get("endpoint")
                    .and_then(|e| e.as_str())
                    .ok_or("Missing Jena endpoint in .ckproject")?;
                let dataset = jena_config.get("dataset")
                    .and_then(|d| d.as_str())
                    .ok_or("Missing Jena dataset in .ckproject")?;
                let username = jena_config.get("username")
                    .and_then(|u| u.as_str())
                    .ok_or("Missing Jena username in .ckproject")?;
                let password = jena_config.get("password")
                    .and_then(|p| p.as_str())
                    .ok_or("Missing Jena password in .ckproject")?;

                // Create Jena storage driver
                let storage = JenaStorage::new_with_auth(
                    endpoint.to_string(),
                    dataset.to_string(),
                    username.to_string(),
                    password.to_string(),
                );

            // Save edge metadata to Jena
            let metadata_json = serde_json::json!({
                "created": edge_metadata.created_at,
                "version": edge_metadata.version
            });

            // Use SPARQL UPDATE instead of /data endpoint (more reliable)
            let urn = edge_metadata.urn.clone();
            let graph_uri = format!("ckp://edges/{}/{}-to-{}",
                predicate, source, target
            );

            let sparql_update = format!(r#"
PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
  GRAPH <{graph_uri}> {{
    <{urn}> a ckp:EdgeConnection ;
        ckp:hasURN "{urn}" ;
        ckp:hasPredicate ckp:Edge-{predicate} ;
        ckp:hasSource ckp:Kernel-{source} ;
        ckp:hasTarget ckp:Kernel-{target} ;
        ckp:version "{version}"^^xsd:string ;
        ckp:createdAt "{created_at}"^^xsd:dateTime ;
        ckp:status "active"^^xsd:string .

    ckp:Kernel-{source} ckp:hasName "{source}" .
    ckp:Kernel-{target} ckp:hasName "{target}" .
    ckp:Edge-{predicate} ckp:predicateName "{predicate}" .
  }}
}}"#,
                graph_uri = graph_uri,
                urn = urn,
                predicate = predicate,
                source = source,
                target = target,
                version = edge_metadata.version,
                created_at = edge_metadata.created_at,
            );

                storage.execute_sparql_update(&sparql_update).await
                    .map_err(|e| format!("Failed to save edge to Jena: {}", e))?;

                println!("✓ Edge saved to Jena via SPARQL UPDATE");
            }
        }
    }

    Ok(())
}

/// Handle `ckr list-edges` command
fn handle_list_edges() -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::EdgeKernel;
    use std::fs;

    println!("Listing all edges...\n");

    // Find project root
    let root = match resolve_project_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Run 'ckp project list' to see available projects or cd into a project directory");
            std::process::exit(1);
        }
    };

    // Read edge-router daemon PID if available
    let pid_file = root.join(".edge-router.pid");
    let router_pid = if pid_file.exists() {
        fs::read_to_string(&pid_file)
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
    } else {
        None
    };

    let mut edge_kernel = EdgeKernel::new(root)?;

    let edges = edge_kernel.list_edges()?;

    if edges.is_empty() {
        println!("No edges found.");
        return Ok(());
    }

    // Print header
    println!("{:<70} {:<8} {}", "URN", "PID", "STATUS");
    println!("{}", "-".repeat(90));

    let total = edges.len();
    for edge_urn in edges {
        let pid_display = router_pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
        let status = if router_pid.is_some() { "ROUTING" } else { "NO_DAEMON" };
        println!("{:<70} {:<8} {}", edge_urn, pid_display, status);
    }

    println!("\nTotal: {} edge(s)", total);
    if let Some(pid) = router_pid {
        println!("Edge router daemon PID: {}", pid);
    } else {
        println!("No edge router daemon running (start with: ck daemon edge-router)");
    }

    Ok(())
}

/// Handle `ckr edges <kernel>` command
fn handle_edges(kernel: &str) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::EdgeKernel;

    println!("Showing edges for kernel: {}\n", kernel);

    let root = std::env::current_dir()?;
    let mut edge_kernel = EdgeKernel::new(root)?;

    let edges = edge_kernel.get_kernel_edges(kernel)?;

    if edges.is_empty() {
        println!("No edges found for kernel '{}'.", kernel);
        return Ok(());
    }

    println!("Outgoing edges:");
    for edge_urn in &edges {
        println!("  → {}", edge_urn);
    }

    println!("\nTotal: {} edge(s)", edges.len());

    Ok(())
}

/// Check if project has running concepts by scanning live processes
fn check_project_status(project_path: &str) -> String {
    use ckp_core::KernelManager;
    use std::path::PathBuf;
    use std::process::Command;

    let path = PathBuf::from(project_path);

    // Get list of concepts in this project
    let kernels = if let Ok(manager) = KernelManager::new(path.clone()) {
        manager.list_kernels().unwrap_or_default()
    } else {
        return "SHUTDOWN".to_string();
    };

    if kernels.is_empty() {
        return "SHUTDOWN".to_string();
    }

    // Scan live processes - no PID files needed!
    #[cfg(unix)]
    {
        if let Ok(output) = Command::new("ps")
            .args(&["aux"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let project_path_str = path.to_string_lossy();

            for kernel in &kernels {
                // Pattern 1: Tool process
                let tool_pattern = format!("{}/concepts/{}/tool/", project_path_str, kernel);
                // Pattern 2: Governor process - {project}/core/src/ConceptKernel.js {kernel}
                let governor_pattern = format!("{}/core/src/ConceptKernel.js {}", project_path_str, kernel);
                // Pattern 3: New unified daemon governor - ckr daemon governor --kernel {kernel}
                let daemon_governor_pattern = format!("ckr daemon governor --kernel {}", kernel);
                // Pattern 4: Old rust governor (backward compat) - ckr-governor --kernel {kernel} --project {project}
                let old_rust_governor_pattern = format!("ckr-governor --kernel {} --project {}", kernel, project_path_str);

                if stdout.contains(&tool_pattern) || stdout.contains(&governor_pattern)
                    || stdout.contains(&daemon_governor_pattern) || stdout.contains(&old_rust_governor_pattern) {
                    return "ONLINE".to_string();
                }
            }
        }
    }

    #[cfg(not(unix))]
    {
        // Windows fallback - check PID files only
        if let Ok(manager) = KernelManager::new(path) {
            for kernel in &kernels {
                if let Ok(pids) = manager.find_running_pids(kernel) {
                    if pids.pid.is_some() || pids.watcher_pid.is_some() {
                        return "ONLINE".to_string();
                    }
                }
            }
        }
    }

    "SHUTDOWN".to_string()
}

/// Handle `ckr project list` command
fn handle_projects_list(verbose: bool) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::ProjectRegistry;

    let mut registry = ProjectRegistry::new()?;
    let projects = registry.list()?;

    if projects.is_empty() {
        println!("No projects registered.");
        println!("\nRun `ckr project create` to create a new project.");
        return Ok(());
    }

    // Get current project by name
    let current_name = registry.get_current_name()?;

    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║  ConceptKernel Multi-Project Registry                     ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    for (index, project) in projects.iter().enumerate() {
        let is_current = current_name.as_ref().map(|n| n == &project.name).unwrap_or(false);
        let marker = if is_current { "→" } else { " " };

        // Check status
        let status = check_project_status(&project.path);

        println!("{} {} ({})", marker, project.name, status);
        println!("  CK Version:     {}", project.version);
        println!("  Discovery Port: {}", project.discovery_port);
        println!("  Port Range:     {}-{}", project.port_range.start, project.port_range.end);
        println!("  Slot:           {}", project.slot);

        if verbose {
            println!("  Project ID:     {}", project.id);
            println!("  Registered:     {}", project.registered_at);
        }

        println!("  Path:           {}", project.path);

        if index < projects.len() - 1 {
            println!();
        }
    }

    println!("\nTotal: {} project(s)", projects.len());

    if let Some(current) = current_name {
        println!("Current: {}\n", current);
    } else {
        println!("Current: (none)\n");
    }

    Ok(())
}

/// Handle `ckr projects current` command
fn handle_projects_current() -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::ProjectRegistry;

    let mut registry = ProjectRegistry::new()?;

    // Get current project from registry marker
    if let Some(current_name) = registry.get_current_name()? {
        if let Some(project) = registry.get(&current_name)? {
            println!("\nCurrent Project:");
            println!("  Name:           {}", project.name);
            println!("  CK Version:     {}", project.version);
            println!("  Discovery Port: {}", project.discovery_port);
            println!("  Port Range:     {}-{}", project.port_range.start, project.port_range.end);
            println!("  Slot:           {}", project.slot);
            println!("  Path:           {}\n", project.path);
            return Ok(());
        }
    }

    println!("No current project set.");
    println!("\nUse `ckr project switch <name>` to set a current project.");

    Ok(())
}

/// Handle `ckr projects remove <name>` command
fn handle_projects_remove(project_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::ProjectRegistry;

    let mut registry = ProjectRegistry::new()?;
    let project = registry.get(project_name)?;

    if project.is_none() {
        eprintln!("Project \"{}\" not found in registry.", project_name);
        println!("\nAvailable projects:");
        let projects = registry.list()?;
        for p in projects {
            println!("  - {}", p.name);
        }
        std::process::exit(1);
    }

    let project = project.unwrap();
    let removed = registry.remove(project_name)?;

    if removed {
        println!("✓ Removed project \"{}\" from registry", project_name);
        println!("  (Project files at {} are unchanged)", project.path);
    } else {
        eprintln!("Failed to remove project");
        std::process::exit(1);
    }

    Ok(())
}

/// Generate a unique project name
fn generate_project_name() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    let suffix: String = (0..5)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    format!("concept-kernel-project-{}", suffix)
}

/// Handle `ckr project create [path]` command with optional path
async fn handle_init_with_path(path: Option<String>, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::{ProjectConfig, ProjectRegistry, ProjectInfo, PortManager};
    use std::path::PathBuf;
    use std::fs;

    let (target_dir, project_name, project_id, is_new_project) = if let Some(p) = path {
        // Path provided - create or register project at this location
        let dir = if p == "." {
            std::env::current_dir()?
        } else {
            PathBuf::from(p)
        };

        let project_file = dir.join(".ckproject");

        if project_file.exists() {
            // Existing project - load and register
            let config = ProjectConfig::load(&project_file)?;
            (dir, config.metadata.name.clone(), config.metadata.id.clone(), false)
        } else {
            // New project - create structure
            fs::create_dir_all(&dir)?;
            fs::create_dir_all(dir.join("concepts"))?;

            // Generate project ID and use directory name as project name
            let id = uuid::Uuid::new_v4().to_string();
            let name = dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed-project")
                .to_string();

            // Create .ckproject file
            let project_config = ProjectConfig {
                api_version: "conceptkernel/v1".to_string(),
                kind: "Project".to_string(),
                metadata: ckp_core::project::config::Metadata {
                    name: name.clone(),
                    id: id.clone(),
                },
                spec: ckp_core::project::config::Spec {
                    domain: "ConceptKernel".to_string(),
                    version: ckp_core::VERSION.to_string(),
                    ports: None,
                    features: None,
                    protocol: None,
                    default_user: None,
                    ontology: None,
                    backends: None,
                },
            };

            project_config.save(&project_file)?;

            (dir, name, id, true)
        }
    } else {
        // No path - create new project in ~/.config/conceptkernel/projects/
        let home_dir = std::env::var("HOME").map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "HOME environment variable not set")
        })?;

        let projects_base = PathBuf::from(home_dir)
            .join(".config")
            .join("conceptkernel")
            .join("projects");

        // Generate unique project name
        let name = generate_project_name();
        let dir = projects_base.join(&name);

        // Create directory
        fs::create_dir_all(&dir)?;

        // Create concepts directory
        fs::create_dir_all(dir.join("concepts"))?;

        // Generate project ID
        let id = uuid::Uuid::new_v4().to_string();

        // Create .ckproject file
        let project_config = ProjectConfig {
            api_version: "conceptkernel/v1".to_string(),
            kind: "Project".to_string(),
            metadata: ckp_core::project::config::Metadata {
                name: name.clone(),
                id: id.clone(),
            },
            spec: ckp_core::project::config::Spec {
                domain: "ConceptKernel".to_string(),
                version: ckp_core::VERSION.to_string(),
                ports: None,
                features: None,
                protocol: None,
                default_user: None,
                ontology: None,
                backends: None,
            },
        };

        let project_file = dir.join(".ckproject");
        project_config.save(&project_file)?;

        (dir, name, id, true)
    };

    // Get CK version
    let ck_version = ckp_core::VERSION.to_string();

    let mut registry = ProjectRegistry::new()?;

    // Check if already registered
    if let Some(existing) = registry.get(&project_name)? {
        if !force {
            println!("Project \"{}\" is already registered.", project_name);
            println!("  Discovery Port: {}", existing.discovery_port);
            println!("  Path: {}", existing.path);
            println!("\nUse --force to re-register.");
            return Ok(());
        }

        // Remove existing if force
        registry.remove(&project_name)?;
        println!("Removing existing registration for \"{}\"...", project_name);
    }

    // Register project
    if !is_new_project {
        println!("Registering project \"{}\"...", project_name);
    }

    let project = registry.register(ProjectInfo {
        name: project_name.clone(),
        id: project_id,
        path: target_dir.to_string_lossy().to_string(),
        version: ck_version,
        preferred_slot: None,
    })?;

    // Create .ckports file for PortManager
    let mut port_manager = PortManager::new(&target_dir)?;
    port_manager.set_base_port(project.discovery_port)?;

    // Update .ckproject with port configuration for discovery
    let project_file = target_dir.join(".ckproject");
    let mut config = ProjectConfig::load(&project_file)?;
    config.spec.ports = Some(ckp_core::project::config::PortConfig {
        base_port: project.discovery_port,
        slot: project.slot,
    });
    config.save(&project_file)?;

    // Set as current project
    registry.set_current(&project.name)?;

    if is_new_project {
        println!("✓ {} (slot {}, port {})", project.name, project.slot, project.discovery_port);
        println!("\n→ cd ~/.config/conceptkernel/projects/{}\n", project.name);
    } else {
        println!("\n✓ Project registered successfully!");
        println!("  Name:           {}", project.name);
        println!("  Slot:           {}", project.slot);
        println!("  Discovery Port: {}", project.discovery_port);
        println!("  Port Range:     {}-{}", project.port_range.start, project.port_range.end);
        println!("  Path:           {}", project.path);
        println!("\nYou can now start your kernels with `ckr concept start <kernel>`\n");
    }

    Ok(())
}

/// Handle `ckr project switch <name>` command
fn handle_project_switch(project_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::ProjectRegistry;

    let mut registry = ProjectRegistry::new()?;
    let project = registry.get(project_name)?;

    if project.is_none() {
        eprintln!("Project \"{}\" not found in registry.", project_name);
        println!("\nAvailable projects:");
        let projects = registry.list()?;
        for p in projects {
            println!("  - {}", p.name);
        }
        std::process::exit(1);
    }

    let project = project.unwrap();

    // Update current project in registry
    registry.set_current(&project.name)?;

    // Show simple cd command
    println!("✓ Switched to: {}", project.name);
    println!("\ncd {}\n", project.path);

    Ok(())
}

/// Print custom help with dynamic kernel commands
fn print_custom_help() {
    use ckp_core::{OntologyReader, ProjectRegistry};
    use std::path::PathBuf;

    // Print standard help
    println!("ConceptKernel Rust Runtime\n");
    println!("Usage: ck <COMMAND>\n");
    println!("Commands:");
    println!("  concept       Manage concepts (list, create, load, unload, start, stop, export, cache)");
    println!("  project       Manage projects (list, create, current, switch, remove)");
    println!("  edge          Manage edges (list, create)");
    println!("  package       Manage packages (list, import, fork)");
    println!("  up            Start all concepts in the project");
    println!("  down          Stop all running concepts in the project");
    println!("  status        Show status of all concepts");
    println!("  emit          Emit an event to a concept");
    println!("  query         Query resources by URN (e.g., ckp://Process?limit=10)");
    println!("  validate-urn  Validate a URN");
    println!("  help          Print this message or the help of the given subcommand(s)");

    // Try to find project and show dynamic commands
    let mut root = std::env::current_dir().ok();

    if let Some(r) = &root {
        if !r.join(".ckproject").exists() {
            if let Ok(mut registry) = ProjectRegistry::new() {
                if let Ok(Some(current_name)) = registry.get_current_name() {
                    if let Ok(Some(project)) = registry.get(&current_name) {
                        root = Some(PathBuf::from(&project.path));
                    }
                }
            }
        }
    }

    if let Some(root) = root {
        let concepts_dir = root.join("concepts");
        if concepts_dir.exists() {
            let ontology_reader = OntologyReader::new(concepts_dir.clone());
            let mut dynamic_commands = Vec::new();
            let mut seen_commands = std::collections::HashSet::new();

            if let Ok(entries) = std::fs::read_dir(&concepts_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let dir_name = entry.file_name().to_string_lossy().to_string();

                        // Skip instance directories (e.g., System.Oidc.User.1, System.Oidc.User.2)
                        // Instance pattern: ends with .N where N is a number
                        if let Some(last_segment) = dir_name.split('.').last() {
                            if last_segment.parse::<u32>().is_ok() {
                                continue;
                            }
                        }

                        let ontology_path = path.join("conceptkernel.yaml");
                        if ontology_path.exists() {
                            match ontology_reader.read(&ontology_path) {
                                Ok(ontology) => {
                                    if let Some(spec) = &ontology.spec {
                                        if let Some(cli) = &spec.cli {
                                            if cli.expose {
                                                // Only add if we haven't seen this command yet
                                                if seen_commands.insert(cli.primary.clone()) {
                                                    let desc = cli.description.as_deref().unwrap_or("");
                                                    dynamic_commands.push((cli.primary.clone(), desc.to_string(), cli.aliases.clone()));
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(_e) => {
                                    // Silently skip ontologies that fail to parse (they may not have CLI contracts)
                                }
                            }
                        }
                    }
                }
            }

            if !dynamic_commands.is_empty() {
                println!("\nKernel Commands:");
                for (cmd, desc, aliases) in dynamic_commands {
                    println!("  {:<13} {}", cmd, desc);
                    for alias in aliases {
                        println!("    (alias: {})", alias);
                    }
                }
            }
        }
    }

    println!("\nOptions:");
    println!("  -h, --help     Print help");
    println!("  -V, --version  Print version");
}

// ============================================================================
// WORKFLOW EXECUTION HANDLERS
// ============================================================================

async fn handle_workflow_execution(
    workflow: &ckp_core::workflow::Workflow,
    workflow_tx_id: &str,
    input: serde_json::Value,
    timeout_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    use async_nats;
    use futures::StreamExt;
    use std::time::Duration;
    use tokio::time::timeout;
    use ckp_core::drivers::factory::DriverFactory;
    use ckp_core::drivers::storage::OccurrentTracker;

    // Initialize occurrent tracker with storage driver
    // Search upward for .ckproject starting from current directory
    let root = find_project_root().unwrap_or_else(|_| std::env::current_dir().unwrap());
    eprintln!("[DEBUG] Project root: {}", root.display());
    eprintln!("[DEBUG] .ckproject exists: {}", root.join(".ckproject").exists());

    let tracker = match DriverFactory::create_jena_from_project(&root).await {
        Some(jena_storage) => {
            println!("✓ Initialized occurrent tracker with Jena storage");
            Some(OccurrentTracker::new(jena_storage))
        }
        None => {
            eprintln!("[DEBUG] create_jena_from_project returned None!");
            println!("⚠ Jena storage not configured - occurrent tracking disabled");
            None
        }
    };

    // TRACK: Workflow start
    if let Some(ref tracker) = tracker {
        if let Err(e) = tracker.track_workflow_start(&workflow.workflow_urn, workflow_tx_id).await {
            eprintln!("⚠ Failed to track workflow start: {}", e);
        }
    }

    // Connect to NATS
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let client = async_nats::connect(&nats_url).await?;
    println!("✓ Connected to NATS at {}", nats_url);
    println!();

    // Subscribe to ALL NATS event streams for comprehensive monitoring
    // This captures: governor events, edge routing events, workflow events
    let mut event_subs = Vec::new();

    // Subscribe to all ConceptKernel events (governor, edge, workflow)
    match client.subscribe("ckp.events.>").await {
        Ok(sub) => {
            event_subs.push(sub);
            println!("✓ Subscribed to: ckp.events.>");
        }
        Err(e) => {
            eprintln!("⚠ Failed to subscribe to ckp.events.>: {}", e);
        }
    }

    // Subscribe to legacy workflow events
    let workflow_subject = format!("workflow.{}.event", workflow_tx_id);
    match client.subscribe(workflow_subject.clone()).await {
        Ok(sub) => {
            event_subs.push(sub);
            println!("✓ Subscribed to: {}", workflow_subject);
        }
        Err(e) => {
            eprintln!("⚠ Failed to subscribe to {}: {}", workflow_subject, e);
        }
    }

    println!();

    // Find entry kernel (first kernel in workflow)
    let entry_kernel = workflow.phases.first()
        .map(|phase| &phase.kernel_urn)
        .ok_or("Workflow has no entry kernel")?;

    // Publish to entry kernel's inbox using NatsTransport subject format
    // Extract kernel name from full URN (e.g., "ckp://Usecase.SimplePassthrough.Source:v1.0.0" -> "Usecase.SimplePassthrough.Source")
    let kernel_name = entry_kernel
        .trim_start_matches("ckp://")
        .split(':')
        .next()
        .unwrap_or(entry_kernel);

    // NatsTransport uses format: {stream_prefix}.{kernel_name}.inbox (default prefix: "ckp")
    let entry_subject = format!("ckp.{}.inbox", kernel_name);

    // Create message with workflow tracking
    let mut message = if let serde_json::Value::Object(obj) = input {
        obj
    } else {
        serde_json::Map::new()
    };

    message.insert("workflow_tx_id".to_string(), serde_json::Value::String(workflow_tx_id.to_string()));
    message.insert("jwt".to_string(), serde_json::Value::String("anonymous".to_string()));

    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("PUBLISHING INPUT TO ENTRY KERNEL");
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("  Entry Kernel: {}", entry_kernel);
    println!("  Subject: {}", entry_subject);
    println!("  Message: {}", serde_json::to_string_pretty(&message)?);
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!();

    // Publish input to entry kernel
    client.publish(entry_subject.to_string(), serde_json::to_vec(&message)?.into()).await?;

    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("WORKFLOW EXECUTION IN PROGRESS");
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!();

    // Monitor events with timeout - listen to ALL subscriptions
    use futures::stream::select_all;
    use std::time::Instant;

    let event_timeout = Duration::from_secs(timeout_secs);
    let mut event_count = 0;
    let mut workflow_completed = false;
    let mut kernel_step_num: usize = 0;
    let start_time = Instant::now();

    println!("═══════════════════════════════════════════════════════════════════════════");
    println!("LIVE EVENT STREAM (monitoring all kernels and edges)");
    println!("═══════════════════════════════════════════════════════════════════════════");
    println!();

    // Merge all subscriptions into single stream
    let mut combined_stream = select_all(event_subs);

    let monitor_result = timeout(event_timeout, async {
        while let Some(msg) = combined_stream.next().await {
            // Calculate milliseconds since start
            let elapsed_ms = start_time.elapsed().as_millis();

            // Try to parse as JSON event
            if let Ok(event_str) = String::from_utf8(msg.payload.to_vec()) {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&event_str) {
                    event_count += 1;

                    // Extract event details
                    let event_type = event.get("event_type")
                        .or_else(|| event.get("type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    let kernel_urn = event.get("kernel_urn")
                        .or_else(|| event.get("kernel"))
                        .or_else(|| event.get("source"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    let timestamp = event.get("timestamp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    // TRACK: Kernel invocation on "accepted" events
                    if event_type == "accepted" || event_type == "JobReceived" {
                        if let Some(ref tracker) = tracker {
                            let kernel_name = kernel_urn.split('.').last().unwrap_or(kernel_urn);
                            if let Err(e) = tracker.track_kernel_invocation(
                                &workflow.workflow_urn,
                                workflow_tx_id,
                                kernel_name,
                                kernel_step_num
                            ).await {
                                eprintln!("⚠ Failed to track kernel invocation: {}", e);
                            }
                            kernel_step_num += 1;
                        }
                    }

                    // Format event symbol
                    let event_symbol = match event_type {
                        "accepted" | "JobReceived" => "▶",
                        "completed" | "JobCompleted" => "✓",
                        "failed" | "JobFailed" => "✗",
                        "workflow_completed" => "🎉",
                        "StartupReady" => "🚀",
                        "EdgeRouted" => "🔀",
                        _ => "•",
                    };

                    // Print event with millisecond timestamp
                    println!("[{:6}ms] [{}] {} {} - {}",
                        elapsed_ms,
                        event_count,
                        event_symbol,
                        event_type.to_uppercase(),
                        kernel_urn
                    );

                    // Show additional details
                    if let Some(details) = event.get("details") {
                        if let Some(step) = details.get("step").and_then(|v| v.as_str()) {
                            println!("           └─ Step: {}", step);
                        }
                        if let Some(order_id) = details.get("order_id").and_then(|v| v.as_str()) {
                            println!("           └─ Order: {}", order_id);
                        }
                    }

                    // Show job ID if present
                    if let Some(job_id) = event.get("job_id").and_then(|v| v.as_str()) {
                        println!("           └─ Job: {}", job_id);
                    }

                    // Show target kernel for edge routing events
                    if let Some(target) = event.get("target").and_then(|v| v.as_str()) {
                        println!("           └─ Target: {}", target);
                    }

                    // Show timestamp if available
                    if !timestamp.is_empty() {
                        println!("           └─ Timestamp: {}", timestamp);
                    }

                    println!();

                    // Check for workflow completion
                    if event_type == "workflow_completed" {
                        workflow_completed = true;
                        break;
                    }
                } else {
                    // Non-JSON message (binary or raw text)
                    let elapsed_ms = start_time.elapsed().as_millis();
                    println!("[{:6}ms] [{}] • RAW MESSAGE - Subject: {}",
                        elapsed_ms, event_count + 1, msg.subject);
                    if event_str.len() < 200 {
                        println!("           └─ {}", event_str);
                    }
                    println!();
                }
            }
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    }).await;

    match monitor_result {
        Ok(_) => {
            if workflow_completed {
                // TRACK: Workflow complete (success)
                if let Some(ref tracker) = tracker {
                    if let Err(e) = tracker.track_workflow_complete(
                        &workflow.workflow_urn,
                        workflow_tx_id,
                        "success"
                    ).await {
                        eprintln!("⚠ Failed to track workflow completion: {}", e);
                    }
                }

                println!("═══════════════════════════════════════════════════════════════════════════");
                println!("✓ Workflow completed successfully");
                println!("  Total events: {}", event_count);
                println!("═══════════════════════════════════════════════════════════════════════════");
            } else {
                // TRACK: Workflow complete (incomplete - no completion event)
                if let Some(ref tracker) = tracker {
                    if let Err(e) = tracker.track_workflow_complete(
                        &workflow.workflow_urn,
                        workflow_tx_id,
                        "incomplete"
                    ).await {
                        eprintln!("⚠ Failed to track workflow completion: {}", e);
                    }
                }

                println!("═══════════════════════════════════════════════════════════════════════════");
                println!("⚠ Event stream ended (no workflow_completed event)");
                println!("  Total events: {}", event_count);
                println!("═══════════════════════════════════════════════════════════════════════════");
            }
        }
        Err(_) => {
            // TRACK: Workflow complete (timeout/failure)
            if let Some(ref tracker) = tracker {
                if let Err(e) = tracker.track_workflow_complete(
                    &workflow.workflow_urn,
                    workflow_tx_id,
                    "timeout"
                ).await {
                    eprintln!("⚠ Failed to track workflow completion: {}", e);
                }
            }

            println!("═══════════════════════════════════════════════════════════════════════════");
            println!("⚠ Workflow execution timeout ({}s)", timeout_secs);
            println!("  Events received: {}", event_count);
            println!("═══════════════════════════════════════════════════════════════════════════");
        }
    }

    Ok(())
}

async fn handle_list_executions(
    workflow_filter: Option<String>,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::drivers::factory::DriverFactory;

    println!("Listing transactions...");
    if let Some(ref workflow) = workflow_filter {
        println!("  Workflow filter: {}", workflow);
    }
    println!("  Limit: {}", limit);
    println!();

    // Initialize Jena storage using the same pattern as handle_run_workflow
    let root = std::env::current_dir()?;
    let jena_storage = match DriverFactory::create_jena_from_project(&root).await {
        Some(storage) => storage,
        None => {
            println!("⚠ Jena storage not configured in project");
            println!("  Configure Jena in .ckproject to enable transaction queries");
            return Ok(());
        }
    };

    // Query transactions from Jena
    let mut transactions: Vec<serde_json::Value> = if let Some(ref workflow) = workflow_filter {
        jena_storage.query_transactions_by_workflow(workflow).await?
    } else {
        jena_storage.query_transactions().await?
    };

    if transactions.is_empty() {
        println!("No transactions found in Jena.");
        return Ok(());
    }

    // Apply limit by taking only the first N transactions
    transactions.truncate(limit);

    // Display results
    println!("TRANSACTION ID           | WORKFLOW                           | TYPE              | TIMESTAMP");
    println!("-------------------------|---------------------------------------|-------------------|---------------------------");

    for tx in &transactions {
        let tx_id = tx.get("transactionId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let workflow_urn = tx.get("workflowUrn")
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");

        let tx_type = tx.get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let timestamp = tx.get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        println!(
            "{:<24} | {:<37} | {:<17} | {}",
            tx_id.chars().take(24).collect::<String>(),
            workflow_urn.chars().take(37).collect::<String>(),
            tx_type,
            timestamp
        );
    }

    println!();
    println!("Total: {} transaction(s)", transactions.len());
    println!();
    println!("To view details: ck tx show <tx_id>");

    Ok(())
}

async fn handle_show_execution(
    tx_id: &str,
    format: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use async_nats;
    use futures::StreamExt;
    use std::time::Duration;
    use tokio::time::timeout;

    // Connect to NATS
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let client = async_nats::connect(&nats_url).await?;

    println!("Fetching execution details for: {}", tx_id);
    println!();

    // Subscribe to specific workflow events
    let event_subject = format!("workflow.{}.event", tx_id);
    let mut event_sub = client.subscribe(event_subject.clone()).await?;

    println!("Listening for events on: {}", event_subject);
    println!("(Waiting 2 seconds to collect events...)");
    println!();

    // Collect events
    let mut events: Vec<serde_json::Value> = Vec::new();

    let scan_result = timeout(Duration::from_secs(2), async {
        while let Some(msg) = event_sub.next().await {
            if let Ok(event_str) = String::from_utf8(msg.payload.to_vec()) {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&event_str) {
                    events.push(event);
                }
            }
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    }).await;

    drop(scan_result);

    if events.is_empty() {
        println!("No events found for transaction: {}", tx_id);
        println!();
        println!("This execution may be too old, or the transaction ID may be incorrect.");
        return Ok(());
    }

    // Display based on format
    match format {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&events)?);
        }
        "timeline" | _ => {
            println!("═══════════════════════════════════════════════════════════════════════════");
            println!("WORKFLOW EXECUTION TIMELINE");
            println!("═══════════════════════════════════════════════════════════════════════════");
            println!("  Transaction ID: {}", tx_id);
            println!("  Total Events: {}", events.len());
            println!("═══════════════════════════════════════════════════════════════════════════");
            println!();

            for (idx, event) in events.iter().enumerate() {
                let event_type = event.get("event_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let kernel_urn = event.get("kernel_urn")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let timestamp = event.get("timestamp")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let event_symbol = match event_type {
                    "accepted" => "▶",
                    "completed" => "✓",
                    "failed" => "✗",
                    "workflow_completed" => "🎉",
                    _ => "•",
                };

                println!("[{}] {} {}", idx + 1, event_symbol, event_type.to_uppercase());
                println!("    Kernel: {}", kernel_urn);
                println!("    Time: {}", timestamp);

                if let Some(details) = event.get("details") {
                    println!("    Details: {}", serde_json::to_string_pretty(details)?);
                }
                println!();
            }

            println!("═══════════════════════════════════════════════════════════════════════════");
            println!("END OF TIMELINE");
            println!("═══════════════════════════════════════════════════════════════════════════");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Intercept --help or no args to show dynamic commands
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 || (args.len() == 2 && (args[1] == "--help" || args[1] == "-h")) {
        print_custom_help();
        return Ok(());
    }

    let cli = Cli::parse();

    match cli.command {
        // ===== CONCEPT COMMANDS =====
        Commands::Concept { command } => {
            match command {
                ConceptCommands::List => {
                    // Delegate to status command for consistent table view
                    handle_status(false).await?;
                }

                ConceptCommands::Create { name, template, version } => {
                    use ckp_core::KernelManager;

                    println!("Creating concept: {} (template: {})", name, template);

                    let root = std::env::current_dir()?;
                    let manager = KernelManager::new(root)?;

                    manager.create_kernel(&name, &template, &version)?;

                    println!("\n✓ Concept created successfully");
                    println!("  Name: {}", name);
                    println!("  Type: {}", template);
                    println!("  Version: {}", version);
                    println!("  Location: concepts/{}/", name);
                    println!("\nYou can now start it with: ckr concept start {}", name);
                }

                ConceptCommands::Load { name, version, arch, runtime, as_name } => {
                    use ckp_core::{PackageManager, ProjectRegistry, CkpError};
                    use std::path::PathBuf;

                    // Try current directory first
                    let mut root = std::env::current_dir()?;

                    // If not in a project, use current project from registry
                    if !root.join(".ckproject").exists() {
                        let mut registry = ProjectRegistry::new()?;
                        if let Some(current_name) = registry.get_current_name()? {
                            if let Some(project) = registry.get(&current_name)? {
                                root = PathBuf::from(&project.path);
                            }
                        }
                    }

                    let pm = PackageManager::new()?;

                    // List cached packages for this concept
                    let packages = pm.list_cached()?;
                    let mut matching: Vec<_> = packages.iter().filter(|p| p.name == name).collect();

                    // Apply filters
                    if let Some(ref a) = arch {
                        matching.retain(|p| &p.arch == a);
                    }
                    if let Some(ref r) = runtime {
                        matching.retain(|p| &p.runtime == r);
                    }

                    // Auto-detect version if not specified
                    let final_version = if let Some(v) = version {
                        v
                    } else {
                        match matching.len() {
                            0 => {
                                eprintln!("Error: No matching packages found for '{}'", name);
                                if arch.is_some() || runtime.is_some() {
                                    eprintln!("  Filters: arch={:?}, runtime={:?}", arch, runtime);
                                }
                                eprintln!("\nAvailable packages for '{}':", name);
                                let all_matching: Vec<_> = packages.iter().filter(|p| p.name == name).collect();
                                for pkg in all_matching {
                                    eprintln!("  {}@{} [{}/{}]", pkg.name, pkg.version, pkg.arch, pkg.runtime);
                                }
                                std::process::exit(1);
                            }
                            1 => {
                                println!("Auto-detected: {}@{} [{}/{}]", matching[0].name, matching[0].version, matching[0].arch, matching[0].runtime);
                                matching[0].version.clone()
                            }
                            _ => {
                                eprintln!("Error: Multiple matching packages found for '{}':", name);
                                for pkg in &matching {
                                    eprintln!("  {}@{} [{}/{}]", pkg.name, pkg.version, pkg.arch, pkg.runtime);
                                }
                                eprintln!("\nPlease specify which package to load:");
                                eprintln!("  ckr concept load {} --runtime <RUNTIME>", name);
                                eprintln!("  ckr concept load {} --arch <ARCH>", name);
                                eprintln!("\nExample:");
                                eprintln!("  ckr concept load {} --runtime rs   # Load Rust version", name);
                                eprintln!("  ckr concept load {} --runtime py   # Load Python version", name);
                                std::process::exit(1);
                            }
                        }
                    };

                    // Find the selected package
                    let selected_pkg = matching.into_iter().find(|p| p.version == final_version).ok_or_else(|| {
                        CkpError::FileNotFound(format!("Package not found: {}@{}", name, final_version))
                    })?;

                    // Resolve instance name (supports multi-instance)
                    let instance_name = pm.resolve_instance_name(&name, as_name.as_deref(), &root)?;

                    println!("Loading concept: {}@{} [{}/{}]", name, final_version, selected_pkg.arch, selected_pkg.runtime);
                    if instance_name != name {
                        println!("  Instance name: {}", instance_name);
                    }

                    let concept_dir = pm.install_from_package(selected_pkg, &root, Some(&instance_name))?;

                    println!("\n✓ Concept loaded successfully");
                    println!("  Location: {}", concept_dir.display());
                }

                ConceptCommands::Unload { name } => {
                    use std::fs;

                    let root = std::env::current_dir()?;
                    let concept_dir = root.join("concepts").join(&name);

                    if !concept_dir.exists() {
                        eprintln!("Concept not found: {}", name);
                        std::process::exit(1);
                    }

                    fs::remove_dir_all(&concept_dir)?;
                    println!("✓ Unloaded concept: {}", name);
                    println!("  (Package remains in cache)");
                }

                ConceptCommands::Start { name, as_name, watch: _ } => {
                    use ckp_core::KernelManager;
                    use std::collections::HashMap;

                    // For now, as_name is a future feature for starting new instances
                    if as_name.is_some() {
                        println!("Note: --as flag for starting new instances is not yet implemented");
                        println!("      Please use: ckr concept load {} --version <ver> --as <name>", name);
                        println!("      Then: ckr concept start <instance-name>");
                        std::process::exit(1);
                    }

                    // Find project root using registry if not in a project directory
                    let root = match resolve_project_root() {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            eprintln!("Run 'ckp project list' to see available projects or cd into a project directory");
                            std::process::exit(1);
                        }
                    };
                    let manager = KernelManager::new(root.clone())?;

                    // Start the kernel using KernelManager
                    let options = HashMap::new();
                    let result = manager.start_kernel(&name, &options).await?;

                    if result.already_running {
                        println!("Concept {} is already running", name);
                        if let Some(pid) = result.pid {
                            println!("  Tool PID: {}", pid);
                        }
                        if let Some(watcher_pid) = result.watcher_pid {
                            println!("  Watcher PID: {}", watcher_pid);
                        }
                    } else {
                        println!("✓ Started concept: {}", name);
                        if let Some(watcher_pid) = result.watcher_pid {
                            println!("  Watcher PID: {}", watcher_pid);
                        }
                        if let Some(pid) = result.pid {
                            println!("  Tool PID: {}", pid);
                        }
                        println!("  Logs: {}/logs/{}.log", root.join("concepts").join(&name).display(), name);
                        println!("\nTo stop: ckr concept stop {}", name);
                    }
                }

                ConceptCommands::Stop { name } => {
                    handle_stop(&name).await?;
                }

                ConceptCommands::Export { name, version } => {
                    use ckp_core::{PackageManager, ProjectRegistry, VersionDriverFactory};
                    use std::path::PathBuf;

                    // Try current directory first
                    let mut root = std::env::current_dir()?;

                    // If not in a project, use current project from registry
                    if !root.join(".ckproject").exists() {
                        let mut registry = ProjectRegistry::new()?;
                        if let Some(current_name) = registry.get_current_name()? {
                            if let Some(project) = registry.get(&current_name)? {
                                root = PathBuf::from(&project.path);
                                println!("Using current project: {}", current_name);
                            }
                        }
                    }

                    // Detect version using unified VersionDriver
                    let concept_path = root.join("concepts").join(&name);
                    let final_version = if let Some(driver) = VersionDriverFactory::detect(&concept_path, &name) {
                        match driver.get_version()? {
                            Some(version_info) => {
                                eprintln!("[VersionDriver] Detected version from {}: {}",
                                    version_info.backend, version_info.version);
                                if !version_info.is_clean {
                                    eprintln!("[VersionDriver] Warning: Version is not clean (uncommitted changes)");
                                }
                                version_info.version
                            }
                            None => {
                                eprintln!("[VersionDriver] No version tags found, using CLI version: {}", version);
                                version.clone()
                            }
                        }
                    } else {
                        eprintln!("[VersionDriver] No versioning detected, using CLI version: {}", version);
                        version.clone()
                    };

                    println!("Exporting concept: {}@{}", name, final_version);

                    let pm = PackageManager::new()?;
                    let package_path = pm.export(&name, &final_version, &root)?;

                    println!("\n✓ Concept exported successfully");
                    println!("  Package: {}", package_path.display());
                }

                ConceptCommands::Package { command } => {
                    match command {
                        PackageCommands::List => {
                            use ckp_core::PackageManager;

                            let pm = PackageManager::new()?;
                            let packages = pm.list_cached()?;

                            if packages.is_empty() {
                                println!("Cache is empty.");
                                println!("\nCache location: {}", pm.get_cache_dir().display());
                            } else {
                                // Print table header with timestamp
                                println!("\nNAME                            VERSION    ARCH            RUNTIME  SIZE     CREATED");
                                println!("----------------------------------------------------------------------------------------");

                                // Print rows
                                for pkg in &packages {
                                    let size_kb = pkg.size_bytes as f64 / 1024.0;
                                    let size_str = if size_kb > 1024.0 {
                                        format!("{:.1}M", size_kb / 1024.0)
                                    } else {
                                        format!("{:.0}K", size_kb)
                                    };

                                    println!(
                                        "{:<32}{:<11}{:<16}{:<9}{:<9}{}",
                                        pkg.name,
                                        pkg.version,
                                        pkg.arch,
                                        pkg.runtime,
                                        size_str,
                                        pkg.created_at
                                    );
                                }

                                println!("\nTotal: {} package(s)", packages.len());
                                println!("Cache location: {}", pm.get_cache_dir().display());
                            }
                        }

                        PackageCommands::Import { file } => {
                            use ckp_core::PackageManager;
                            use std::path::Path;

                            println!("Importing package: {}", file);

                            let pm = PackageManager::new()?;
                            let pkg_info = pm.import(Path::new(&file))?;

                            println!("\n✓ Package imported successfully");
                            println!("  Name: {}", pkg_info.name);
                            println!("  Version: {}", pkg_info.version);
                            println!("  Size: {} bytes", pkg_info.size_bytes);
                        }

                        PackageCommands::Unload { name_version } => {
                            use std::fs;

                            // Parse name:version format
                            let parts: Vec<&str> = name_version.split(':').collect();
                            if parts.len() != 2 {
                                eprintln!("Error: Invalid format. Expected <name>:<version>");
                                eprintln!("Example: ckr concept package unload System.Gateway.HTTP:v0.1");
                                std::process::exit(1);
                            }

                            let name = parts[0];
                            let _version = parts[1];  // Version stored for future validation

                            let root = std::env::current_dir()?;
                            let concept_dir = root.join("concepts").join(name);

                            if !concept_dir.exists() {
                                eprintln!("Concept not found: {}", name);
                                std::process::exit(1);
                            }

                            fs::remove_dir_all(&concept_dir)?;
                            println!("✓ Unloaded package: {}", name_version);
                            println!("  (Package remains in cache)");
                        }
                    }
                }

                ConceptCommands::Build { kernel, release, incremental } => {
                    use ckp_core::KernelBuilder;

                    // Use the common pattern to get project root
                    let root = resolve_project_root()?;
                    let builder = KernelBuilder::new(root);

                    if let Some(kernel_name) = kernel {
                        // Build single kernel
                        if incremental {
                            // Check if rebuild needed
                            let needs_rebuild = builder.needs_rebuild(&kernel_name, release)?;
                            if !needs_rebuild {
                                println!("[KernelBuilder] Kernel {} is up to date, skipping build", kernel_name);
                                return Ok(());
                            }
                        }

                        builder.build_kernel(&kernel_name, release)?;
                    } else {
                        // Build all kernels
                        let built = builder.build_all(release)?;

                        if built.is_empty() {
                            println!("\n[KernelBuilder] No Rust kernels found to build");
                        }
                    }
                }
            }
        }

        // ===== FORK COMMAND =====
        Commands::Fork { source, name, clean, tag, no_start } => {
            use ckp_core::PackageManager;

            let pm = PackageManager::new()?;
            let root = std::env::current_dir()?;

            println!("Forking package: {} → {}", source, name);
            if clean {
                println!("  Mode: Clean fork (removing runtime data)");
            }

            // Fork the package
            pm.fork_package(&source, &name, &root, clean, tag.as_deref())?;

            println!("\n✓ Fork completed successfully");
            println!("  New kernel: {}", name);
            println!("  Location: concepts/{}/", name);

            // Auto-start unless --no-start
            if !no_start {
                use ckp_core::KernelManager;
                use std::collections::HashMap;

                println!("\nStarting forked kernel...");
                let manager = KernelManager::new(root)?;
                let options = HashMap::new();
                let result = manager.start_kernel(&name, &options).await?;

                if result.already_running {
                    println!("Kernel {} is already running", name);
                } else {
                    println!("✓ Started kernel: {}", name);
                    if let Some(pid) = result.pid {
                        println!("  PID: {}", pid);
                    }
                }
            } else {
                println!("\nTo start: ckr concept start {}", name);
            }
        }

        // ===== PROJECT COMMANDS =====
        Commands::Project { command } => {
            match command {
                ProjectCommands::List { verbose } => {
                    handle_projects_list(verbose)?;
                }

                ProjectCommands::Create { path, force } => {
                    handle_init_with_path(path, force).await?;
                }

                ProjectCommands::Current => {
                    handle_projects_current()?;
                }

                ProjectCommands::Switch { name } => {
                    handle_project_switch(&name)?;
                }

                ProjectCommands::Remove { name } => {
                    handle_projects_remove(&name)?;
                }
            }
        }

        // ===== EDGE COMMANDS =====
        Commands::Edge { command } => {
            match command {
                EdgeCommands::List { concept } => {
                    if let Some(kernel) = concept {
                        handle_edges(&kernel)?;
                    } else {
                        handle_list_edges()?;
                    }
                }

                EdgeCommands::Create { predicate, source, target } => {
                    handle_create_edge(&predicate, &source, &target).await?;
                }
            }
        }

        // ===== KERNEL COMMANDS =====
        Commands::Kernel { command } => {
            match command {
                KernelCommands::List => {
                    use ckp_core::ontology::OntologyLibrary;

                    // Get project root and initialize library
                    let root = std::env::current_dir()?;
                    let library = OntologyLibrary::new(root.clone())?;

                    // Query Jena for all kernels using library's query_sparql method
                    let sparql_query = r#"
                        PREFIX ckp: <urn:ckp:>
                        PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
                        SELECT DISTINCT ?label ?runtime ?type WHERE {
                            ?kernel a ckp:Kernel ;
                                    rdfs:label ?label .
                            OPTIONAL { ?kernel ckp:runtime ?runtime }
                            OPTIONAL { ?kernel ckp:type ?type }
                        }
                        ORDER BY ?label
                    "#;

                    match library.query_sparql(sparql_query) {
                        Ok(results) => {
                            println!("KERNEL NAME                                    RUNTIME    TYPE");
                            println!("──────────────────────────────────────────────────────────────────────────");

                            let kernel_count = results.len();
                            for row in results {
                                let label = row.get("label").map(|s| s.as_str()).unwrap_or("?");
                                let runtime = row.get("runtime").map(|s| s.as_str()).unwrap_or("-");
                                let kernel_type = row.get("type").map(|s| s.as_str()).unwrap_or("-");

                                println!("{:<45} {:<10} {}",
                                    label, runtime, kernel_type);
                            }

                            println!("\nTotal: {} kernel(s)", kernel_count);
                        }
                        Err(e) => {
                            eprintln!("✗ Failed to query kernels from Jena: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
        }

        // ===== WORKFLOW COMMANDS =====
        Commands::Workflow { command } => {
            use ckp_core::workflow::WorkflowAPI;
            use ckp_core::ontology::OntologyLibrary;
            use std::path::PathBuf;

            // Get project root
            let root = std::env::current_dir()?;
            let library = OntologyLibrary::new(root.clone())?;
            let mut workflow_api = WorkflowAPI::new(library);

            match command {
                WorkflowCommands::Add { file } => {
                    println!("Adding workflow from CKDL file: {}", file);

                    let ckdl_path = PathBuf::from(&file);
                    match workflow_api.load_workflow_from_ckdl(&ckdl_path) {
                        Ok(workflow_urn) => {
                            println!("\n✓ Workflow added successfully");
                            println!("  URN: {}", workflow_urn);
                            println!("  File: {}", file);
                            println!("\nYou can now:");
                            println!("  - List workflows: ck workflow list");
                            println!("  - Validate workflow: ck workflow validate {}", workflow_urn);
                            println!("  - Apply workflow: ck workflow apply {}", workflow_urn);
                        }
                        Err(e) => {
                            eprintln!("✗ Failed to add workflow: {}", e);
                            std::process::exit(1);
                        }
                    }
                }

                WorkflowCommands::List => {
                    match workflow_api.query_all_workflows() {
                        Ok(workflows) => {
                            if workflows.is_empty() {
                                println!("No workflows found.");
                                println!("\nAdd a workflow with: ck workflow add <file.ckdl>");
                            } else {
                                println!("\nWORKFLOW URN                                      LABEL                            STATUS");
                                println!("----------------------------------------------------------------------------------------");

                                for workflow in &workflows {
                                    let status_str = format!("{:?}", workflow.status);
                                    println!(
                                        "{:<50}{:<33}{:?}",
                                        workflow.workflow_urn,
                                        workflow.label,
                                        status_str
                                    );
                                }

                                println!("\nTotal: {} workflow(s)", workflows.len());
                            }
                        }
                        Err(e) => {
                            eprintln!("✗ Failed to list workflows: {}", e);
                            std::process::exit(1);
                        }
                    }
                }

                WorkflowCommands::Delete { urn } => {
                    println!("✗ Workflow deletion not implemented yet");
                    println!("  URN: {}", urn);
                    std::process::exit(1);
                }

                WorkflowCommands::Apply { urn } => {
                    println!("Applying workflow (scaffolding kernels): {}", urn);
                    println!();

                    // Find CKDL file by URN
                    let workflows_dir = root.join("workflows");
                    let ckdl_files: Vec<_> = std::fs::read_dir(&workflows_dir)
                        .unwrap_or_else(|_| {
                            eprintln!("✗ No workflows directory found");
                            std::process::exit(1);
                        })
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().extension().map(|ext| ext == "ckdl").unwrap_or(false))
                        .collect();

                    let mut ckdl_path = None;
                    for file in ckdl_files {
                        // Parse each CKDL to find matching URN
                        if let Ok(content) = std::fs::read_to_string(file.path()) {
                            if content.contains(&format!("WORKFLOW {}", urn)) {
                                ckdl_path = Some(file.path());
                                break;
                            }
                        }
                    }

                    let ckdl_path = ckdl_path.ok_or_else(|| {
                        eprintln!("✗ No CKDL file found for workflow: {}", urn);
                        eprintln!("  Searched in: {}", workflows_dir.display());
                        std::process::exit(1);
                    }).unwrap();

                    println!("Found CKDL: {}", ckdl_path.display());

                    // Parse CKDL to get kernel definitions
                    use ckp_core::workflow::ckdl_parser::parse_ckdl_file;
                    let ckdl_workflow = match parse_ckdl_file(&ckdl_path, &root) {
                        Ok(w) => w,
                        Err(e) => {
                            eprintln!("✗ Failed to parse CKDL: {}", e);
                            std::process::exit(1);
                        }
                    };

                    println!("Workflow: {}", ckdl_workflow.label);
                    println!("  Kernels: {}", ckdl_workflow.workflow_kernels.len());
                    println!();

                    // Scaffold each kernel
                    use std::fs;
                    let mut scaffolded_count = 0;

                    for kernel in &ckdl_workflow.workflow_kernels {
                        let kernel_urn = &kernel.urn;

                        // Extract kernel name from URN (ckp://Usecase.Bakery.OrderProcessor:v1.0.0 → Usecase.Bakery.OrderProcessor)
                        let kernel_name = kernel_urn
                            .trim_start_matches("ckp://")
                            .split(':')
                            .next()
                            .unwrap_or(kernel_urn);

                        let concept_dir = root.join("concepts").join(kernel_name);

                        // Check if already exists
                        if concept_dir.exists() {
                            println!("  ⊙ {:<50} (exists)", kernel_name);
                            continue;
                        }

                        // Create concept directory
                        fs::create_dir_all(&concept_dir).unwrap_or_else(|e| {
                            eprintln!("✗ Failed to create directory for {}: {}", kernel_name, e);
                            std::process::exit(1);
                        });

                        // Create kernel metadata using proper struct
                        use ckp_core::kernel::KernelMetadata;

                        // Extract version from URN
                        let version = kernel_urn.split(':').last().unwrap_or("v1.0.0");

                        // Use kernel type and runtime from CKDL, or defaults
                        let kernel_type = if kernel.kernel_type.is_empty() {
                            "passthrough:daemon" // Default to passthrough for auto-scaffolded kernels
                        } else {
                            &kernel.kernel_type
                        };

                        let runtime = kernel.runtime.as_deref().unwrap_or("daemon");
                        let description = if kernel.description.is_empty() {
                            format!("Auto-scaffolded from workflow: {}", ckdl_workflow.label)
                        } else {
                            kernel.description.clone()
                        };

                        // Create metadata struct
                        let mut metadata = KernelMetadata::new(
                            kernel_name,
                            kernel_type,
                            version,
                            runtime,
                            &description,
                        );

                        // Add capabilities from CKDL
                        metadata.set_capabilities(kernel.capabilities.clone());

                        // Serialize to YAML and write
                        let yaml_content = metadata.to_yaml().unwrap_or_else(|e| {
                            eprintln!("✗ Failed to serialize kernel metadata to YAML: {}", e);
                            std::process::exit(1);
                        });
                        fs::write(concept_dir.join("conceptkernel.yaml"), yaml_content).unwrap();

                        // Create ontology.ttl using RDF serialization
                        let ontology_content = metadata.to_rdf();
                        fs::write(concept_dir.join("ontology.ttl"), ontology_content).unwrap();

                        // Create tool directory - but only create stub for non-passthrough kernels
                        let tool_dir = concept_dir.join("tool");
                        fs::create_dir_all(&tool_dir).unwrap();

                        // For passthrough:daemon kernels, create a minimal README instead of stub script
                        if kernel_type.starts_with("passthrough") {
                            let readme_content = format!(
                                "# {} - Passthrough Kernel\n\n\
                                URN: {}\n\
                                Type: {}\n\
                                Runtime: {}\n\n\
                                This is a **passthrough kernel** that automatically forwards data between kernels\n\
                                without requiring custom tool implementation.\n\n\
                                ## How it works\n\n\
                                - Subscribes to incoming data via queue/NATS\n- Validates schema compatibility\n\
                                - Forwards to target kernel(s) via edges\n- No custom code required\n\n\
                                ## Configuration\n\n\
                                Edit `conceptkernel.yaml` to customize capabilities and edge connections.\n\
                                See workflow CKDL for edge definitions.\n",
                                kernel_name, kernel_urn, kernel_type, runtime
                            );
                            fs::write(tool_dir.join("README.md"), readme_content).unwrap();
                        } else {
                            // For other kernel types, create language-appropriate stub
                            let (script_name, script_content) = if kernel_type.starts_with("rust") {
                                let name = kernel_name.split('.').last().unwrap_or(kernel_name).to_lowercase();
                                let content = format!(
                                    "// Auto-scaffolded from workflow: {}\n\
                                    // URN: {}\n\n\
                                    use ckp_core::{{Kernel, JobFile}};\n\n\
                                    fn main() {{\n    \
                                        println!(\"[{}] Starting...\");\n    \
                                        // TODO: Implement kernel logic\n\
                                    }}\n",
                                    ckdl_workflow.label, kernel_urn, kernel_urn
                                );
                                (format!("{}.rs", name), content)
                            } else {
                                let name = kernel_name.split('.').last().unwrap_or(kernel_name).to_lowercase();
                                let content = format!(
                                    "#!/usr/bin/env python3\n\
                                    # Auto-scaffolded from workflow: {}\n\
                                    # URN: {}\n\n\
                                    import asyncio\nimport os\n\
                                    from nats.aio.client import Client as NATS\n\n\
                                    KERNEL_URN = os.environ.get(\"KERNEL_URN\", \"{}\")\n\
                                    NATS_URL = os.environ.get(\"NATS_URL\", \"nats://localhost:4222\")\n\n\
                                    async def main():\n    \
                                        nc = NATS()\n    \
                                        await nc.connect(NATS_URL)\n    \
                                        print(f\"[{{KERNEL_URN}}] ✓ Connected to NATS\")\n    \
                                        # TODO: Implement kernel logic\n    \
                                        while True:\n        \
                                            await asyncio.sleep(1)\n\n\
                                    if __name__ == \"__main__\":\n    \
                                        try:\n        \
                                            asyncio.run(main())\n    \
                                        except KeyboardInterrupt:\n        \
                                            print(f\"\\n[{{KERNEL_URN}}] Shutting down...\")\n",
                                    ckdl_workflow.label, kernel_urn, kernel_urn
                                );
                                (format!("{}.py", name), content)
                            };

                            let script_path = tool_dir.join(script_name);
                            fs::write(&script_path, script_content).unwrap();

                            // Make script executable
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                let mut perms = fs::metadata(&script_path).unwrap().permissions();
                                perms.set_mode(0o755);
                                fs::set_permissions(&script_path, perms).unwrap();
                            }
                        }

                        println!("  ✓ {:<50} (scaffolded)", kernel_name);
                        scaffolded_count += 1;
                    }

                    println!();
                    println!("═══════════════════════════════════════════════════════════════════════════");
                    println!("✓ Workflow applied successfully");
                    println!("  Scaffolded: {} kernel(s)", scaffolded_count);
                    println!("═══════════════════════════════════════════════════════════════════════════");
                    println!();
                    println!("Next steps:");
                    println!("  1. Implement kernel logic in concepts/*/tool/*.py");
                    println!("  2. Start kernels: ck concept start <kernel-name>");
                    println!("  3. Execute workflow: ck tx run {}", urn);
                }

                WorkflowCommands::Validate { urn } => {
                    println!("Validating workflow: {}", urn);

                    match workflow_api.validate_workflow(&urn) {
                        Ok(validation) => {
                            if validation.is_valid {
                                println!("\n✓ Workflow is valid");
                            } else {
                                println!("\n✗ Workflow validation failed");
                            }

                            if !validation.cycles.is_empty() {
                                println!("\nCycles detected:");
                                for cycle in &validation.cycles {
                                    let cycle_type = format!("{:?}", cycle.cycle_type);
                                    println!("  - {:?} ({})", cycle.kernels, cycle_type);
                                    if cycle.is_intentional {
                                        println!("    ✓ Intentional loop");
                                    } else {
                                        println!("    ⚠ Problematic cycle");
                                    }
                                }
                            }

                            if !validation.missing_kernels.is_empty() {
                                println!("\nMissing kernels:");
                                for kernel in &validation.missing_kernels {
                                    println!("  - {}", kernel);
                                }
                            }

                            if !validation.warnings.is_empty() {
                                println!("\nWarnings:");
                                for warning in &validation.warnings {
                                    println!("  ⚠ {}", warning);
                                }
                            }

                            if !validation.errors.is_empty() {
                                println!("\nErrors:");
                                for error in &validation.errors {
                                    println!("  ✗ {}", error);
                                }
                            }

                            if !validation.is_valid {
                                std::process::exit(1);
                            }
                        }
                        Err(e) => {
                            eprintln!("✗ Failed to validate workflow: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
        }

        // ===== TRANSACTION COMMANDS =====
        Commands::Tx { command } => {
            use ckp_core::workflow::WorkflowAPI;
            use ckp_core::ontology::OntologyLibrary;
            use std::path::PathBuf;

            match command {
                TxCommands::Run { workflow_urn, input, timeout } => {
                    println!("Executing workflow: {}", workflow_urn);
                    println!("Input: {}", input);
                    println!("Timeout: {}s", timeout);
                    println!();

                    // Parse input JSON
                    let input_json: serde_json::Value = match serde_json::from_str(&input) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!("✗ Invalid JSON input: {}", e);
                            std::process::exit(1);
                        }
                    };

                    // Get project root and query workflow
                    let root = std::env::current_dir()?;
                    let library = OntologyLibrary::new(root.clone())?;
                    let workflow_api = WorkflowAPI::new(library);

                    let all_workflows = match workflow_api.query_all_workflows() {
                        Ok(workflows) => workflows,
                        Err(e) => {
                            eprintln!("✗ Failed to query workflows: {}", e);
                            std::process::exit(1);
                        }
                    };

                    // Normalize workflow URN (strip angle brackets if present)
                    let normalize_urn = |urn: &str| -> String {
                        urn.trim_start_matches('<').trim_end_matches('>').to_string()
                    };

                    let normalized_input = normalize_urn(&workflow_urn);

                    let workflow = all_workflows.iter()
                        .find(|w| normalize_urn(&w.workflow_urn) == normalized_input)
                        .cloned()
                        .ok_or_else(|| {
                            eprintln!("✗ Workflow not found: {}", workflow_urn);
                            eprintln!("Available workflows:");
                            for w in &all_workflows {
                                eprintln!("  - {}", normalize_urn(&w.workflow_urn));
                            }
                            std::process::exit(1);
                        })
                        .unwrap();

                    // Generate unique transaction ID
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let timestamp = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis();
                    let tx_id = format!("tx-{}", timestamp);

                    println!("═══════════════════════════════════════════════════════════════════════════");
                    println!("TRANSACTION STARTED");
                    println!("═══════════════════════════════════════════════════════════════════════════");
                    println!("  Workflow: {}", workflow.label);
                    println!("  URN: {}", workflow_urn);
                    println!("  Transaction ID: {}", tx_id);
                    println!("═══════════════════════════════════════════════════════════════════════════");
                    println!();

                    // Execute workflow
                    let result = handle_workflow_execution(
                        &workflow,
                        &tx_id,
                        input_json,
                        timeout,
                    ).await;

                    match result {
                        Ok(_) => {
                            println!();
                            println!("═══════════════════════════════════════════════════════════════════════════");
                            println!("✓ TRANSACTION COMPLETE");
                            println!("═══════════════════════════════════════════════════════════════════════════");
                            println!("  Transaction ID: {}", tx_id);
                            println!();
                            println!("To view all occurrents:");
                            println!("  ck tx show {}", tx_id);
                            println!("═══════════════════════════════════════════════════════════════════════════");
                        }
                        Err(e) => {
                            eprintln!();
                            eprintln!("═══════════════════════════════════════════════════════════════════════════");
                            eprintln!("✗ TRANSACTION FAILED");
                            eprintln!("═══════════════════════════════════════════════════════════════════════════");
                            eprintln!("  Error: {}", e);
                            eprintln!("  Transaction ID: {}", tx_id);
                            eprintln!("═══════════════════════════════════════════════════════════════════════════");
                            std::process::exit(1);
                        }
                    }
                }

                TxCommands::List { workflow, limit } => {
                    let result = handle_list_executions(workflow.clone(), limit).await;
                    match result {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("✗ Failed to list transactions: {}", e);
                            std::process::exit(1);
                        }
                    }
                }

                TxCommands::Show { tx_id, format } => {
                    println!("Showing transaction details for: {}", tx_id);
                    println!("  Format: {}", format);
                    println!();

                    let result = handle_show_execution(&tx_id, &format).await;
                    match result {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("✗ Failed to show transaction: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
        }

        // ===== PACKAGE COMMANDS =====
        Commands::Package { command } => {
            match command {
                TopLevelPackageCommands::List => {
                    use ckp_core::PackageManager;

                    let pm = PackageManager::new()?;
                    let packages = pm.list_cached()?;

                    if packages.is_empty() {
                        println!("Cache is empty.");
                        println!("\nCache location: {}", pm.get_cache_dir().display());
                    } else {
                        // Print table header with timestamp
                        println!("\nNAME                            VERSION    ARCH            RUNTIME  SIZE     CREATED");
                        println!("----------------------------------------------------------------------------------------");

                        // Print rows
                        for pkg in &packages {
                            let size_kb = pkg.size_bytes as f64 / 1024.0;
                            let size_str = if size_kb > 1024.0 {
                                format!("{:.1}M", size_kb / 1024.0)
                            } else {
                                format!("{:.0}K", size_kb)
                            };

                            println!(
                                "{:<32}{:<11}{:<16}{:<9}{:<9}{}",
                                pkg.name,
                                pkg.version,
                                pkg.arch,
                                pkg.runtime,
                                size_str,
                                pkg.created_at
                            );
                        }

                        println!("\nTotal: {} package(s)", packages.len());
                        println!("Cache location: {}", pm.get_cache_dir().display());
                    }
                }

                TopLevelPackageCommands::Import { file } => {
                    use ckp_core::PackageManager;
                    use std::path::Path;

                    println!("Importing package: {}", file);

                    let pm = PackageManager::new()?;
                    let pkg_info = pm.import(Path::new(&file))?;

                    println!("\n✓ Package imported successfully");
                    println!("  Name: {}", pkg_info.name);
                    println!("  Version: {}", pkg_info.version);
                    println!("  Size: {} bytes", pkg_info.size_bytes);
                }

                TopLevelPackageCommands::Fork { source, name, clean, tag, no_start } => {
                    use ckp_core::PackageManager;

                    let pm = PackageManager::new()?;
                    let root = std::env::current_dir()?;

                    println!("Forking package: {} → {}", source, name);
                    if clean {
                        println!("  Mode: Clean fork (removing runtime data)");
                    }

                    // Fork the package
                    pm.fork_package(&source, &name, &root, clean, tag.as_deref())?;

                    println!("\n✓ Fork completed successfully");
                    println!("  New kernel: {}", name);
                    println!("  Location: concepts/{}/", name);

                    // Auto-start unless --no-start
                    if !no_start {
                        use ckp_core::KernelManager;
                        use std::collections::HashMap;

                        println!("\nStarting forked kernel...");
                        let manager = KernelManager::new(root)?;
                        let options = HashMap::new();
                        let result = manager.start_kernel(&name, &options).await?;

                        if result.already_running {
                            println!("Kernel {} is already running", name);
                        } else {
                            println!("✓ Started kernel: {}", name);
                            if let Some(pid) = result.pid {
                                println!("  PID: {}", pid);
                            }
                        }
                    } else {
                        println!("\nTo start: ckr concept start {}", name);
                    }
                }
            }
        }

        // ===== TOP-LEVEL COMMANDS (Ergonomic shortcuts) =====

        Commands::Up => {
            use ckp_core::{KernelManager, ProjectRegistry};

            println!("Starting all concepts...\n");

            // Try current directory first
            let mut root = std::env::current_dir()?;

            // If not in a project, use current project from registry
            if !root.join(".ckproject").exists() {
                let mut registry = ProjectRegistry::new()?;
                if let Some(current_name) = registry.get_current_name()? {
                    if let Some(project) = registry.get(&current_name)? {
                        root = std::path::PathBuf::from(&project.path);
                    }
                }
            }

            let manager = KernelManager::new(root)?;
            let results = manager.start_all().await?;

            let started = results.iter().filter(|r| !r.already_running).count();
            let already_running = results.iter().filter(|r| r.already_running).count();

            if started > 0 {
                println!("\n✓ Started {} concept(s)", started);
            }
            if already_running > 0 {
                println!("  {} concept(s) already running", already_running);
            }
            if results.is_empty() {
                println!("No concepts found.");
            }
        }

        Commands::Down => {
            use ckp_core::{KernelManager, ProjectRegistry};

            println!("Stopping all concepts...\n");

            // Try current directory first
            let mut root = std::env::current_dir()?;

            // If not in a project, use current project from registry
            if !root.join(".ckproject").exists() {
                let mut registry = ProjectRegistry::new()?;
                if let Some(current_name) = registry.get_current_name()? {
                    if let Some(project) = registry.get(&current_name)? {
                        root = std::path::PathBuf::from(&project.path);
                    }
                }
            }

            let manager = KernelManager::new(root)?;
            let results = manager.stop_all().await?;

            let stopped = results.iter().filter(|(_, stopped)| *stopped).count();

            if stopped > 0 {
                println!("\n✓ Stopped {} concept(s)", stopped);
            } else if !results.is_empty() {
                println!("No running concepts to stop.");
            } else {
                println!("No concepts found.");
            }
        }

        Commands::Status { wide } => {
            handle_status(wide).await?;
        }

        Commands::Emit { target, payload } => {
            handle_emit(&target, &payload).await?;
        }

        Commands::ValidateUrn { urn } => {
            use ckp_core::{UrnValidator, UrnResolver};

            println!("Validating URN: {}", urn);
            let result = UrnValidator::validate(&urn);

            if result.valid {
                println!("✓ Valid URN");

                // Try to parse and show details
                if UrnResolver::is_edge_urn(&urn) {
                    match UrnResolver::parse_edge_urn(&urn) {
                        Ok(parsed) => {
                            println!("\nEdge URN Details:");
                            println!("  Predicate: {}", parsed.predicate);
                            println!("  Source:    {}", parsed.source);
                            println!("  Target:    {}", parsed.target);
                            println!("  Version:   {}", parsed.version.as_deref().unwrap_or("none"));
                            println!("  Queue:     {}", parsed.queue_path);
                        }
                        Err(e) => println!("Error parsing: {}", e),
                    }
                } else {
                    match UrnResolver::parse(&urn) {
                        Ok(parsed) => {
                            println!("\nKernel URN Details:");
                            println!("  Kernel:  {}", parsed.kernel);
                            println!("  Version: {}", parsed.version);
                            if let Some(stage) = parsed.stage {
                                println!("  Stage:   {}", stage);
                            }
                            if let Some(path) = parsed.path {
                                println!("  Path:    {}", path);
                            }
                        }
                        Err(e) => println!("Error parsing: {}", e),
                    }
                }
            } else {
                println!("✗ Invalid URN");
                for error in result.errors {
                    println!("  - {}", error);
                }
                std::process::exit(1);
            }
        }

        Commands::Query { urn, format } => {
            handle_query(&urn, &format).await?;
        }

        Commands::Daemon { command } => {
            match command {
                DaemonCommands::EdgeRouter { project, verbose } => {
                    use ckp_core::drivers::{NatsTransport, JenaStorage};
                    use ckp_core::daemon::EdgeRouterDaemonAsync;
                    use std::sync::Arc;

                    // Resolve project path
                    let project_path = if project.is_absolute() {
                        project.clone()
                    } else {
                        std::env::current_dir()?.join(project)
                    };

                    println!("[Daemon] Starting edge router (NATS-based) for project: {}", project_path.display());

                    // Read configuration from .ckproject
                    let nats_url = "nats://localhost:4222";  // From .ckproject backends.nats.endpoint
                    let jena_url = "https://jena.conceptkernel.dev";
                    let jena_dataset = "dataset";
                    let jena_user = "admin";
                    let jena_password = "uIL@xlp8tgG-6s{MR*mJ+re>";

                    // Create NATS transport driver
                    println!("[Daemon] Initializing NATS transport at {}", nats_url);
                    let transport = Arc::new(NatsTransport::new(nats_url).await?) as Arc<dyn ckp_core::drivers::TransportDriver>;

                    // Create Jena storage driver
                    println!("[Daemon] Initializing Jena storage at {}", jena_url);
                    let storage = Arc::new(JenaStorage::new_with_auth(
                        jena_url.to_string(),
                        jena_dataset.to_string(),
                        jena_user.to_string(),
                        jena_password.to_string(),
                    )) as Arc<dyn ckp_core::drivers::StorageDriver>;

                    // Create shutdown channel
                    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
                    let shutdown_tx_clone = shutdown_tx.clone();

                    // Set up SIGTERM/SIGINT handler
                    ctrlc::set_handler(move || {
                        eprintln!("[EdgeRouter] Received SIGTERM/SIGINT, shutting down gracefully...");
                        let _ = shutdown_tx_clone.send(());
                    })?;

                    // Create and start async edge router daemon
                    let daemon = EdgeRouterDaemonAsync::new(
                        project_path,
                        transport,
                        Some(storage),
                        verbose
                    ).await?;

                    println!("[Daemon] Edge router ready - monitoring NATS streams");
                    daemon.start(shutdown_rx).await?;

                    eprintln!("[EdgeRouter] Shutdown complete");
                }
                DaemonCommands::Governor { kernel, project, storage: _, nats_url: _, verbose } => {
                    use ckp_core::drivers::factory::{DriverFactory, StorageConfig, TransportConfig};

                    // Resolve project path
                    let project_path = if project.is_absolute() {
                        project.clone()
                    } else {
                        std::env::current_dir()?.join(project)
                    };

                    if verbose {
                        eprintln!("[Daemon] Starting governor for kernel: {} in project: {}", kernel, project_path.display());
                    }

                    // Read .ckproject to build driver configuration (SU02: Project Config Loaded)
                    let ckproject_path = project_path.join(".ckproject");
                    if !ckproject_path.exists() {
                        return Err(format!("Missing .ckproject file at {}", ckproject_path.display()).into());
                    }

                    let content = std::fs::read_to_string(&ckproject_path)
                        .map_err(|e| format!("Failed to read .ckproject: {}", e))?;

                    let yaml: serde_yaml::Value = serde_yaml::from_str(&content)
                        .map_err(|e| format!("Failed to parse .ckproject: {}", e))?;

                    if verbose {
                        eprintln!("[Daemon] [SU02] Project config loaded from .ckproject");
                    }

                    // Parse storage configuration
                    let storage_type = yaml["spec"]["edges"]["daemon"]["storage"]["type"]
                        .as_str()
                        .unwrap_or("local");

                    let storage_config = match storage_type {
                        "jena" => {
                            let endpoint = yaml["spec"]["backends"]["jena"]["endpoint"]
                                .as_str()
                                .ok_or("Missing backends.jena.endpoint in .ckproject")?
                                .to_string();
                            let dataset = yaml["spec"]["backends"]["jena"]["dataset"]
                                .as_str()
                                .ok_or("Missing backends.jena.dataset in .ckproject")?
                                .to_string();
                            let username = yaml["spec"]["backends"]["jena"]["username"]
                                .as_str()
                                .ok_or("Missing backends.jena.username in .ckproject")?;
                            let password = yaml["spec"]["backends"]["jena"]["password"]
                                .as_str()
                                .ok_or("Missing backends.jena.password in .ckproject")?;

                            // Write credentials to temp file for DriverFactory
                            let creds_content = format!("{}:{}", username, password);
                            let creds_path = std::env::temp_dir().join(format!(".ckp-jena-creds-{}", std::process::id()));
                            std::fs::write(&creds_path, creds_content)?;

                            if verbose {
                                eprintln!("[Daemon] Storage: Jena endpoint={} dataset={}", endpoint, dataset);
                            }

                            StorageConfig::Jena {
                                endpoint,
                                dataset,
                                credentials: Some(creds_path.to_string_lossy().to_string()),
                            }
                        }
                        _ => {
                            if verbose {
                                eprintln!("[Daemon] Storage: Local filesystem");
                            }
                            StorageConfig::Local {
                                root: project_path.clone(),
                            }
                        }
                    };

                    // Parse transport configuration
                    let transport_type = yaml["spec"]["edges"]["daemon"]["transport"]["type"]
                        .as_str()
                        .unwrap_or("local");

                    let transport_config = match transport_type {
                        "nats" => {
                            let endpoint = yaml["spec"]["backends"]["nats"]["endpoint"]
                                .as_str()
                                .ok_or("Missing backends.nats.endpoint in .ckproject")?
                                .to_string();

                            if verbose {
                                eprintln!("[Daemon] Transport: NATS endpoint={}", endpoint);
                            }

                            TransportConfig::Nats {
                                endpoint,
                                credentials: None,  // NATS credentials via URL or env
                            }
                        }
                        _ => {
                            if verbose {
                                eprintln!("[Daemon] Transport: Local filesystem + notify");
                            }
                            TransportConfig::Local {
                                root: project_path.clone(),
                                kernel_name: kernel.clone(),
                            }
                        }
                    };

                    // Create drivers using DriverFactory
                    let storage_driver = DriverFactory::create_storage(&storage_config).await
                        .map_err(|e| format!("Failed to create storage driver: {}", e))?;

                    if verbose {
                        eprintln!("[Daemon] [SU14] Jena storage driver created and connected");
                    }

                    let transport_driver = match DriverFactory::create_transport(&transport_config).await {
                        Ok(driver) => {
                            if verbose {
                                eprintln!("[Daemon] [SU15] NATS transport driver created and connected");
                            }
                            Some(driver)
                        }
                        Err(e) => {
                            eprintln!("[Daemon] Warning: Failed to create transport driver: {}", e);
                            None
                        }
                    };

                    // Set up shutdown handling
                    let shutdown = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let shutdown_clone = shutdown.clone();

                    ctrlc::set_handler(move || {
                        eprintln!("[Governor] Received SIGTERM/SIGINT, shutting down gracefully...");
                        shutdown_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                    })?;

                    // Create event publisher before governor creation (v1.3.20)
                    let nats_url = yaml["spec"]["backends"]["nats"]["endpoint"]
                        .as_str()
                        .map(|s| s.to_string());

                    if verbose && nats_url.is_some() {
                        eprintln!("[Daemon] Initializing event publisher for {}", kernel);
                    }

                    let event_publisher = ckp_core::KernelEventPublisher::new(
                        kernel.clone(),
                        nats_url.as_deref()
                    ).await?;

                    // Query Jena for kernel config (FILELESS MODE - all config from Jena)
                    if verbose {
                        eprintln!("[Daemon] Querying Jena for kernel configuration...");
                    }

                    let (kernel_type, entrypoint) = ckp_core::ConceptKernelGovernor::load_kernel_config_from_jena(
                        &kernel,
                        &storage_driver
                    ).await?;

                    if verbose {
                        eprintln!("[Daemon] Loaded from Jena - Type: {}, Entrypoint: {:?}", kernel_type, entrypoint);
                    }

                    // Perform git validation with event publishing
                    let kernel_dir = project_path.join("concepts").join(&kernel);
                    if verbose {
                        eprintln!("[Daemon] Validating git repository and tags...");
                    }

                    event_publisher.publish_git_validation_started(&kernel_dir.display().to_string()).await;

                    use std::process::Command as StdCommand;
                    // Check git repository
                    let git_check = StdCommand::new("git")
                        .args(&["rev-parse", "--git-dir"])
                        .current_dir(&kernel_dir)
                        .output();

                    match git_check {
                        Ok(result) if result.status.success() => {
                            // Check for v0.1 tag
                            let tag_check = StdCommand::new("git")
                                .args(&["tag"])
                                .current_dir(&kernel_dir)
                                .output();

                            match tag_check {
                                Ok(tag_result) if tag_result.status.success() => {
                                    let tags = String::from_utf8_lossy(&tag_result.stdout);
                                    if tags.lines().any(|tag| tag == "v0.1") {
                                        event_publisher.publish_git_validation_passed(&kernel_dir.display().to_string()).await;
                                        if verbose {
                                            eprintln!("[Daemon] Git validation passed (repo exists, v0.1 tag found)");
                                        }
                                    } else {
                                        event_publisher.publish_startup_failed(
                                            &format!("Required tag 'v0.1' not found. Available tags: {}",
                                                if tags.is_empty() { "none".to_string() } else { tags.lines().collect::<Vec<_>>().join(", ") }),
                                            "startup-git-validation-failed"
                                        ).await;
                                        return Err(format!("Git validation failed: v0.1 tag not found").into());
                                    }
                                }
                                _ => {
                                    event_publisher.publish_startup_failed("Failed to check git tags", "startup-git-validation-failed").await;
                                    return Err("Git validation failed: cannot check tags".into());
                                }
                            }
                        }
                        _ => {
                            event_publisher.publish_startup_failed("Not a git repository", "startup-git-validation-failed").await;
                            return Err("Git validation failed: not a git repository".into());
                        }
                    }

                    // Create governor with drivers and pre-queried config
                    if verbose {
                        eprintln!("[Daemon] Creating governor with configured drivers");
                    }

                    let mut governor = ckp_core::ConceptKernelGovernor::new_with_drivers(
                        &kernel,
                        project_path,
                        kernel_type,
                        entrypoint,
                        storage_driver,
                        transport_driver,
                    )?;

                    if verbose {
                        eprintln!("[Daemon] [SU16] Governor ready, starting inbox watch");
                    }

                    governor.start(shutdown).await?;

                    if verbose {
                        eprintln!("[Governor] Shutdown complete");
                    }
                }
            }
        }

        Commands::Dynamic(args) => {
            handle_dynamic_command(args).await?;
        }
    }

    Ok(())
}

/// Handle dynamic CLI commands by discovering registered kernels and routing to emit
/// Handle generic list action - list instances from kernel storage
async fn handle_generic_list(
    kernel_name: &str,
    kernel_root: &std::path::Path,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::InstanceScanner;

    let scanner = InstanceScanner::new(kernel_root.to_path_buf(), kernel_name.to_string());
    let instances = scanner.list_instances(50)?; // Top 50
    let total = scanner.count_instances()?;

    if json_output {
        // JSON output
        let json = serde_json::json!({
            "instances": instances,
            "total": total,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        // Table output (like ck status)
        if instances.is_empty() {
            println!("No instances found in {}", kernel_name);
            return Ok(());
        }

        println!("\nNAME                            ID                      CREATED             ");
        println!("------------------------------------------------------------------------");

        for inst in &instances {
            let created = inst.timestamp.format("%Y-%m-%d %H:%M:%S");
            println!(
                "{:<32}{:<24}{:<20}",
                inst.name,
                inst.id,
                created
            );
        }

        println!("\nTotal: {} instances", total);
    }

    Ok(())
}

/// Handle generic describe action - show detailed instance view
async fn handle_generic_describe(
    kernel_name: &str,
    kernel_root: &std::path::Path,
    instance_name: &str,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::InstanceScanner;

    let scanner = InstanceScanner::new(kernel_root.to_path_buf(), kernel_name.to_string());
    let detail = scanner.describe_instance(instance_name)?;

    if json_output {
        // JSON output
        println!("{}", serde_json::to_string_pretty(&detail)?);
    } else {
        // Human-readable output
        println!("\n{}", "=".repeat(72));
        println!("Instance: {}", detail.name);
        println!("{}", "=".repeat(72));
        println!("ID:        {}", detail.id);
        println!("Kernel:    {}", detail.kernel);
        println!("Created:   {}", detail.timestamp.format("%Y-%m-%d %H:%M:%S %Z"));

        if let Some(action) = &detail.action {
            println!("Action:    {}", action);
        }

        if let Some(success) = detail.success {
            println!("Success:   {}", success);
        }

        println!("\nData:");
        println!("{}", "─".repeat(72));
        println!("{}", serde_json::to_string_pretty(&detail.data)?);
        println!("{}", "=".repeat(72));
    }

    Ok(())
}

/// Handle `ckp query <URN>` command
async fn handle_query(urn: &str, format: &str) -> Result<(), Box<dyn std::error::Error>> {
    use ckp_core::{UrnResolver, ProcessTracker, QueryFilters, ProjectRegistry};
    use std::path::PathBuf;

    // Try current directory first
    let mut root = std::env::current_dir()?;

    // If not in a project, use current project from registry
    if !root.join(".ckproject").exists() {
        let mut registry = ProjectRegistry::new()?;
        if let Some(current_name) = registry.get_current_name()? {
            if let Some(project) = registry.get(&current_name)? {
                root = PathBuf::from(&project.path);
            }
        } else {
            eprintln!("Error: Not in a ConceptKernel project and no current project set");
            eprintln!("Run 'ckp project list' to see available projects");
            std::process::exit(1);
        }
    }

    // Parse query URN
    let parsed = UrnResolver::parse_query_urn(urn)?;

    match parsed.resource.as_str() {
        "Process" => {
            let tracker = ProcessTracker::new(root)?;

            // Check if fragment is present (e.g., ckp://Process#txId)
            // If so, lookup specific process instead of querying
            if let Some(fragment) = parsed.fragment {
                // Fragment format: {process_type}-{txId}
                // Construct full process URN for lookup
                let process_urn = format!("ckp://Process#{}", fragment);

                match tracker.load_process(&process_urn) {
                    Some(process) => {
                        // Format single process result
                        match format {
                            "json" => {
                                println!("{}", serde_json::to_string_pretty(&process)?);
                            }
                            "yaml" => {
                                println!("{}", serde_yaml::to_string(&process)?);
                            }
                            _ => {
                                // Table format for single process
                                println!("\nProcess: {}", process.urn);
                                println!("Type:    {}", process.process_type);
                                println!("TxID:    {}", process.tx_id);
                                println!("Status:  {}", process.status);
                                println!("Created: {}", process.created_at);
                                println!("Updated: {}", process.updated_at);

                                if let Some(end) = &process.temporal_region.end {
                                    println!("Ended:   {}", end);
                                }
                                if let Some(duration) = process.temporal_region.duration {
                                    println!("Duration: {}ms", duration);
                                }

                                if !process.temporal_parts.is_empty() {
                                    println!("\nTemporal Parts ({}):", process.temporal_parts.len());
                                    for part in &process.temporal_parts {
                                        println!("  - {}: {}", part.phase, part.timestamp);
                                    }
                                }

                                if let Some(error) = &process.error {
                                    println!("\nError: {}", error);
                                }
                            }
                        }
                        return Ok(());
                    }
                    None => {
                        eprintln!("Error: Process not found: {}", process_urn);
                        eprintln!("\nThe process may not exist or may have been cleaned up.");
                        std::process::exit(1);
                    }
                }
            }

            // No fragment - query with filters
            let limit = parsed.params.get("limit")
                .and_then(|v| v.parse().ok());
            let order = parsed.params.get("order").cloned();
            let sort_field = parsed.params.get("sort").cloned();
            let process_type = parsed.params.get("type").cloned();
            let kernel = parsed.params.get("kernel").cloned();
            let status = parsed.params.get("status").cloned();

            let filters = QueryFilters {
                process_type,
                kernel,
                status,
                start_after: None,
                start_before: None,
                limit,
                order,
                sort_field,
            };

            // Query processes
            let processes = tracker.query_processes(filters)?;

            // Format output
            match format {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&processes)?);
                }
                "yaml" => {
                    println!("{}", serde_yaml::to_string(&processes)?);
                }
                "table" => {
                    if processes.is_empty() {
                        println!("No processes found matching query.");
                        return Ok(());
                    }

                    println!("\n{:<22} {:<12} {:<12} {:<26} {:<26}",
                        "TX_ID", "TYPE", "STATUS", "CREATED", "KERNEL");
                    println!("{}", "-".repeat(100));

                    for process in &processes {
                        // created_at is already a String (RFC3339), extract date+time part
                        let created = if process.created_at.len() >= 19 {
                            process.created_at[0..19].replace('T', " ")
                        } else {
                            process.created_at.clone()
                        };

                        let kernel_name = process.participants.get("kernel")
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .unwrap_or("-");

                        println!("{:<22} {:<12} {:<12} {:<26} {:<26}",
                            process.tx_id,
                            process.process_type,
                            process.status,
                            created,
                            kernel_name
                        );
                    }

                    println!("\nTotal: {} process(es)", processes.len());
                }
                _ => {
                    eprintln!("Error: Unknown format '{}'. Use: table, json, or yaml", format);
                    std::process::exit(1);
                }
            }

            Ok(())
        }
        _ => {
            eprintln!("Error: Unsupported resource type '{}'. Currently supported: Process", parsed.resource);
            eprintln!("\nExample: ckp query \"ckp://Process?limit=10&order=desc\"");
            std::process::exit(1);
        }
    }
}

async fn handle_dynamic_command(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::path::PathBuf;
    use ckp_core::{OntologyReader, ProjectRegistry};

    if args.is_empty() {
        eprintln!("Error: No command specified");
        std::process::exit(1);
    }

    let command_name = &args[0];

    // Try current directory first
    let mut root = std::env::current_dir()?;

    // If not in a project, use current project from registry
    if !root.join(".ckproject").exists() {
        let mut registry = ProjectRegistry::new()?;
        if let Some(current_name) = registry.get_current_name()? {
            if let Some(project) = registry.get(&current_name)? {
                root = PathBuf::from(&project.path);
            }
        } else {
            eprintln!("Error: Not in a ConceptKernel project and no current project set");
            eprintln!("Run 'ck project list' to see available projects");
            std::process::exit(1);
        }
    }

    let concepts_dir = root.join("concepts");

    if !concepts_dir.exists() {
        eprintln!("Error: concepts/ directory not found in {}", root.display());
        std::process::exit(1);
    }

    // Create ontology reader
    let ontology_reader = OntologyReader::new(concepts_dir.clone());

    // Scan concepts/ for kernels with CLI exposure
    let mut matched: Option<(String, ckp_core::Ontology)> = None;

    for entry in fs::read_dir(&concepts_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let kernel_name = path.file_name().unwrap().to_str().unwrap();
            let ontology_path = path.join("conceptkernel.yaml");

            if ontology_path.exists() {
                match ontology_reader.read(&ontology_path) {
                    Ok(ontology) => {
                        if let Some(spec) = &ontology.spec {
                            if let Some(cli) = &spec.cli {
                                if cli.expose {
                                    // Check primary command
                                    if &cli.primary == command_name {
                                        matched = Some((kernel_name.to_string(), ontology));
                                        break;
                                    }
                                    // Check aliases
                                    if cli.aliases.iter().any(|a| a == command_name) {
                                        matched = Some((kernel_name.to_string(), ontology));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to parse {}: {}", kernel_name, e);
                    }
                }
            }
        }
    }

    if let Some((kernel_name, ontology)) = matched {
        let remaining_args = &args[1..];

        // Check if first arg is a subcommand
        let payload = if !remaining_args.is_empty() && remaining_args[0] != "--json" {
            let first_arg = &remaining_args[0];

            // Check if this matches a subcommand
            let cli = ontology.spec.as_ref().and_then(|s| s.cli.as_ref());
            let subcommand = cli.and_then(|c| {
                c.subcommands.iter().find(|sc| &sc.name == first_arg)
            });

            if let Some(subcmd) = subcommand {
                // Found a subcommand - build payload with action
                let subcmd_args = &remaining_args[1..];
                // Filter out --json flag from payload args
                let filtered_args: Vec<&String> = subcmd_args.iter()
                    .filter(|arg| *arg != "--json")
                    .collect();

                if filtered_args.is_empty() {
                    format!("{{\"action\": \"{}\"}}", subcmd.action)
                } else {
                    // Parse key=value args
                    let arg_str = filtered_args.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" ");
                    if arg_str.trim().starts_with('{') {
                        // JSON provided directly
                        let mut json: serde_json::Value = serde_json::from_str(&arg_str)?;
                        if let Some(obj) = json.as_object_mut() {
                            obj.insert("action".to_string(), serde_json::json!(subcmd.action));
                        }
                        serde_json::to_string(&json)?
                    } else {
                        // Simple args array for now
                        format!("{{\"action\": \"{}\", \"args\": {:?}}}", subcmd.action, filtered_args)
                    }
                }
            } else {
                // No subcommand - pass through as before
                let arg_str = remaining_args.join(" ");
                if arg_str.trim().starts_with('{') {
                    arg_str
                } else {
                    format!("{{\"args\": {:?}}}", remaining_args)
                }
            }
        } else {
            // No args provided - use first subcommand as default (usually "list")
            let cli = ontology.spec.as_ref().and_then(|s| s.cli.as_ref());
            let default_subcommand = cli.and_then(|c| c.subcommands.first());

            if let Some(subcmd) = default_subcommand {
                format!("{{\"action\": \"{}\"}}", subcmd.action)
            } else {
                "{}".to_string()
            }
        };

        // Parse payload to check for generic operations (list/describe)
        let payload_json: serde_json::Value = serde_json::from_str(&payload)?;
        let action = payload_json.get("action").and_then(|v| v.as_str());

        // Check for --json flag in all args (including those after subcommand)
        let json_output = args.iter().any(|arg| arg == "--json");

        // Intercept generic list/describe operations
        if let Some(action_str) = action {
            if action_str.starts_with("list_") {
                // Handle list generically
                let kernel_path = concepts_dir.join(&kernel_name);
                return handle_generic_list(&kernel_name, &kernel_path, json_output).await;
            } else if action_str.starts_with("describe_") {
                // Handle describe generically
                // Extract instance name from args
                if remaining_args.len() >= 2 {
                    let instance_name = &remaining_args[1];
                    let kernel_path = concepts_dir.join(&kernel_name);
                    return handle_generic_describe(&kernel_name, &kernel_path, instance_name, json_output).await;
                } else {
                    eprintln!("Error: describe requires an instance name");
                    eprintln!("Usage: ck {} describe <name>", command_name);
                    std::process::exit(1);
                }
            }
        }

        // Route to emit (for non-generic operations)
        println!("Routing {} to kernel: {}", command_name, kernel_name);
        handle_emit(&kernel_name, &payload).await?;
    } else {
        eprintln!("Error: Unknown command '{}'\n", command_name);
        print_custom_help();
        std::process::exit(1);
    }

    Ok(())
}
