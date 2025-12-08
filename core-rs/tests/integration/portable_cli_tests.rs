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
    let ckp_path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/debug/ckp");
    assert!(
        std::path::Path::new(ckp_path).exists(),
        "ckp binary not found at {}. Run 'cargo build --bin ckp' first.",
        ckp_path
    );
}

#[test]
fn test_cli_version_output() {
    let ckp_path = concat!(env!("CARGO_MANIFEST_DIR"), "/target/debug/ckp");

    let output = Command::new(ckp_path)
        .arg("--version")
        .output()
        .expect("Failed to run ckp --version");

    assert!(output.status.success(), "ckp --version failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ckp") && stdout.contains("1.3."),
        "Version output should contain 'ckp' and version number"
    );
}
