//! Integration tests for complete project lifecycle
//!
//! Tests the full lifecycle of project operations including:
//! - Registration
//! - Discovery
//! - Port allocation
//! - Removal

use ckp_core::{ProjectConfig, ProjectRegistry, ProjectInfo, PortManager};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_complete_project_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().join("test-project");
    fs::create_dir_all(&project_path).unwrap();

    // 1. Create .ckproject file
    let config = ProjectConfig::new(
        "lifecycle-test".to_string(),
        "proj-lifecycle-001".to_string(),
        "Test.Lifecycle".to_string(),
        "1.3.14".to_string(),
    );

    let ckproject_path = project_path.join(".ckproject");
    config.save(&ckproject_path).unwrap();

    // 2. Create registry in temp location
    let registry_dir = temp_dir.path().join(".ckregistry");
    fs::create_dir_all(&registry_dir).unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let mut registry = ProjectRegistry::new().unwrap();

    // 3. Register project
    let project_info = ProjectInfo {
        name: "lifecycle-test".to_string(),
        id: "proj-lifecycle-001".to_string(),
        path: project_path.to_string_lossy().to_string(),
        version: "1.3.14".to_string(),
        preferred_slot: None,
    };

    let registered = registry.register(project_info).unwrap();

    // 4. Verify registration
    assert_eq!(registered.name, "lifecycle-test");
    assert!(registered.slot >= 1);
    assert_eq!(
        registered.discovery_port,
        56000 + (registered.slot - 1) as u16 * 200
    );
    assert_eq!(registered.port_range.start, registered.discovery_port);
    assert_eq!(registered.port_range.end, registered.discovery_port + 199);

    // 5. Verify port manager integration
    let mut port_manager = PortManager::new(&project_path).unwrap();
    port_manager.set_base_port(registered.discovery_port).unwrap();

    let base_port = port_manager.get_base_port();
    assert_eq!(base_port, Some(registered.discovery_port));

    // 6. Test project discovery
    let found = registry.get("lifecycle-test").unwrap();
    assert!(found.is_some());
    let found_project = found.unwrap();
    assert_eq!(found_project.name, "lifecycle-test");
    assert_eq!(found_project.slot, registered.slot);

    // 7. List all projects
    let projects = registry.list().unwrap();
    assert!(!projects.is_empty());
    assert!(projects.iter().any(|p| p.name == "lifecycle-test"));

    // 8. Remove project
    registry.remove("lifecycle-test").unwrap();

    // 9. Verify removal
    let not_found = registry.get("lifecycle-test").unwrap();
    assert!(not_found.is_none());
}

#[test]
fn test_port_isolation_between_projects() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let mut registry = ProjectRegistry::new().unwrap();

    // Register multiple projects
    let projects: Vec<_> = (1..=5)
        .map(|i| {
            let project_path = temp_dir.path().join(format!("project{}", i));
            fs::create_dir_all(&project_path).unwrap();

            let info = ProjectInfo {
                name: format!("test-project-{}", i),
                id: format!("proj-{}", i),
                path: project_path.to_string_lossy().to_string(),
                version: "1.3.14".to_string(),
                preferred_slot: None,
            };

            registry.register(info).unwrap()
        })
        .collect();

    // Verify port isolation (200 ports apart)
    for i in 0..projects.len() - 1 {
        let port1 = projects[i].discovery_port;
        let port2 = projects[i + 1].discovery_port;

        // Consecutive projects should be 200 ports apart (unless port was unavailable)
        let diff = (port2 as i32 - port1 as i32).abs();
        assert!(
            diff >= 200,
            "Projects should be at least 200 ports apart, got {} (ports: {}, {})",
            diff,
            port1,
            port2
        );

        // Ranges should not overlap
        assert!(
            projects[i].port_range.end < projects[i + 1].port_range.start
                || projects[i + 1].port_range.end < projects[i].port_range.start,
            "Port ranges should not overlap"
        );
    }

    // Clean up
    for project in projects {
        registry.remove(&project.name).unwrap();
    }
}

#[test]
fn test_project_config_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let ckproject_path = temp_dir.path().join(".ckproject");

    // Create and save config
    let config = ProjectConfig::new(
        "persist-test".to_string(),
        "proj-persist-001".to_string(),
        "Test.Persist".to_string(),
        "1.3.14".to_string(),
    );
    config.save(&ckproject_path).unwrap();

    // Load config back
    let loaded = ProjectConfig::load(&ckproject_path).unwrap();

    // Verify fields
    assert_eq!(loaded.metadata.name, "persist-test");
    assert_eq!(loaded.metadata.id, "proj-persist-001");
    assert_eq!(loaded.spec.domain, "Test.Persist");
    assert_eq!(loaded.spec.version, "1.3.14");
    assert_eq!(loaded.api_version, "conceptkernel/v1");
    assert_eq!(loaded.kind, "Project");
}

#[test]
fn test_duplicate_registration_prevention() {
    let temp_dir = TempDir::new().unwrap();
    std::env::set_var("HOME", temp_dir.path());

    let mut registry = ProjectRegistry::new().unwrap();

    let project_path = temp_dir.path().join("dup-test");
    fs::create_dir_all(&project_path).unwrap();

    let info = ProjectInfo {
        name: "duplicate-test".to_string(),
        id: "proj-dup-001".to_string(),
        path: project_path.to_string_lossy().to_string(),
        version: "1.3.14".to_string(),
        preferred_slot: None,
    };

    // First registration should succeed
    registry.register(info.clone()).unwrap();

    // Second registration should fail
    let result = registry.register(info);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already registered"));

    // Clean up
    registry.remove("duplicate-test").unwrap();
}
