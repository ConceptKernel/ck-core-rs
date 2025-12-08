// EdgeRouterDaemon - Routes instances based on notification_contract
//
// Responsibilities:
// - Watch all storage/ directories across all kernels
// - Read notification_contract from conceptkernel.yaml
// - Auto-create edges (PRODUCES predicate by default)
// - Route instances to target kernels via EdgeKernel
// - Track routing with Process URNs
//
// Architecture:
// - Storage Watcher (notify crate) - Detects *.inst creation
// - Notification Contract Resolver - Reads targets from ontology
// - Edge Lifecycle Manager - Auto-creates edges on first instance
// - Routing Engine - Wraps EdgeKernel::route_instance()

use crate::edge::EdgeKernel;
use crate::ontology::{OntologyReader, OntologyLibrary};
use crate::process_tracker::ProcessTracker;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher, Config as NotifyConfig};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::HashMap;

pub struct EdgeRouterDaemon {
    root: PathBuf,
    edge_kernel: Arc<std::sync::Mutex<EdgeKernel>>,
    ontology_reader: OntologyReader,
    _ontology_library: Option<Arc<OntologyLibrary>>,
    _process_tracker: Arc<ProcessTracker>,
    verbose: bool,
    // Cache: kernel_name -> List<(target, predicate)>
    notification_cache: Arc<std::sync::Mutex<HashMap<String, Vec<(String, String)>>>>,
}

impl EdgeRouterDaemon {
    pub fn new(root: PathBuf, verbose: bool) -> Result<Self, Box<dyn std::error::Error>> {
        // Initialize EdgeKernel with OntologyLibrary and ProcessTracker
        let ontology_library = OntologyLibrary::new(root.clone()).ok().map(Arc::new);
        let process_tracker = Arc::new(ProcessTracker::new(root.clone())?);

        let edge_kernel = EdgeKernel::with_ontology(
            root.clone(),
            ontology_library.clone(),
            Some(process_tracker.clone()),
        )?;

        Ok(Self {
            root: root.clone(),
            edge_kernel: Arc::new(std::sync::Mutex::new(edge_kernel)),
            ontology_reader: OntologyReader::new(root.clone()),
            _ontology_library: ontology_library,
            _process_tracker: process_tracker,
            verbose,
            notification_cache: Arc::new(std::sync::Mutex::new(HashMap::new())),
        })
    }

    pub fn start(&self, shutdown: Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error>> {
        self.log("[EdgeRouter] Starting daemon...");
        self.log(&format!("[EdgeRouter] Project: {}", self.root.display()));

        // Set up filesystem watcher
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, NotifyConfig::default())?;

        // Watch concepts/ recursively
        let concepts_path = self.root.join("concepts");
        if !concepts_path.exists() {
            return Err(format!("Concepts directory not found: {}", concepts_path.display()).into());
        }

        self.log(&format!("[EdgeRouter] Watching: {}", concepts_path.display()));
        watcher.watch(&concepts_path, RecursiveMode::Recursive)?;

        self.log("[EdgeRouter] Ready - Waiting for instance creation events");

        // Event loop
        loop {
            if shutdown.load(Ordering::SeqCst) {
                self.log("[EdgeRouter] Shutdown signal received, exiting...");
                break;
            }

            match rx.recv_timeout(std::time::Duration::from_millis(1000)) {
                Ok(Ok(event)) => {
                    self.handle_filesystem_event(event);
                }
                Ok(Err(e)) => {
                    eprintln!("[EdgeRouter] Watcher error: {}", e);
                }
                Err(_) => {
                    // Timeout - continue
                }
            }
        }

        Ok(())
    }

    fn handle_filesystem_event(&self, event: Event) {
        // Only care about Create events
        if !matches!(event.kind, EventKind::Create(_)) {
            return;
        }

        for path in &event.paths {
            // Check if this is a storage instance: concepts/{Kernel}/storage/{tx-id}.inst
            let path_str = path.to_string_lossy();

            if !path_str.contains("/storage/") {
                continue;
            }

            // Check if it's an .inst directory
            if !path_str.ends_with(".inst") {
                continue;
            }

            // Extract kernel name from path
            let kernel_name = match self.extract_kernel_from_path(path) {
                Some(name) => name,
                None => {
                    if self.verbose {
                        eprintln!("[EdgeRouter] Could not extract kernel name from: {}", path.display());
                    }
                    continue;
                }
            };

            self.log(&format!("[EdgeRouter] Instance created: {} (kernel: {})", path.display(), kernel_name));

            // Get notification contract
            let targets = match self.get_notification_targets(&kernel_name) {
                Ok(targets) => targets,
                Err(e) => {
                    if self.verbose {
                        eprintln!("[EdgeRouter] Error reading notification contract for {}: {}", kernel_name, e);
                    }
                    continue;
                }
            };

            if targets.is_empty() {
                if self.verbose {
                    self.log(&format!("[EdgeRouter] No notification targets for {}", kernel_name));
                }
                continue;
            }

            self.log(&format!("[EdgeRouter] Routing to {} target(s)", targets.len()));

            // Route to each target
            for (target, predicate) in targets {
                if let Err(e) = self.route_to_target(path, &kernel_name, &target, &predicate) {
                    eprintln!("[EdgeRouter] Failed to route to {}: {}", target, e);
                }
            }
        }
    }

    fn extract_kernel_from_path(&self, path: &Path) -> Option<String> {
        // Path format: /path/to/concepts/{KernelName}/storage/tx-123.inst
        let components: Vec<_> = path.components().collect();

        for (i, comp) in components.iter().enumerate() {
            if comp.as_os_str() == "concepts" && i + 1 < components.len() {
                return Some(components[i + 1].as_os_str().to_string_lossy().to_string());
            }
        }

        None
    }

    fn get_notification_targets(&self, kernel_name: &str) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
        // Check cache first
        {
            let cache = self.notification_cache.lock().unwrap();
            if let Some(targets) = cache.get(kernel_name) {
                if self.verbose {
                    self.log(&format!("[EdgeRouter] Cache hit for {}", kernel_name));
                }
                return Ok(targets.clone());
            }
        }

        if self.verbose {
            self.log(&format!("[EdgeRouter] Reading notification_contract for {}", kernel_name));
        }

        // Read from ontology
        let contract = self.ontology_reader.read_notification_contract(kernel_name)?;

        // Convert to (target, predicate) tuples
        // Default predicate: PRODUCES
        let targets: Vec<(String, String)> = contract
            .into_iter()
            .map(|notif| {
                (notif.target_kernel, "PRODUCES".to_string())
            })
            .collect();

        // Update cache
        {
            let mut cache = self.notification_cache.lock().unwrap();
            cache.insert(kernel_name.to_string(), targets.clone());
        }

        Ok(targets)
    }

    fn route_to_target(&self, instance_path: &Path, source: &str, target: &str, predicate: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut edge_kernel = self.edge_kernel.lock().unwrap();

        // Check if edge exists, create if not
        let edge_urn = format!("ckp://Edge.{}.{}-to-{}:v1.3.16", predicate, source, target);

        if edge_kernel.get_edge(&edge_urn)?.is_none() {
            self.log(&format!("[EdgeRouter] Creating edge: {} -> {} ({})", source, target, predicate));
            edge_kernel.create_edge(predicate, source, target)?;
        }

        // Route instance
        let routed_paths = edge_kernel.route_instance(instance_path, source)?;

        self.log(&format!("[EdgeRouter] Routed {} to {} (created {} symlink(s))",
                  instance_path.file_name().unwrap().to_string_lossy(),
                  target,
                  routed_paths.len()));

        if self.verbose {
            for path in &routed_paths {
                self.log(&format!("[EdgeRouter]   -> {}", path.display()));
            }
        }

        Ok(())
    }

    fn log(&self, message: &str) {
        eprintln!("{}", message);
    }
}
