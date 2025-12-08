//! Kernel Lifecycle Integration Tests
//!
//! Tests full kernel lifecycle: start, stop, status, PID tracking, cleanup
//! Converted from test-kernel-lifecycle-complete.sh
//!
//! **Pattern 3: Direct API Testing**
//! Uses KernelManager API directly instead of CLI commands for:
//! - 2-3x faster execution
//! - Direct state inspection
//! - Better error messages
//! - No CLI binary dependency
//!
//! Uses tempfile for isolation - NO /tmp pollution

use ckp_core::KernelManager;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Helper to create test project structure
fn create_test_project() -> (TempDir, PathBuf) {
    let temp = TempDir::new().unwrap();
    let concepts_dir = temp.path().join("concepts");
    fs::create_dir_all(&concepts_dir).unwrap();

    // Create .ckproject file
    let project_config = format!(
        r#"apiVersion: conceptkernel/v1
kind: Project
metadata:
  name: test-lifecycle-{}
  id: test-{}
spec:
  domain: Test.Lifecycle
  version: 1.3.16
"#,
        uuid::Uuid::new_v4().to_string().split('-').next().unwrap(),
        uuid::Uuid::new_v4()
    );

    fs::write(temp.path().join(".ckproject"), project_config).unwrap();

    (temp, concepts_dir)
}

/// Helper to create cold kernel ontology
fn create_cold_kernel(concepts_dir: &Path, name: &str) {
    let kernel_dir = concepts_dir.join(name);
    fs::create_dir_all(kernel_dir.join("tool")).unwrap();

    let ontology = format!(
        r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: {}
  type: rust:cold
  entrypoint: tool/test-cold
  version: v0.1
"#,
        name
    );

    fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();

    // Create dummy tool script
    let tool_script = r#"#!/bin/bash
echo "Cold tool running"
sleep 1
"#;
    let tool_path = kernel_dir.join("tool/test-cold");
    fs::write(&tool_path, tool_script).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&tool_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tool_path, perms).unwrap();
    }
}

/// Helper to create hot kernel ontology
fn create_hot_kernel(concepts_dir: &Path, name: &str, port: u16) {
    let kernel_dir = concepts_dir.join(name);
    fs::create_dir_all(kernel_dir.join("tool")).unwrap();

    let ontology = format!(
        r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: {}
  type: rust:hot
  entrypoint: tool/test-hot
  port: {}
  version: v0.1
"#,
        name, port
    );

    fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();

    // Create dummy long-running tool
    let tool_script = r#"#!/bin/bash
echo "Hot tool starting on port $CK_PORT"
while true; do
    sleep 1
done
"#;
    let tool_path = kernel_dir.join("tool/test-hot");
    fs::write(&tool_path, tool_script).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&tool_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tool_path, perms).unwrap();
    }
}

#[tokio::test]
async fn test_cold_kernel_creates_watcher_pid() {
    let (temp, concepts_dir) = create_test_project();

    // Create cold kernel
    create_cold_kernel(&concepts_dir, "Test.Cold");

    // Start kernel using KernelManager API (Pattern 3)
    let manager = KernelManager::new(temp.path().to_path_buf()).unwrap();
    let options = HashMap::new();
    let result = manager.start_kernel("Test.Cold", &options).await.unwrap();

    // Verify start result
    assert!(result.watcher_pid.is_some(), "Cold kernel should have watcher PID");
    assert_eq!(result.kernel_type, "rust:cold");

    // Check for .watcher.pid
    let watcher_pid_file = concepts_dir.join("Test.Cold/.watcher.pid");
    assert!(
        watcher_pid_file.exists(),
        "Cold kernel should create .watcher.pid file"
    );

    // Validate PID file format: PID:START_TIME
    let content = fs::read_to_string(&watcher_pid_file).unwrap();
    assert!(
        content.contains(':'),
        "PID file should have format PID:START_TIME, got: {}",
        content
    );

    let parts: Vec<&str> = content.trim().split(':').collect();
    assert_eq!(parts.len(), 2, "PID file should have exactly 2 parts");

    let pid: u32 = parts[0]
        .parse()
        .expect("First part should be numeric PID");
    let _start_time: u64 = parts[1]
        .parse()
        .expect("Second part should be numeric timestamp");

    // Verify process is running
    #[cfg(unix)]
    {
        use sysinfo::{Pid, System, ProcessesToUpdate};
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All);
        let pid_obj = Pid::from_u32(pid);
        assert!(
            sys.process(pid_obj).is_some(),
            "Watcher process should be running"
        );
    }

    // Verify NO .tool.pid for cold kernel
    let tool_pid_file = concepts_dir.join("Test.Cold/.tool.pid");
    assert!(
        !tool_pid_file.exists(),
        "Cold kernel should NOT have .tool.pid when idle"
    );
}

#[tokio::test]
#[ignore] // Ignore until hot kernel implementation is complete
async fn test_hot_kernel_creates_both_pids() {
    let (temp, concepts_dir) = create_test_project();

    // Create hot kernel
    create_hot_kernel(&concepts_dir, "Test.Hot", 58000);

    // Start kernel using KernelManager API (Pattern 3)
    let manager = KernelManager::new(temp.path().to_path_buf()).unwrap();
    let options = HashMap::new();
    let result = manager.start_kernel("Test.Hot", &options).await.unwrap();

    // Verify start result
    assert!(result.watcher_pid.is_some(), "Hot kernel should have watcher PID");
    assert!(result.pid.is_some(), "Hot kernel should have tool PID");
    assert_eq!(result.kernel_type, "rust:hot");

    // Check watcher PID
    let watcher_pid_file = concepts_dir.join("Test.Hot/.watcher.pid");
    assert!(
        watcher_pid_file.exists(),
        "Hot kernel should have .watcher.pid"
    );

    // Check tool PID (hot kernels run immediately)
    let tool_pid_file = concepts_dir.join("Test.Hot/.tool.pid");
    assert!(
        tool_pid_file.exists(),
        "Hot kernel should have .tool.pid for running service"
    );

    // Both PIDs should be valid format
    let watcher_content = fs::read_to_string(&watcher_pid_file).unwrap();
    assert!(watcher_content.contains(':'));

    let tool_content = fs::read_to_string(&tool_pid_file).unwrap();
    assert!(tool_content.contains(':'));

    // PIDs should be different
    let watcher_pid: u32 = watcher_content.split(':').next().unwrap().parse().unwrap();
    let tool_pid: u32 = tool_content.split(':').next().unwrap().parse().unwrap();
    assert_ne!(watcher_pid, tool_pid, "Watcher and tool PIDs must be different");
}

#[tokio::test]
async fn test_kernel_stop_removes_pid_files() {
    let (temp, concepts_dir) = create_test_project();

    create_cold_kernel(&concepts_dir, "Test.StopTest");

    // Start kernel using KernelManager API (Pattern 3)
    let manager = KernelManager::new(temp.path().to_path_buf()).unwrap();
    let options = HashMap::new();
    manager.start_kernel("Test.StopTest", &options).await.unwrap();

    // Verify PID file exists
    let watcher_pid_file = concepts_dir.join("Test.StopTest/.watcher.pid");
    assert!(watcher_pid_file.exists(), "PID file should exist after start");

    // Stop kernel using KernelManager API (Pattern 3)
    let stopped = manager.stop_kernel("Test.StopTest").await.unwrap();
    assert!(stopped, "Kernel should have been stopped");

    // PID file should be removed
    assert!(
        !watcher_pid_file.exists(),
        "PID file should be removed after stop"
    );
}

#[tokio::test]
async fn test_kernel_status_shows_correct_state() {
    let (temp, concepts_dir) = create_test_project();

    create_cold_kernel(&concepts_dir, "Test.Status");

    // Use KernelManager API (Pattern 3)
    let manager = KernelManager::new(temp.path().to_path_buf()).unwrap();

    // Initially, kernel doesn't exist in status (not started)
    // Note: get_kernel_status returns error if kernel is not running
    let _status_result = manager.get_kernel_status("Test.Status").await;

    // Start kernel
    let options = HashMap::new();
    manager.start_kernel("Test.Status", &options).await.unwrap();

    // Should show as running - direct state inspection
    let status = manager.get_kernel_status("Test.Status").await.unwrap();
    assert_eq!(status.name, "Test.Status", "Status should show kernel name");
    assert_eq!(status.kernel_type, "rust:cold", "Status should show kernel type");
    assert!(status.watcher_pid.is_some(), "Status should show watcher PID");
    assert_eq!(status.mode, "IDLE", "Cold kernel should be in IDLE mode when no jobs");
}

#[tokio::test]
async fn test_pid_file_survives_watcher_restart() {
    let (temp, concepts_dir) = create_test_project();

    create_cold_kernel(&concepts_dir, "Test.Persist");

    // Use KernelManager API (Pattern 3)
    let manager = KernelManager::new(temp.path().to_path_buf()).unwrap();

    // Start kernel
    let options = HashMap::new();
    manager.start_kernel("Test.Persist", &options).await.unwrap();

    // Read original PID file
    let watcher_pid_file = concepts_dir.join("Test.Persist/.watcher.pid");
    let original_content = fs::read_to_string(&watcher_pid_file).unwrap();

    // Stop and restart
    manager.stop_kernel("Test.Persist").await.unwrap();
    manager.start_kernel("Test.Persist", &options).await.unwrap();

    // New PID file should exist with different content
    assert!(watcher_pid_file.exists(), "PID file should exist after restart");
    let new_content = fs::read_to_string(&watcher_pid_file).unwrap();
    assert_ne!(
        original_content, new_content,
        "Restarted kernel should have new PID"
    );
}

#[tokio::test]
async fn test_multiple_kernels_isolated() {
    let (temp, concepts_dir) = create_test_project();

    // Create multiple kernels
    create_cold_kernel(&concepts_dir, "Test.Alpha");
    create_cold_kernel(&concepts_dir, "Test.Beta");
    create_cold_kernel(&concepts_dir, "Test.Gamma");

    // Use KernelManager API (Pattern 3)
    let manager = KernelManager::new(temp.path().to_path_buf()).unwrap();
    let options = HashMap::new();

    // Start all - no need for sleep, API is synchronous
    manager.start_kernel("Test.Alpha", &options).await.unwrap();
    manager.start_kernel("Test.Beta", &options).await.unwrap();
    manager.start_kernel("Test.Gamma", &options).await.unwrap();

    // Each should have its own PID file
    assert!(concepts_dir.join("Test.Alpha/.watcher.pid").exists());
    assert!(concepts_dir.join("Test.Beta/.watcher.pid").exists());
    assert!(concepts_dir.join("Test.Gamma/.watcher.pid").exists());

    // All PIDs should be different
    let pid_a = fs::read_to_string(concepts_dir.join("Test.Alpha/.watcher.pid"))
        .unwrap()
        .split(':')
        .next()
        .unwrap()
        .to_string();

    let pid_b = fs::read_to_string(concepts_dir.join("Test.Beta/.watcher.pid"))
        .unwrap()
        .split(':')
        .next()
        .unwrap()
        .to_string();

    let pid_c = fs::read_to_string(concepts_dir.join("Test.Gamma/.watcher.pid"))
        .unwrap()
        .split(':')
        .next()
        .unwrap()
        .to_string();

    assert_ne!(pid_a, pid_b, "Alpha and Beta PIDs should be different");
    assert_ne!(pid_b, pid_c, "Beta and Gamma PIDs should be different");
    assert_ne!(pid_a, pid_c, "Alpha and Gamma PIDs should be different");

    // Verify all kernels show correct status
    let status_a = manager.get_kernel_status("Test.Alpha").await.unwrap();
    let status_b = manager.get_kernel_status("Test.Beta").await.unwrap();
    let status_c = manager.get_kernel_status("Test.Gamma").await.unwrap();

    assert_eq!(status_a.name, "Test.Alpha");
    assert_eq!(status_b.name, "Test.Beta");
    assert_eq!(status_c.name, "Test.Gamma");

    assert_eq!(status_a.mode, "IDLE");
    assert_eq!(status_b.mode, "IDLE");
    assert_eq!(status_c.mode, "IDLE");
}
