//! Integration tests for portable CLI functionality
//!
//! These tests run the portable shell script to verify cross-runtime compatibility
//! and end-to-end CLI functionality.

use std::process::Command;

#[test]
fn test_portable_cli_suite() {
    // Get the path to the test script
    let script_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/test-projects-portable.sh");

    // Run the portable test script
    let output = Command::new("bash")
        .arg(script_path)
        .output()
        .expect("Failed to run portable test script");

    // Print output for debugging
    if !output.status.success() {
        eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
    }

    // Assert the script succeeded
    assert!(
        output.status.success(),
        "Portable CLI test suite failed. Check output above for details."
    );

    // Verify we got the success message
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("All language-agnostic tests passed"),
        "Expected success message not found in output"
    );
    assert!(
        stdout.contains("Certified: v1.3.14 Multi-Project Infrastructure"),
        "Certification message not found"
    );
}

#[test]
fn test_cli_binary_exists() {
    let ckr_path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/debug/ckr");
    assert!(
        std::path::Path::new(ckr_path).exists(),
        "ckr binary not found at {}. Run 'cargo build --bin ckr' first.",
        ckr_path
    );
}

#[test]
fn test_cli_version_output() {
    let ckr_path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/debug/ckr");

    let output = Command::new(ckr_path)
        .arg("--version")
        .output()
        .expect("Failed to run ckr --version");

    assert!(output.status.success(), "ckr --version failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ckr") && stdout.contains("1.3."),
        "Version output should contain 'ckr' and version number"
    );
}
