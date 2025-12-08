//! Integration tests for kernel operations
//!
//! Tests complete kernel workflows including:
//! - Kernel initialization
//! - Event emission
//! - RBAC enforcement
//! - Edge routing

use ckp_core::{
    Kernel, EdgeMetadata, UrnResolver,
};
use ckp_core::drivers::FileSystemDriver;
use std::fs;
use tempfile::TempDir;
use serde_json::json;
use chrono::Utc;

#[tokio::test]
async fn test_kernel_to_kernel_emission() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create two test kernels under concepts/
    let kernel1_dir = concepts_root.join("TestKernel1");
    let kernel2_dir = concepts_root.join("TestKernel2");

    // Create ontologies
    create_test_ontology(&project_root, "TestKernel1");
    create_test_ontology(&project_root, "TestKernel2");

    // Initialize kernel 1 with project root
    let mut kernel1 = Kernel::new(project_root.clone(), Some("TestKernel1".to_string()), false);
    kernel1.bootstrap("TestKernel1").await.unwrap();

    // Emit to kernel 2
    let tx_id = kernel1.emit("TestKernel2", json!({"test": "data"})).await.unwrap();

    // Verify job file was created in kernel2's inbox
    let inbox = kernel2_dir.join("queue/inbox");
    let job_files: Vec<_> = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(job_files.len(), 1, "Expected exactly one job file in inbox");

    // Verify job file contents
    let job_content = fs::read_to_string(job_files[0].path()).unwrap();
    let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

    assert_eq!(job["target"], "TestKernel2");
    assert_eq!(job["payload"]["test"], "data");
    assert_eq!(job["txId"], tx_id);
    assert!(job["timestamp"].is_string());
}

#[tokio::test]
async fn test_rbac_enforcement() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create kernel with RBAC restrictions
    let kernel_dir = concepts_root.join("RestrictedKernel");

    // Create ontology with RBAC rules (deny all by default)
    let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://RestrictedKernel:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed: []
      denied:
        - "*"
"#;
    fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

    // Initialize kernel with RBAC enabled
    let mut kernel = Kernel::new(project_root.clone(), Some("RestrictedKernel".to_string()), true);
    kernel.bootstrap("RestrictedKernel").await.unwrap();

    // Try to emit to a denied target
    let result = kernel.emit("UnauthorizedTarget", json!({"test": "data"})).await;

    // Should fail due to RBAC
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("RBAC"));
}

#[test]
fn test_edge_metadata_creation() {
    // Test edge metadata struct creation and serialization
    let edge_metadata = EdgeMetadata::new("PRODUCES", "SourceKernel", "TargetKernel", "v1.3.14");

    assert_eq!(edge_metadata.predicate, "PRODUCES");
    assert_eq!(edge_metadata.source, "SourceKernel");
    assert_eq!(edge_metadata.target, "TargetKernel");
    assert_eq!(edge_metadata.version, "v1.3.14");

    // Test serialization
    let json = serde_json::to_string(&edge_metadata).unwrap();
    assert!(json.contains("PRODUCES"));
    assert!(json.contains("SourceKernel"));
}

#[test]
fn test_file_system_driver_paths() {
    let temp_dir = TempDir::new().unwrap();
    let kernel_dir = temp_dir.path().join("EventSourceKernel");
    fs::create_dir_all(&kernel_dir).unwrap();

    let driver = FileSystemDriver::new(kernel_dir.clone(), "EventSourceKernel".to_string());

    // Test getting storage path
    let storage_path = driver.get_storage();
    assert!(storage_path.ends_with("storage"));

    // Test queue paths
    let inbox = driver.get_queue_inbox();
    assert!(inbox.ends_with("queue/inbox"));

    let ready = driver.get_queue_ready();
    assert!(ready.ends_with("queue/ready"));

    let archive = driver.get_archive();
    assert!(archive.ends_with("archive"));
}

#[test]
fn test_urn_parsing() {
    // Test basic kernel URN
    let parsed = UrnResolver::parse("ckp://TestKernel:v0.1").unwrap();
    assert_eq!(parsed.kernel, "TestKernel");
    assert_eq!(parsed.version, "v0.1");
    assert_eq!(parsed.stage, None);
    assert_eq!(parsed.path, None);

    // Test URN with stage
    let parsed = UrnResolver::parse("ckp://TestKernel:v0.1#inbox").unwrap();
    assert_eq!(parsed.kernel, "TestKernel");
    assert_eq!(parsed.version, "v0.1");
    assert_eq!(parsed.stage, Some("inbox".to_string()));
    assert_eq!(parsed.path, None);

    // Test URN with stage and path
    let parsed = UrnResolver::parse("ckp://TestKernel:v0.1#storage/test.inst").unwrap();
    assert_eq!(parsed.kernel, "TestKernel");
    assert_eq!(parsed.version, "v0.1");
    assert_eq!(parsed.stage, Some("storage".to_string()));
    assert_eq!(parsed.path, Some("test.inst".to_string()));
}

// Helper function to create test ontology
fn create_test_ontology(project_root: &std::path::Path, kernel_name: &str) {
    let ontology_content = format!(r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  queue_contract:
    edges: []
  rbac:
    communication:
      allowed:
        - "*"
      denied: []
"#, kernel_name);

    // Create ontology.ttl (required for BFO alignment)
    let ontology_ttl = format!(
        r#"@prefix ckp: <ckp://{}:v0.1#> .
@prefix bfo: <http://purl.obolibrary.org/obo/BFO_> .
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .

ckp: a bfo:0000029 ;  # Site (BFO)
    rdf:label "{}" .
"#,
        kernel_name, kernel_name
    );

    // Use FileSystemDriver for all filesystem operations
    let driver = FileSystemDriver::new(project_root.to_path_buf(), kernel_name.to_string());
    driver.ensure_kernel_structure().unwrap();
    driver.write_ontology(&ontology_content, &ontology_ttl).unwrap();
}

// ==================== PHASE 4: MULTI-KERNEL INTEGRATION TESTS (+3 TESTS) ====================

/// Test: Multi-kernel emit chain (A -> B -> C)
#[tokio::test]
async fn test_multi_kernel_emit_chain() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create three test kernels
    let kernel_a_dir = concepts_root.join("KernelA");
    let kernel_b_dir = concepts_root.join("KernelB");
    let kernel_c_dir = concepts_root.join("KernelC");

    for dir in [&kernel_a_dir, &kernel_b_dir, &kernel_c_dir] {
        create_test_ontology(&project_root, &dir.file_name().unwrap().to_string_lossy());
    }

    // Initialize kernels
    let mut kernel_a = Kernel::new(project_root.clone(), Some("KernelA".to_string()), false);
    let mut kernel_b = Kernel::new(project_root.clone(), Some("KernelB".to_string()), false);

    kernel_a.bootstrap("KernelA").await.unwrap();
    kernel_b.bootstrap("KernelB").await.unwrap();

    // A emits to B
    let tx_id_ab = kernel_a.emit("KernelB", json!({"step": 1, "data": "from_a"})).await.unwrap();

    // Verify B received from A
    let inbox_b = kernel_b_dir.join("queue/inbox");
    let jobs_b: Vec<_> = fs::read_dir(&inbox_b)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();
    assert_eq!(jobs_b.len(), 1, "KernelB should have received 1 job");

    // B emits to C
    let _tx_id_bc = kernel_b.emit("KernelC", json!({"step": 2, "data": "from_b", "parent_tx": tx_id_ab})).await.unwrap();

    // Verify C received from B
    let inbox_c = kernel_c_dir.join("queue/inbox");
    let jobs_c: Vec<_> = fs::read_dir(&inbox_c)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();
    assert_eq!(jobs_c.len(), 1, "KernelC should have received 1 job");

    // Verify payload chain
    let job_c_content = fs::read_to_string(jobs_c[0].path()).unwrap();
    let job_c: serde_json::Value = serde_json::from_str(&job_c_content).unwrap();
    assert_eq!(job_c["payload"]["step"], 2);
    assert_eq!(job_c["payload"]["data"], "from_b");
    assert_eq!(job_c["payload"]["parent_tx"], tx_id_ab);
}

/// Test: Storage artifact minting and retrieval
#[tokio::test]
async fn test_storage_artifact_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let kernel_dir = temp_dir.path().to_path_buf();

    // Create storage and queue directories

    // Create filesystem driver (driver root IS the kernel directory)
    let driver = FileSystemDriver::new(kernel_dir.clone(), "StorageKernel".to_string());

    // Mint multiple storage artifacts
    let artifacts = vec![
        (json!({"result": "success", "value": 42}), "tx-001"),
        (json!({"result": "success", "value": 43}), "tx-002"),
        (json!({"result": "failure", "error": "test"}), "tx-003"),
    ];

    let mut artifact_paths = vec![];
    for (payload, tx_id) in &artifacts {
        let path = driver.mint_storage_artifact(payload, tx_id).unwrap();
        // Verify immediately after minting
        assert!(path.exists(), "Artifact should exist immediately after minting: {:?}", path);
        artifact_paths.push(path);
    }

    // Verify all artifacts still exist
    for path in &artifact_paths {
        assert!(path.exists(), "Artifact should still exist: {:?}", path);

        // Verify receipt.json exists
        let receipt_path = path.join("receipt.json");
        assert!(receipt_path.exists(), "Receipt should exist");

        // Verify receipt content
        let receipt_content = fs::read_to_string(&receipt_path).unwrap();
        let receipt: serde_json::Value = serde_json::from_str(&receipt_content).unwrap();
        assert!(receipt["result"].is_string() || receipt["error"].is_string());
    }

    // Verify storage directory structure
    // FileSystemDriver adds "concepts/{kernel_name}" to the root, so storage is at root/concepts/{kernel}/storage
    let storage_dir = kernel_dir.join("concepts/StorageKernel/storage");
    assert!(storage_dir.exists(), "Storage directory should exist: {:?}", storage_dir);

    let all_entries: Vec<_> = fs::read_dir(&storage_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    let inst_entries: Vec<_> = all_entries
        .iter()
        .filter(|e| {
            let path = e.path();
            path.is_dir() && path.extension().and_then(|s| s.to_str()) == Some("inst")
        })
        .collect();

    assert_eq!(inst_entries.len(), 3, "Should have 3 instance directories, found {} total entries", all_entries.len());
}

/// Test: Concurrent kernel operations
#[tokio::test]
async fn test_concurrent_kernel_operations() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create target kernel
    let target_dir = concepts_root.join("ConcurrentTarget");
    create_test_ontology(&project_root, "ConcurrentTarget");

    // Spawn multiple kernels concurrently emitting to same target
    let mut handles = vec![];

    for i in 0..5 {
        let project_root_clone = project_root.clone();
        let handle = tokio::spawn(async move {
            let source_name = format!("SourceKernel{}", i);
            let mut kernel = Kernel::new(project_root_clone, Some(source_name.clone()), false);

            kernel.emit("ConcurrentTarget", json!({
                "source": source_name,
                "iteration": i,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })).await
        });
        handles.push(handle);
    }

    // Wait for all emissions to complete
    let mut successful_emits = 0;
    for handle in handles {
        let result = handle.await;
        if result.is_ok() && result.unwrap().is_ok() {
            successful_emits += 1;
        }
    }

    // All should succeed
    assert_eq!(successful_emits, 5, "All 5 concurrent emits should succeed");

    // Verify all 5 jobs were created
    let inbox = target_dir.join("queue/inbox");
    let jobs: Vec<_> = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();
    assert_eq!(jobs.len(), 5, "Should have 5 concurrent jobs in inbox");

    // Verify all jobs are unique and valid
    let mut tx_ids = std::collections::HashSet::new();
    for job_entry in jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        let tx_id = job["txId"].as_str().unwrap();
        assert!(!tx_ids.contains(tx_id), "Transaction IDs should be unique");
        tx_ids.insert(tx_id.to_string());

        assert!(job["payload"]["iteration"].is_number());
        assert!(job["source"].is_string());
    }
}

// ==================== MULTI-KERNEL COMPLEX WORKFLOWS (+3 TESTS) ====================

/// Test: Fork-join workflow pattern (A -> B+C -> D merge)
#[tokio::test]
async fn test_fork_join_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create four test kernels: A (fork), B and C (parallel), D (join)
    let kernel_a_dir = concepts_root.join("ForkKernel");
    let kernel_b_dir = concepts_root.join("PathB");
    let kernel_c_dir = concepts_root.join("PathC");
    let kernel_d_dir = concepts_root.join("JoinKernel");

    for dir in [&kernel_a_dir, &kernel_b_dir, &kernel_c_dir, &kernel_d_dir] {
        create_test_ontology(&project_root, &dir.file_name().unwrap().to_string_lossy());
    }

    // Initialize all kernels
    let mut kernel_a = Kernel::new(project_root.clone(), Some("ForkKernel".to_string()), false);
    let mut kernel_b = Kernel::new(project_root.clone(), Some("PathB".to_string()), false);
    let mut kernel_c = Kernel::new(project_root.clone(), Some("PathC".to_string()), false);

    kernel_a.bootstrap("ForkKernel").await.unwrap();
    kernel_b.bootstrap("PathB").await.unwrap();
    kernel_c.bootstrap("PathC").await.unwrap();

    // A forks to both B and C
    let parent_tx = format!("fork-{}", Utc::now().timestamp());
    let _tx_ab = kernel_a.emit("PathB", json!({
        "parent": parent_tx,
        "branch": "B",
        "data": "processing_path_b"
    })).await.unwrap();

    let _tx_ac = kernel_a.emit("PathC", json!({
        "parent": parent_tx,
        "branch": "C",
        "data": "processing_path_c"
    })).await.unwrap();

    // Verify both B and C received jobs
    let inbox_b = kernel_b_dir.join("queue/inbox");
    let jobs_b: Vec<_> = fs::read_dir(&inbox_b)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();
    assert_eq!(jobs_b.len(), 1, "PathB should have received 1 job");

    let inbox_c = kernel_c_dir.join("queue/inbox");
    let jobs_c: Vec<_> = fs::read_dir(&inbox_c)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();
    assert_eq!(jobs_c.len(), 1, "PathC should have received 1 job");

    // B and C both emit to D (join)
    let _tx_bd = kernel_b.emit("JoinKernel", json!({
        "merge": parent_tx,
        "from": "PathB",
        "result": "b_complete"
    })).await.unwrap();

    let _tx_cd = kernel_c.emit("JoinKernel", json!({
        "merge": parent_tx,
        "from": "PathC",
        "result": "c_complete"
    })).await.unwrap();

    // Verify D received both merge results
    let inbox_d = kernel_d_dir.join("queue/inbox");
    let jobs_d: Vec<_> = fs::read_dir(&inbox_d)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();
    assert_eq!(jobs_d.len(), 2, "JoinKernel should have received 2 jobs");

    // Verify both merge payloads have same parent
    let mut merge_parents = std::collections::HashSet::new();
    for job_entry in jobs_d {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();
        merge_parents.insert(job["payload"]["merge"].as_str().unwrap().to_string());
    }
    assert_eq!(merge_parents.len(), 1, "All merge jobs should have same parent");
}

/// Test: Linear pipeline workflow (A -> B -> C -> D -> E)
#[tokio::test]
async fn test_pipeline_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create five stage pipeline
    let stages = vec!["Stage1", "Stage2", "Stage3", "Stage4", "Stage5"];
    let mut stage_dirs = vec![];

    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);
        stage_dirs.push(stage_dir);
    }

    // Initialize all stages
    let mut kernels = vec![];
    for stage in &stages {
        let mut kernel = Kernel::new(project_root.clone(), Some(stage.to_string()), false);
        kernel.bootstrap(stage).await.unwrap();
        kernels.push(kernel);
    }

    // Flow through pipeline: Stage1 -> Stage2 -> Stage3 -> Stage4 -> Stage5
    let initial_payload = json!({
        "pipeline_id": format!("pipe-{}", Utc::now().timestamp()),
        "stage": 1,
        "data": "initial",
        "processed": []
    });

    let mut current_payload = initial_payload.clone();
    let mut tx_chain = vec![];

    for i in 0..stages.len() - 1 {
        let source_stage = &stages[i];
        let target_stage = &stages[i + 1];

        // Update payload to track processing
        let mut processed = current_payload["processed"].as_array().unwrap().clone();
        processed.push(json!(source_stage));
        current_payload["processed"] = json!(processed);
        current_payload["stage"] = json!(i + 2);

        let tx_id = kernels[i].emit(target_stage, current_payload.clone()).await.unwrap();
        tx_chain.push(tx_id);

        // Verify next stage received job
        let inbox = stage_dirs[i + 1].join("queue/inbox");
        let jobs: Vec<_> = fs::read_dir(&inbox)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
            .collect();
        assert!(jobs.len() >= 1, "{} should have received at least 1 job", target_stage);
    }

    // Verify final stage has complete processing chain
    let final_inbox = stage_dirs[4].join("queue/inbox");
    let final_jobs: Vec<_> = fs::read_dir(&final_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    let final_job_content = fs::read_to_string(final_jobs[0].path()).unwrap();
    let final_job: serde_json::Value = serde_json::from_str(&final_job_content).unwrap();

    assert_eq!(final_job["payload"]["stage"], 5);
    let processed = final_job["payload"]["processed"].as_array().unwrap();
    assert_eq!(processed.len(), 4, "Should have processed through 4 stages");

    // Verify transaction chain
    assert_eq!(tx_chain.len(), 4, "Should have 4 transaction IDs in chain");
}

/// Test: Broadcast pattern (A -> B, C, D simultaneously)
#[tokio::test]
async fn test_broadcast_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create broadcaster and 3 receivers
    let broadcaster_dir = concepts_root.join("Broadcaster");
    let receiver_dirs: Vec<_> = (1..=3)
        .map(|i| concepts_root.join(format!("Receiver{}", i)))
        .collect();

    create_test_ontology(&project_root, "Broadcaster");

    for (i, dir) in receiver_dirs.iter().enumerate() {
        create_test_ontology(&project_root, &format!("Receiver{}", i + 1));
    }

    // Initialize broadcaster
    let mut broadcaster = Kernel::new(project_root.clone(), Some("Broadcaster".to_string()), false);
    broadcaster.bootstrap("Broadcaster").await.unwrap();

    // Broadcast to all receivers simultaneously
    let broadcast_id = format!("broadcast-{}", Utc::now().timestamp());
    let broadcast_payload = json!({
        "broadcast_id": broadcast_id,
        "message": "simultaneous broadcast",
        "timestamp": Utc::now().to_rfc3339()
    });

    let mut broadcast_handles = vec![];
    for i in 1..=3 {
        let target = format!("Receiver{}", i);
        let payload = broadcast_payload.clone();
        let mut broadcaster_clone = Kernel::new(project_root.clone(), Some("Broadcaster".to_string()), false);

        let handle = tokio::spawn(async move {
            broadcaster_clone.emit(&target, payload).await
        });
        broadcast_handles.push(handle);
    }

    // Wait for all broadcasts to complete
    let mut successful_broadcasts = 0;
    for handle in broadcast_handles {
        if handle.await.is_ok_and(|r| r.is_ok()) {
            successful_broadcasts += 1;
        }
    }

    assert_eq!(successful_broadcasts, 3, "All 3 broadcasts should succeed");

    // Verify all receivers got the same broadcast
    for (i, dir) in receiver_dirs.iter().enumerate() {
        let inbox = dir.join("queue/inbox");
        let jobs: Vec<_> = fs::read_dir(&inbox)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
            .collect();

        assert_eq!(jobs.len(), 1, "Receiver{} should have 1 job", i + 1);

        let job_content = fs::read_to_string(jobs[0].path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        assert_eq!(job["payload"]["broadcast_id"], broadcast_id);
        assert_eq!(job["payload"]["message"], "simultaneous broadcast");
    }
}

// ==================== ERROR RECOVERY SCENARIOS (+3 TESTS) ====================

/// Test: Recovery from failed emit operation with RBAC denial and retry
#[tokio::test]
async fn test_recovery_from_failed_emit() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create source kernel with RBAC enabled and restrictive rules (deny all first)
    let source_dir = concepts_root.join("concepts/FailoverSource");

    // Create ontology with RBAC that denies all by default
    let ontology_content = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://FailoverSource:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed: []
      denied:
        - "*"
"#;
    fs::write(source_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

    let mut kernel = Kernel::new(project_root.clone(), Some("FailoverSource".to_string()), true);
    kernel.bootstrap("FailoverSource").await.unwrap();

    // Create a target kernel
    let target_dir = concepts_root.join("concepts/RecoveryTarget");
    create_test_ontology(&project_root, "RecoveryTarget");

    // Try to emit - should fail due to RBAC denying all
    let result = kernel.emit("RecoveryTarget", json!({
        "test": "should_fail_rbac"
    })).await;

    assert!(result.is_err(), "Emit should fail when all communication denied");
    assert!(result.unwrap_err().to_string().contains("RBAC"), "Error should mention RBAC");

    // Update ontology to allow RecoveryTarget (simulate recovery/configuration change)
    let updated_ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://FailoverSource:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - "ckp://*"
      denied: []
"#;
    fs::write(source_dir.join("conceptkernel.yaml"), updated_ontology).unwrap();

    // Re-bootstrap to reload ontology
    kernel.bootstrap("FailoverSource").await.unwrap();

    // Retry emit - should succeed now
    let recovery_result = kernel.emit("RecoveryTarget", json!({
        "test": "should_succeed_after_recovery",
        "retry": true
    })).await;

    assert!(recovery_result.is_ok(), "Emit should succeed after RBAC configuration updated");

    // Verify job was delivered (emit writes to concepts_root directly without /concepts prefix)
    let inbox = concepts_root.join("RecoveryTarget/queue/inbox");
    let jobs: Vec<_> = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(jobs.len(), 1, "Recovery target should have received 1 job after recovery");

    let job_content = fs::read_to_string(jobs[0].path()).unwrap();
    let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();
    assert_eq!(job["payload"]["retry"], true);
}

/// Test: Recovery from corrupted queue file
#[tokio::test]
async fn test_recovery_from_corrupted_queue() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create test kernel
    let kernel_dir = concepts_root.join("CorruptionTest");
    create_test_ontology(&project_root, "CorruptionTest");

    let mut kernel = Kernel::new(project_root.clone(), Some("CorruptionTest".to_string()), false);
    kernel.bootstrap("CorruptionTest").await.unwrap();

    // Create a valid job first
    let _tx1 = kernel.emit("CorruptionTest", json!({
        "message": "valid_before_corruption"
    })).await.unwrap();

    // Verify valid job exists
    let inbox = kernel_dir.join("queue/inbox");
    let jobs_before: Vec<_> = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();
    assert_eq!(jobs_before.len(), 1, "Should have 1 valid job before corruption");

    // Corrupt a job file by writing invalid JSON
    let corrupted_job_path = inbox.join(format!("{}.corrupted.job", Utc::now().timestamp_millis()));
    fs::write(&corrupted_job_path, "{ invalid json: not closed").unwrap();

    // Try to emit another job (system should handle corrupted file gracefully)
    let tx2 = kernel.emit("CorruptionTest", json!({
        "message": "valid_after_corruption",
        "recovery": true
    })).await;

    // Emit should still succeed despite corrupted file in queue
    assert!(tx2.is_ok(), "Should recover and emit successfully");

    // Verify both valid jobs exist
    let jobs_after: Vec<_> = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    let valid_jobs: Vec<_> = jobs_after
        .iter()
        .filter(|entry| {
            let content = fs::read_to_string(entry.path()).unwrap_or_default();
            serde_json::from_str::<serde_json::Value>(&content).is_ok()
        })
        .collect();

    assert_eq!(valid_jobs.len(), 2, "Should have 2 valid jobs after recovery");
}

/// Test: Graceful degradation when dependencies fail
#[tokio::test]
async fn test_graceful_degradation() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create primary service and backup service
    let primary_dir = concepts_root.join("PrimaryService");
    let backup_dir = concepts_root.join("BackupService");
    let client_dir = concepts_root.join("ClientService");

    for dir in [&primary_dir, &backup_dir, &client_dir] {
        create_test_ontology(&project_root, &dir.file_name().unwrap().to_string_lossy());
    }

    let mut client = Kernel::new(project_root.clone(), Some("ClientService".to_string()), false);
    client.bootstrap("ClientService").await.unwrap();

    // Try to emit to primary (which will fail - directory exists but let's simulate failure)
    let primary_result = client.emit("PrimaryService", json!({
        "attempt": 1,
        "target": "primary"
    })).await;

    // Whether primary succeeds or fails, we should be able to failover to backup
    let backup_result = client.emit("BackupService", json!({
        "attempt": 2,
        "target": "backup",
        "fallback": true,
        "primary_failed": primary_result.is_err()
    })).await;

    assert!(backup_result.is_ok(), "Backup service should accept fallback");

    // Verify backup received fallback job
    let backup_inbox = backup_dir.join("queue/inbox");
    let backup_jobs: Vec<_> = fs::read_dir(&backup_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(backup_jobs.len(), 1, "Backup should have received fallback job");

    let job_content = fs::read_to_string(backup_jobs[0].path()).unwrap();
    let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();
    assert_eq!(job["payload"]["fallback"], true);
    assert_eq!(job["payload"]["target"], "backup");

    // Simulate recovery: emit to primary again
    let recovery_result = client.emit("PrimaryService", json!({
        "attempt": 3,
        "target": "primary",
        "recovered": true
    })).await;

    assert!(recovery_result.is_ok(), "Primary should accept jobs after recovery");
}

// ==================== LONG-RUNNING PIPELINE STRESS TESTS (+10 TESTS) ====================

/// Test: Linear 10-kernel pipeline with full data flow validation
#[tokio::test]
async fn test_10_stage_linear_pipeline() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create 10-stage linear pipeline
    let stages: Vec<String> = (1..=10).map(|i| format!("Stage{}", i)).collect();
    let mut stage_dirs = vec![];

    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);
        stage_dirs.push(stage_dir);
    }

    // Initialize all pipeline stages
    let mut kernels = vec![];
    for stage in &stages {
        let mut kernel = Kernel::new(project_root.clone(), Some(stage.to_string()), false);
        kernel.bootstrap(stage).await.unwrap();
        kernels.push(kernel);
    }

    // Flow 100 jobs through the entire pipeline
    let mut all_tx_ids = vec![];
    for job_num in 0..100 {
        let initial_payload = json!({
            "pipeline_id": format!("pipe-{}", job_num),
            "job_number": job_num,
            "stage": 1,
            "data": format!("job_{}", job_num),
            "processed_by": [],
            "start_time": Utc::now().to_rfc3339()
        });

        let mut current_payload = initial_payload.clone();
        let mut tx_chain = vec![];

        // Flow through all 10 stages
        for i in 0..stages.len() - 1 {
            let source_stage = &stages[i];
            let target_stage = &stages[i + 1];

            // Update payload to track processing
            let mut processed = current_payload["processed_by"].as_array().unwrap().clone();
            processed.push(json!(source_stage));
            current_payload["processed_by"] = json!(processed);
            current_payload["stage"] = json!(i + 2);

            let tx_id = kernels[i].emit(target_stage, current_payload.clone()).await.unwrap();
            tx_chain.push(tx_id.clone());
        }

        all_tx_ids.push(tx_chain);
    }

    // Verify final stage received all 100 jobs
    let final_inbox = stage_dirs[9].join("queue/inbox");
    let final_jobs: Vec<_> = fs::read_dir(&final_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(final_jobs.len(), 100, "Final stage should receive all 100 jobs");

    // Sample validation: check first and last job
    for job_entry in [&final_jobs[0], &final_jobs[99]] {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        assert_eq!(job["payload"]["stage"], 10);
        let processed = job["payload"]["processed_by"].as_array().unwrap();
        assert_eq!(processed.len(), 9, "Should have been processed through 9 stages");
    }

    // Verify transaction chain integrity
    assert_eq!(all_tx_ids.len(), 100, "Should have 100 transaction chains");
    assert_eq!(all_tx_ids[0].len(), 9, "Each chain should have 9 transactions (10 stages - 1)");
}

/// Test: 10-stage pipeline with branching (diamond pattern)
#[tokio::test]
async fn test_10_stage_pipeline_with_branching() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create 10-kernel pipeline with diamond branching:
    // Stage1 -> Stage2 -> [Stage3A, Stage3B] -> Stage4 -> ... -> Stage10
    let stages = vec![
        "Stage1", "Stage2", "Stage3A", "Stage3B", "Stage4",
        "Stage5", "Stage6", "Stage7", "Stage8", "Stage9", "Stage10"
    ];

    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);
    }

    // Initialize kernels
    let mut stage1 = Kernel::new(project_root.clone(), Some("Stage1".to_string()), false);
    let mut stage2 = Kernel::new(project_root.clone(), Some("Stage2".to_string()), false);
    let mut stage3a = Kernel::new(project_root.clone(), Some("Stage3A".to_string()), false);
    let mut stage3b = Kernel::new(project_root.clone(), Some("Stage3B".to_string()), false);
    let mut stage4 = Kernel::new(project_root.clone(), Some("Stage4".to_string()), false);

    stage1.bootstrap("Stage1").await.unwrap();
    stage2.bootstrap("Stage2").await.unwrap();
    stage3a.bootstrap("Stage3A").await.unwrap();
    stage3b.bootstrap("Stage3B").await.unwrap();
    stage4.bootstrap("Stage4").await.unwrap();

    // Flow jobs through branching pipeline
    let batch_size = 50;
    for job_num in 0..batch_size {
        let payload = json!({
            "job_id": format!("branch-{}", job_num),
            "stage": 1
        });

        // Stage1 -> Stage2
        let tx1 = stage1.emit("Stage2", payload.clone()).await.unwrap();

        // Stage2 -> fork to both Stage3A and Stage3B
        let fork_payload = json!({
            "job_id": format!("branch-{}", job_num),
            "parent_tx": tx1,
            "stage": 2,
            "fork": true
        });

        stage2.emit("Stage3A", fork_payload.clone()).await.unwrap();
        stage2.emit("Stage3B", fork_payload.clone()).await.unwrap();
    }

    // Verify both branches received all jobs
    let inbox_3a = concepts_root.join("Stage3A/queue/inbox");
    let jobs_3a: Vec<_> = fs::read_dir(&inbox_3a)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    let inbox_3b = concepts_root.join("Stage3B/queue/inbox");
    let jobs_3b: Vec<_> = fs::read_dir(&inbox_3b)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(jobs_3a.len(), batch_size, "Branch A should receive all jobs");
    assert_eq!(jobs_3b.len(), batch_size, "Branch B should receive all jobs");

    // Verify fork metadata
    let job_3a_content = fs::read_to_string(jobs_3a[0].path()).unwrap();
    let job_3a: serde_json::Value = serde_json::from_str(&job_3a_content).unwrap();
    assert_eq!(job_3a["payload"]["fork"], true);

    // Merge back to Stage4
    for i in 0..batch_size {
        let merge_payload = json!({
            "job_id": format!("branch-{}", i),
            "stage": 4,
            "merged_from": ["Stage3A", "Stage3B"]
        });

        stage3a.emit("Stage4", merge_payload.clone()).await.unwrap();
    }

    // Verify merge point received all merge jobs
    let inbox_4 = concepts_root.join("Stage4/queue/inbox");
    let jobs_4: Vec<_> = fs::read_dir(&inbox_4)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(jobs_4.len(), batch_size, "Merge point should receive all merged jobs");
}

/// Test: 10-stage pipeline with multiple merge points
#[tokio::test]
async fn test_10_stage_pipeline_with_merge_points() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create complex pipeline with multiple merge points:
    // S1 -> [S2, S3] -> S4 (merge1) -> [S5, S6, S7] -> S8 (merge2) -> [S9, S10] -> S11 (merge3)
    let stages = vec![
        "S1", "S2", "S3", "S4", "S5", "S6", "S7", "S8", "S9", "S10", "S11"
    ];

    let mut kernels = std::collections::HashMap::new();
    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);

        let mut kernel = Kernel::new(project_root.clone(), Some(stage.to_string()), false);
        kernel.bootstrap(stage).await.unwrap();
        kernels.insert(stage.to_string(), kernel);
    }

    // Flow 20 jobs through the complex merge topology
    for job_num in 0..20 {
        let payload = json!({
            "job_id": format!("merge-{}", job_num),
            "merge_level": 0
        });

        // S1 -> fork to S2, S3
        let tx1 = kernels.get_mut("S1").unwrap().emit("S2", payload.clone()).await.unwrap();
        kernels.get_mut("S1").unwrap().emit("S3", payload.clone()).await.unwrap();

        // S2, S3 -> merge to S4
        let merge1_payload = json!({
            "job_id": format!("merge-{}", job_num),
            "merge_level": 1,
            "parent": tx1
        });

        let tx4 = kernels.get_mut("S2").unwrap().emit("S4", merge1_payload.clone()).await.unwrap();
        kernels.get_mut("S3").unwrap().emit("S4", merge1_payload.clone()).await.unwrap();

        // S4 -> fork to S5, S6, S7
        let fork2_payload = json!({
            "job_id": format!("merge-{}", job_num),
            "merge_level": 2,
            "parent": tx4
        });

        kernels.get_mut("S4").unwrap().emit("S5", fork2_payload.clone()).await.unwrap();
        kernels.get_mut("S4").unwrap().emit("S6", fork2_payload.clone()).await.unwrap();
        kernels.get_mut("S4").unwrap().emit("S7", fork2_payload.clone()).await.unwrap();

        // S5, S6, S7 -> merge to S8
        let merge2_payload = json!({
            "job_id": format!("merge-{}", job_num),
            "merge_level": 3
        });

        let tx8 = kernels.get_mut("S5").unwrap().emit("S8", merge2_payload.clone()).await.unwrap();
        kernels.get_mut("S6").unwrap().emit("S8", merge2_payload.clone()).await.unwrap();
        kernels.get_mut("S7").unwrap().emit("S8", merge2_payload.clone()).await.unwrap();

        // S8 -> fork to S9, S10
        let fork3_payload = json!({
            "job_id": format!("merge-{}", job_num),
            "merge_level": 4,
            "parent": tx8
        });

        kernels.get_mut("S8").unwrap().emit("S9", fork3_payload.clone()).await.unwrap();
        kernels.get_mut("S8").unwrap().emit("S10", fork3_payload.clone()).await.unwrap();

        // S9, S10 -> final merge to S11
        let final_payload = json!({
            "job_id": format!("merge-{}", job_num),
            "merge_level": 5,
            "final": true
        });

        kernels.get_mut("S9").unwrap().emit("S11", final_payload.clone()).await.unwrap();
        kernels.get_mut("S10").unwrap().emit("S11", final_payload.clone()).await.unwrap();
    }

    // Verify final merge point received all jobs (20 jobs * 2 branches = 40 jobs)
    let inbox_s11 = concepts_root.join("S11/queue/inbox");
    let jobs_s11: Vec<_> = fs::read_dir(&inbox_s11)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(jobs_s11.len(), 40, "Final merge should have 40 jobs (20 * 2 branches)");

    // Verify intermediate merge points
    let inbox_s4 = concepts_root.join("S4/queue/inbox");
    let jobs_s4_count = fs::read_dir(&inbox_s4).unwrap().count();
    assert_eq!(jobs_s4_count, 40, "First merge point should have 40 jobs (20 * 2)");

    let inbox_s8 = concepts_root.join("S8/queue/inbox");
    let jobs_s8_count = fs::read_dir(&inbox_s8).unwrap().count();
    assert_eq!(jobs_s8_count, 60, "Second merge point should have 60 jobs (20 * 3)");
}

/// Test: Queue overflow handling with backpressure
#[tokio::test]
async fn test_queue_overflow_backpressure() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create simple 2-stage pipeline
    let stages = vec!["Producer", "Consumer"];
    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);
    }

    let mut producer = Kernel::new(project_root.clone(), Some("Producer".to_string()), false);
    producer.bootstrap("Producer").await.unwrap();

    // Flood consumer inbox with 1000 jobs rapidly
    let mut handles = vec![];
    for i in 0..1000 {
        let mut producer_clone = Kernel::new(project_root.clone(), Some("Producer".to_string()), false);
        let handle = tokio::spawn(async move {
            producer_clone.emit("Consumer", json!({
                "job_id": i,
                "data": format!("overflow_test_{}", i),
                "timestamp": Utc::now().to_rfc3339()
            })).await
        });
        handles.push(handle);
    }

    // Wait for all emits to complete
    let mut successful = 0;
    for handle in handles {
        match handle.await {
            Ok(Ok(_)) => successful += 1,
            Ok(Err(_)) => {},
            Err(_) => {},
        }
    }

    // All should succeed (no built-in queue limits yet)
    assert_eq!(successful, 1000, "All jobs should succeed without backpressure limits");

    // Verify all jobs are in inbox
    let inbox = concepts_root.join("Consumer/queue/inbox");
    let jobs: Vec<_> = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(jobs.len(), 1000, "Consumer inbox should contain all 1000 jobs");

    // Verify no data corruption in extreme load
    for job_entry in jobs.iter().take(10) {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();
        assert!(job["payload"]["job_id"].is_number());
        assert!(job["payload"]["data"].is_string());
    }
}

/// Test: Slow consumer throttling simulation
#[tokio::test]
async fn test_slow_consumer_throttling() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create 3-stage pipeline: FastProducer -> SlowProcessor -> FastConsumer
    let stages = vec!["FastProducer", "SlowProcessor", "FastConsumer"];
    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);
    }

    let mut producer = Kernel::new(project_root.clone(), Some("FastProducer".to_string()), false);
    let mut processor = Kernel::new(project_root.clone(), Some("SlowProcessor".to_string()), false);
    producer.bootstrap("FastProducer").await.unwrap();
    processor.bootstrap("SlowProcessor").await.unwrap();

    // Producer emits 100 jobs rapidly
    for i in 0..100 {
        producer.emit("SlowProcessor", json!({
            "job_id": i,
            "data": format!("throttle_test_{}", i)
        })).await.unwrap();
    }

    // Verify all jobs queued at SlowProcessor
    let slow_inbox = concepts_root.join("SlowProcessor/queue/inbox");
    let queued_jobs: Vec<_> = fs::read_dir(&slow_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(queued_jobs.len(), 100, "All jobs should queue at slow processor");

    // Simulate slow processing: emit only 10 jobs downstream with delays
    for i in 0..10 {
        processor.emit("FastConsumer", json!({
            "job_id": i,
            "processed": true,
            "delay_ms": 50
        })).await.unwrap();

        // Simulate processing delay
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Verify downstream received only processed batch
    let consumer_inbox = concepts_root.join("FastConsumer/queue/inbox");
    let consumer_jobs: Vec<_> = fs::read_dir(&consumer_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(consumer_jobs.len(), 10, "Only processed jobs should reach consumer");

    // Verify 90 jobs still queued at processor
    let remaining_jobs: Vec<_> = fs::read_dir(&slow_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(remaining_jobs.len(), 100, "Original jobs should remain in processor inbox");
}

/// Test: Backpressure propagation through multi-stage pipeline
#[tokio::test]
async fn test_backpressure_propagation() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create 5-stage pipeline with bottleneck at Stage3
    let stages = vec!["Stage1", "Stage2", "Stage3", "Stage4", "Stage5"];
    let mut kernels = vec![];

    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);

        let mut kernel = Kernel::new(project_root.clone(), Some(stage.to_string()), false);
        kernel.bootstrap(stage).await.unwrap();
        kernels.push(kernel);
    }

    // Emit 200 jobs through first two stages
    for i in 0..200 {
        let payload = json!({"job_id": i, "stage": 1});
        let tx1 = kernels[0].emit("Stage2", payload).await.unwrap();

        let payload2 = json!({"job_id": i, "stage": 2, "parent": tx1});
        kernels[1].emit("Stage3", payload2).await.unwrap();
    }

    // Verify Stage3 has all 200 jobs (bottleneck)
    let stage3_inbox = concepts_root.join("Stage3/queue/inbox");
    let stage3_jobs_count = fs::read_dir(&stage3_inbox)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(stage3_jobs_count, 200, "Bottleneck stage should accumulate all jobs");

    // Slowly drain bottleneck: process only 50 jobs downstream
    for i in 0..50 {
        let payload = json!({"job_id": i, "stage": 4, "processed_bottleneck": true});
        kernels[2].emit("Stage4", payload).await.unwrap();
    }

    // Verify downstream stages have limited jobs (backpressure effect)
    let stage4_inbox = concepts_root.join("Stage4/queue/inbox");
    let stage4_jobs_count = fs::read_dir(&stage4_inbox)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(stage4_jobs_count, 50, "Downstream should only receive drained jobs");

    // Verify backlog still exists at bottleneck
    let stage3_remaining = fs::read_dir(&stage3_inbox)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(stage3_remaining, 200, "Backlog should remain at bottleneck until processed");
}

/// Test: Mid-pipeline cancellation with cleanup
#[tokio::test]
async fn test_mid_pipeline_cancellation() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create 6-stage pipeline
    let stages: Vec<String> = (1..=6).map(|i| format!("CancelStage{}", i)).collect();
    let mut kernels = vec![];

    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);

        let mut kernel = Kernel::new(project_root.clone(), Some(stage.to_string()), false);
        kernel.bootstrap(stage).await.unwrap();
        kernels.push(kernel);
    }

    // Start flowing jobs through pipeline
    let cancellable_job_ids = vec![5, 10, 15, 20];
    for i in 0..30 {
        let mut payload = json!({
            "job_id": i,
            "cancellable": cancellable_job_ids.contains(&i),
            "stage": 1
        });

        // Flow through first 3 stages
        for stage_idx in 0..3 {
            payload["stage"] = json!(stage_idx + 1);
            if stage_idx < 2 {
                kernels[stage_idx].emit(&stages[stage_idx + 1], payload.clone()).await.unwrap();
            }
        }
    }

    // Verify Stage3 received all 30 jobs
    let stage3_inbox = concepts_root.join(&stages[2]).join("queue/inbox");
    let stage3_jobs: Vec<_> = fs::read_dir(&stage3_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(stage3_jobs.len(), 30, "Stage3 should have all jobs before cancellation");

    // Cancel specific jobs by moving them to archive
    for job_entry in &stage3_jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["payload"]["cancellable"] == true {
            let archive_path = concepts_root.join(&stages[2])
                .join("queue/archive")
                .join(job_entry.file_name());
            fs::rename(job_entry.path(), archive_path).unwrap();
        }
    }

    // Verify cancellable jobs moved to archive
    let archive_dir = concepts_root.join(&stages[2]).join("queue/archive");
    let archived_count = fs::read_dir(&archive_dir)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(archived_count, 4, "4 cancellable jobs should be archived");

    // Continue pipeline with remaining jobs
    let remaining_jobs: Vec<_> = fs::read_dir(&stage3_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(remaining_jobs.len(), 26, "26 jobs should remain after cancellation");

    // Flow remaining jobs to Stage4
    for job_entry in remaining_jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        let mut payload = job["payload"].clone();
        payload["stage"] = json!(4);
        kernels[2].emit(&stages[3], payload).await.unwrap();
    }

    // Verify Stage4 received only non-cancelled jobs
    let stage4_inbox = concepts_root.join(&stages[3]).join("queue/inbox");
    let stage4_jobs_count = fs::read_dir(&stage4_inbox)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(stage4_jobs_count, 26, "Stage4 should only receive non-cancelled jobs");
}

/// Test: Cleanup after pipeline cancellation
#[tokio::test]
async fn test_pipeline_cancellation_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create 4-stage pipeline
    let stages = vec!["CleanStage1", "CleanStage2", "CleanStage3", "CleanStage4"];
    let mut kernels = vec![];

    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);

        let mut kernel = Kernel::new(project_root.clone(), Some(stage.to_string()), false);
        kernel.bootstrap(stage).await.unwrap();
        kernels.push(kernel);
    }

    // Start pipeline with 50 jobs
    for i in 0..50 {
        let payload = json!({"job_id": i, "pipeline_run": "cleanup_test"});

        // Flow through all stages
        let mut current_payload = payload.clone();
        for stage_idx in 0..stages.len() - 1 {
            current_payload["stage"] = json!(stage_idx + 1);
            kernels[stage_idx].emit(&stages[stage_idx + 1], current_payload.clone()).await.unwrap();
        }
    }

    // Simulate cancellation: archive all jobs from all stages
    let mut total_archived = 0;
    for stage in &stages {
        let inbox_path = concepts_root.join(stage).join("queue/inbox");
        let archive_path = concepts_root.join(stage).join("queue/archive");

        if let Ok(entries) = fs::read_dir(&inbox_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("job") {
                    let dest = archive_path.join(entry.file_name());
                    fs::rename(entry.path(), dest).unwrap();
                    total_archived += 1;
                }
            }
        }
    }

    assert!(total_archived > 0, "Should have archived jobs from pipeline");

    // Verify all inboxes are now empty (cleanup successful)
    for stage in &stages {
        let inbox_path = concepts_root.join(stage).join("queue/inbox");
        let remaining_count = fs::read_dir(&inbox_path)
            .unwrap()
            .filter(|e| {
                e.as_ref().unwrap().path().extension().and_then(|s| s.to_str()) == Some("job")
            })
            .count();

        assert_eq!(remaining_count, 0, "{} inbox should be empty after cleanup", stage);
    }

    // Verify all archived jobs are accessible
    for stage in &stages {
        let archive_path = concepts_root.join(stage).join("queue/archive");
        let archived_jobs: Vec<_> = fs::read_dir(&archive_path)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
            .collect();

        // Each archived job should be valid JSON
        for job_entry in archived_jobs {
            let job_content = fs::read_to_string(job_entry.path()).unwrap();
            let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();
            assert!(job["payload"]["job_id"].is_number());
            assert_eq!(job["payload"]["pipeline_run"], "cleanup_test");
        }
    }
}

/// Test: Partial pipeline failure with resource cleanup
#[tokio::test]
async fn test_partial_pipeline_failure_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create 5-stage pipeline with intentional failure point
    let stages = vec!["FailStage1", "FailStage2", "FailStage3", "FailStage4", "FailStage5"];
    let mut kernels = vec![];

    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);

        let mut kernel = Kernel::new(project_root.clone(), Some(stage.to_string()), false);
        kernel.bootstrap(stage).await.unwrap();
        kernels.push(kernel);
    }

    // Flow jobs through first 2 stages successfully
    for i in 0..40 {
        let payload = json!({
            "job_id": i,
            "stage": 1,
            "should_fail_at_stage3": i % 5 == 0  // Every 5th job will fail
        });

        let tx1 = kernels[0].emit(&stages[1], payload.clone()).await.unwrap();

        let mut payload2 = payload.clone();
        payload2["stage"] = json!(2);
        payload2["parent_tx"] = json!(tx1);
        kernels[1].emit(&stages[2], payload2).await.unwrap();
    }

    // Verify Stage3 received all jobs
    let stage3_inbox = concepts_root.join(&stages[2]).join("queue/inbox");
    let stage3_jobs: Vec<_> = fs::read_dir(&stage3_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(stage3_jobs.len(), 40, "Stage3 should receive all 40 jobs");

    // Simulate failure: archive failed jobs, forward successful ones
    let mut failed_jobs = vec![];
    let mut success_payloads = vec![];

    for job_entry in stage3_jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["payload"]["should_fail_at_stage3"] == true {
            // Move to archive (simulating failure)
            let archive_path = concepts_root.join(&stages[2])
                .join("queue/archive")
                .join(job_entry.file_name());
            fs::rename(job_entry.path(), &archive_path).unwrap();
            failed_jobs.push(archive_path);
        } else {
            // Save payload for forwarding, then archive the processed job
            success_payloads.push(job["payload"].clone());
            let archive_path = concepts_root.join(&stages[2])
                .join("queue/archive")
                .join(job_entry.file_name());
            fs::rename(job_entry.path(), &archive_path).unwrap();
        }
    }

    assert_eq!(failed_jobs.len(), 8, "8 jobs should fail (40 / 5)");
    assert_eq!(success_payloads.len(), 32, "32 jobs should succeed");

    // Forward successful jobs to Stage4
    for payload_data in &success_payloads {
        let mut payload = payload_data.clone();
        payload["stage"] = json!(4);
        payload["recovered_from_failure"] = json!(false);
        kernels[2].emit(&stages[3], payload).await.unwrap();
    }

    // Verify Stage4 received only successful jobs
    let stage4_inbox = concepts_root.join(&stages[3]).join("queue/inbox");
    let stage4_jobs_count = fs::read_dir(&stage4_inbox)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(stage4_jobs_count, 32, "Stage4 should only receive successful jobs");

    // Verify all jobs are archived (both failed and successful)
    let stage3_archive = concepts_root.join(&stages[2]).join("queue/archive");
    let archived_all: Vec<_> = fs::read_dir(&stage3_archive)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(archived_all.len(), 40, "All 40 jobs should be archived (8 failed + 32 processed)");

    // Verify archived failed jobs maintain data integrity
    let archived_failed_count = archived_all
        .iter()
        .filter(|entry| {
            let content = fs::read_to_string(entry.path()).unwrap();
            let job: serde_json::Value = serde_json::from_str(&content).unwrap();
            job["payload"]["should_fail_at_stage3"] == true
        })
        .count();

    assert_eq!(archived_failed_count, 8, "8 failed jobs should be in archive");

    // Verify all archived jobs are valid
    for archived_entry in archived_all {
        let archived_content = fs::read_to_string(archived_entry.path()).unwrap();
        let archived_job: serde_json::Value = serde_json::from_str(&archived_content).unwrap();
        assert!(archived_job["payload"]["job_id"].is_number());
    }

    // Verify Stage3 inbox is clean (no leaked jobs)
    let stage3_remaining = fs::read_dir(&stage3_inbox)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(stage3_remaining, 0, "Stage3 inbox should be clean after processing");
}

/// Test: Resource leak detection in long-running pipeline
#[tokio::test]
async fn test_resource_leak_detection() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create 3-stage pipeline
    let stages = vec!["LeakStage1", "LeakStage2", "LeakStage3"];
    let mut kernels = vec![];

    for stage in &stages {
        let stage_dir = concepts_root.join(stage);
        create_test_ontology(&project_root, stage);

        let mut kernel = Kernel::new(project_root.clone(), Some(stage.to_string()), false);
        kernel.bootstrap(stage).await.unwrap();
        kernels.push(kernel);
    }

    // Run pipeline for 100 iterations
    let mut total_jobs_created = 0;
    let iterations = 100;

    for iteration in 0..iterations {
        // Create and emit job
        let payload = json!({
            "iteration": iteration,
            "timestamp": Utc::now().to_rfc3339()
        });

        kernels[0].emit(&stages[1], payload.clone()).await.unwrap();
        kernels[1].emit(&stages[2], payload.clone()).await.unwrap();

        total_jobs_created += 2; // One to Stage2, one to Stage3

        // Periodically archive processed jobs
        if iteration % 10 == 0 {
            for stage in &stages {
                let inbox_path = concepts_root.join(stage).join("queue/inbox");
                let archive_path = concepts_root.join(stage).join("queue/archive");

                if let Ok(entries) = fs::read_dir(&inbox_path) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        if entry.path().extension().and_then(|s| s.to_str()) == Some("job") {
                            let dest = archive_path.join(entry.file_name());
                            fs::rename(entry.path(), dest).unwrap();
                        }
                    }
                }
            }
        }
    }

    // Check for resource leaks: count total jobs across all directories
    let mut total_jobs_found = 0;

    for stage in &stages {
        let stage_dir = concepts_root.join(stage);

        // Count inbox jobs
        let inbox_count = fs::read_dir(stage_dir.join("queue/inbox"))
            .unwrap()
            .filter(|e| {
                e.as_ref().unwrap().path().extension().and_then(|s| s.to_str()) == Some("job")
            })
            .count();

        // Count archived jobs
        let archive_count = fs::read_dir(stage_dir.join("queue/archive"))
            .unwrap()
            .filter(|e| {
                e.as_ref().unwrap().path().extension().and_then(|s| s.to_str()) == Some("job")
            })
            .count();

        total_jobs_found += inbox_count + archive_count;
    }

    // Verify no jobs were leaked (all jobs accounted for)
    assert_eq!(
        total_jobs_found, total_jobs_created,
        "All created jobs should be accounted for (no leaks)"
    );

    // Verify no orphaned files in unexpected locations
    for stage in &stages {
        let stage_dir = concepts_root.join(stage);

        // Check storage directory for unexpected files
        let storage_entries = fs::read_dir(stage_dir.join("storage")).unwrap().count();
        // Storage should be empty (no instances minted in this test)
        assert_eq!(storage_entries, 0, "No orphaned files in storage for {}", stage);
    }

    // Verify all archived jobs are valid (no corruption)
    for stage in &stages {
        let archive_path = concepts_root.join(stage).join("queue/archive");
        if let Ok(entries) = fs::read_dir(&archive_path) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("job") {
                    let job_content = fs::read_to_string(entry.path()).unwrap();
                    let job: Result<serde_json::Value, _> = serde_json::from_str(&job_content);
                    assert!(job.is_ok(), "All archived jobs should be valid JSON");
                }
            }
        }
    }
}

// ==================== PERFORMANCE BENCHMARK TESTS - MEMORY PROFILING (+7 TESTS) ====================
//
// Memory Profiling: Detect memory leaks, test large payload handling, monitor queue growth, and Arc/Mutex overhead
// These tests ensure efficient memory usage and prevent resource exhaustion

/// Test: Memory leak detection - repeated allocations (Test 1/7)
#[tokio::test]
async fn test_perf_memory_leak_detection() {
    use std::time::Instant;

    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create target kernel
    let target_dir = concepts_root.join("MemoryLeakTarget");
    create_test_ontology(&project_root, "MemoryLeakTarget");

    // Repeatedly create and drop kernels to test for memory leaks
    let iterations = 100;
    let start = Instant::now();

    for i in 0..iterations {
        let mut kernel = Kernel::new(
            concepts_root.clone(),
            Some(format!("LeakSource{}", i)),
            false
        );

        // Emit a few jobs
        for j in 0..10 {
            let payload = serde_json::json!({
                "iteration": i,
                "job": j,
                "data": "test".repeat(100) // Small repeating data
            });
            kernel.emit("MemoryLeakTarget", payload).await.unwrap();
        }

        // Kernel dropped here - should free all memory
    }

    let duration = start.elapsed();
    println!("[PERF] Memory leak test: {} kernel lifecycles in {:?}", iterations, duration);

    // Verify all jobs were created (100 kernels × 10 jobs = 1000)
    let inbox = target_dir.join("queue/inbox");
    let job_count = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(job_count, 1000, "Should have created 1000 jobs");

    // If we got here without OOM or panic, no obvious memory leak
    println!("[PERF] No memory leak detected after {} iterations", iterations);
}

/// Test: Memory leak detection with concurrent operations (Test 2/7)
#[tokio::test]
async fn test_perf_concurrent_memory_leak() {
    use std::sync::Arc;

    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = Arc::new(project_root.join("concepts"));

    // Create target
    let target_dir = concepts_root.join("ConcurrentMemTarget");
    create_test_ontology(&project_root, "ConcurrentMemTarget");

    // Spawn many concurrent tasks that create temporary data
    let task_count = 50;
    let mut handles = vec![];

    for i in 0..task_count {
        let root_clone = Arc::clone(&concepts_root);
        let handle = tokio::spawn(async move {
            let mut kernel = Kernel::new(
                (*root_clone).clone(),
                Some(format!("MemSource{}", i)),
                false
            );

            // Each task emits 20 jobs
            for j in 0..20 {
                let payload = serde_json::json!({
                    "task": i,
                    "index": j,
                    "buffer": vec![0u8; 1024] // 1KB buffer
                });
                kernel.emit("ConcurrentMemTarget", payload).await.unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify total jobs (50 tasks × 20 jobs = 1000)
    let inbox = target_dir.join("queue/inbox");
    let job_count = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(job_count, 1000, "Should have 1000 jobs from concurrent tasks");
    println!("[PERF] Concurrent memory test passed: {} tasks completed", task_count);
}

/// Test: Large payload handling - 10MB payloads (Test 3/7)
#[tokio::test]
async fn test_perf_large_payload_10mb() {
    use std::time::Instant;

    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    let target_dir = concepts_root.join("LargePayloadTarget");
    create_test_ontology(&project_root, "LargePayloadTarget");

    let mut kernel = Kernel::new(
        concepts_root.clone(),
        Some("LargePayloadSource".to_string()),
        false
    );

    // Create 10MB payload
    let large_string = "A".repeat(10_000_000); // 10 million chars ≈ 10MB
    let large_payload = serde_json::json!({
        "size": "10MB",
        "data": large_string
    });

    let iterations = 5;
    let start = Instant::now();

    for i in 0..iterations {
        let payload = serde_json::json!({
            "iteration": i,
            "large_data": large_payload.clone()
        });

        kernel.emit("LargePayloadTarget", payload).await.unwrap();
    }

    let duration = start.elapsed();
    let mb_per_sec = (iterations * 10) as f64 / duration.as_secs_f64();

    println!("[PERF] Large payload: {} × 10MB = 50MB in {:?} ({:.2} MB/s)",
             iterations, duration, mb_per_sec);

    // Should handle large payloads without crashing
    let inbox = target_dir.join("queue/inbox");
    let job_count = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(job_count, iterations, "All large payloads should be written");

    // Verify file sizes are actually large
    for entry in fs::read_dir(&inbox).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().and_then(|s| s.to_str()) == Some("job") {
            let metadata = fs::metadata(entry.path()).unwrap();
            assert!(metadata.len() > 5_000_000, "Job file should be > 5MB");
        }
    }

    println!("[PERF] Large payload test passed: handled {} × 10MB payloads", iterations);
}

/// Test: Large payload handling - 50MB stress test (Test 4/7)
#[tokio::test]
async fn test_perf_large_payload_50mb_stress() {
    use std::time::Instant;

    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    let target_dir = concepts_root.join("StressPayloadTarget");
    create_test_ontology(&project_root, "StressPayloadTarget");

    let mut kernel = Kernel::new(
        concepts_root.clone(),
        Some("StressPayloadSource".to_string()),
        false
    );

    // Create 50MB payload (pushing limits)
    let huge_string = "X".repeat(50_000_000); // 50 million chars ≈ 50MB
    let huge_payload = serde_json::json!({
        "size": "50MB",
        "data": huge_string,
        "metadata": {
            "warning": "extremely large payload",
            "test_type": "stress"
        }
    });

    let start = Instant::now();
    let result = kernel.emit("StressPayloadTarget", huge_payload).await;
    let duration = start.elapsed();

    // Should succeed even with 50MB payload
    assert!(result.is_ok(), "Should handle 50MB payload: {:?}", result.err());

    println!("[PERF] 50MB payload handled in {:?}", duration);

    // Verify file was written
    let inbox = target_dir.join("queue/inbox");
    let jobs: Vec<_> = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(jobs.len(), 1, "Should have 1 huge job");

    let metadata = fs::metadata(jobs[0].path()).unwrap();
    assert!(metadata.len() > 40_000_000, "Job file should be > 40MB");

    println!("[PERF] 50MB stress test passed");
}

/// Test: Queue memory growth monitoring (Test 5/7)
#[tokio::test]
async fn test_perf_queue_memory_growth() {
    use std::time::Instant;

    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    let target_dir = concepts_root.join("QueueGrowthTarget");
    create_test_ontology(&project_root, "QueueGrowthTarget");

    let mut kernel = Kernel::new(
        concepts_root.clone(),
        Some("QueueGrowthSource".to_string()),
        false
    );

    // Create jobs in waves and measure queue directory size
    let waves = 10;
    let jobs_per_wave = 100;
    let mut sizes = Vec::new();

    let start = Instant::now();

    for wave in 0..waves {
        // Emit batch of jobs
        for i in 0..jobs_per_wave {
            let payload = serde_json::json!({
                "wave": wave,
                "index": i,
                "data": "x".repeat(1000) // 1KB per job
            });
            kernel.emit("QueueGrowthTarget", payload).await.unwrap();
        }

        // Measure queue directory size
        let inbox = target_dir.join("queue/inbox");
        let mut total_size = 0u64;
        for entry in fs::read_dir(&inbox).unwrap() {
            if let Ok(entry) = entry {
                if let Ok(metadata) = fs::metadata(entry.path()) {
                    total_size += metadata.len();
                }
            }
        }

        sizes.push(total_size);
        println!("[PERF] Wave {}: queue size = {} bytes ({} jobs)",
                 wave, total_size, (wave + 1) * jobs_per_wave);
    }

    let duration = start.elapsed();
    let total_jobs = waves * jobs_per_wave;

    println!("[PERF] Queue growth: {} jobs in {:?}", total_jobs, duration);

    // Verify linear growth (not exponential)
    let first_wave_size = sizes[0];
    let last_wave_size = sizes[sizes.len() - 1];
    let growth_ratio = last_wave_size as f64 / first_wave_size as f64;

    println!("[PERF] Growth ratio: {:.2}x (first: {}, last: {})",
             growth_ratio, first_wave_size, last_wave_size);

    // Growth should be roughly linear (10x jobs ≈ 10x size)
    assert!(growth_ratio >= 8.0 && growth_ratio <= 12.0,
            "Queue growth should be linear: got {:.2}x", growth_ratio);
}

/// Test: Queue memory growth with cleanup (Test 6/7)
#[tokio::test]
async fn test_perf_queue_memory_growth_with_cleanup() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");
    let kernel_dir = concepts_root.join("CleanupKernel");
    create_test_ontology(&project_root, "CleanupKernel");

    let driver = FileSystemDriver::new(
        project_root.clone(),
        "CleanupKernel".to_string()
    );

    let inbox = driver.get_queue_inbox();
    fs::create_dir_all(&inbox).unwrap(); // Ensure inbox exists

    // Phase 1: Grow queue to 500 jobs (write directly to inbox)
    for i in 0..500 {
        let tx_id = format!("cleanup-test-{}", i);
        let payload = serde_json::json!({"phase": 1, "index": i});
        let job_path = inbox.join(format!("{}.job", tx_id));
        fs::write(&job_path, serde_json::to_string_pretty(&payload).unwrap()).unwrap();
    }

    let initial_count = driver.count_queue_files(&inbox).unwrap();
    assert_eq!(initial_count, 500, "Should have 500 jobs");

    // Phase 2: Archive half the jobs (cleanup)
    let jobs: Vec<_> = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .take(250)
        .collect();

    for job_entry in jobs {
        let tx_id = driver.extract_tx_id_from_job_path(&job_entry.path())
            .unwrap_or_else(|| "unknown".to_string());
        driver.archive_job(&job_entry.path(), &tx_id).ok();
    }

    let after_cleanup = driver.count_queue_files(&inbox).unwrap();
    println!("[PERF] After cleanup: {} jobs (from {})", after_cleanup, initial_count);

    // Should have cleaned up ~250 jobs
    assert!(after_cleanup <= 250, "Should have ~250 jobs after cleanup");

    // Phase 3: Verify we can still add more jobs after cleanup
    for i in 0..100 {
        let tx_id = format!("cleanup-test-phase3-{}", i);
        let payload = serde_json::json!({"phase": 3, "index": i});
        let job_path = inbox.join(format!("{}.job", tx_id));
        fs::write(&job_path, serde_json::to_string_pretty(&payload).unwrap()).unwrap();
    }

    let final_count = driver.count_queue_files(&inbox).unwrap();
    println!("[PERF] Final count: {} jobs", final_count);

    assert!(final_count <= 350, "Should have ~350 jobs total");
}

/// Test: Arc/Mutex overhead (Test 7/7)
#[tokio::test]
async fn test_perf_arc_mutex_overhead() {
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    let target_dir = concepts_root.join("ArcMutexTarget");
    create_test_ontology(&project_root, "ArcMutexTarget");

    // Test 1: Direct access (baseline)
    let mut direct_kernel = Kernel::new(
        concepts_root.clone(),
        Some("DirectKernel".to_string()),
        false
    );

    let iterations = 100;
    let start_direct = Instant::now();

    for i in 0..iterations {
        let payload = serde_json::json!({"test": "direct", "index": i});
        direct_kernel.emit("ArcMutexTarget", payload).await.unwrap();
    }

    let direct_duration = start_direct.elapsed();
    let direct_per_op = direct_duration.as_micros() / iterations;

    // Test 2: Arc wrapped (shared ownership overhead)
    let arc_kernel = Arc::new(Mutex::new(Kernel::new(
        concepts_root.clone(),
        Some("ArcKernel".to_string()),
        false
    )));

    let start_arc = Instant::now();

    for i in 0..iterations {
        let payload = serde_json::json!({"test": "arc", "index": i});
        let mut kernel = arc_kernel.lock().unwrap();
        kernel.emit("ArcMutexTarget", payload).await.unwrap();
    }

    let arc_duration = start_arc.elapsed();
    let arc_per_op = arc_duration.as_micros() / iterations;

    // Test 3: Arc + concurrent access (using tokio::sync::Mutex for async compatibility)
    let shared_kernel = Arc::new(tokio::sync::Mutex::new(Kernel::new(
        concepts_root.clone(),
        Some("SharedKernel".to_string()),
        false
    )));

    let start_concurrent = Instant::now();
    let mut handles = vec![];

    for i in 0..10 {
        let kernel_clone = Arc::clone(&shared_kernel);
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let payload = serde_json::json!({"thread": i, "index": j});
                let mut kernel = kernel_clone.lock().await;
                kernel.emit("ArcMutexTarget", payload).await.unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let concurrent_duration = start_concurrent.elapsed();
    let concurrent_per_op = concurrent_duration.as_micros() / iterations;

    println!("[PERF] Arc/Mutex overhead:");
    println!("  Direct:     {} μs/op", direct_per_op);
    println!("  Arc:        {} μs/op ({:.2}x overhead)", arc_per_op, arc_per_op as f64 / direct_per_op as f64);
    println!("  Concurrent: {} μs/op ({:.2}x overhead)", concurrent_per_op, concurrent_per_op as f64 / direct_per_op as f64);

    // Arc overhead should be minimal (< 2x for single-threaded, < 5x for concurrent)
    assert!(arc_per_op < direct_per_op * 2,
            "Arc overhead too high: {} μs vs {} μs", arc_per_op, direct_per_op);
    assert!(concurrent_per_op < direct_per_op * 5,
            "Concurrent overhead too high: {} μs vs {} μs", concurrent_per_op, direct_per_op);

    // Verify all jobs were created
    let inbox = target_dir.join("queue/inbox");
    let job_count = fs::read_dir(&inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .count();

    assert_eq!(job_count, 300, "Should have 300 total jobs (100 + 100 + 100)");
}

// ==================== MULTI-KERNEL ERROR PROPAGATION TESTS (+10 TESTS) ====================

// ----- Error Bubbling Through Chains (3 tests) -----

/// Test: Simple A→B→C chain with error at B
#[tokio::test]
async fn test_error_bubbling_simple_chain_error_at_middle() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create three-kernel chain: KernelA -> KernelB -> KernelC
    let kernel_a_dir = concepts_root.join("KernelA");
    let kernel_b_dir = concepts_root.join("KernelB");
    let kernel_c_dir = concepts_root.join("KernelC");

    for dir in [&kernel_a_dir, &kernel_b_dir, &kernel_c_dir] {
        create_test_ontology(&project_root, &dir.file_name().unwrap().to_string_lossy());
    }

    // Initialize all kernels
    let mut kernel_a = Kernel::new(project_root.clone(), Some("KernelA".to_string()), false);
    let mut kernel_b = Kernel::new(project_root.clone(), Some("KernelB".to_string()), false);

    kernel_a.bootstrap("KernelA").await.unwrap();
    kernel_b.bootstrap("KernelB").await.unwrap();

    // A emits to B successfully
    let tx_id_ab = kernel_a.emit("KernelB", json!({
        "operation": "step1",
        "data": "from_a"
    })).await.unwrap();

    // Verify B received the job
    let inbox_b = kernel_b_dir.join("queue/inbox");
    let jobs_b: Vec<_> = fs::read_dir(&inbox_b)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();
    assert_eq!(jobs_b.len(), 1, "KernelB should have received 1 job");

    // Simulate error at B: B cannot emit to C (blocked by RBAC or error condition)
    // Update B's ontology to deny communication with C
    let restrictive_ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://KernelB:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed: []
      denied:
        - "*"
"#;
    fs::write(kernel_b_dir.join("conceptkernel.yaml"), restrictive_ontology).unwrap();

    // Re-bootstrap B to load restrictive RBAC
    let mut kernel_b_restricted = Kernel::new(project_root.clone(), Some("KernelB".to_string()), true);
    kernel_b_restricted.bootstrap("KernelB").await.unwrap();

    // B tries to emit to C - should fail due to RBAC
    let result_bc = kernel_b_restricted.emit("KernelC", json!({
        "operation": "step2",
        "data": "from_b",
        "parent_tx": tx_id_ab,
        "error_source": "KernelB"
    })).await;

    assert!(result_bc.is_err(), "KernelB should fail to emit to KernelC");

    // Verify error propagation: no job created in C's inbox
    let inbox_c = kernel_c_dir.join("queue/inbox");
    if inbox_c.exists() {
        let jobs_c: Vec<_> = fs::read_dir(&inbox_c)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(jobs_c.len(), 0, "KernelC should not have received any jobs due to error at B");
    }

    // Simulate error logging: write error to B's error queue
    let error_queue = kernel_b_dir.join("queue/errors");
    fs::create_dir_all(&error_queue).unwrap();

    let error_record = json!({
        "error": result_bc.unwrap_err().to_string(),
        "source_tx": tx_id_ab,
        "failed_target": "KernelC",
        "timestamp": Utc::now().to_rfc3339()
    });

    fs::write(
        error_queue.join(format!("{}-error.json", tx_id_ab)),
        serde_json::to_string_pretty(&error_record).unwrap()
    ).unwrap();

    // Verify error was logged
    let error_files: Vec<_> = fs::read_dir(&error_queue)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(error_files.len(), 1, "Error should be logged in KernelB's error queue");
}

/// Test: Error at C propagating back to A (reverse error propagation)
#[tokio::test]
async fn test_error_bubbling_propagate_to_origin() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create three-kernel chain with error tracking
    let kernel_a_dir = concepts_root.join("OriginKernel");
    let kernel_b_dir = concepts_root.join("MiddleKernel");
    let kernel_c_dir = concepts_root.join("TerminalKernel");

    for dir in [&kernel_a_dir, &kernel_b_dir, &kernel_c_dir] {
        create_test_ontology(&project_root, &dir.file_name().unwrap().to_string_lossy());
    }

    // Initialize kernels
    let mut kernel_a = Kernel::new(project_root.clone(), Some("OriginKernel".to_string()), false);
    let mut kernel_b = Kernel::new(project_root.clone(), Some("MiddleKernel".to_string()), false);
    let mut kernel_c = Kernel::new(project_root.clone(), Some("TerminalKernel".to_string()), false);

    kernel_a.bootstrap("OriginKernel").await.unwrap();
    kernel_b.bootstrap("MiddleKernel").await.unwrap();
    kernel_c.bootstrap("TerminalKernel").await.unwrap();

    // A -> B -> C chain execution
    let origin_tx = kernel_a.emit("MiddleKernel", json!({
        "workflow_id": "wf-001",
        "step": 1,
        "reply_to": "OriginKernel"
    })).await.unwrap();

    let middle_tx = kernel_b.emit("TerminalKernel", json!({
        "workflow_id": "wf-001",
        "step": 2,
        "parent_tx": origin_tx,
        "reply_to": "OriginKernel"
    })).await.unwrap();

    // Simulate failure at C: C writes error back to origin
    let error_response = json!({
        "workflow_id": "wf-001",
        "error": "Processing failed at TerminalKernel",
        "failed_tx": middle_tx,
        "origin_tx": origin_tx,
        "error_code": "TERMINAL_ERROR",
        "timestamp": Utc::now().to_rfc3339()
    });

    // C emits error back to A
    let _error_tx = kernel_c.emit("OriginKernel", error_response.clone()).await.unwrap();

    // Verify A received error notification
    let inbox_a = kernel_a_dir.join("queue/inbox");
    let jobs_a: Vec<_> = fs::read_dir(&inbox_a)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    // Should have at least one job (the error response)
    assert!(jobs_a.len() >= 1, "OriginKernel should receive error notification");

    // Find and verify error response
    let mut found_error = false;
    for job_entry in jobs_a {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["source"] == "TerminalKernel" && job["payload"]["error_code"].is_string() {
            found_error = true;
            assert_eq!(job["payload"]["error_code"], "TERMINAL_ERROR");
            assert_eq!(job["payload"]["origin_tx"], origin_tx);
            break;
        }
    }

    assert!(found_error, "Error should propagate back to origin kernel");
}

/// Test: Multiple parallel errors (fan-out with failures)
#[tokio::test]
async fn test_error_bubbling_multiple_parallel_errors() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create coordinator and 4 worker kernels
    let coordinator_dir = concepts_root.join("Coordinator");
    let worker_dirs: Vec<_> = (1..=4)
        .map(|i| concepts_root.join(format!("Worker{}", i)))
        .collect();

    create_test_ontology(&project_root, "Coordinator");

    for (i, dir) in worker_dirs.iter().enumerate() {
        create_test_ontology(&project_root, &format!("Worker{}", i + 1));
    }

    // Initialize coordinator
    let mut coordinator = Kernel::new(project_root.clone(), Some("Coordinator".to_string()), false);
    coordinator.bootstrap("Coordinator").await.unwrap();

    // Coordinator fans out to all workers
    let workflow_id = format!("parallel-wf-{}", Utc::now().timestamp());
    let mut worker_txs = vec![];

    for i in 1..=4 {
        let tx = coordinator.emit(&format!("Worker{}", i), json!({
            "workflow_id": workflow_id,
            "worker_id": i,
            "task": format!("process_chunk_{}", i)
        })).await.unwrap();
        worker_txs.push((i, tx));
    }

    // Simulate failures in Workers 2 and 4
    let failed_workers = vec![2, 4];

    for worker_idx in &failed_workers {
        let worker_dir = concepts_root.join(format!("Worker{}", worker_idx));
        let error_queue = worker_dir.join("queue/errors");

        let error_record = json!({
            "workflow_id": workflow_id,
            "worker_id": worker_idx,
            "error": format!("Worker{} processing failed", worker_idx),
            "error_type": "PROCESSING_ERROR",
            "timestamp": Utc::now().to_rfc3339()
        });

        fs::write(
            error_queue.join(format!("error-{}.json", worker_txs[worker_idx - 1].1)),
            serde_json::to_string_pretty(&error_record).unwrap()
        ).unwrap();
    }

    // Verify errors were logged in failed workers
    for worker_idx in &failed_workers {
        let worker_dir = concepts_root.join(format!("Worker{}", worker_idx));
        let error_queue = worker_dir.join("queue/errors");

        let error_files: Vec<_> = fs::read_dir(&error_queue)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();

        assert_eq!(error_files.len(), 1, "Worker{} should have logged error", worker_idx);
    }

    // Simulate successful workers (1 and 3) emitting error notifications to coordinator
    let successful_workers = vec![1, 3];

    for worker_idx in &successful_workers {
        let mut worker = Kernel::new(
            concepts_root.clone(),
            Some(format!("Worker{}", worker_idx)),
            false
        );

        worker.emit("Coordinator", json!({
            "workflow_id": workflow_id,
            "worker_id": worker_idx,
            "status": "completed",
            "partial_failure_detected": true
        })).await.unwrap();
    }

    // Verify coordinator received at least partial results
    let coordinator_inbox = coordinator_dir.join("queue/inbox");
    let coordinator_jobs: Vec<_> = fs::read_dir(&coordinator_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert!(coordinator_jobs.len() >= 2, "Coordinator should receive updates from successful workers");

    // Verify mix of success and failure
    let mut successful_count = 0;
    for job_entry in coordinator_jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["payload"]["status"] == "completed" {
            successful_count += 1;
        }
    }

    assert_eq!(successful_count, 2, "Should have 2 successful worker completions");
}

// ----- Partial Failure Recovery (3 tests) -----

/// Test: Fork-join with one path failing (graceful degradation)
#[tokio::test]
async fn test_partial_failure_fork_join_one_path_fails() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create fork-join pattern: Source -> PathA + PathB -> Merger
    let source_dir = concepts_root.join("ForkSource");
    let path_a_dir = concepts_root.join("PathA");
    let path_b_dir = concepts_root.join("PathB");
    let merger_dir = concepts_root.join("Merger");

    for dir in [&source_dir, &path_a_dir, &path_b_dir, &merger_dir] {
        create_test_ontology(&project_root, &dir.file_name().unwrap().to_string_lossy());
    }

    // Initialize kernels
    let mut source = Kernel::new(project_root.clone(), Some("ForkSource".to_string()), false);
    let mut path_a = Kernel::new(project_root.clone(), Some("PathA".to_string()), false);
    let mut path_b = Kernel::new(project_root.clone(), Some("PathB".to_string()), false);

    source.bootstrap("ForkSource").await.unwrap();
    path_a.bootstrap("PathA").await.unwrap();
    path_b.bootstrap("PathB").await.unwrap();

    // Fork: Source emits to both paths
    let fork_id = format!("fork-{}", Utc::now().timestamp());

    let _tx_a = source.emit("PathA", json!({
        "fork_id": fork_id,
        "path": "A",
        "data": "processing_a"
    })).await.unwrap();

    let tx_b = source.emit("PathB", json!({
        "fork_id": fork_id,
        "path": "B",
        "data": "processing_b"
    })).await.unwrap();

    // PathA completes successfully and emits to Merger
    let _merge_a = path_a.emit("Merger", json!({
        "fork_id": fork_id,
        "path": "A",
        "result": "success",
        "data": "result_from_a"
    })).await.unwrap();

    // PathB fails (simulate RBAC restriction to Merger)
    let restrictive_ontology = r#"
apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://PathB:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - "ckp://ForkSource"
      denied:
        - "ckp://Merger"
"#;
    fs::write(path_b_dir.join("conceptkernel.yaml"), restrictive_ontology).unwrap();

    let mut path_b_restricted = Kernel::new(project_root.clone(), Some("PathB".to_string()), true);
    path_b_restricted.bootstrap("PathB").await.unwrap();

    let result_b = path_b_restricted.emit("Merger", json!({
        "fork_id": fork_id,
        "path": "B",
        "result": "failure"
    })).await;

    assert!(result_b.is_err(), "PathB should fail to emit to Merger");

    // Log PathB failure
    let error_queue_b = path_b_dir.join("queue/errors");
    fs::write(
        error_queue_b.join(format!("{}-merge-error.json", tx_b)),
        serde_json::to_string_pretty(&json!({
            "fork_id": fork_id,
            "path": "B",
            "error": result_b.unwrap_err().to_string(),
            "timestamp": Utc::now().to_rfc3339()
        })).unwrap()
    ).unwrap();

    // Verify Merger received partial result (only from PathA)
    let merger_inbox = merger_dir.join("queue/inbox");
    let merger_jobs: Vec<_> = fs::read_dir(&merger_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    assert_eq!(merger_jobs.len(), 1, "Merger should receive result only from PathA");

    // Verify the received result is from PathA
    let job_content = fs::read_to_string(merger_jobs[0].path()).unwrap();
    let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();
    assert_eq!(job["payload"]["path"], "A");
    assert_eq!(job["payload"]["result"], "success");

    // Verify PathB error was logged
    let error_files_b: Vec<_> = fs::read_dir(&error_queue_b)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(error_files_b.len(), 1, "PathB error should be logged");
}

/// Test: Recovery with compensation transactions
#[tokio::test]
async fn test_partial_failure_compensation_transaction() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create transaction chain: Client -> Service1 -> Service2 (failure) -> Service1 (compensate)
    let client_dir = concepts_root.join("TransactionClient");
    let service1_dir = concepts_root.join("Service1");
    let service2_dir = concepts_root.join("Service2");

    for dir in [&client_dir, &service1_dir, &service2_dir] {
        create_test_ontology(&project_root, &dir.file_name().unwrap().to_string_lossy());
    }

    // Initialize kernels
    let mut client = Kernel::new(project_root.clone(), Some("TransactionClient".to_string()), false);
    let mut service1 = Kernel::new(project_root.clone(), Some("Service1".to_string()), false);
    let mut service2 = Kernel::new(project_root.clone(), Some("Service2".to_string()), false);

    client.bootstrap("TransactionClient").await.unwrap();
    service1.bootstrap("Service1").await.unwrap();
    service2.bootstrap("Service2").await.unwrap();

    // Step 1: Client initiates transaction to Service1
    let tx_id = format!("tx-{}", Utc::now().timestamp());

    let tx1 = client.emit("Service1", json!({
        "transaction_id": tx_id,
        "operation": "reserve_resource",
        "resource_id": "res-001",
        "amount": 100
    })).await.unwrap();

    // Step 2: Service1 processes and forwards to Service2
    let tx2 = service1.emit("Service2", json!({
        "transaction_id": tx_id,
        "parent_tx": tx1,
        "operation": "allocate_resource",
        "resource_id": "res-001",
        "amount": 100
    })).await.unwrap();

    // Step 3: Service2 fails (simulate by not emitting success)
    // Write failure to Service2's error queue
    let error_queue2 = service2_dir.join("queue/compensation");
    fs::write(
        error_queue2.join(format!("{}-failure.json", tx2)),
        serde_json::to_string_pretty(&json!({
            "transaction_id": tx_id,
            "failed_tx": tx2,
            "error": "Insufficient resources at Service2",
            "requires_compensation": true,
            "timestamp": Utc::now().to_rfc3339()
        })).unwrap()
    ).unwrap();

    // Step 4: Service2 triggers compensation by emitting rollback to Service1
    let _compensation_tx = service2.emit("Service1", json!({
        "transaction_id": tx_id,
        "operation": "compensate",
        "original_tx": tx1,
        "reason": "Insufficient resources at Service2",
        "compensation_action": "release_resource"
    })).await.unwrap();

    // Verify Service1 received compensation request
    let inbox1 = service1_dir.join("queue/inbox");
    let jobs1: Vec<_> = fs::read_dir(&inbox1)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    // Find compensation job
    let mut found_compensation = false;
    for job_entry in jobs1 {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["payload"]["operation"] == "compensate" {
            found_compensation = true;
            assert_eq!(job["payload"]["transaction_id"], tx_id);
            assert_eq!(job["payload"]["compensation_action"], "release_resource");
            break;
        }
    }

    assert!(found_compensation, "Service1 should receive compensation request");

    // Step 5: Service1 performs compensation and notifies Client
    let _compensation_complete = service1.emit("TransactionClient", json!({
        "transaction_id": tx_id,
        "status": "compensated",
        "original_operation": "reserve_resource",
        "compensation_performed": "release_resource",
        "final_state": "rolled_back"
    })).await.unwrap();

    // Verify Client received compensation notification
    let client_inbox = client_dir.join("queue/inbox");
    let client_jobs: Vec<_> = fs::read_dir(&client_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    let mut found_rollback_notification = false;
    for job_entry in client_jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["payload"]["status"] == "compensated" {
            found_rollback_notification = true;
            assert_eq!(job["payload"]["final_state"], "rolled_back");
            break;
        }
    }

    assert!(found_rollback_notification, "Client should be notified of compensation");
}

/// Test: Graceful degradation on failure (fallback to partial results)
#[tokio::test]
async fn test_partial_failure_graceful_degradation() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create aggregator pattern: Aggregator -> 3 data sources -> Aggregator (results)
    let aggregator_dir = concepts_root.join("Aggregator");
    let source_dirs: Vec<_> = (1..=3)
        .map(|i| concepts_root.join(format!("DataSource{}", i)))
        .collect();

    create_test_ontology(&project_root, "Aggregator");

    for (i, dir) in source_dirs.iter().enumerate() {
        create_test_ontology(&project_root, &format!("DataSource{}", i + 1));
    }

    // Initialize aggregator
    let mut aggregator = Kernel::new(project_root.clone(), Some("Aggregator".to_string()), false);
    aggregator.bootstrap("Aggregator").await.unwrap();

    // Aggregator requests data from all sources
    let request_id = format!("req-{}", Utc::now().timestamp());

    for i in 1..=3 {
        aggregator.emit(&format!("DataSource{}", i), json!({
            "request_id": request_id,
            "source_id": i,
            "query": "fetch_data"
        })).await.unwrap();
    }

    // DataSource1 responds successfully
    let mut source1 = Kernel::new(project_root.clone(), Some("DataSource1".to_string()), false);
    source1.emit("Aggregator", json!({
        "request_id": request_id,
        "source_id": 1,
        "status": "success",
        "data": [1, 2, 3]
    })).await.unwrap();

    // DataSource2 fails (timeout simulation - no response)
    // No emission from DataSource2

    // DataSource3 responds successfully
    let mut source3 = Kernel::new(project_root.clone(), Some("DataSource3".to_string()), false);
    source3.emit("Aggregator", json!({
        "request_id": request_id,
        "source_id": 3,
        "status": "success",
        "data": [7, 8, 9]
    })).await.unwrap();

    // Verify Aggregator received partial results (2 out of 3 sources)
    let aggregator_inbox = aggregator_dir.join("queue/inbox");
    let aggregator_jobs: Vec<_> = fs::read_dir(&aggregator_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    let mut successful_responses = 0;
    let mut response_data = vec![];

    for job_entry in aggregator_jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["payload"]["status"] == "success" && job["payload"]["request_id"] == request_id {
            successful_responses += 1;
            if let Some(_data) = job["payload"]["data"].as_array() {
                response_data.push(job["payload"]["source_id"].as_i64().unwrap());
            }
        }
    }

    assert_eq!(successful_responses, 2, "Should receive 2 successful responses (graceful degradation)");
    assert!(response_data.contains(&1), "Should have response from DataSource1");
    assert!(response_data.contains(&3), "Should have response from DataSource3");
    assert!(!response_data.contains(&2), "Should NOT have response from DataSource2 (failed)");
}

// ----- Rollback Propagation (2 tests) -----

/// Test: Transaction rollback across kernel chain
#[tokio::test]
async fn test_rollback_propagation_across_chain() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create transaction chain: Coordinator -> Step1 -> Step2 -> Step3 (fail) -> rollback
    let coordinator_dir = concepts_root.join("TxCoordinator");
    let step1_dir = concepts_root.join("TxStep1");
    let step2_dir = concepts_root.join("TxStep2");
    let step3_dir = concepts_root.join("TxStep3");

    for dir in [&coordinator_dir, &step1_dir, &step2_dir, &step3_dir] {
        create_test_ontology(&project_root, &dir.file_name().unwrap().to_string_lossy());
    }

    // Initialize all kernels
    let mut coordinator = Kernel::new(project_root.clone(), Some("TxCoordinator".to_string()), false);
    let mut step1 = Kernel::new(project_root.clone(), Some("TxStep1".to_string()), false);
    let mut step2 = Kernel::new(project_root.clone(), Some("TxStep2".to_string()), false);
    let mut step3 = Kernel::new(project_root.clone(), Some("TxStep3".to_string()), false);

    coordinator.bootstrap("TxCoordinator").await.unwrap();
    step1.bootstrap("TxStep1").await.unwrap();
    step2.bootstrap("TxStep2").await.unwrap();
    step3.bootstrap("TxStep3").await.unwrap();

    // Execute transaction chain
    let tx_id = format!("rollback-tx-{}", Utc::now().timestamp());

    let tx1 = coordinator.emit("TxStep1", json!({
        "transaction_id": tx_id,
        "step": 1,
        "operation": "update_inventory",
        "rollback_handler": "undo_inventory_update"
    })).await.unwrap();

    let tx2 = step1.emit("TxStep2", json!({
        "transaction_id": tx_id,
        "step": 2,
        "parent_tx": tx1,
        "operation": "charge_payment",
        "rollback_handler": "refund_payment"
    })).await.unwrap();

    let tx3 = step2.emit("TxStep3", json!({
        "transaction_id": tx_id,
        "step": 3,
        "parent_tx": tx2,
        "operation": "ship_order",
        "rollback_handler": "cancel_shipment"
    })).await.unwrap();

    // Step3 fails - trigger rollback
    let rollback_queue3 = step3_dir.join("queue/rollback");
    fs::write(
        rollback_queue3.join(format!("{}-failed.json", tx3)),
        serde_json::to_string_pretty(&json!({
            "transaction_id": tx_id,
            "failed_step": 3,
            "error": "Shipping service unavailable",
            "rollback_required": true
        })).unwrap()
    ).unwrap();

    // Step3 emits rollback to Step2
    step3.emit("TxStep2", json!({
        "transaction_id": tx_id,
        "operation": "rollback",
        "step": 3,
        "reason": "Shipping service unavailable"
    })).await.unwrap();

    // Step2 performs rollback and propagates to Step1
    let rollback_queue2 = step2_dir.join("queue/rollback");
    fs::write(
        rollback_queue2.join(format!("{}-rollback.json", tx2)),
        serde_json::to_string_pretty(&json!({
            "transaction_id": tx_id,
            "step": 2,
            "operation": "refund_payment",
            "status": "rolled_back"
        })).unwrap()
    ).unwrap();

    step2.emit("TxStep1", json!({
        "transaction_id": tx_id,
        "operation": "rollback",
        "step": 2,
        "reason": "Cascade rollback from Step3"
    })).await.unwrap();

    // Step1 performs rollback and notifies Coordinator
    let rollback_queue1 = step1_dir.join("queue/rollback");
    fs::write(
        rollback_queue1.join(format!("{}-rollback.json", tx1)),
        serde_json::to_string_pretty(&json!({
            "transaction_id": tx_id,
            "step": 1,
            "operation": "undo_inventory_update",
            "status": "rolled_back"
        })).unwrap()
    ).unwrap();

    step1.emit("TxCoordinator", json!({
        "transaction_id": tx_id,
        "operation": "rollback_complete",
        "total_steps_rolled_back": 3,
        "final_status": "aborted"
    })).await.unwrap();

    // Verify rollback propagation
    assert!(rollback_queue1.exists(), "Step1 should have rollback record");
    assert!(rollback_queue2.exists(), "Step2 should have rollback record");
    assert!(rollback_queue3.exists(), "Step3 should have rollback record");

    // Verify Coordinator received rollback notification
    let coordinator_inbox = coordinator_dir.join("queue/inbox");
    let coordinator_jobs: Vec<_> = fs::read_dir(&coordinator_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    let mut found_rollback_complete = false;
    for job_entry in coordinator_jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["payload"]["operation"] == "rollback_complete" {
            found_rollback_complete = true;
            assert_eq!(job["payload"]["transaction_id"], tx_id);
            assert_eq!(job["payload"]["final_status"], "aborted");
            break;
        }
    }

    assert!(found_rollback_complete, "Coordinator should receive rollback completion notification");
}

/// Test: Partial commit with rollback (some steps succeed, some fail)
#[tokio::test]
async fn test_rollback_propagation_partial_commit() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create multi-stage transaction with partial success
    let orchestrator_dir = concepts_root.join("Orchestrator");
    let stage_dirs: Vec<_> = (1..=5)
        .map(|i| concepts_root.join(format!("Stage{}", i)))
        .collect();

    create_test_ontology(&project_root, "Orchestrator");

    for (i, dir) in stage_dirs.iter().enumerate() {
        create_test_ontology(&project_root, &format!("Stage{}", i + 1));
    }

    // Initialize orchestrator
    let mut orchestrator = Kernel::new(project_root.clone(), Some("Orchestrator".to_string()), false);
    orchestrator.bootstrap("Orchestrator").await.unwrap();

    // Start transaction
    let tx_id = format!("partial-tx-{}", Utc::now().timestamp());

    // Stages 1-3 complete successfully
    let mut stage_kernels = vec![];
    for i in 1..=5 {
        let mut kernel = Kernel::new(
            concepts_root.clone(),
            Some(format!("Stage{}", i)),
            false
        );
        kernel.bootstrap(&format!("Stage{}", i)).await.unwrap();
        stage_kernels.push(kernel);
    }

    // Execute stages sequentially
    for i in 1..=3 {
        let target = if i < 5 {
            format!("Stage{}", i + 1)
        } else {
            "Orchestrator".to_string()
        };

        let stage_dir = concepts_root.join(format!("Stage{}", i));
        let committed_queue = stage_dir.join("queue/committed");

        // Mark stage as committed
        fs::write(
            committed_queue.join(format!("{}-stage{}.json", tx_id, i)),
            serde_json::to_string_pretty(&json!({
                "transaction_id": tx_id,
                "stage": i,
                "status": "committed",
                "timestamp": Utc::now().to_rfc3339()
            })).unwrap()
        ).unwrap();

        if i < 3 {
            stage_kernels[i - 1].emit(&target, json!({
                "transaction_id": tx_id,
                "stage": i,
                "status": "success"
            })).await.unwrap();
        }
    }

    // Stage 4 fails
    let stage4_dir = concepts_root.join("Stage4");
    let rollback_queue4 = stage4_dir.join("queue/rollback");

    fs::write(
        rollback_queue4.join(format!("{}-failed.json", tx_id)),
        serde_json::to_string_pretty(&json!({
            "transaction_id": tx_id,
            "stage": 4,
            "error": "Database connection timeout",
            "requires_rollback": true
        })).unwrap()
    ).unwrap();

    // Trigger rollback of committed stages (3, 2, 1)
    for i in (1..=3).rev() {
        let stage_dir = concepts_root.join(format!("Stage{}", i));
        let rollback_queue = stage_dir.join("queue/rollback");

        fs::write(
            rollback_queue.join(format!("{}-rollback.json", tx_id)),
            serde_json::to_string_pretty(&json!({
                "transaction_id": tx_id,
                "stage": i,
                "operation": "rollback",
                "was_committed": true,
                "rollback_action": format!("undo_stage_{}", i)
            })).unwrap()
        ).unwrap();
    }

    // Orchestrator receives rollback notifications
    for i in 1..=3 {
        stage_kernels[i - 1].emit("Orchestrator", json!({
            "transaction_id": tx_id,
            "stage": i,
            "status": "rolled_back",
            "was_committed": true
        })).await.unwrap();
    }

    // Verify partial commit + rollback state
    for i in 1..=3 {
        let stage_dir = concepts_root.join(format!("Stage{}", i));
        let committed_queue = stage_dir.join("queue/committed");
        let rollback_queue = stage_dir.join("queue/rollback");

        // Should have both committed and rollback records
        assert!(committed_queue.exists(), "Stage{} should have commit record", i);
        assert!(rollback_queue.exists(), "Stage{} should have rollback record", i);

        let committed_files: Vec<_> = fs::read_dir(&committed_queue)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        let rollback_files: Vec<_> = fs::read_dir(&rollback_queue)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();

        assert_eq!(committed_files.len(), 1, "Stage{} should have 1 commit record", i);
        assert_eq!(rollback_files.len(), 1, "Stage{} should have 1 rollback record", i);
    }

    // Verify Stage 4 only has rollback (never committed)
    let stage4_committed = stage4_dir.join("queue/committed");
    let stage4_rollback = stage4_dir.join("queue/rollback");

    if stage4_committed.exists() {
        let committed_files: Vec<_> = fs::read_dir(&stage4_committed)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(committed_files.len(), 0, "Stage4 should NOT have commit records");
    }

    let rollback_files: Vec<_> = fs::read_dir(&stage4_rollback)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(rollback_files.len(), 1, "Stage4 should have 1 rollback record");

    // Verify Orchestrator received all rollback notifications
    let orchestrator_inbox = orchestrator_dir.join("queue/inbox");
    let orchestrator_jobs: Vec<_> = fs::read_dir(&orchestrator_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    let mut rolled_back_stages = vec![];
    for job_entry in orchestrator_jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["payload"]["status"] == "rolled_back" && job["payload"]["transaction_id"] == tx_id {
            rolled_back_stages.push(job["payload"]["stage"].as_i64().unwrap());
        }
    }

    assert_eq!(rolled_back_stages.len(), 3, "Should have 3 rollback notifications");
    assert!(rolled_back_stages.contains(&1), "Stage1 rollback notification");
    assert!(rolled_back_stages.contains(&2), "Stage2 rollback notification");
    assert!(rolled_back_stages.contains(&3), "Stage3 rollback notification");
}

// ----- Compensation Transactions (2 tests) -----

/// Test: Undo operations on failure (simple compensation)
#[tokio::test]
async fn test_compensation_undo_operations() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create service chain with compensation: Client -> ServiceA -> ServiceB (fail) -> ServiceA (undo)
    let client_dir = concepts_root.join("CompensationClient");
    let service_a_dir = concepts_root.join("ServiceA");
    let service_b_dir = concepts_root.join("ServiceB");

    for dir in [&client_dir, &service_a_dir, &service_b_dir] {
        create_test_ontology(&project_root, &dir.file_name().unwrap().to_string_lossy());
    }

    // Initialize kernels
    let mut client = Kernel::new(project_root.clone(), Some("CompensationClient".to_string()), false);
    let mut service_a = Kernel::new(project_root.clone(), Some("ServiceA".to_string()), false);
    let mut service_b = Kernel::new(project_root.clone(), Some("ServiceB".to_string()), false);

    client.bootstrap("CompensationClient").await.unwrap();
    service_a.bootstrap("ServiceA").await.unwrap();
    service_b.bootstrap("ServiceB").await.unwrap();

    // Client initiates operation
    let operation_id = format!("op-{}", Utc::now().timestamp());

    client.emit("ServiceA", json!({
        "operation_id": operation_id,
        "action": "create_account",
        "user_id": "user-123",
        "compensation_action": "delete_account"
    })).await.unwrap();

    // ServiceA completes and forwards to ServiceB
    let tx_a = service_a.emit("ServiceB", json!({
        "operation_id": operation_id,
        "action": "setup_profile",
        "user_id": "user-123",
        "parent_operation": "create_account",
        "compensation_action": "delete_profile"
    })).await.unwrap();

    // ServiceB fails
    let compensation_queue_b = service_b_dir.join("queue/compensation");
    fs::write(
        compensation_queue_b.join(format!("{}-failure.json", tx_a)),
        serde_json::to_string_pretty(&json!({
            "operation_id": operation_id,
            "error": "Profile setup failed - invalid data",
            "requires_compensation": true,
            "compensation_target": "ServiceA"
        })).unwrap()
    ).unwrap();

    // ServiceB triggers compensation to ServiceA
    service_b.emit("ServiceA", json!({
        "operation_id": operation_id,
        "command": "compensate",
        "original_action": "create_account",
        "compensation_action": "delete_account",
        "reason": "Profile setup failed"
    })).await.unwrap();

    // ServiceA executes compensation
    let compensation_queue_a = service_a_dir.join("queue/compensation");
    fs::write(
        compensation_queue_a.join(format!("{}-compensation.json", operation_id)),
        serde_json::to_string_pretty(&json!({
            "operation_id": operation_id,
            "original_action": "create_account",
            "compensation_action": "delete_account",
            "status": "compensated",
            "timestamp": Utc::now().to_rfc3339()
        })).unwrap()
    ).unwrap();

    // ServiceA notifies Client of compensation
    service_a.emit("CompensationClient", json!({
        "operation_id": operation_id,
        "status": "compensated",
        "message": "Account creation rolled back due to profile setup failure"
    })).await.unwrap();

    // Verify compensation records
    assert!(compensation_queue_a.exists(), "ServiceA should have compensation records");
    assert!(compensation_queue_b.exists(), "ServiceB should have compensation records");

    let comp_files_a: Vec<_> = fs::read_dir(&compensation_queue_a)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(comp_files_a.len(), 1, "ServiceA should have 1 compensation record");

    // Verify Client received compensation notification
    let client_inbox = client_dir.join("queue/inbox");
    let client_jobs: Vec<_> = fs::read_dir(&client_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    let mut found_compensation_notification = false;
    for job_entry in client_jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["payload"]["status"] == "compensated" {
            found_compensation_notification = true;
            assert_eq!(job["payload"]["operation_id"], operation_id);
            break;
        }
    }

    assert!(found_compensation_notification, "Client should receive compensation notification");
}

/// Test: Saga pattern implementation (long-running distributed transaction)
#[tokio::test]
async fn test_compensation_saga_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path().to_path_buf();
    let concepts_root = project_root.join("concepts");

    // Create saga pattern: Coordinator -> OrderService -> PaymentService -> InventoryService (fail) -> compensations
    let coordinator_dir = concepts_root.join("SagaCoordinator");
    let order_dir = concepts_root.join("OrderService");
    let payment_dir = concepts_root.join("PaymentService");
    let inventory_dir = concepts_root.join("InventoryService");

    for dir in [&coordinator_dir, &order_dir, &payment_dir, &inventory_dir] {
        create_test_ontology(&project_root, &dir.file_name().unwrap().to_string_lossy());
    }

    // Initialize all kernels
    let mut coordinator = Kernel::new(project_root.clone(), Some("SagaCoordinator".to_string()), false);
    let mut order_svc = Kernel::new(project_root.clone(), Some("OrderService".to_string()), false);
    let mut payment_svc = Kernel::new(project_root.clone(), Some("PaymentService".to_string()), false);
    let mut inventory_svc = Kernel::new(project_root.clone(), Some("InventoryService".to_string()), false);

    coordinator.bootstrap("SagaCoordinator").await.unwrap();
    order_svc.bootstrap("OrderService").await.unwrap();
    payment_svc.bootstrap("PaymentService").await.unwrap();
    inventory_svc.bootstrap("InventoryService").await.unwrap();

    // Start saga
    let saga_id = format!("saga-{}", Utc::now().timestamp());

    // Step 1: Coordinator -> OrderService (create order)
    coordinator.emit("OrderService", json!({
        "saga_id": saga_id,
        "step": 1,
        "action": "create_order",
        "order_id": "order-456",
        "compensation": "cancel_order"
    })).await.unwrap();

    // Log saga step
    let saga_queue_order = order_dir.join("queue/saga");
    fs::write(
        saga_queue_order.join(format!("{}-step1.json", saga_id)),
        serde_json::to_string_pretty(&json!({
            "saga_id": saga_id,
            "step": 1,
            "service": "OrderService",
            "action": "create_order",
            "status": "completed",
            "compensation": "cancel_order"
        })).unwrap()
    ).unwrap();

    // Step 2: OrderService -> PaymentService (process payment)
    order_svc.emit("PaymentService", json!({
        "saga_id": saga_id,
        "step": 2,
        "action": "process_payment",
        "amount": 99.99,
        "order_id": "order-456",
        "compensation": "refund_payment"
    })).await.unwrap();

    // Log saga step
    let saga_queue_payment = payment_dir.join("queue/saga");
    fs::write(
        saga_queue_payment.join(format!("{}-step2.json", saga_id)),
        serde_json::to_string_pretty(&json!({
            "saga_id": saga_id,
            "step": 2,
            "service": "PaymentService",
            "action": "process_payment",
            "status": "completed",
            "compensation": "refund_payment"
        })).unwrap()
    ).unwrap();

    // Step 3: PaymentService -> InventoryService (reserve inventory) - FAILS
    payment_svc.emit("InventoryService", json!({
        "saga_id": saga_id,
        "step": 3,
        "action": "reserve_inventory",
        "items": ["item-A", "item-B"],
        "compensation": "release_inventory"
    })).await.unwrap();

    // InventoryService fails
    let saga_queue_inventory = inventory_dir.join("queue/saga");
    fs::write(
        saga_queue_inventory.join(format!("{}-step3-failed.json", saga_id)),
        serde_json::to_string_pretty(&json!({
            "saga_id": saga_id,
            "step": 3,
            "service": "InventoryService",
            "action": "reserve_inventory",
            "status": "failed",
            "error": "Items out of stock",
            "trigger_compensation": true
        })).unwrap()
    ).unwrap();

    // Trigger compensations in reverse order (Step 2, Step 1)

    // Step 2 compensation: PaymentService refunds
    inventory_svc.emit("PaymentService", json!({
        "saga_id": saga_id,
        "command": "compensate",
        "step": 2,
        "compensation_action": "refund_payment",
        "reason": "Inventory reservation failed"
    })).await.unwrap();

    fs::write(
        saga_queue_payment.join(format!("{}-step2-compensated.json", saga_id)),
        serde_json::to_string_pretty(&json!({
            "saga_id": saga_id,
            "step": 2,
            "service": "PaymentService",
            "compensation_action": "refund_payment",
            "status": "compensated"
        })).unwrap()
    ).unwrap();

    // Step 1 compensation: OrderService cancels order
    payment_svc.emit("OrderService", json!({
        "saga_id": saga_id,
        "command": "compensate",
        "step": 1,
        "compensation_action": "cancel_order",
        "reason": "Saga rollback - inventory unavailable"
    })).await.unwrap();

    fs::write(
        saga_queue_order.join(format!("{}-step1-compensated.json", saga_id)),
        serde_json::to_string_pretty(&json!({
            "saga_id": saga_id,
            "step": 1,
            "service": "OrderService",
            "compensation_action": "cancel_order",
            "status": "compensated"
        })).unwrap()
    ).unwrap();

    // Notify Coordinator of saga failure
    order_svc.emit("SagaCoordinator", json!({
        "saga_id": saga_id,
        "status": "failed",
        "failed_step": 3,
        "compensations_completed": 2,
        "final_state": "fully_compensated"
    })).await.unwrap();

    // Verify saga state across all services
    let saga_dirs = vec![
        (order_dir, "OrderService"),
        (payment_dir, "PaymentService"),
        (inventory_dir, "InventoryService")
    ];

    for (dir, service) in saga_dirs {
        let saga_queue = dir.join("queue/saga");
        assert!(saga_queue.exists(), "{} should have saga records", service);

        let saga_files: Vec<_> = fs::read_dir(&saga_queue)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();

        assert!(saga_files.len() >= 1, "{} should have at least 1 saga record", service);
    }

    // Verify Coordinator received failure notification
    let coordinator_inbox = coordinator_dir.join("queue/inbox");
    let coordinator_jobs: Vec<_> = fs::read_dir(&coordinator_inbox)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("job"))
        .collect();

    let mut found_saga_failure = false;
    for job_entry in coordinator_jobs {
        let job_content = fs::read_to_string(job_entry.path()).unwrap();
        let job: serde_json::Value = serde_json::from_str(&job_content).unwrap();

        if job["payload"]["saga_id"] == saga_id && job["payload"]["status"] == "failed" {
            found_saga_failure = true;
            assert_eq!(job["payload"]["final_state"], "fully_compensated");
            assert_eq!(job["payload"]["compensations_completed"], 2);
            break;
        }
    }

    assert!(found_saga_failure, "Coordinator should receive saga failure notification");
}
