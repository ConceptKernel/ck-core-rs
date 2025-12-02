//! Edge Kernel implementation
//!
//! Manages edge relationships between kernels and routes instances
//!
//! Reference: Node.js v1.3.14 - EdgeKernel.js

use crate::drivers::FileSystemDriver;
use crate::edge::EdgeMetadata;
use crate::errors::{CkpError, Result};
use crate::ontology::{OntologyLibrary, OntologyReader};
use crate::process_tracker::ProcessTracker;
use crate::continuant_tracker::ContinuantTracker;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// EdgeKernel - manages edge metadata and instance routing
pub struct EdgeKernel {
    /// Root directory (concepts/)
    root: PathBuf,

    /// Edges directory (.edges/)
    edges_dir: PathBuf,

    /// Metadata cache
    metadata_cache: HashMap<String, EdgeMetadata>,

    /// Ontology library for semantic edge validation (Phase 4 Stage 1)
    ontology_library: Option<Arc<OntologyLibrary>>,

    /// Process tracker for Process URN tracking (Phase 4 Stage 1)
    process_tracker: Option<Arc<ProcessTracker>>,
}

impl EdgeKernel {
    /// Create new EdgeKernel
    ///
    /// # Arguments
    /// * `root` - Root directory (e.g., /path/to/project)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::edge::EdgeKernel;
    /// use std::path::PathBuf;
    ///
    /// let kernel = EdgeKernel::new(PathBuf::from("/project")).unwrap();
    /// ```
    pub fn new(root: PathBuf) -> Result<Self> {
        let edges_dir = root.join("concepts").join(".edges");

        // Create edges directory if it doesn't exist
        if !edges_dir.exists() {
            fs::create_dir_all(&edges_dir)?;
        }

        Ok(EdgeKernel {
            root,
            edges_dir,
            metadata_cache: HashMap::new(),
            ontology_library: None,
            process_tracker: None,
        })
    }

    /// Create EdgeKernel with OntologyLibrary and ProcessTracker (Phase 4 Stage 1)
    ///
    /// # Arguments
    /// * `root` - Root directory (e.g., /path/to/project)
    /// * `ontology_library` - Optional OntologyLibrary for semantic validation
    /// * `process_tracker` - Optional ProcessTracker for Process URN tracking
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::edge::EdgeKernel;
    /// use ckp_core::ontology::OntologyLibrary;
    /// use ckp_core::process_tracker::ProcessTracker;
    /// use std::path::PathBuf;
    /// use std::sync::Arc;
    ///
    /// let ontology = Arc::new(OntologyLibrary::new(PathBuf::from("/project")).unwrap());
    /// let tracker = Arc::new(ProcessTracker::new(PathBuf::from("/project")));
    /// let kernel = EdgeKernel::with_ontology(
    ///     PathBuf::from("/project"),
    ///     Some(ontology),
    ///     Some(tracker)
    /// ).unwrap();
    /// ```
    pub fn with_ontology(
        root: PathBuf,
        ontology_library: Option<Arc<OntologyLibrary>>,
        process_tracker: Option<Arc<ProcessTracker>>,
    ) -> Result<Self> {
        let edges_dir = root.join("concepts").join(".edges");

        // Create edges directory if it doesn't exist
        if !edges_dir.exists() {
            fs::create_dir_all(&edges_dir)?;
        }

        Ok(EdgeKernel {
            root,
            edges_dir,
            metadata_cache: HashMap::new(),
            ontology_library,
            process_tracker,
        })
    }

    /// Create a new edge between two kernels
    ///
    /// # Arguments
    /// * `predicate` - Edge predicate (PRODUCES, NOTIFIES, VALIDATES, TRIGGERS)
    /// * `source` - Source kernel name
    /// * `target` - Target kernel name
    ///
    /// # Returns
    /// EdgeMetadata for the created edge
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::edge::EdgeKernel;
    /// use std::path::PathBuf;
    ///
    /// let mut kernel = EdgeKernel::new(PathBuf::from("/project")).unwrap();
    /// let metadata = kernel.create_edge("PRODUCES", "Source", "Target").unwrap();
    /// assert_eq!(metadata.predicate, "PRODUCES");
    /// ```
    pub fn create_edge(
        &mut self,
        predicate: &str,
        source: &str,
        target: &str,
    ) -> Result<EdgeMetadata> {
        // Validate predicate
        self.validate_predicate(predicate)?;

        // Generate edge URN
        let urn = EdgeMetadata::generate_urn(predicate, source, target, "v1.3.16");

        // Check if edge already exists
        if self.get_edge(&urn)?.is_some() {
            return Err(CkpError::EdgeAlreadyExists(format!(
                "Edge already exists: {}",
                urn
            )));
        }

        // Create metadata
        let metadata = EdgeMetadata::new(predicate, source, target, "v1.3.16");

        // Create edge directory
        let edge_dir = self.edges_dir.join(&metadata.get_edge_name());
        fs::create_dir_all(&edge_dir)?;

        // Save metadata
        let metadata_path = edge_dir.join("edgekernel.yaml");
        let yaml = metadata.to_yaml()
            .map_err(|e| CkpError::IoError(format!("Failed to serialize metadata: {}", e)))?;
        fs::write(&metadata_path, yaml)?;

        // Update cache
        self.metadata_cache.insert(urn.clone(), metadata.clone());

        Ok(metadata)
    }

    /// Validate edge predicate (Phase 4 Stage 1: Semantic validation)
    ///
    /// Validates predicates against RDF ontology if OntologyLibrary is available.
    /// Falls back to hardcoded list for backward compatibility.
    fn validate_predicate(&self, predicate: &str) -> Result<()> {
        // Phase 4 Stage 1: Try semantic validation via OntologyLibrary
        if let Some(ontology) = &self.ontology_library {
            match ontology.get_edge_predicate(predicate) {
                Ok(rdf_predicate) => {
                    // Semantic validation succeeded
                    eprintln!(
                        "[EdgeKernel] Semantic validation: {} → {}",
                        predicate, rdf_predicate
                    );
                    return Ok(());
                }
                Err(e) => {
                    // Predicate not found in ontology, fall back to hardcoded list
                    eprintln!(
                        "[EdgeKernel] Predicate '{}' not in ontology ({}), trying fallback",
                        predicate, e
                    );
                    // Don't return error - continue to fallback validation below
                }
            }
        }

        // Fallback: Hardcoded validation for backward compatibility
        const VALID_PREDICATES: &[&str] = &[
            "PRODUCES",
            "NOTIFIES",
            "VALIDATES",
            "TRIGGERS",
            "REQUIRES",
            "LLM_ASSIST",
            "PROVIDES",
            "INVOKES",
            "GOVERNS",
            "AUDITS",
            "PRECEDES",
        ];

        if !VALID_PREDICATES.contains(&predicate) {
            return Err(CkpError::ValidationError(format!(
                "Invalid predicate: {}. Must be one of: {} (OntologyLibrary not available, using fallback validation)",
                predicate,
                VALID_PREDICATES.join(", ")
            )));
        }

        Ok(())
    }

    /// Get edge by URN
    ///
    /// # Arguments
    /// * `edge_urn` - Edge URN
    ///
    /// # Returns
    /// Some(EdgeMetadata) if found, None otherwise
    pub fn get_edge(&mut self, edge_urn: &str) -> Result<Option<EdgeMetadata>> {
        // Check cache first
        if let Some(metadata) = self.metadata_cache.get(edge_urn) {
            return Ok(Some(metadata.clone()));
        }

        // Load all edges and search
        let edges = self.list_all_edges()?;
        Ok(edges.into_iter().find(|e| e.urn == edge_urn))
    }

    /// List all edges
    ///
    /// # Returns
    /// Vector of all edge metadata
    pub fn list_all_edges(&mut self) -> Result<Vec<EdgeMetadata>> {
        if !self.edges_dir.exists() {
            return Ok(Vec::new());
        }

        let mut edges = Vec::new();

        for entry in fs::read_dir(&self.edges_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            // Load edgekernel.yaml (YAML-only format)
            let yaml_path = path.join("edgekernel.yaml");

            if !yaml_path.exists() {
                continue;
            }

            let content = fs::read_to_string(&yaml_path)?;
            let metadata = EdgeMetadata::from_yaml(&content)
                .map_err(|e| CkpError::IoError(format!("Failed to parse edge metadata from YAML: {}", e)))?;

            // Update cache
            self.metadata_cache
                .insert(metadata.urn.clone(), metadata.clone());

            edges.push(metadata);
        }

        Ok(edges)
    }

    /// Remove edge by URN
    ///
    /// # Arguments
    /// * `edge_urn` - Edge URN
    ///
    /// # Returns
    /// true if removed, false if not found
    pub fn remove_edge(&mut self, edge_urn: &str) -> Result<bool> {
        // Find edge metadata
        let metadata = match self.get_edge(edge_urn)? {
            Some(m) => m,
            None => return Ok(false),
        };

        // Remove edge directory
        let edge_dir = self.edges_dir.join(&metadata.get_edge_name());
        if edge_dir.exists() {
            fs::remove_dir_all(&edge_dir)?;
        }

        // Remove from cache
        self.metadata_cache.remove(edge_urn);

        Ok(true)
    }

    /// Get outgoing edges from a kernel
    ///
    /// # Arguments
    /// * `kernel_name` - Source kernel name
    ///
    /// # Returns
    /// Vector of edges where kernel is the source
    pub fn get_outgoing_edges(&mut self, kernel_name: &str) -> Result<Vec<EdgeMetadata>> {
        let all_edges = self.list_all_edges()?;
        Ok(all_edges
            .into_iter()
            .filter(|e| e.source == kernel_name)
            .collect())
    }

    /// Get target queue path for per-edge queue routing
    ///
    /// # Arguments
    /// * `target` - Target kernel name
    /// * `predicate` - Edge predicate
    /// * `source` - Source kernel name
    ///
    /// # Returns
    /// Path to target's per-edge queue
    ///
    /// # Example
    /// Returns: /concepts/BakeCake/queue/edges/PRODUCES.MixIngredients/
    pub fn get_target_queue_path(&self, target: &str, predicate: &str, source: &str) -> PathBuf {
        self.root
            .join("concepts")
            .join(target)
            .join("queue")
            .join("edges")
            .join(format!("{}.{}", predicate, source))
    }

    /// Route instance to target kernels (Phase 4 Stage 1: Process URN tracking)
    ///
    /// Creates symlinks in per-edge queues for each target with BFO temporal tracking
    ///
    /// # Arguments
    /// * `instance_path` - Path to source instance
    /// * `source_kernel` - Source kernel name
    ///
    /// # Returns
    /// Vector of created symlink paths
    pub fn route_instance(
        &mut self,
        instance_path: &Path,
        source_kernel: &str,
    ) -> Result<Vec<PathBuf>> {
        // Extract txId from instance path
        let tx_id = instance_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        // Get outgoing edges
        let edges = self.get_outgoing_edges(source_kernel)?;

        if edges.is_empty() {
            return Ok(Vec::new());
        }

        // Phase 4 Stage 1: Create Process URN for edge routing
        if let Some(tracker) = &self.process_tracker {
            let process_urn = tracker.generate_process_urn("EdgeRoute", tx_id);

            let mut participants = HashMap::new();
            participants.insert("source_kernel".to_string(), serde_json::json!(source_kernel));
            participants.insert("tx_id".to_string(), serde_json::json!(tx_id));
            participants.insert("instance_path".to_string(), serde_json::json!(instance_path.display().to_string()));

            let mut metadata = HashMap::new();
            metadata.insert("edge_count".to_string(), serde_json::json!(edges.len()));

            if let Err(e) = tracker.create_process("EdgeRoute", tx_id, participants, metadata) {
                eprintln!("[EdgeKernel] Failed to create process tracking: {}", e);
            } else {
                eprintln!("[EdgeKernel] Created Process URN: {}", process_urn);

                // Add temporal part: routing started
                let mut routing_data = HashMap::new();
                routing_data.insert("phase".to_string(), serde_json::json!("routing_started"));
                routing_data.insert("source".to_string(), serde_json::json!(source_kernel));
                let _ = tracker.add_temporal_part(&process_urn, "routing", routing_data);

                // Phase 4 Stage 2: Record Continuant participation (BFO participates_in relation)
                let continuant_tracker = ContinuantTracker::new(self.root.clone());
                let source_urn = continuant_tracker.generate_continuant_urn("Kernel", source_kernel);
                let mut participation_metadata = HashMap::new();
                participation_metadata.insert("tx_id".to_string(), serde_json::json!(tx_id));
                let _ = continuant_tracker.record_participation(
                    &source_urn,
                    &process_urn,
                    "source",
                    participation_metadata,
                );
            }
        }

        let mut routed_paths = Vec::new();

        for edge in edges {
            // Check authorization
            if !self.is_edge_authorized(&edge.target, &edge.urn)? {
                eprintln!(
                    "[EdgeKernel] Edge not authorized: {} -> {}",
                    source_kernel, edge.target
                );

                // Track authorization failure
                if let Some(tracker) = &self.process_tracker {
                    let process_urn = tracker.generate_process_urn("EdgeRoute", tx_id);
                    let mut failed_data = HashMap::new();
                    failed_data.insert("target".to_string(), serde_json::json!(edge.target));
                    failed_data.insert("reason".to_string(), serde_json::json!("not_authorized"));
                    let _ = tracker.add_temporal_part(&process_urn, "failed", failed_data);
                }

                continue;
            }

            // Get target queue path
            let target_queue = self.get_target_queue_path(&edge.target, &edge.predicate, &edge.source);

            // Create per-edge queue if not exists
            fs::create_dir_all(&target_queue)?;

            // Create symlink using FileSystemDriver
            let driver = FileSystemDriver::new(self.root.clone(), edge.target.clone());
            let symlink_path = driver.create_symlink(instance_path, &target_queue, None)?;

            routed_paths.push(symlink_path.clone());

            // Track successful delivery
            if let Some(tracker) = &self.process_tracker {
                let process_urn = tracker.generate_process_urn("EdgeRoute", tx_id);
                let mut delivered_data = HashMap::new();
                delivered_data.insert("target".to_string(), serde_json::json!(edge.target));
                delivered_data.insert("predicate".to_string(), serde_json::json!(edge.predicate));
                delivered_data.insert("symlink_path".to_string(), serde_json::json!(symlink_path.display().to_string()));
                let _ = tracker.add_temporal_part(&process_urn, "delivered", delivered_data);
            }
        }

        Ok(routed_paths)
    }

    /// Check if edge is authorized by target kernel
    ///
    /// Reads target kernel's conceptkernel.yaml and checks if edge is in allowed list
    ///
    /// # Arguments
    /// * `target_kernel` - Target kernel name
    /// * `edge_urn` - Edge URN
    ///
    /// # Returns
    /// true if authorized, false otherwise
    pub fn is_edge_authorized(&self, target_kernel: &str, edge_urn: &str) -> Result<bool> {
        let ontology_path = self
            .root
            .join("concepts")
            .join(target_kernel)
            .join("conceptkernel.yaml");

        if !ontology_path.exists() {
            // No ontology = no restrictions (allow by default)
            return Ok(true);
        }

        let reader = OntologyReader::new(self.root.clone());
        let is_authorized = reader.is_edge_authorized(target_kernel, edge_urn)?;

        Ok(is_authorized)
    }

    /// Load edge contracts from kernel's ontology
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name
    ///
    /// # Returns
    /// Vector of authorized edge URNs
    pub fn load_edge_contracts(&self, kernel_name: &str) -> Result<Vec<String>> {
        let reader = OntologyReader::new(self.root.clone());
        let edges = reader.read_edges(kernel_name)?;
        Ok(edges)
    }

    /// Get edges directory path
    pub fn get_edges_dir(&self) -> &Path {
        &self.edges_dir
    }

    /// List all edge URNs
    ///
    /// # Returns
    /// Vector of edge URN strings
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::edge::EdgeKernel;
    /// use std::path::PathBuf;
    ///
    /// let mut kernel = EdgeKernel::new(PathBuf::from("/project")).unwrap();
    /// let urns = kernel.list_edges().unwrap();
    /// for urn in urns {
    ///     println!("Edge: {}", urn);
    /// }
    /// ```
    pub fn list_edges(&mut self) -> Result<Vec<String>> {
        let edges = self.list_all_edges()?;
        Ok(edges.into_iter().map(|e| e.urn).collect())
    }

    /// Get edge URNs for a specific kernel
    ///
    /// # Arguments
    /// * `kernel_name` - Source kernel name
    ///
    /// # Returns
    /// Vector of edge URN strings where kernel is the source
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::edge::EdgeKernel;
    /// use std::path::PathBuf;
    ///
    /// let mut kernel = EdgeKernel::new(PathBuf::from("/project")).unwrap();
    /// let urns = kernel.get_kernel_edges("MyKernel").unwrap();
    /// for urn in urns {
    ///     println!("Outgoing edge: {}", urn);
    /// }
    /// ```
    pub fn get_kernel_edges(&mut self, kernel_name: &str) -> Result<Vec<String>> {
        let edges = self.get_outgoing_edges(kernel_name)?;
        Ok(edges.into_iter().map(|e| e.urn).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, EdgeKernel) {
        let temp_dir = TempDir::new().unwrap();
        let kernel = EdgeKernel::new(temp_dir.path().to_path_buf()).unwrap();
        (temp_dir, kernel)
    }

    #[test]
    fn test_edge_kernel_initialization() {
        let (_temp, kernel) = setup_test_env();

        assert!(kernel.edges_dir.exists());
        assert_eq!(kernel.metadata_cache.len(), 0);
    }

    #[test]
    fn test_edges_directory_creation() {
        let temp_dir = TempDir::new().unwrap();
        let kernel = EdgeKernel::new(temp_dir.path().to_path_buf()).unwrap();

        let edges_dir = temp_dir.path().join("concepts").join(".edges");
        assert!(edges_dir.exists());
        assert!(edges_dir.is_dir());
    }

    #[test]
    fn test_create_edge() {
        let (_temp, mut kernel) = setup_test_env();

        let metadata = kernel
            .create_edge("PRODUCES", "Source", "Target")
            .unwrap();

        assert_eq!(metadata.predicate, "PRODUCES");
        assert_eq!(metadata.source, "Source");
        assert_eq!(metadata.target, "Target");
        assert!(metadata.urn.contains("Edge.PRODUCES.Source-to-Target"));

        // Verify directory was created
        let edge_dir = kernel.edges_dir.join("PRODUCES.Source");
        assert!(edge_dir.exists());

        // Verify metadata file was created
        let metadata_file = edge_dir.join("edgekernel.yaml");
        assert!(metadata_file.exists());
    }

    #[test]
    fn test_create_duplicate_edge() {
        let (_temp, mut kernel) = setup_test_env();

        kernel.create_edge("PRODUCES", "Source", "Target").unwrap();

        let result = kernel.create_edge("PRODUCES", "Source", "Target");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already exists"));
    }

    #[test]
    fn test_create_edge_with_invalid_predicate() {
        let (_temp, mut kernel) = setup_test_env();

        let result = kernel.create_edge("INVALID", "Source", "Target");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid predicate"));
    }

    #[test]
    fn test_edge_metadata_persistence() {
        let (temp, mut kernel) = setup_test_env();

        kernel.create_edge("PRODUCES", "Source", "Target").unwrap();

        // Create new kernel instance to test persistence
        let mut kernel2 = EdgeKernel::new(temp.path().to_path_buf()).unwrap();
        let edges = kernel2.list_all_edges().unwrap();

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].predicate, "PRODUCES");
        assert_eq!(edges[0].source, "Source");
    }

    #[test]
    fn test_list_all_edges() {
        let (_temp, mut kernel) = setup_test_env();

        kernel.create_edge("PRODUCES", "Source1", "Target1").unwrap();
        kernel.create_edge("NOTIFIES", "Source2", "Target2").unwrap();
        kernel.create_edge("VALIDATES", "Source3", "Target3").unwrap();

        let edges = kernel.list_all_edges().unwrap();
        assert_eq!(edges.len(), 3);

        let predicates: Vec<_> = edges.iter().map(|e| e.predicate.as_str()).collect();
        assert!(predicates.contains(&"PRODUCES"));
        assert!(predicates.contains(&"NOTIFIES"));
        assert!(predicates.contains(&"VALIDATES"));
    }

    #[test]
    fn test_get_edge_by_urn() {
        let (_temp, mut kernel) = setup_test_env();

        let created = kernel.create_edge("PRODUCES", "Source", "Target").unwrap();

        let found = kernel.get_edge(&created.urn).unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().urn, created.urn);

        let not_found = kernel.get_edge("ckp://Edge.NOTIFIES.Foo-to-Bar:v1.0.0").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_remove_edge() {
        let (_temp, mut kernel) = setup_test_env();

        let metadata = kernel.create_edge("PRODUCES", "Source", "Target").unwrap();

        let removed = kernel.remove_edge(&metadata.urn).unwrap();
        assert!(removed);

        let found = kernel.get_edge(&metadata.urn).unwrap();
        assert!(found.is_none());

        // Removing again should return false
        let removed_again = kernel.remove_edge(&metadata.urn).unwrap();
        assert!(!removed_again);
    }

    #[test]
    fn test_get_outgoing_edges() {
        let (_temp, mut kernel) = setup_test_env();

        kernel.create_edge("PRODUCES", "Source", "Target1").unwrap();
        kernel.create_edge("NOTIFIES", "Source", "Target2").unwrap();
        kernel.create_edge("PRODUCES", "Other", "Target3").unwrap();

        let edges = kernel.get_outgoing_edges("Source").unwrap();
        assert_eq!(edges.len(), 2);

        for edge in &edges {
            assert_eq!(edge.source, "Source");
        }
    }

    #[test]
    fn test_get_target_queue_path() {
        let (_temp, kernel) = setup_test_env();

        let path = kernel.get_target_queue_path("BakeCake", "PRODUCES", "MixIngredients");

        assert!(path.to_string_lossy().contains("concepts"));
        assert!(path.to_string_lossy().contains("BakeCake"));
        assert!(path.to_string_lossy().contains("queue/edges"));
        assert!(path.to_string_lossy().contains("PRODUCES.MixIngredients"));
    }

    #[test]
    fn test_per_edge_queue_creation() {
        let (temp, mut kernel) = setup_test_env();

        // Create source kernel
        let source_dir = temp.path().join("concepts/Source/storage");
        fs::create_dir_all(&source_dir).unwrap();

        // Create instance
        let instance_dir = source_dir.join("test-123.inst");
        fs::create_dir_all(&instance_dir).unwrap();
        fs::write(instance_dir.join("receipt.json"), "{}").unwrap();

        // Create edge
        kernel.create_edge("PRODUCES", "Source", "Target").unwrap();

        // Route instance
        let paths = kernel.route_instance(&instance_dir, "Source").unwrap();

        // Verify per-edge queue was created
        let target_queue = temp
            .path()
            .join("concepts/Target/queue/edges/PRODUCES.Source");
        assert!(target_queue.exists());
    }

    #[test]
    fn test_route_with_no_edges() {
        let (temp, mut kernel) = setup_test_env();

        let instance_dir = temp.path().join("concepts/Source/storage/test-123.inst");
        fs::create_dir_all(&instance_dir).unwrap();

        let paths = kernel.route_instance(&instance_dir, "Source").unwrap();
        assert_eq!(paths.len(), 0);
    }

    #[test]
    fn test_metadata_cache_management() {
        let (_temp, mut kernel) = setup_test_env();

        assert_eq!(kernel.metadata_cache.len(), 0);

        let metadata = kernel.create_edge("PRODUCES", "Source", "Target").unwrap();
        assert_eq!(kernel.metadata_cache.len(), 1);
        assert!(kernel.metadata_cache.contains_key(&metadata.urn));

        kernel.remove_edge(&metadata.urn).unwrap();
        assert_eq!(kernel.metadata_cache.len(), 0);
    }

    // Additional routing tests

    #[test]
    fn test_route_instance_to_single_target() {
        let (temp, mut kernel) = setup_test_env();

        // Create source kernel
        let source_dir = temp.path().join("concepts/Source/storage");
        fs::create_dir_all(&source_dir).unwrap();

        // Create instance
        let instance_dir = source_dir.join("test-123.inst");
        fs::create_dir_all(&instance_dir).unwrap();
        fs::write(instance_dir.join("receipt.json"), "{}").unwrap();

        // Create edge
        kernel.create_edge("PRODUCES", "Source", "Target").unwrap();

        // Route instance
        let paths = kernel.route_instance(&instance_dir, "Source").unwrap();

        assert_eq!(paths.len(), 1);
        assert!(paths[0].to_string_lossy().contains("Target"));
        assert!(paths[0].to_string_lossy().contains("PRODUCES.Source"));
    }

    #[test]
    fn test_route_instance_to_multiple_targets() {
        let (temp, mut kernel) = setup_test_env();

        // Create source kernel
        let source_dir = temp.path().join("concepts/Source/storage");
        fs::create_dir_all(&source_dir).unwrap();

        // Create instance
        let instance_dir = source_dir.join("test-123.inst");
        fs::create_dir_all(&instance_dir).unwrap();
        fs::write(instance_dir.join("receipt.json"), "{}").unwrap();

        // Create multiple edges
        kernel.create_edge("PRODUCES", "Source", "Target1").unwrap();
        kernel.create_edge("NOTIFIES", "Source", "Target2").unwrap();

        // Route instance
        let paths = kernel.route_instance(&instance_dir, "Source").unwrap();

        assert_eq!(paths.len(), 2);

        // Verify both targets received symlinks
        let has_target1 = paths.iter().any(|p| p.to_string_lossy().contains("Target1"));
        let has_target2 = paths.iter().any(|p| p.to_string_lossy().contains("Target2"));
        assert!(has_target1);
        assert!(has_target2);
    }

    #[test]
    fn test_instance_symlink_creation() {
        let (temp, mut kernel) = setup_test_env();

        // Create source kernel with instance
        let source_dir = temp.path().join("concepts/Source/storage");
        fs::create_dir_all(&source_dir).unwrap();

        let instance_dir = source_dir.join("test-123.inst");
        fs::create_dir_all(&instance_dir).unwrap();
        fs::write(instance_dir.join("receipt.json"), "{}").unwrap();

        // Create edge
        kernel.create_edge("PRODUCES", "Source", "Target").unwrap();

        // Route instance
        let paths = kernel.route_instance(&instance_dir, "Source").unwrap();

        assert_eq!(paths.len(), 1);

        // Verify symlink exists and points to original instance
        let symlink = &paths[0];
        assert!(symlink.symlink_metadata().is_ok());

        // Verify symlink points to source instance
        let real_path = fs::canonicalize(symlink).unwrap();
        assert_eq!(real_path, fs::canonicalize(&instance_dir).unwrap());
    }

    // Authorization tests

    #[test]
    fn test_edge_authorization_allowed() {
        let (temp, kernel) = setup_test_env();

        // Create target kernel with ontology
        let target_dir = temp.path().join("concepts/Target");
        fs::create_dir_all(&target_dir).unwrap();

        let edge_urn = "ckp://Edge.PRODUCES.Source-to-Target:v1.3.14";
        let ontology_content = format!(
            r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Target:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - {}
"#,
            edge_urn
        );

        fs::write(target_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

        // Check authorization
        let is_authorized = kernel.is_edge_authorized("Target", edge_urn).unwrap();
        assert!(is_authorized);
    }

    #[test]
    fn test_edge_authorization_denied() {
        let (temp, kernel) = setup_test_env();

        // Create target kernel with ontology that doesn't include the edge
        let target_dir = temp.path().join("concepts/Target");
        fs::create_dir_all(&target_dir).unwrap();

        let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Target:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - ckp://Edge.NOTIFIES.OtherSource-to-Target:v1.3.14
"#;

        fs::write(target_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

        // Check authorization for different edge
        let edge_urn = "ckp://Edge.PRODUCES.Source-to-Target:v1.3.14";
        let is_authorized = kernel.is_edge_authorized("Target", edge_urn).unwrap();
        assert!(!is_authorized);
    }

    #[test]
    fn test_load_edge_contracts_from_ontology() {
        let (temp, kernel) = setup_test_env();

        // Create kernel with ontology
        let kernel_dir = temp.path().join("concepts/TestKernel");
        fs::create_dir_all(&kernel_dir).unwrap();

        let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://TestKernel:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - ckp://Edge.PRODUCES.Source1-to-TestKernel:v1.3.14
      - ckp://Edge.NOTIFIES.Source2-to-TestKernel:v1.3.14
      - ckp://Edge.VALIDATES.Proof-to-TestKernel:v1.3.14
"#;

        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

        // Load contracts
        let contracts = kernel.load_edge_contracts("TestKernel").unwrap();

        assert_eq!(contracts.len(), 3);
        assert!(contracts.contains(&"ckp://Edge.PRODUCES.Source1-to-TestKernel:v1.3.14".to_string()));
        assert!(contracts.contains(&"ckp://Edge.NOTIFIES.Source2-to-TestKernel:v1.3.14".to_string()));
        assert!(contracts.contains(&"ckp://Edge.VALIDATES.Proof-to-TestKernel:v1.3.14".to_string()));
    }

    #[test]
    fn test_authorization_with_missing_ontology() {
        let (_temp, kernel) = setup_test_env();

        // Check authorization for non-existent kernel (no ontology)
        let edge_urn = "ckp://Edge.PRODUCES.Source-to-NonExistent:v1.3.14";
        let is_authorized = kernel.is_edge_authorized("NonExistent", edge_urn).unwrap();

        // Should return true (no restrictions = allow by default)
        assert!(is_authorized);
    }

    // ==================== PHASE 2.1: AUTHORIZATION EDGE CASES (+6 TESTS) ====================

    /// Test: Unauthorized edge rejection
    #[test]
    fn test_unauthorized_edge_rejection() {
        let (temp, kernel) = setup_test_env();

        // Create target kernel with strict whitelist
        let kernel_dir = temp.path().join("concepts/RestrictedTarget");
        fs::create_dir_all(&kernel_dir).unwrap();

        let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://RestrictedTarget:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - ckp://Edge.PRODUCES.AllowedSource-to-RestrictedTarget:v1.3.14
"#;

        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

        // Try unauthorized edge
        let unauthorized_edge = "ckp://Edge.PRODUCES.UnauthorizedSource-to-RestrictedTarget:v1.3.14";
        let is_authorized = kernel.is_edge_authorized("RestrictedTarget", unauthorized_edge).unwrap();

        assert!(!is_authorized, "Unlisted edge should be rejected");

        // Try authorized edge
        let authorized_edge = "ckp://Edge.PRODUCES.AllowedSource-to-RestrictedTarget:v1.3.14";
        let is_authorized = kernel.is_edge_authorized("RestrictedTarget", authorized_edge).unwrap();

        assert!(is_authorized, "Listed edge should be authorized");
    }

    /// Test: Wildcard authorization
    #[test]
    fn test_wildcard_authorization() {
        let (temp, kernel) = setup_test_env();

        // Create kernel with wildcard permission
        let kernel_dir = temp.path().join("concepts/WildcardTarget");
        fs::create_dir_all(&kernel_dir).unwrap();

        let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://WildcardTarget:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - "*"
"#;

        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

        // Any edge should be authorized with wildcard
        let edge1 = "ckp://Edge.PRODUCES.AnySource-to-WildcardTarget:v1.3.14";
        let edge2 = "ckp://Edge.NOTIFIES.OtherSource-to-WildcardTarget:v1.3.14";

        assert!(kernel.is_edge_authorized("WildcardTarget", edge1).unwrap());
        assert!(kernel.is_edge_authorized("WildcardTarget", edge2).unwrap());
    }

    /// Test: Authorization with version mismatch
    #[test]
    fn test_authorization_with_version_mismatch() {
        let (temp, kernel) = setup_test_env();

        // Create kernel with specific version requirement
        let kernel_dir = temp.path().join("concepts/VersionedTarget");
        fs::create_dir_all(&kernel_dir).unwrap();

        let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://VersionedTarget:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - ckp://Edge.PRODUCES.Source-to-VersionedTarget:v1.3.14
"#;

        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

        // Matching version should be authorized
        let matching_version = "ckp://Edge.PRODUCES.Source-to-VersionedTarget:v1.3.14";
        assert!(kernel.is_edge_authorized("VersionedTarget", matching_version).unwrap());

        // Different version should not match (exact URN matching)
        let different_version = "ckp://Edge.PRODUCES.Source-to-VersionedTarget:v1.3.15";
        assert!(!kernel.is_edge_authorized("VersionedTarget", different_version).unwrap());
    }

    /// Test: Authorization with empty whitelist
    #[test]
    fn test_authorization_empty_whitelist() {
        let (temp, kernel) = setup_test_env();

        // Create kernel with empty whitelist (deny all)
        let kernel_dir = temp.path().join("concepts/EmptyWhitelistTarget");
        fs::create_dir_all(&kernel_dir).unwrap();

        let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://EmptyWhitelistTarget:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges: []
"#;

        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

        // Any edge should be rejected with empty whitelist
        let edge = "ckp://Edge.PRODUCES.Source-to-EmptyWhitelistTarget:v1.3.14";
        assert!(!kernel.is_edge_authorized("EmptyWhitelistTarget", edge).unwrap());
    }

    /// Test: Authorization blacklist priority over whitelist
    #[test]
    fn test_authorization_blacklist_priority() {
        let (temp, kernel) = setup_test_env();

        // Create kernel with both whitelist and blacklist
        let kernel_dir = temp.path().join("concepts/BlacklistTarget");
        fs::create_dir_all(&kernel_dir).unwrap();

        let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://BlacklistTarget:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - "*"
      denied:
        - "ckp://Edge.PRODUCES.BlockedSource-to-BlacklistTarget:v1.3.14"
"#;

        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

        // Explicitly denied edge should be rejected even with wildcard allow
        let denied_edge = "ckp://Edge.PRODUCES.BlockedSource-to-BlacklistTarget:v1.3.14";
        assert!(!kernel.is_edge_authorized("BlacklistTarget", denied_edge).unwrap());

        // Other edges should be allowed
        let allowed_edge = "ckp://Edge.PRODUCES.AllowedSource-to-BlacklistTarget:v1.3.14";
        assert!(kernel.is_edge_authorized("BlacklistTarget", allowed_edge).unwrap());
    }

    /// Test: Authorization with partial URN patterns
    #[test]
    fn test_authorization_partial_patterns() {
        let (temp, kernel) = setup_test_env();

        // Create kernel with pattern-based authorization
        let kernel_dir = temp.path().join("concepts/PatternTarget");
        fs::create_dir_all(&kernel_dir).unwrap();

        let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://PatternTarget:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - ckp://Edge.PRODUCES.TrustedSource-to-PatternTarget:v1.3.14
      - ckp://Edge.NOTIFIES.*-to-PatternTarget:v1.3.14
"#;

        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

        // Exact match should work
        let exact_match = "ckp://Edge.PRODUCES.TrustedSource-to-PatternTarget:v1.3.14";
        assert!(kernel.is_edge_authorized("PatternTarget", exact_match).unwrap());

        // Pattern match with NOTIFIES - should match wildcard pattern
        let pattern_match = "ckp://Edge.NOTIFIES.AnySource-to-PatternTarget:v1.3.14";
        // Updated: Now supports wildcard pattern matching in queue_contract.edges
        assert!(kernel.is_edge_authorized("PatternTarget", pattern_match).unwrap());

        // Should not match non-NOTIFIES edges
        let non_match = "ckp://Edge.CONSUMES.AnySource-to-PatternTarget:v1.3.14";
        assert!(!kernel.is_edge_authorized("PatternTarget", non_match).unwrap());
    }

    // ==================== PHASE 2.2: COMPLEX ROUTING SCENARIOS (+6 TESTS) ====================

    /// Test: Route to multiple edges
    #[test]
    fn test_route_to_multiple_edges() {
        let (temp, mut kernel) = setup_test_env();

        // Create source kernel with multiple outgoing edges
        let source = "MultiSource";
        kernel.create_edge("PRODUCES", source, "Target1").unwrap();
        kernel.create_edge("NOTIFIES", source, "Target2").unwrap();
        kernel.create_edge("VALIDATES", source, "Target3").unwrap();

        // Create target kernels
        for target in &["Target1", "Target2", "Target3"] {
            let kernel_dir = temp.path().join(format!("concepts/{}", target));
            fs::create_dir_all(kernel_dir.join("queue/edges")).unwrap();

            // Create ontology that allows all edges
            let ontology = format!(r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - "*"
"#, target);
            fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();
        }

        // Create instance to route
        let storage_dir = temp.path().join(format!("concepts/{}/storage", source));
        fs::create_dir_all(&storage_dir).unwrap();
        let instance_dir = storage_dir.join("test-instance.inst");
        fs::create_dir_all(&instance_dir).unwrap();
        fs::write(instance_dir.join("receipt.json"), "{}").unwrap();

        // Route instance
        let routed_paths = kernel.route_instance(&instance_dir, source).unwrap();

        // Should have created symlinks in all 3 target queues
        assert_eq!(routed_paths.len(), 3, "Should route to all 3 targets");
    }

    /// Test: Route with missing target directory
    #[test]
    fn test_route_with_missing_target() {
        let (temp, mut kernel) = setup_test_env();

        let source = "ValidSource";
        let target = "MissingTarget";

        // Create edge but don't create target kernel directory
        kernel.create_edge("PRODUCES", source, target).unwrap();

        // Create instance
        let storage_dir = temp.path().join(format!("concepts/{}/storage", source));
        fs::create_dir_all(&storage_dir).unwrap();
        let instance_dir = storage_dir.join("test-instance.inst");
        fs::create_dir_all(&instance_dir).unwrap();
        fs::write(instance_dir.join("receipt.json"), "{}").unwrap();

        // Route should auto-create target queue directory
        let result = kernel.route_instance(&instance_dir, source);

        // Should succeed (auto-creates directories)
        assert!(result.is_ok());
        let routed_paths = result.unwrap();
        assert_eq!(routed_paths.len(), 1);

        // Verify target queue was created
        let target_queue = temp.path().join(format!("concepts/{}/queue/edges/PRODUCES.{}", target, source));
        assert!(target_queue.exists());
    }

    /// Test: Route with circular edge detection
    #[test]
    fn test_route_with_circular_edges() {
        let (temp, mut kernel) = setup_test_env();

        // Create circular edge chain: A -> B -> A
        kernel.create_edge("PRODUCES", "KernelA", "KernelB").unwrap();
        kernel.create_edge("PRODUCES", "KernelB", "KernelA").unwrap();

        // Create kernel directories
        for k in &["KernelA", "KernelB"] {
            let kernel_dir = temp.path().join(format!("concepts/{}", k));
            fs::create_dir_all(kernel_dir.join("queue/edges")).unwrap();

            let ontology = format!(r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - "*"
"#, k);
            fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();
        }

        // Create instance in KernelA
        let storage_dir = temp.path().join("concepts/KernelA/storage");
        fs::create_dir_all(&storage_dir).unwrap();
        let instance_dir = storage_dir.join("circular-test.inst");
        fs::create_dir_all(&instance_dir).unwrap();
        fs::write(instance_dir.join("receipt.json"), "{}").unwrap();

        // Route from KernelA (should route to KernelB only)
        let routed_paths = kernel.route_instance(&instance_dir, "KernelA").unwrap();
        assert_eq!(routed_paths.len(), 1);

        // Routing itself doesn't prevent circles - that's the governor's responsibility
        // This test documents that circular edges can exist
    }

    /// Test: Route instance with concurrent operations
    #[test]
    fn test_route_instance_concurrent() {
        use std::sync::Arc;
        use std::thread;

        let temp = TempDir::new().unwrap();
        let kernel = Arc::new(std::sync::Mutex::new(EdgeKernel::new(temp.path().to_path_buf()).unwrap()));

        // Create edge
        {
            let mut k = kernel.lock().unwrap();
            k.create_edge("PRODUCES", "ConcurrentSource", "ConcurrentTarget").unwrap();
        }

        // Create target kernel
        let target_dir = temp.path().join("concepts/ConcurrentTarget");
        fs::create_dir_all(target_dir.join("queue/edges")).unwrap();
        let ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://ConcurrentTarget:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - "*"
"#;
        fs::write(target_dir.join("conceptkernel.yaml"), ontology).unwrap();

        // Create multiple instances concurrently
        let mut handles = vec![];
        for i in 0..5 {
            let kernel_clone = Arc::clone(&kernel);
            let temp_path = temp.path().to_path_buf();

            let handle = thread::spawn(move || {
                let storage_dir = temp_path.join("concepts/ConcurrentSource/storage");
                fs::create_dir_all(&storage_dir).unwrap();
                let instance_dir = storage_dir.join(format!("concurrent-{}.inst", i));
                fs::create_dir_all(&instance_dir).unwrap();
                fs::write(instance_dir.join("receipt.json"), "{}").unwrap();

                let mut k = kernel_clone.lock().unwrap();
                k.route_instance(&instance_dir, "ConcurrentSource")
            });
            handles.push(handle);
        }

        // Wait for all threads
        let mut success_count = 0;
        for handle in handles {
            if handle.join().unwrap().is_ok() {
                success_count += 1;
            }
        }

        // All concurrent routes should succeed
        assert_eq!(success_count, 5, "All 5 concurrent routes should succeed");
    }

    /// Test: Edge queue with many symlinks (no overflow, just capacity)
    #[test]
    fn test_edge_queue_capacity() {
        let (temp, mut kernel) = setup_test_env();

        let source = "HighVolumeSource";
        let target = "HighVolumeTarget";

        kernel.create_edge("PRODUCES", source, target).unwrap();

        // Create target kernel
        let target_dir = temp.path().join(format!("concepts/{}", target));
        fs::create_dir_all(target_dir.join("queue/edges")).unwrap();
        let ontology = format!(r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - "*"
"#, target);
        fs::write(target_dir.join("conceptkernel.yaml"), ontology).unwrap();

        // Route 100 instances
        for i in 0..100 {
            let storage_dir = temp.path().join(format!("concepts/{}/storage", source));
            fs::create_dir_all(&storage_dir).unwrap();
            let instance_dir = storage_dir.join(format!("volume-{}.inst", i));
            fs::create_dir_all(&instance_dir).unwrap();
            fs::write(instance_dir.join("receipt.json"), "{}").unwrap();

            let result = kernel.route_instance(&instance_dir, source);
            assert!(result.is_ok(), "Routing instance {} should succeed", i);
        }

        // Verify all symlinks were created
        let target_queue = temp.path().join(format!("concepts/{}/queue/edges/PRODUCES.{}", target, source));
        let symlink_count = fs::read_dir(&target_queue)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("inst"))
            .count();

        assert_eq!(symlink_count, 100, "Should have 100 symlinks in target queue");
    }

    /// Test: Edge routing with symlinks verification
    #[test]
    fn test_edge_routing_with_symlinks() {
        let (temp, mut kernel) = setup_test_env();

        let source = "SymlinkSource";
        let target = "SymlinkTarget";

        kernel.create_edge("PRODUCES", source, target).unwrap();

        // Create target kernel
        let target_dir = temp.path().join(format!("concepts/{}", target));
        fs::create_dir_all(target_dir.join("queue/edges")).unwrap();
        let ontology = format!(r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - "*"
"#, target);
        fs::write(target_dir.join("conceptkernel.yaml"), ontology).unwrap();

        // Create instance with known content
        let storage_dir = temp.path().join(format!("concepts/{}/storage", source));
        fs::create_dir_all(&storage_dir).unwrap();
        let instance_dir = storage_dir.join("symlink-test.inst");
        fs::create_dir_all(&instance_dir).unwrap();
        let test_data = r#"{"test": "symlink_data", "value": 42}"#;
        fs::write(instance_dir.join("receipt.json"), test_data).unwrap();

        // Route instance
        let routed_paths = kernel.route_instance(&instance_dir, source).unwrap();
        assert_eq!(routed_paths.len(), 1);

        let symlink_path = &routed_paths[0];

        // Verify symlink exists
        assert!(symlink_path.symlink_metadata().is_ok(), "Symlink should exist");

        // Verify it's actually a symlink
        let metadata = fs::symlink_metadata(symlink_path).unwrap();
        assert!(metadata.is_symlink(), "Should be a symbolic link");

        // Read through symlink
        let receipt_path = symlink_path.join("receipt.json");
        let content = fs::read_to_string(&receipt_path).unwrap();
        assert_eq!(content, test_data, "Content should match original through symlink");

        // Verify symlink uses relative path
        let link_target = fs::read_link(symlink_path).unwrap();
        let link_str = link_target.to_string_lossy();
        assert!(!link_str.starts_with('/'), "Symlink should use relative path");
        assert!(link_str.contains("../"), "Relative path should have ../ components");
    }

    // ==================== PHASE 2.3: EDGE KERNEL POLISH TESTS (+4 TESTS) ====================

    /// Test: Advanced circular edge detection (A -> B -> C -> A)
    #[test]
    fn test_advanced_circular_edge_detection() {
        let (temp, mut kernel) = setup_test_env();

        // Create circular edge chain: A -> B -> C -> A
        kernel.create_edge("PRODUCES", "KernelA", "KernelB").unwrap();
        kernel.create_edge("PRODUCES", "KernelB", "KernelC").unwrap();
        kernel.create_edge("PRODUCES", "KernelC", "KernelA").unwrap();

        // Verify all edges were created successfully
        let all_edges = kernel.list_all_edges().unwrap();
        assert_eq!(all_edges.len(), 3, "Should have 3 edges in circular chain");

        // Verify each kernel has exactly one outgoing edge
        let a_edges = kernel.get_outgoing_edges("KernelA").unwrap();
        assert_eq!(a_edges.len(), 1);
        assert_eq!(a_edges[0].target, "KernelB");

        let b_edges = kernel.get_outgoing_edges("KernelB").unwrap();
        assert_eq!(b_edges.len(), 1);
        assert_eq!(b_edges[0].target, "KernelC");

        let c_edges = kernel.get_outgoing_edges("KernelC").unwrap();
        assert_eq!(c_edges.len(), 1);
        assert_eq!(c_edges[0].target, "KernelA");

        // Create kernel directories with wildcard authorization
        for k in &["KernelA", "KernelB", "KernelC"] {
            let kernel_dir = temp.path().join(format!("concepts/{}", k));
            fs::create_dir_all(kernel_dir.join("queue/edges")).unwrap();

            let ontology = format!(r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - "*"
"#, k);
            fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();
        }

        // Create instance in KernelA
        let storage_dir = temp.path().join("concepts/KernelA/storage");
        fs::create_dir_all(&storage_dir).unwrap();
        let instance_dir = storage_dir.join("circular-multi-hop.inst");
        fs::create_dir_all(&instance_dir).unwrap();
        fs::write(instance_dir.join("receipt.json"), "{}").unwrap();

        // Route from KernelA - should route to KernelB only (immediate target)
        let routed_paths = kernel.route_instance(&instance_dir, "KernelA").unwrap();
        assert_eq!(routed_paths.len(), 1);
        assert!(routed_paths[0].to_string_lossy().contains("KernelB"));

        // EdgeKernel doesn't prevent circular routing - it's stateless
        // Each routing call only routes to immediate targets
        // Circle detection/prevention is the governor's responsibility
    }

    /// Test: Circular edge prevention with multi-hop cycles
    #[test]
    fn test_circular_edge_prevention_multi_hop() {
        let (temp, mut kernel) = setup_test_env();

        // Create multi-hop circular chains
        // Chain 1: A -> B -> A
        kernel.create_edge("PRODUCES", "ChainA1", "ChainA2").unwrap();
        kernel.create_edge("PRODUCES", "ChainA2", "ChainA1").unwrap();

        // Chain 2: X -> Y -> Z -> X
        kernel.create_edge("NOTIFIES", "ChainX", "ChainY").unwrap();
        kernel.create_edge("NOTIFIES", "ChainY", "ChainZ").unwrap();
        kernel.create_edge("NOTIFIES", "ChainZ", "ChainX").unwrap();

        // Verify both cycles exist
        let all_edges = kernel.list_all_edges().unwrap();
        assert_eq!(all_edges.len(), 5, "Should have 5 edges across both chains");

        // Verify Chain 1 forms a cycle
        let a1_edges = kernel.get_outgoing_edges("ChainA1").unwrap();
        assert_eq!(a1_edges.len(), 1);
        assert_eq!(a1_edges[0].target, "ChainA2");

        let a2_edges = kernel.get_outgoing_edges("ChainA2").unwrap();
        assert_eq!(a2_edges.len(), 1);
        assert_eq!(a2_edges[0].target, "ChainA1");

        // Verify Chain 2 forms a cycle
        let x_edges = kernel.get_outgoing_edges("ChainX").unwrap();
        assert_eq!(x_edges.len(), 1);
        assert_eq!(x_edges[0].target, "ChainY");

        let y_edges = kernel.get_outgoing_edges("ChainY").unwrap();
        assert_eq!(y_edges.len(), 1);
        assert_eq!(y_edges[0].target, "ChainZ");

        let z_edges = kernel.get_outgoing_edges("ChainZ").unwrap();
        assert_eq!(z_edges.len(), 1);
        assert_eq!(z_edges[0].target, "ChainX");

        // EdgeKernel allows circular edges to be created
        // It's the responsibility of the governor/runtime to detect and prevent infinite loops
        // This test documents that circular topologies are structurally valid
    }

    /// Test: Edge metadata propagation through routing
    #[test]
    fn test_edge_metadata_propagation() {
        let (temp, mut kernel) = setup_test_env();

        let source = "MetadataSource";
        let target = "MetadataTarget";

        // Create edge and verify metadata
        let metadata = kernel.create_edge("PRODUCES", source, target).unwrap();

        assert_eq!(metadata.source, source);
        assert_eq!(metadata.target, target);
        assert_eq!(metadata.predicate, "PRODUCES");
        assert_eq!(metadata.version, "v1.3.16");
        assert!(metadata.urn.contains("Edge.PRODUCES.MetadataSource-to-MetadataTarget"));

        // Retrieve metadata via URN
        let retrieved_metadata = kernel.get_edge(&metadata.urn).unwrap();
        assert!(retrieved_metadata.is_some());
        let retrieved = retrieved_metadata.unwrap();

        // Verify all metadata fields propagated correctly
        assert_eq!(retrieved.source, metadata.source);
        assert_eq!(retrieved.target, metadata.target);
        assert_eq!(retrieved.predicate, metadata.predicate);
        assert_eq!(retrieved.version, metadata.version);
        assert_eq!(retrieved.urn, metadata.urn);

        // Verify metadata is cached
        assert!(kernel.metadata_cache.contains_key(&metadata.urn));
        let cached = kernel.metadata_cache.get(&metadata.urn).unwrap();
        assert_eq!(cached.source, metadata.source);
        assert_eq!(cached.predicate, metadata.predicate);

        // Verify metadata persisted to disk
        let edge_dir = kernel.edges_dir.join(&metadata.get_edge_name());
        let metadata_file = edge_dir.join("edgekernel.yaml");
        assert!(metadata_file.exists(), "Metadata file should exist on disk");

        let content = fs::read_to_string(&metadata_file).unwrap();
        let disk_metadata = EdgeMetadata::from_yaml(&content).unwrap();
        assert_eq!(disk_metadata.api_version, "conceptkernel/v1");
        assert_eq!(disk_metadata.kind, "Edge");
        assert_eq!(disk_metadata.urn, metadata.urn);
        assert_eq!(disk_metadata.source, metadata.source);
        assert_eq!(disk_metadata.target, metadata.target);
    }

    /// Test: Edge routing preserves metadata throughout the routing chain
    #[test]
    fn test_edge_routing_with_metadata() {
        let (temp, mut kernel) = setup_test_env();

        let source = "RoutingSource";
        let intermediate = "RoutingIntermediate";
        let final_target = "RoutingFinal";

        // Create routing chain: Source -> Intermediate -> Final
        let metadata1 = kernel.create_edge("PRODUCES", source, intermediate).unwrap();
        let metadata2 = kernel.create_edge("NOTIFIES", intermediate, final_target).unwrap();

        // Create kernel directories with wildcard authorization
        for k in &[source, intermediate, final_target] {
            let kernel_dir = temp.path().join(format!("concepts/{}", k));
            fs::create_dir_all(kernel_dir.join("queue/edges")).unwrap();

            let ontology = format!(r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - "*"
"#, k);
            fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();
        }

        // Create instance in source
        let storage_dir = temp.path().join(format!("concepts/{}/storage", source));
        fs::create_dir_all(&storage_dir).unwrap();
        let instance_dir = storage_dir.join("routing-metadata.inst");
        fs::create_dir_all(&instance_dir).unwrap();
        fs::write(instance_dir.join("receipt.json"), r#"{"step": 1}"#).unwrap();

        // Route from source to intermediate
        let routed_paths = kernel.route_instance(&instance_dir, source).unwrap();
        assert_eq!(routed_paths.len(), 1);

        // Verify routing preserves edge metadata
        let intermediate_queue = temp.path().join(format!("concepts/{}/queue/edges/PRODUCES.{}", intermediate, source));
        assert!(intermediate_queue.exists(), "Intermediate queue should exist");

        // Verify the queue name encodes the metadata (predicate.source)
        let queue_name = intermediate_queue.file_name().unwrap().to_string_lossy();
        assert_eq!(queue_name, format!("PRODUCES.{}", source));

        // Create instance in intermediate for next hop
        let intermediate_storage = temp.path().join(format!("concepts/{}/storage", intermediate));
        fs::create_dir_all(&intermediate_storage).unwrap();
        let intermediate_instance = intermediate_storage.join("routing-metadata-step2.inst");
        fs::create_dir_all(&intermediate_instance).unwrap();
        fs::write(intermediate_instance.join("receipt.json"), r#"{"step": 2}"#).unwrap();

        // Route from intermediate to final
        let routed_paths2 = kernel.route_instance(&intermediate_instance, intermediate).unwrap();
        assert_eq!(routed_paths2.len(), 1);

        // Verify final target queue preserves edge metadata
        let final_queue = temp.path().join(format!("concepts/{}/queue/edges/NOTIFIES.{}", final_target, intermediate));
        assert!(final_queue.exists(), "Final queue should exist");

        let final_queue_name = final_queue.file_name().unwrap().to_string_lossy();
        assert_eq!(final_queue_name, format!("NOTIFIES.{}", intermediate));

        // Verify metadata is retrievable for both edges
        let retrieved1 = kernel.get_edge(&metadata1.urn).unwrap().unwrap();
        assert_eq!(retrieved1.predicate, "PRODUCES");
        assert_eq!(retrieved1.source, source);
        assert_eq!(retrieved1.target, intermediate);

        let retrieved2 = kernel.get_edge(&metadata2.urn).unwrap().unwrap();
        assert_eq!(retrieved2.predicate, "NOTIFIES");
        assert_eq!(retrieved2.source, intermediate);
        assert_eq!(retrieved2.target, final_target);

        // Verify both edges are cached
        assert_eq!(kernel.metadata_cache.len(), 2, "Should have 2 edges in cache");
    }
}
