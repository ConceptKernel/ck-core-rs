//! Integration tests for multi-instance workflow
//!
//! Tests the complete workflow:
//! 1. Create/prepare a concept package
//! 2. Export to cache
//! 3. Load multiple instances from cache
//! 4. Verify instances work independently

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Helper to create a minimal test concept
fn create_test_concept(temp_dir: &TempDir, concept_name: &str) -> PathBuf {
    let concepts_dir = temp_dir.path().join("concepts");
    let concept_dir = concepts_dir.join(concept_name);

    fs::create_dir_all(&concept_dir).unwrap();

    // Create minimal conceptkernel.yaml
    let ontology_content = format!(
        r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v1.0
  type: node:cold
  version: v1.0

spec:
  queue_contract:
    edges: []
  storage_contract:
    strategy: file
"#,
        concept_name
    );

    fs::write(concept_dir.join("conceptkernel.yaml"), ontology_content).unwrap();

    // Create required directories
    fs::create_dir_all(concept_dir.join("queue/inbox")).unwrap();
    fs::create_dir_all(concept_dir.join("queue/staging")).unwrap();
    fs::create_dir_all(concept_dir.join("queue/ready")).unwrap();
    fs::create_dir_all(concept_dir.join("queue/archive")).unwrap();
    fs::create_dir_all(concept_dir.join("storage")).unwrap();
    fs::create_dir_all(concept_dir.join("logs")).unwrap();
    fs::create_dir_all(concept_dir.join("tool")).unwrap();

    // Create minimal tool.js
    let tool_content = r#"#!/usr/bin/env node
// Minimal test tool
console.log("Test concept processing job");
process.exit(0);
"#;

    fs::write(concept_dir.join("tool/tool.js"), tool_content).unwrap();

    // Create tx.jsonl
    fs::write(concept_dir.join("tx.jsonl"), "").unwrap();

    concept_dir
}

#[test]
fn test_complete_multi_instance_workflow() {
    use ckp_core::PackageManager;

    let temp = TempDir::new().unwrap();
    let concept_name = "TestWorker.MultiInstance";

    println!("\n=== Multi-Instance Workflow Test ===\n");

    // Step 1: Create a test concept
    println!("Step 1: Creating test concept '{}'", concept_name);
    let concept_dir = create_test_concept(&temp, concept_name);
    assert!(concept_dir.exists(), "Concept directory should exist");
    println!("  ✓ Concept created at: {}", concept_dir.display());

    // Step 2: Export to cache
    println!("\nStep 2: Exporting to cache");
    let pm = PackageManager::new().unwrap();
    let package_path = pm.export(concept_name, "v1.0", temp.path()).unwrap();
    assert!(package_path.exists(), "Package should exist in cache");
    println!("  ✓ Exported to: {}", package_path.display());
    println!("  ✓ Package size: {} bytes", fs::metadata(&package_path).unwrap().len());

    // Step 3: Clean up original concept (to test loading from cache)
    println!("\nStep 3: Removing original concept");
    fs::remove_dir_all(&concept_dir).unwrap();
    assert!(!concept_dir.exists(), "Original concept should be removed");
    println!("  ✓ Original concept removed");

    // Step 4: Load first instance (no suffix)
    println!("\nStep 4: Loading first instance (primary)");
    let instance1_name = pm.resolve_instance_name(concept_name, None, temp.path()).unwrap();
    assert_eq!(instance1_name, concept_name, "First instance should have no suffix");

    let instance1_dir = pm.install(concept_name, "v1.0", temp.path(), Some(&instance1_name)).unwrap();
    assert!(instance1_dir.exists(), "First instance should exist");
    println!("  ✓ Loaded: {} → {}", concept_name, instance1_dir.display());

    // Step 5: Load second instance (auto .1 suffix)
    println!("\nStep 5: Loading second instance (auto-numbered)");
    let instance2_name = pm.resolve_instance_name(concept_name, None, temp.path()).unwrap();
    assert_eq!(instance2_name, format!("{}.1", concept_name), "Second instance should have .1 suffix");

    let instance2_dir = pm.install(concept_name, "v1.0", temp.path(), Some(&instance2_name)).unwrap();
    assert!(instance2_dir.exists(), "Second instance should exist");
    println!("  ✓ Loaded: {} → {}", instance2_name, instance2_dir.display());

    // Step 6: Load third instance (custom name)
    println!("\nStep 6: Loading third instance (custom name)");
    let instance3_name = format!("{}.worker1", concept_name);
    let instance3_dir = pm.install(concept_name, "v1.0", temp.path(), Some(&instance3_name)).unwrap();
    assert!(instance3_dir.exists(), "Third instance should exist");
    println!("  ✓ Loaded: {} → {}", instance3_name, instance3_dir.display());

    // Step 7: Verify all instances are independent
    println!("\nStep 7: Verifying instance independence");

    // Check that each instance has its own directory
    let concepts_dir = temp.path().join("concepts");
    let mut entries: Vec<_> = fs::read_dir(&concepts_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    entries.sort();

    println!("  Loaded instances:");
    for entry in &entries {
        println!("    - {}", entry);
    }

    // Verify the exact instances we created exist
    assert!(instance1_dir.exists(), "Primary instance should exist at {}", instance1_dir.display());
    assert!(instance2_dir.exists(), "Auto-numbered instance should exist at {}", instance2_dir.display());
    assert!(instance3_dir.exists(), "Custom instance should exist at {}", instance3_dir.display());

    assert_eq!(entries.len(), 3, "Should have exactly 3 instances");
    assert!(entries.contains(&concept_name.to_string()), "Primary instance should exist");
    assert!(entries.contains(&format!("{}.1", concept_name)), "Auto-numbered instance should exist");
    assert!(entries.contains(&format!("{}.worker1", concept_name)), "Custom instance should exist");

    // Verify each instance has its own ontology
    for instance_name in &[concept_name, &format!("{}.1", concept_name), &format!("{}.worker1", concept_name)] {
        let ontology_path = concepts_dir.join(instance_name).join("conceptkernel.yaml");
        assert!(ontology_path.exists(), "Instance {} should have ontology", instance_name);

        let ontology_content = fs::read_to_string(&ontology_path).unwrap();
        assert!(ontology_content.contains("apiVersion: conceptkernel/v1"), "Valid ontology");
        println!("  ✓ Instance '{}' has valid ontology", instance_name);
    }

    // Step 8: Verify each instance has independent queue directories
    println!("\nStep 8: Verifying independent queue systems");
    for instance_name in &[concept_name, &format!("{}.1", concept_name), &format!("{}.worker1", concept_name)] {
        let inbox = concepts_dir.join(instance_name).join("queue/inbox");
        assert!(inbox.exists(), "Instance {} should have inbox", instance_name);
        println!("  ✓ Instance '{}' has independent queue system", instance_name);
    }

    // Step 9: Test that instances can receive independent jobs
    println!("\nStep 9: Testing independent job processing");
    for (i, instance_name) in [concept_name, &format!("{}.1", concept_name), &format!("{}.worker1", concept_name)].iter().enumerate() {
        let inbox = concepts_dir.join(instance_name).join("queue/inbox");
        let test_job = format!("test-job-{}.json", i);
        let job_path = inbox.join(&test_job);

        fs::write(&job_path, format!(r#"{{"test": "job", "instance": {}}}"#, i)).unwrap();
        assert!(job_path.exists(), "Test job should be created");
        println!("  ✓ Created test job in instance '{}'", instance_name);
    }

    // Step 10: Summary
    println!("\n=== Multi-Instance Workflow Summary ===");
    println!("  ✓ Created concept package: {}", concept_name);
    println!("  ✓ Exported to cache: {}@v1.0.tar.gz", concept_name);
    println!("  ✓ Loaded 3 independent instances:");
    println!("    1. {} (primary)", concept_name);
    println!("    2. {}.1 (auto-numbered)", concept_name);
    println!("    3. {}.worker1 (custom name)", concept_name);
    println!("  ✓ Verified instance independence");
    println!("  ✓ Tested independent job processing");
    println!("\n=== Test PASSED ===\n");
}

#[test]
fn test_auto_numbering_sequence() {
    use ckp_core::PackageManager;

    let temp = TempDir::new().unwrap();
    let concept_name = "AutoNumber.Test";

    // Create and export a test concept
    create_test_concept(&temp, concept_name);
    let pm = PackageManager::new().unwrap();
    pm.export(concept_name, "v1.0", temp.path()).unwrap();

    // Remove original
    let concept_dir = temp.path().join("concepts").join(concept_name);
    fs::remove_dir_all(&concept_dir).unwrap();

    // Load multiple instances and verify numbering
    let mut created_names = Vec::new();

    for i in 0..5 {
        let resolved = pm.resolve_instance_name(concept_name, None, temp.path()).unwrap();

        let expected = if i == 0 {
            concept_name.to_string()
        } else {
            format!("{}.{}", concept_name, i)
        };

        assert_eq!(resolved, expected, "Auto-numbering should be sequential (iteration {})", i);

        pm.install(concept_name, "v1.0", temp.path(), Some(&resolved)).unwrap();
        created_names.push(resolved.clone());
        println!("  ✓ Created instance: {}", resolved);
    }

    // Verify all instances exist
    let concepts_dir = temp.path().join("concepts");
    for created_name in &created_names {
        let instance_dir = concepts_dir.join(created_name);
        assert!(instance_dir.exists(), "Instance {} should exist", created_name);
    }

    println!("\n✓ Auto-numbering test passed: Created {} sequential instances", created_names.len());
}

#[test]
fn test_custom_name_override() {
    use ckp_core::PackageManager;

    let temp = TempDir::new().unwrap();
    let concept_name = "CustomName.Test";

    // Create and export a test concept
    create_test_concept(&temp, concept_name);
    let pm = PackageManager::new().unwrap();
    pm.export(concept_name, "v1.0", temp.path()).unwrap();

    // Remove original
    let concept_dir = temp.path().join("concepts").join(concept_name);
    fs::remove_dir_all(&concept_dir).unwrap();

    // Load with custom names
    let custom_names = vec![
        "MyCustomWorker.Primary",
        "MyCustomWorker.Secondary",
        "MyCustomWorker.Canary",
    ];

    for custom_name in &custom_names {
        let resolved = pm.resolve_instance_name(concept_name, Some(custom_name), temp.path()).unwrap();
        assert_eq!(&resolved, custom_name, "Custom name should be used directly");

        pm.install(concept_name, "v1.0", temp.path(), Some(&resolved)).unwrap();
        println!("  ✓ Created custom instance: {}", resolved);
    }

    // Verify all custom instances exist
    let concepts_dir = temp.path().join("concepts");
    for custom_name in &custom_names {
        let instance_dir = concepts_dir.join(custom_name);
        assert!(instance_dir.exists(), "Custom instance {} should exist", custom_name);
    }

    println!("\n✓ Custom name test passed: Created {} custom instances", custom_names.len());
}

#[test]
fn test_mixed_auto_and_custom_instances() {
    use ckp_core::PackageManager;

    let temp = TempDir::new().unwrap();
    let concept_name = "Mixed.Test";

    // Create and export a test concept
    create_test_concept(&temp, concept_name);
    let pm = PackageManager::new().unwrap();
    pm.export(concept_name, "v1.0", temp.path()).unwrap();

    // Remove original
    let concept_dir = temp.path().join("concepts").join(concept_name);
    fs::remove_dir_all(&concept_dir).unwrap();

    // Load mix of auto and custom
    let sequence = vec![
        (None, "auto"),           // Should get base name
        (Some("Custom.Worker1"), "custom"),  // Custom
        (None, "auto"),           // Should get .1
        (Some("Custom.Worker2"), "custom"),  // Custom
        (None, "auto"),           // Should get .2
    ];

    let mut created_instances = Vec::new();
    let mut auto_counter = 0;

    for (custom_name, label) in &sequence {
        let resolved = pm.resolve_instance_name(concept_name, custom_name.as_deref(), temp.path()).unwrap();

        // Verify the resolved name
        if custom_name.is_some() {
            assert_eq!(&resolved, custom_name.as_ref().unwrap(), "Custom name should be used");
        } else {
            let expected = if auto_counter == 0 {
                concept_name.to_string()
            } else {
                format!("{}.{}", concept_name, auto_counter)
            };
            assert_eq!(resolved, expected, "Auto name should be sequential");
            auto_counter += 1;
        }

        pm.install(concept_name, "v1.0", temp.path(), Some(&resolved)).unwrap();
        created_instances.push(resolved.clone());
        println!("  ✓ Created instance: {} ({})", resolved, label);
    }

    println!("\n✓ Mixed naming test passed: Created {} instances (auto + custom)", created_instances.len());
}

#[test]
fn test_max_instances_safety_limit() {
    use ckp_core::PackageManager;

    let temp = TempDir::new().unwrap();
    let concept_name = "SafetyLimit.Test";

    // Create and export a test concept
    create_test_concept(&temp, concept_name);
    let pm = PackageManager::new().unwrap();
    pm.export(concept_name, "v1.0", temp.path()).unwrap();

    // Manually create instances up to the limit
    let concepts_dir = temp.path().join("concepts");
    fs::remove_dir_all(&concepts_dir).unwrap();
    fs::create_dir_all(&concepts_dir).unwrap();

    // Create base name + .1 through .1000 (1001 total instances)
    fs::create_dir_all(concepts_dir.join(concept_name)).unwrap();
    for i in 1..=1000 {
        let instance_name = format!("{}.{}", concept_name, i);
        fs::create_dir_all(concepts_dir.join(&instance_name)).unwrap();
    }

    println!("  Created 1001 instances (base + .1 through .1000)");

    // Next attempt should hit safety limit (would be .1001, but counter > 1000)
    let result = pm.resolve_instance_name(concept_name, None, temp.path());
    assert!(result.is_err(), "Should hit safety limit after 1001 instances");
    println!("  ✓ Safety limit enforced at 1001 instances");
}
