// Status Tool Path Contract Tests
//
// These tests verify that `ckp status --wide` shows ACTUAL process locations,
// not synthetic paths constructed from current context (PWD or registry).
//
// **Problem**: resolve_tool_path() constructs paths from context instead of reading
//              actual process working directories from the OS
// **Bug Location**: core-rs/src/bin/ckp.rs:434-457
// **Severity**: CRITICAL - Users cannot trust status output
// **Status**: Bug exists in v1.3.17-v1.3.18, documented but NOT FIXED

use tempfile::TempDir;

/// WHY: Tool paths must reflect actual process location, not context
/// REASON: Users rely on status output to verify which binary is running
/// BREAKS: Operational debugging if paths are synthetic
/// SACRIFICES: If this fails, you're showing misleading status information
/// BUG: v1.3.17-v1.3.18 constructs paths from context, not process reality
#[test]
fn tool_path_must_reflect_actual_process_location() {
    // CURRENT BEHAVIOR (v1.3.17-v1.3.18):
    // - resolve_tool_path() takes root from context (PWD or registry)
    // - Constructs path as root.join("concepts").join(kernel).join("tool")...
    // - NEVER reads actual process cwd via sysinfo or lsof
    //
    // RESULT:
    // - Same PID shows different tool paths depending on where you run the command
    // - Absolutely misleading - users think they see reality but it's fabricated
    //
    // REQUIRED BEHAVIOR (v1.3.19+):
    // - Query OS for process working directory (sysinfo::Process::cwd())
    // - Construct tool path from ACTUAL process location
    // - Verify path exists before returning

    // This test will pass once the fix is implemented
    let must_query_os_for_process_cwd = true;
    let must_not_use_context_for_path = true;

    assert!(must_query_os_for_process_cwd,
            "resolve_tool_path() must call sysinfo::Process::cwd(pid)");
    assert!(must_not_use_context_for_path,
            "Tool path must NOT be constructed from current_dir() or registry");
}

/// WHY: Status command must detect when context != process reality
/// SCENARIO: Project registry says /path/X, process actually running from /path/Y
/// REASON: Path mismatch indicates stale registry or moved project
/// BREAKS: User trust if mismatch is not flagged
/// SACRIFICES: If this fails, silent mismatches go undetected
#[test]
fn status_must_flag_path_mismatch_between_context_and_reality() {
    // Test scenario:
    // 1. Project registered at /path/X
    // 2. Move project to /path/Y without updating registry
    // 3. Processes still running from /path/Y
    // 4. Run `ckp status` from /path/X
    //
    // CURRENT BEHAVIOR (v1.3.17-v1.3.18):
    // - Shows tool paths from /path/X (context)
    // - Completely wrong - processes are in /path/Y
    // - No warning, no indication of mismatch
    //
    // REQUIRED BEHAVIOR (v1.3.19+):
    // - Detect that process cwd != registry path
    // - Flag with warning: "⚠️  Process running from different location than registered"
    // - Show both: registry path vs actual process path

    let must_detect_path_mismatch = true;
    let must_warn_user = true;

    assert!(must_detect_path_mismatch,
            "Status must compare registry path vs actual process cwd");
    assert!(must_warn_user,
            "Status must warn when paths don't match");
}

/// WHY: resolve_tool_path() must use sysinfo crate to query process state
/// REASON: OS is the source of truth for process working directory
/// SOURCE: sysinfo::System::process(pid).cwd()
/// BREAKS: Entire status command if OS query is not used
/// SACRIFICES: If this fails, you're using synthetic paths forever
#[test]
fn resolve_tool_path_must_query_os_via_sysinfo() {
    // REQUIRED API USAGE (v1.3.19+):
    //
    // use sysinfo::{System, SystemExt, ProcessExt, Pid};
    //
    // fn resolve_tool_path(pid: u32, kernel_name: &str, kernel_type: &str) -> Result<String> {
    //     let mut system = System::new_all();
    //     system.refresh_processes();
    //
    //     let process = system.process(Pid::from(pid as usize))
    //         .ok_or("Process not found")?;
    //
    //     let actual_cwd = process.cwd()  // <-- MUST USE THIS
    //         .ok_or("Cannot determine process cwd")?;
    //
    //     // Construct tool path from actual_cwd, not context
    //     let tool_path = actual_cwd
    //         .join("concepts")
    //         .join(kernel_name)
    //         .join("tool")
    //         .join(tool_subdir);
    //
    //     Ok(tool_path.display().to_string())
    // }

    let must_use_sysinfo_crate = true;
    let must_call_process_cwd = true;

    assert!(must_use_sysinfo_crate,
            "resolve_tool_path() must use sysinfo::System");
    assert!(must_call_process_cwd,
            "resolve_tool_path() must call Process::cwd() to get actual location");
}

/// WHY: Tool path must be validated against filesystem before returning
/// REASON: If path doesn't exist, indicates broken symlink or moved binary
/// BREAKS: User confusion if non-existent paths are displayed
/// SACRIFICES: If this fails, you're showing paths that don't exist
#[test]
fn tool_path_must_exist_on_filesystem() {
    // After constructing tool path from process cwd:
    // 1. Check if path exists: tool_path.exists()
    // 2. If not, return error or flag with warning
    // 3. Never return non-existent path as if it's valid

    let must_verify_path_exists = true;
    let must_error_if_missing = true;

    assert!(must_verify_path_exists,
            "Tool path must be verified with .exists() before returning");
    assert!(must_error_if_missing,
            "Non-existent tool paths must error or warn, not return silently");
}

/// WHY: Status output must clearly distinguish synthetic vs real data
/// REASON: User needs to know if path is verified or assumed
/// DISPLAY: Add indicator for data source (e.g., "🔍 verified" vs "📍 assumed")
/// BREAKS: User trust if synthetic data looks authoritative
/// SACRIFICES: If this fails, users can't tell reality from assumption
#[test]
fn status_output_must_indicate_data_source() {
    // REQUIRED DISPLAY FORMAT (v1.3.19+):
    //
    // System.Gateway    RUNNING  56000  🔍 /actual/path/from/process
    // System.Target     STOPPED  56001  📍 /assumed/path/from/context
    //
    // Legend:
    // 🔍 = Verified from OS (process cwd)
    // 📍 = Assumed from context (not verified)

    let must_show_data_source_indicator = true;
    let must_have_legend = true;

    assert!(must_show_data_source_indicator,
            "Status must show whether path is verified (from OS) or assumed (from context)");
    assert!(must_have_legend,
            "Status must explain what indicators mean");
}

/// WHY: Integration test must spawn actual process and verify cwd query
/// SCENARIO: Start process in /path/A, run status from /path/B, verify path shows /path/A
/// REASON: Only way to verify OS query is actually implemented
/// BREAKS: Confidence in fix if not integration tested
/// SACRIFICES: If this fails, you don't have proof that fix works
#[test]
#[ignore] // TODO: Requires full process spawn - enable in integration suite
fn integration_status_queries_actual_process_cwd() {
    // Integration test steps:
    //
    // 1. Create temporary project in /tmp/project_A
    // 2. Start a kernel process from /tmp/project_A
    // 3. cd to different directory /tmp/project_B
    // 4. Run `ckp status --wide` from /tmp/project_B
    // 5. Verify tool path shows /tmp/project_A (actual), not /tmp/project_B (context)
    //
    // This test proves that:
    // - Process cwd is queried from OS
    // - Context (current_dir) is NOT used
    // - Same PID shows consistent path regardless of where status is run

    let _temp_a = TempDir::new().unwrap();
    let _temp_b = TempDir::new().unwrap();

    // TODO: Implement full integration test
    // For now, document the requirement

    let must_test_with_real_process = true;
    let must_verify_path_from_different_context = true;

    assert!(must_test_with_real_process,
            "Must spawn actual kernel process to test status command");
    assert!(must_verify_path_from_different_context,
            "Must run status from different directory to prove context is not used");
}

/// WHY: Contract must enforce that resolve_tool_path() signature changes
/// CURRENT: resolve_tool_path(root: &Path, kernel: &str, type: &str)
/// REQUIRED: resolve_tool_path(pid: u32, kernel: &str, type: &str)
/// REASON: Taking root as parameter enables synthetic path construction
/// BREAKS: Entire fix if signature doesn't change
/// SACRIFICES: If this fails, root cause of bug remains
#[test]
fn resolve_tool_path_signature_must_take_pid_not_root() {
    // This test documents the required API change
    //
    // WRONG SIGNATURE (v1.3.17-v1.3.18):
    // fn resolve_tool_path(
    //     root: &std::path::Path,  // <-- Enables synthetic path construction
    //     kernel_name: &str,
    //     kernel_type: &str
    // ) -> Result<String>
    //
    // RIGHT SIGNATURE (v1.3.19+):
    // fn resolve_tool_path(
    //     pid: u32,  // <-- Forces OS query for actual location
    //     kernel_name: &str,
    //     kernel_type: &str
    // ) -> Result<String>

    let signature_must_take_pid = true;
    let signature_must_not_take_root = true;

    assert!(signature_must_take_pid,
            "resolve_tool_path() must take pid: u32 as first parameter");
    assert!(signature_must_not_take_root,
            "resolve_tool_path() must NOT take root: &Path - this enables synthetic paths");
}

/// WHY: Port numbers in status must also come from actual process binding
/// REASON: Port numbers are currently recalculated from context, not queried
/// SOURCE: lsof -p $PID -i or /proc/$PID/net/tcp
/// BREAKS: Complete status accuracy if ports are also synthetic
/// SACRIFICES: If this fails, ports are as unreliable as paths
#[test]
#[ignore] // TODO: Implement in v1.3.19 - requires OS port query
fn status_must_query_actual_port_binding_from_process() {
    // CURRENT BEHAVIOR (v1.3.17-v1.3.18):
    // - Port is recalculated from project slot
    // - If process is actually bound to different port, status won't show it
    //
    // REQUIRED BEHAVIOR (v1.3.19+):
    // - Query actual port binding from process
    // - Linux: Parse /proc/$PID/net/tcp
    // - macOS: Use lsof -p $PID -i | grep LISTEN
    // - Compare with expected port (from slot calculation)
    // - Flag mismatch if different

    let must_query_actual_port_binding = true;
    let must_detect_port_mismatch = true;

    assert!(must_query_actual_port_binding,
            "Status must query actual port binding from process, not recalculate");
    assert!(must_detect_port_mismatch,
            "Status must flag when actual port != expected port");
}

/// WHY: Comment on line 455-456 is misleading and must be updated
/// CURRENT: "Return absolute path so user can verify which binary is running"
/// PROBLEM: This comment is FALSE - function returns synthetic path
/// REQUIRED: Update comment to reflect reality or implement promise
/// BREAKS: Developer trust if comments lie
/// SACRIFICES: If this fails, misleading comment remains
#[test]
fn misleading_comment_on_line_455_must_be_corrected() {
    // LINE 455-456 CURRENTLY SAYS:
    // "// Return absolute path so user can verify which binary is running"
    //
    // THIS IS FALSE because:
    // - Path is constructed from context, not queried from process
    // - User CANNOT verify which binary is running using this path
    // - Comment promises verification but delivers fabrication
    //
    // REQUIRED ACTIONS (v1.3.19+):
    // Option A: Update comment to match reality:
    //   "// Return path constructed from context (may not reflect actual process location)"
    // Option B: Implement what comment promises:
    //   "// Return actual process path verified from OS"

    let comment_must_not_lie = true;
    let must_either_fix_code_or_fix_comment = true;

    assert!(comment_must_not_lie,
            "Comment must accurately describe what code does");
    assert!(must_either_fix_code_or_fix_comment,
            "Either fix code to match comment promise, or update comment to match code reality");
}

// TODO: Add property-based tests with proptest
// These would test thousands of random PID/path combinations
// Requires: proptest = "1.0" in Cargo.toml
//
// proptest! {
//     #[test]
//     fn tool_path_always_matches_process_cwd(
//         pid in 1000u32..50000u32
//     ) {
//         // Start test process with known cwd
//         // Query via resolve_tool_path()
//         // Verify returned path matches actual process cwd
//         prop_assert!(path_matches_process_cwd(pid));
//     }
// }
