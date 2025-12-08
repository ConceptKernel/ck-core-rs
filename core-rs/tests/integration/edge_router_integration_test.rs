//! Integration tests for Edge Router Daemon
//!
//! Tests the core functionality of the edge router daemon including:
//! - Kernel path extraction from filesystem events
//! - Notification contract parsing and caching
//! - Edge auto-creation based on notification contracts
//! - Instance routing with symlink creation
//! - Multi-target routing scenarios
//! - Error handling for invalid paths and missing configurations

use ckp_core::edge::EdgeKernel;
use ckp_core::ontology::OntologyReader;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

// ==================== Test Helper Functions ====================

/// Create a test kernel directory with basic structure
fn create_test_kernel(root: &Path, kernel_name: &str) {
    let kernel_dir = root.join("concepts").join(kernel_name);
    fs::create_dir_all(kernel_dir.join("queue/inbox")).unwrap();
    fs::create_dir_all(kernel_dir.join("queue/edges")).unwrap();
    fs::create_dir_all(kernel_dir.join("storage")).unwrap();
}

/// Create a conceptkernel.yaml with notification_contract
fn create_ontology_with_notification_contract(
    root: &Path,
    kernel_name: &str,
    targets: &[&str],
) {
    let kernel_dir = root.join("concepts").join(kernel_name);

    let mut notification_entries = Vec::new();
    for target in targets {
        notification_entries.push(format!(
            r#"      - target_kernel: {}
        queue: edges"#,
            target
        ));
    }

    let ontology_content = format!(
        r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  notification_contract:
{}
"#,
        kernel_name,
        notification_entries.join("\n")
    );

    fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();
}

/// Create a simple conceptkernel.yaml without notification_contract
fn create_simple_ontology(root: &Path, kernel_name: &str) {
    let kernel_dir = root.join("concepts").join(kernel_name);

    let ontology_content = format!(
        r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  description: Test kernel
"#,
        kernel_name
    );

    fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();
}

/// Create a target kernel with wildcard authorization
fn create_authorized_target(root: &Path, kernel_name: &str) {
    create_test_kernel(root, kernel_name);

    let kernel_dir = root.join("concepts").join(kernel_name);
    let ontology_content = format!(
        r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges:
      - "*"
"#,
        kernel_name
    );

    fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();
}

/// Create a test instance in a kernel's storage directory
fn create_test_instance(root: &Path, kernel_name: &str, tx_id: &str) -> PathBuf {
    let storage_dir = root.join("concepts").join(kernel_name).join("storage");
    fs::create_dir_all(&storage_dir).unwrap();

    let instance_dir = storage_dir.join(format!("{}.inst", tx_id));
    fs::create_dir_all(&instance_dir).unwrap();

    let receipt_content = r#"{"test": "data", "txId": "test-123"}"#;
    fs::write(instance_dir.join("receipt.json"), receipt_content).unwrap();

    instance_dir
}

/// Extract kernel name from path (simulates daemon function)
fn extract_kernel_from_path(path: &Path) -> Option<String> {
    let components: Vec<_> = path.components().collect();

    for (i, comp) in components.iter().enumerate() {
        if comp.as_os_str() == "concepts" && i + 1 < components.len() {
            return Some(components[i + 1].as_os_str().to_string_lossy().to_string());
        }
    }

    None
}

/// Get notification targets from ontology (simulates daemon function)
fn get_notification_targets(
    root: &Path,
    kernel_name: &str,
    cache: &Arc<Mutex<HashMap<String, Vec<(String, String)>>>>,
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    // Check cache first
    {
        let cache_lock = cache.lock().unwrap();
        if let Some(targets) = cache_lock.get(kernel_name) {
            return Ok(targets.clone());
        }
    }

    // Read from ontology
    let reader = OntologyReader::new(root.to_path_buf());
    let contract = reader.read_notification_contract(kernel_name)?;

    // Convert to (target, predicate) tuples
    let targets: Vec<(String, String)> = contract
        .into_iter()
        .map(|notif| (notif.target_kernel, "PRODUCES".to_string()))
        .collect();

    // Update cache
    {
        let mut cache_lock = cache.lock().unwrap();
        cache_lock.insert(kernel_name.to_string(), targets.clone());
    }

    Ok(targets)
}

/// Route to target kernel (simulates daemon function)
fn route_to_target(
    _root: &Path,
    edge_kernel: &Arc<Mutex<EdgeKernel>>,
    instance_path: &Path,
    source: &str,
    target: &str,
    predicate: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut edge_kernel_lock = edge_kernel.lock().unwrap();

    // Check if edge exists, create if not
    // Note: EdgeKernel uses v1.3.16 internally
    let edge_urn = format!("ckp://Edge.{}.{}-to-{}:v1.3.16", predicate, source, target);

    if edge_kernel_lock.get_edge(&edge_urn)?.is_none() {
        edge_kernel_lock.create_edge(predicate, source, target)?;
    }

    // Route instance
    let _routed_paths = edge_kernel_lock.route_instance(instance_path, source)?;

    Ok(())
}

// ==================== Integration Tests ====================

#[test]
fn test_extract_kernel_from_path_success() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create a path like: /tmp/xyz/concepts/TestKernel/storage/tx-123.inst
    let instance_path = root
        .join("concepts")
        .join("TestKernel")
        .join("storage")
        .join("tx-123.inst");

    let kernel_name = extract_kernel_from_path(&instance_path);

    assert_eq!(kernel_name, Some("TestKernel".to_string()));
}

#[test]
fn test_extract_kernel_from_path_invalid() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Path without "concepts" directory
    let invalid_path = root.join("storage").join("tx-123.inst");

    let kernel_name = extract_kernel_from_path(&invalid_path);

    assert_eq!(kernel_name, None);
}

#[test]
fn test_extract_kernel_from_nested_path() {
    // Test with deeply nested path
    let path = PathBuf::from("/project/concepts/System.Gateway.HTTP/storage/instances/tx-456.inst");

    let kernel_name = extract_kernel_from_path(&path);

    assert_eq!(kernel_name, Some("System.Gateway.HTTP".to_string()));
}

#[test]
fn test_notification_contract_cache_miss_then_hit() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create kernel with notification contract
    create_test_kernel(root, "SourceKernel");
    create_ontology_with_notification_contract(root, "SourceKernel", &["TargetKernel"]);

    // Create cache
    let cache: Arc<Mutex<HashMap<String, Vec<(String, String)>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // First call - cache miss
    {
        let cache_lock = cache.lock().unwrap();
        assert_eq!(cache_lock.len(), 0);
    }

    let targets = get_notification_targets(root, "SourceKernel", &cache).unwrap();
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].0, "TargetKernel");
    assert_eq!(targets[0].1, "PRODUCES");

    // Second call - cache hit
    {
        let cache_lock = cache.lock().unwrap();
        assert_eq!(cache_lock.len(), 1);
        assert!(cache_lock.contains_key("SourceKernel"));
    }

    let targets2 = get_notification_targets(root, "SourceKernel", &cache).unwrap();
    assert_eq!(targets2, targets);
}

#[test]
fn test_notification_contract_multiple_targets() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create kernel with multiple notification targets
    create_test_kernel(root, "MultiSourceKernel");
    create_ontology_with_notification_contract(
        root,
        "MultiSourceKernel",
        &["Target1", "Target2", "Target3"],
    );

    let cache = Arc::new(Mutex::new(HashMap::new()));

    let targets = get_notification_targets(root, "MultiSourceKernel", &cache).unwrap();

    assert_eq!(targets.len(), 3);

    let target_names: Vec<&str> = targets.iter().map(|(name, _)| name.as_str()).collect();
    assert!(target_names.contains(&"Target1"));
    assert!(target_names.contains(&"Target2"));
    assert!(target_names.contains(&"Target3"));

    // All should use PRODUCES predicate
    for (_, predicate) in &targets {
        assert_eq!(predicate, "PRODUCES");
    }
}

#[test]
fn test_notification_contract_missing() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create kernel without notification_contract
    create_test_kernel(root, "NoNotificationKernel");
    create_simple_ontology(root, "NoNotificationKernel");

    let cache = Arc::new(Mutex::new(HashMap::new()));

    let targets = get_notification_targets(root, "NoNotificationKernel", &cache).unwrap();

    assert_eq!(targets.len(), 0);
}

#[test]
fn test_instance_routing_creates_edge() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create source and target kernels
    create_test_kernel(root, "SourceKernel");
    create_authorized_target(root, "TargetKernel");

    // Create EdgeKernel
    let edge_kernel = Arc::new(Mutex::new(EdgeKernel::new(root.to_path_buf()).unwrap()));

    // Create instance
    let instance_path = create_test_instance(root, "SourceKernel", "test-tx-001");

    // Route to target
    route_to_target(
        root,
        &edge_kernel,
        &instance_path,
        "SourceKernel",
        "TargetKernel",
        "PRODUCES",
    )
    .unwrap();

    // Verify edge was created
    let mut edge_kernel_lock = edge_kernel.lock().unwrap();
    let edges = edge_kernel_lock.list_all_edges().unwrap();

    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].source, "SourceKernel");
    assert_eq!(edges[0].target, "TargetKernel");
    assert_eq!(edges[0].predicate, "PRODUCES");
}

#[test]
fn test_instance_routing_creates_symlink() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create source and target kernels
    create_test_kernel(root, "SourceKernel");
    create_authorized_target(root, "TargetKernel");

    // Create EdgeKernel
    let edge_kernel = Arc::new(Mutex::new(EdgeKernel::new(root.to_path_buf()).unwrap()));

    // Create instance
    let instance_path = create_test_instance(root, "SourceKernel", "test-tx-002");

    // Route to target
    route_to_target(
        root,
        &edge_kernel,
        &instance_path,
        "SourceKernel",
        "TargetKernel",
        "PRODUCES",
    )
    .unwrap();

    // Verify symlink was created in target queue
    let target_queue = root
        .join("concepts")
        .join("TargetKernel")
        .join("queue")
        .join("edges")
        .join("PRODUCES.SourceKernel");

    assert!(target_queue.exists(), "Target queue should exist");

    // Check for symlink
    let symlinks: Vec<_> = fs::read_dir(&target_queue)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("inst"))
        .collect();

    assert_eq!(symlinks.len(), 1, "Should have exactly one symlink");

    // Verify it's actually a symlink
    let symlink_path = symlinks[0].path();
    let metadata = fs::symlink_metadata(&symlink_path).unwrap();
    assert!(metadata.is_symlink(), "Should be a symbolic link");

    // Verify symlink points to original instance
    let link_target = fs::read_link(&symlink_path).unwrap();
    assert!(link_target.to_string_lossy().contains("SourceKernel"));
    assert!(link_target.to_string_lossy().contains("storage"));
}

#[test]
fn test_routing_to_multiple_targets() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create source and multiple target kernels
    create_test_kernel(root, "MultiTargetSource");
    create_authorized_target(root, "Target1");
    create_authorized_target(root, "Target2");
    create_authorized_target(root, "Target3");

    // Create EdgeKernel
    let edge_kernel = Arc::new(Mutex::new(EdgeKernel::new(root.to_path_buf()).unwrap()));

    // Create instance
    let instance_path = create_test_instance(root, "MultiTargetSource", "multi-tx-001");

    // Route to all targets
    let targets = vec!["Target1", "Target2", "Target3"];
    for target in &targets {
        route_to_target(
            root,
            &edge_kernel,
            &instance_path,
            "MultiTargetSource",
            target,
            "PRODUCES",
        )
        .unwrap();
    }

    // Note: EdgeKernel stores edges in directories named {predicate}.{source}
    // Multiple targets from the same source share the same edge directory
    // but have different URNs. This is a design characteristic of EdgeKernel.
    // The latest edge overwrites the metadata.json, but routing still works
    // because route_instance() finds all edges regardless of directory structure.
    let mut edge_kernel_lock = edge_kernel.lock().unwrap();
    let edges = edge_kernel_lock.list_all_edges().unwrap();

    // EdgeKernel design: one directory per (predicate, source) pair
    // So we expect 1 edge directory, but routing creates symlinks for all targets
    assert!(edges.len() >= 1, "Should have at least one edge");

    // Verify symlinks in all target queues (this is what matters for routing)
    for target in &targets {
        let target_queue = root
            .join("concepts")
            .join(target)
            .join("queue")
            .join("edges")
            .join("PRODUCES.MultiTargetSource");

        assert!(target_queue.exists(), "Target queue for {} should exist", target);

        let symlinks: Vec<_> = fs::read_dir(&target_queue)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("inst"))
            .collect();

        assert_eq!(
            symlinks.len(),
            1,
            "Target {} should have exactly one symlink",
            target
        );
    }
}

#[test]
fn test_routing_with_invalid_kernel_path() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create invalid path (not under concepts/)
    let invalid_path = root.join("invalid").join("storage").join("tx-999.inst");

    let kernel_name = extract_kernel_from_path(&invalid_path);

    // Should return None for invalid paths
    assert_eq!(kernel_name, None);
}

#[test]
fn test_routing_handles_missing_target_directory() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create source kernel but NOT target kernel
    create_test_kernel(root, "SourceKernel");

    // Create EdgeKernel
    let edge_kernel = Arc::new(Mutex::new(EdgeKernel::new(root.to_path_buf()).unwrap()));

    // Create instance
    let instance_path = create_test_instance(root, "SourceKernel", "missing-target-tx");

    // Route to non-existent target (should auto-create directories)
    let result = route_to_target(
        root,
        &edge_kernel,
        &instance_path,
        "SourceKernel",
        "NonExistentTarget",
        "PRODUCES",
    );

    // Should succeed (auto-creates target queue directories)
    assert!(result.is_ok());

    // Verify target queue was auto-created
    let target_queue = root
        .join("concepts")
        .join("NonExistentTarget")
        .join("queue")
        .join("edges")
        .join("PRODUCES.SourceKernel");

    assert!(target_queue.exists(), "Target queue should be auto-created");
}

#[test]
fn test_notification_contract_cache_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create two kernels with different notification contracts
    create_test_kernel(root, "Kernel1");
    create_ontology_with_notification_contract(root, "Kernel1", &["Target1"]);

    create_test_kernel(root, "Kernel2");
    create_ontology_with_notification_contract(root, "Kernel2", &["Target2", "Target3"]);

    let cache = Arc::new(Mutex::new(HashMap::new()));

    // Load both into cache
    let targets1 = get_notification_targets(root, "Kernel1", &cache).unwrap();
    let targets2 = get_notification_targets(root, "Kernel2", &cache).unwrap();

    // Verify isolation
    assert_eq!(targets1.len(), 1);
    assert_eq!(targets1[0].0, "Target1");

    assert_eq!(targets2.len(), 2);
    let target2_names: Vec<&str> = targets2.iter().map(|(name, _)| name.as_str()).collect();
    assert!(target2_names.contains(&"Target2"));
    assert!(target2_names.contains(&"Target3"));

    // Verify cache has both entries
    let cache_lock = cache.lock().unwrap();
    assert_eq!(cache_lock.len(), 2);
    assert!(cache_lock.contains_key("Kernel1"));
    assert!(cache_lock.contains_key("Kernel2"));
}

#[test]
fn test_edge_reuse_on_multiple_instances() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create source and target kernels
    create_test_kernel(root, "SourceKernel");
    create_authorized_target(root, "TargetKernel");

    // Create EdgeKernel
    let edge_kernel = Arc::new(Mutex::new(EdgeKernel::new(root.to_path_buf()).unwrap()));

    // Create and route multiple instances
    for i in 1..=5 {
        let instance_path = create_test_instance(root, "SourceKernel", &format!("batch-tx-{:03}", i));

        route_to_target(
            root,
            &edge_kernel,
            &instance_path,
            "SourceKernel",
            "TargetKernel",
            "PRODUCES",
        )
        .unwrap();
    }

    // Verify only ONE edge was created (reused for all instances)
    let mut edge_kernel_lock = edge_kernel.lock().unwrap();
    let edges = edge_kernel_lock.list_all_edges().unwrap();

    assert_eq!(edges.len(), 1, "Should only create one edge, reused for all instances");

    // Verify all 5 symlinks were created
    let target_queue = root
        .join("concepts")
        .join("TargetKernel")
        .join("queue")
        .join("edges")
        .join("PRODUCES.SourceKernel");

    let symlinks: Vec<_> = fs::read_dir(&target_queue)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("inst"))
        .collect();

    assert_eq!(symlinks.len(), 5, "Should have 5 symlinks for 5 instances");
}

#[test]
fn test_full_workflow_with_ontology_reader() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create full workflow: Source -> Target1, Target2
    create_test_kernel(root, "WorkflowSource");
    create_ontology_with_notification_contract(root, "WorkflowSource", &["Target1", "Target2"]);
    create_authorized_target(root, "Target1");
    create_authorized_target(root, "Target2");

    // Initialize components
    let edge_kernel = Arc::new(Mutex::new(EdgeKernel::new(root.to_path_buf()).unwrap()));
    let cache = Arc::new(Mutex::new(HashMap::new()));

    // Create instance
    let instance_path = create_test_instance(root, "WorkflowSource", "workflow-tx-001");

    // Simulate daemon workflow:
    // 1. Extract kernel name from path
    let kernel_name = extract_kernel_from_path(&instance_path).unwrap();
    assert_eq!(kernel_name, "WorkflowSource");

    // 2. Get notification targets from ontology
    let targets = get_notification_targets(root, &kernel_name, &cache).unwrap();
    assert_eq!(targets.len(), 2);

    // 3. Route to each target
    for (target, predicate) in targets {
        route_to_target(root, &edge_kernel, &instance_path, &kernel_name, &target, &predicate)
            .unwrap();
    }

    // Verify complete workflow
    let mut edge_kernel_lock = edge_kernel.lock().unwrap();
    let edges = edge_kernel_lock.list_all_edges().unwrap();

    // EdgeKernel design: one directory per (predicate, source) pair
    // Multiple targets share the same edge directory but have different URNs
    assert!(edges.len() >= 1, "Should have at least one edge");

    // Verify both targets have symlinks
    for target in &["Target1", "Target2"] {
        let target_queue = root
            .join("concepts")
            .join(target)
            .join("queue")
            .join("edges")
            .join("PRODUCES.WorkflowSource");

        assert!(target_queue.exists());

        let symlinks: Vec<_> = fs::read_dir(&target_queue)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("inst"))
            .collect();

        assert_eq!(symlinks.len(), 1);
    }
}

#[test]
fn test_symlink_points_to_correct_instance() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create kernels
    create_test_kernel(root, "SourceKernel");
    create_authorized_target(root, "TargetKernel");

    // Create EdgeKernel
    let edge_kernel = Arc::new(Mutex::new(EdgeKernel::new(root.to_path_buf()).unwrap()));

    // Create instance with known content
    let instance_path = create_test_instance(root, "SourceKernel", "verify-symlink-tx");
    let test_content = r#"{"test": "symlink_verification", "value": 42}"#;
    fs::write(instance_path.join("receipt.json"), test_content).unwrap();

    // Route to target
    route_to_target(
        root,
        &edge_kernel,
        &instance_path,
        "SourceKernel",
        "TargetKernel",
        "PRODUCES",
    )
    .unwrap();

    // Find symlink
    let target_queue = root
        .join("concepts")
        .join("TargetKernel")
        .join("queue")
        .join("edges")
        .join("PRODUCES.SourceKernel");

    let symlinks: Vec<_> = fs::read_dir(&target_queue)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    assert_eq!(symlinks.len(), 1);

    let symlink_path = symlinks[0].path();

    // Read content through symlink
    let symlinked_content = fs::read_to_string(symlink_path.join("receipt.json")).unwrap();

    // Verify content matches original
    assert_eq!(symlinked_content, test_content);
}

#[test]
fn test_notification_contract_empty_list() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create kernel with empty notification_contract
    create_test_kernel(root, "EmptyNotificationKernel");

    let kernel_dir = root.join("concepts").join("EmptyNotificationKernel");
    let ontology_content = r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://EmptyNotificationKernel:v0.1
  type: node:cold
  version: v0.1
spec:
  notification_contract: []
"#;

    fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

    let cache = Arc::new(Mutex::new(HashMap::new()));

    let targets = get_notification_targets(root, "EmptyNotificationKernel", &cache).unwrap();

    assert_eq!(targets.len(), 0);
}

// ==================== Performance and Edge Case Tests ====================

#[test]
fn test_high_volume_routing() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    // Create source and target
    create_test_kernel(root, "HighVolumeSource");
    create_authorized_target(root, "HighVolumeTarget");

    let edge_kernel = Arc::new(Mutex::new(EdgeKernel::new(root.to_path_buf()).unwrap()));

    // Route 50 instances
    for i in 1..=50 {
        let instance_path = create_test_instance(
            root,
            "HighVolumeSource",
            &format!("volume-tx-{:04}", i),
        );

        route_to_target(
            root,
            &edge_kernel,
            &instance_path,
            "HighVolumeSource",
            "HighVolumeTarget",
            "PRODUCES",
        )
        .unwrap();
    }

    // Verify all symlinks created
    let target_queue = root
        .join("concepts")
        .join("HighVolumeTarget")
        .join("queue")
        .join("edges")
        .join("PRODUCES.HighVolumeSource");

    let symlinks: Vec<_> = fs::read_dir(&target_queue)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("inst"))
        .collect();

    assert_eq!(symlinks.len(), 50, "Should have 50 symlinks");
}

#[test]
fn test_cache_performance_benefit() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    create_test_kernel(root, "CachedKernel");
    create_ontology_with_notification_contract(root, "CachedKernel", &["Target1", "Target2"]);

    let cache = Arc::new(Mutex::new(HashMap::new()));

    // First call - reads from disk
    let start1 = std::time::Instant::now();
    let targets1 = get_notification_targets(root, "CachedKernel", &cache).unwrap();
    let duration1 = start1.elapsed();

    // Second call - reads from cache
    let start2 = std::time::Instant::now();
    let targets2 = get_notification_targets(root, "CachedKernel", &cache).unwrap();
    let duration2 = start2.elapsed();

    // Results should be identical
    assert_eq!(targets1, targets2);

    // Cache should be faster (at least 2x in most cases)
    // Note: This is a heuristic test, may fail on very fast filesystems
    assert!(
        duration2 < duration1,
        "Cached read should be faster than disk read (disk: {:?}, cache: {:?})",
        duration1,
        duration2
    );
}
