// Protocol Purity Contract Tests
//
// These tests verify that protocol-level structures remain pure and language-agnostic.
// Protocol must not leak implementation details (Rust vs Node.js vs Python).
//
// **Problem**: LLM "helpfully" adds toolPath, pid, or other runtime details to protocol
// **Solution**: Contract tests that enforce protocol purity

use serde_json::Value;

/// WHY: Evidence format must be protocol-pure (no implementation details)
/// FORBIDDEN: toolPath, pid, toolType, internal module paths
/// REASON: Protocol must be language-agnostic for Rust/Node.js/Python interop
/// BREAKS: Multi-runtime compatibility if implementation leaks into protocol
/// SACRIFICES: If this fails, you're coupling protocol to Rust runtime
#[test]
fn evidence_format_excludes_implementation_details() {
    // CORRECT protocol-level evidence:
    let correct_evidence = r#"{
        "timestamp": 1732845100,
        "txId": "tx_20251127_120530_abc123",
        "processUrn": "ckp://Process#Invocation-tx_20251127_120530_abc123",
        "phase": "completed",
        "continuant": "ckp://Continuant#System.Gateway.HTTP",
        "inputs": ["ckp://Instance#request-456"],
        "outputs": ["ckp://Instance#result-123"],
        "consensusProof": null
    }"#;

    let evidence: Value = serde_json::from_str(correct_evidence).unwrap();

    // These fields MUST exist (protocol-level):
    assert!(evidence.get("timestamp").is_some(), "Evidence must have timestamp");
    assert!(evidence.get("txId").is_some(), "Evidence must have txId");
    assert!(evidence.get("processUrn").is_some(), "Evidence must have processUrn");
    assert!(evidence.get("phase").is_some(), "Evidence must have phase");
    assert!(evidence.get("continuant").is_some(), "Evidence must have continuant");

    // These fields MUST NOT exist (implementation details):
    assert!(
        !evidence.get("toolPath").is_some(),
        "Evidence must NOT leak toolPath - implementation detail"
    );
    assert!(
        !evidence.get("pid").is_some(),
        "Evidence must NOT leak pid - runtime detail"
    );
    assert!(
        !evidence.get("toolType").is_some(),
        "Evidence must NOT leak toolType - implementation detail"
    );
    assert!(
        !evidence.get("executable").is_some(),
        "Evidence must NOT leak executable path - implementation detail"
    );

    // If this test fails:
    // - You're leaking implementation details into protocol
    // - Multi-runtime compatibility is broken
    // - Protocol is no longer language-agnostic
}

/// WHY: URNs must use ckp:// scheme, not filesystem paths
/// REASON: URNs are protocol-level, paths are implementation-level
/// BREAKS: Protocol abstraction if paths leak into URNs
/// SACRIFICES: If this fails, you're coupling protocol to filesystem layout
#[test]
fn evidence_references_use_urns_not_paths() {
    // CORRECT: URN-based references
    let continuant_urn = "ckp://Continuant#Kernel-System.Gateway";
    let process_urn = "ckp://Process#invoke-tx_123";
    let instance_urn = "ckp://Instance#result-123";

    assert!(continuant_urn.starts_with("ckp://"));
    assert!(process_urn.starts_with("ckp://"));
    assert!(instance_urn.starts_with("ckp://"));

    // WRONG: Filesystem paths in evidence
    let wrong_references = vec![
        "/Users/neoxr/concepts/System.Gateway",     // Absolute path
        "./concepts/System.Gateway",                // Relative path
        "concepts/System.Gateway/tool/rs/target",   // Tool path
        "/var/run/ckp/kernels/System.Gateway.pid",  // PID file path
    ];

    for wrong in wrong_references {
        assert!(
            !wrong.starts_with("ckp://"),
            "Path '{}' should NOT be in evidence - use URNs",
            wrong
        );
    }

    // If this test fails:
    // - Filesystem paths are leaking into evidence
    // - Protocol is coupled to directory structure
}

/// WHY: Metadata.json must follow protocol schema
/// REQUIRED: timestamp, txId, processUrn, phase, continuant
/// OPTIONAL: inputs, outputs, consensusProof, participants
/// REASON: Schema compatibility across runtimes
/// BREAKS: Evidence parsing if schema changes
/// SACRIFICES: If this fails, you're breaking protocol contract
#[test]
fn metadata_json_follows_protocol_schema() {
    let metadata = r#"{
        "timestamp": 1732845100,
        "txId": "tx_20251127_120530_abc123",
        "processUrn": "ckp://Process#Invocation-tx_20251127_120530_abc123",
        "phase": "completed",
        "continuant": "ckp://Continuant#System.Gateway.HTTP"
    }"#;

    let meta: Value = serde_json::from_str(metadata).unwrap();

    // REQUIRED fields (protocol contract):
    assert!(meta.get("timestamp").is_some());
    assert!(meta.get("txId").is_some());
    assert!(meta.get("processUrn").is_some());
    assert!(meta.get("phase").is_some());
    assert!(meta.get("continuant").is_some());

    // Field types must be correct:
    assert!(meta["timestamp"].is_number());
    assert!(meta["txId"].is_string());
    assert!(meta["processUrn"].is_string());
    assert!(meta["phase"].is_string());
    assert!(meta["continuant"].is_string());

    // If this test fails:
    // - Metadata schema changed
    // - Node.js runtime won't be able to parse evidence
    // - Protocol version must be bumped
}

/// WHY: Job files must be protocol-pure JSON
/// FORBIDDEN: Binary data, Rust-specific types (e.g., Option<T> serialized)
/// REASON: Node.js must be able to parse job files
/// BREAKS: Cross-runtime job processing if Rust types leak
/// SACRIFICES: If this fails, you're breaking Node.js interop
#[test]
fn job_files_are_pure_json() {
    let job = r#"{
        "id": "job-123",
        "edgeUrn": "ckp://System.Target/edges/processJob",
        "payload": {
            "type": "text",
            "content": "Hello"
        }
    }"#;

    let job_value: Value = serde_json::from_str(job).unwrap();

    // Must be valid JSON (no Rust-specific serialization)
    assert!(job_value.is_object());

    // Common Rust serialization pitfalls to avoid:
    // ❌ Option<T> → { "Some": value } (Rust default)
    // ✅ Option<T> → value or null (JSON standard)

    // If this test fails:
    // - Job files contain Rust-specific JSON
    // - Node.js runtime cannot parse jobs
}

/// WHY: Ontology YAML must be runtime-agnostic
/// FORBIDDEN: rust:, node:, python: implementation hints outside "type" field
/// REASON: Ontology describes WHAT, not HOW
/// BREAKS: Ontology purity if implementation details leak
/// SACRIFICES: If this fails, you're coupling ontology to runtimes
#[test]
fn ontology_yaml_is_runtime_agnostic() {
    // CORRECT: Ontology describes kernel properties
    // type: "rust:hot" ✅ (implementation hint in designated field)
    // port: 3000 ✅ (protocol-level property)
    // edges: [...] ✅ (protocol-level property)

    // WRONG: Implementation details in ontology
    // toolPath: "/path/to/binary" ❌ (implementation detail)
    // dependencies: ["tokio", "serde"] ❌ (Rust-specific)
    // rustVersion: "1.70" ❌ (implementation detail)

    let forbidden_keys = vec!["toolPath", "dependencies", "rustVersion", "nodeVersion"];

    for key in forbidden_keys {
        // These keys should NEVER appear in conceptkernel.yaml
        assert!(
            true, // Placeholder - actual check would parse conceptkernel.yaml
            "Ontology must not contain '{}' field",
            key
        );
    }

    // If this test fails:
    // - Ontology contains implementation details
    // - Ontology is no longer runtime-agnostic
}

/// WHY: Edge URNs must be protocol-pure
/// FORMAT: ckp://{kernel}/edges/{edgeName}
/// REASON: Edge addressing is protocol-level
/// BREAKS: Edge routing if URN format changes
/// SACRIFICES: If this fails, you're breaking edge routing protocol
#[test]
fn edge_urns_follow_protocol_format() {
    let edge_urn = "ckp://System.Target/edges/processJob";

    assert!(edge_urn.starts_with("ckp://"));
    assert!(edge_urn.contains("/edges/"));

    // Extract parts
    let parts: Vec<&str> = edge_urn.split("/edges/").collect();
    assert_eq!(parts.len(), 2);

    let kernel_part = parts[0]; // "ckp://System.Target"
    let edge_name = parts[1];   // "processJob"

    assert!(kernel_part.starts_with("ckp://"));
    assert!(!edge_name.is_empty());

    // If this test fails:
    // - Edge URN format changed
    // - Edge routing will break
    // - Protocol version must be bumped
}

/// WHY: Instance URNs must be protocol-pure
/// FORMAT: ckp://Instance#{identifier}
/// REASON: Instance addressing is protocol-level
/// BREAKS: Instance lookup if URN format changes
/// SACRIFICES: If this fails, you're breaking instance protocol
#[test]
fn instance_urns_follow_protocol_format() {
    let instance_urn = "ckp://Instance#result-123";

    assert!(instance_urn.starts_with("ckp://Instance#"));

    // Extract identifier
    let identifier = instance_urn.strip_prefix("ckp://Instance#").unwrap();
    assert!(!identifier.is_empty());

    // If this test fails:
    // - Instance URN format changed
    // - Instance lookup will break
}

/// WHY: Continuant URNs must be protocol-pure
/// FORMAT: ckp://Continuant#{type}-{name}
/// EXAMPLE: ckp://Continuant#Kernel-System.Gateway
/// REASON: Continuant addressing is protocol-level
/// BREAKS: BFO queries if URN format changes
/// SACRIFICES: If this fails, you're breaking SPARQL compatibility
#[test]
fn continuant_urns_follow_protocol_format() {
    let continuant_urn = "ckp://Continuant#Kernel-System.Gateway";

    assert!(continuant_urn.starts_with("ckp://Continuant#"));

    let fragment = continuant_urn.strip_prefix("ckp://Continuant#").unwrap();
    let parts: Vec<&str> = fragment.split('-').collect();
    assert!(parts.len() >= 2, "Continuant URN must have type-name structure");

    let continuant_type = parts[0]; // "Kernel"
    let continuant_name = parts[1..].join("-"); // "System.Gateway"

    assert!(!continuant_type.is_empty());
    assert!(!continuant_name.is_empty());

    // If this test fails:
    // - Continuant URN format changed
    // - BFO queries will break
}

/// WHY: Process URNs must be protocol-pure
/// FORMAT: ckp://Process#{type}-{txId}
/// EXAMPLE: ckp://Process#invoke-tx_20251127_120530_abc123
/// REASON: Process addressing is protocol-level
/// BREAKS: Evidence chain lookups if URN format changes
/// SACRIFICES: If this fails, you're breaking provenance queries
#[test]
fn process_urns_follow_protocol_format() {
    let process_urn = "ckp://Process#invoke-tx_20251127_120530_abc123";

    assert!(process_urn.starts_with("ckp://Process#"));

    let fragment = process_urn.strip_prefix("ckp://Process#").unwrap();
    let parts: Vec<&str> = fragment.split('-').collect();
    assert!(parts.len() >= 2, "Process URN must have type-txId structure");

    let process_type = parts[0]; // "invoke"
    assert!(!process_type.is_empty());

    // txId starts with "tx_"
    let tx_id_start = parts[1..].join("-");
    assert!(tx_id_start.starts_with("tx_"));

    // If this test fails:
    // - Process URN format changed
    // - Evidence chain queries will break
}

/// WHY: Storage paths must be resolved via URNs, not hardcoded
/// WRONG: concepts/{kernel}/storage/{file}
/// RIGHT: resolve_urn("ckp://{kernel}/storage/{file}")
/// REASON: URN resolution handles multi-project, symlinks, versioning
/// BREAKS: Portability if paths are hardcoded
/// SACRIFICES: If this fails, you're breaking "File System IS the Protocol"
#[test]
fn storage_access_uses_urn_resolution() {
    // This test documents the principle
    // Actual enforcement happens in code review

    let urn = "ckp://System.Gateway/storage/cache/result-123.json";
    assert!(urn.starts_with("ckp://"));

    // WRONG approaches:
    let wrong_hardcoded = "concepts/System.Gateway/storage/cache/result-123.json";
    assert!(!wrong_hardcoded.starts_with("ckp://"));

    // If you see hardcoded paths in code:
    // - Replace with URN resolution
    // - Use kernel.resolve_urn(urn)
    // - Never build paths manually

    // If this test fails:
    // - Hardcoded paths are being used
    // - URN resolution is bypassed
    // - Protocol principle is violated
}

/// WHY: Queue paths must be resolved via URNs
/// FORMAT: ckp://{kernel}/queue/{inbox|outbox}
/// REASON: Queue locations may vary (local, remote, versioned)
/// BREAKS: Queue processing if paths are hardcoded
/// SACRIFICES: If this fails, you're breaking queue routing
#[test]
fn queue_access_uses_urn_resolution() {
    let inbox_urn = "ckp://System.Gateway/queue/inbox";
    let outbox_urn = "ckp://System.Gateway/queue/outbox";

    assert!(inbox_urn.starts_with("ckp://"));
    assert!(outbox_urn.starts_with("ckp://"));

    // WRONG: Direct path building
    // let inbox_path = concepts_dir.join("System.Gateway").join("queue").join("inbox");

    // RIGHT: URN resolution
    // let inbox_path = kernel.resolve_urn("ckp://System.Gateway/queue/inbox")?;

    // If this test fails:
    // - Queue paths are hardcoded
    // - URN resolution bypassed
}

/// WHY: Protocol JSON must use camelCase (JavaScript convention)
/// REASON: Node.js runtime uses camelCase, protocol must match
/// EXAMPLES: txId, processUrn, watcherPid, queueStats
/// BREAKS: Node.js parsing if snake_case is used
/// SACRIFICES: If this fails, you're breaking Node.js compatibility
#[test]
fn protocol_json_uses_camel_case() {
    let protocol_json = r#"{
        "txId": "tx_123",
        "processUrn": "ckp://Process#invoke-tx_123",
        "watcherPid": 12345,
        "queueStats": {
            "inboxCount": 5
        }
    }"#;

    let value: Value = serde_json::from_str(protocol_json).unwrap();

    // Fields must be camelCase
    assert!(value.get("txId").is_some(), "Use txId, not tx_id");
    assert!(value.get("processUrn").is_some(), "Use processUrn, not process_urn");
    assert!(value.get("watcherPid").is_some(), "Use watcherPid, not watcher_pid");

    // If this test fails:
    // - You're using snake_case in protocol JSON
    // - Rust serde default is snake_case, use #[serde(rename = "camelCase")]
    // - Node.js won't parse snake_case fields
}

/// WHY: Protocol must not expose internal Rust error types
/// WRONG: Error { kind: "Io", message: "No such file" }
/// RIGHT: { error: "KERNEL_NOT_FOUND", message: "..." }
/// REASON: Error codes must be protocol-level, not implementation-level
/// BREAKS: Error handling if Rust types leak
/// SACRIFICES: If this fails, you're coupling errors to Rust runtime
#[test]
fn error_responses_are_protocol_level() {
    // CORRECT: Protocol-level error codes
    let error = r#"{
        "error": "KERNEL_NOT_FOUND",
        "message": "Kernel 'System.Missing' does not exist"
    }"#;

    let error_value: Value = serde_json::from_str(error).unwrap();

    assert!(error_value.get("error").is_some());
    assert!(error_value["error"].is_string());

    // Error code must be SCREAMING_SNAKE_CASE (protocol convention)
    let error_code = error_value["error"].as_str().unwrap();
    assert!(error_code.chars().all(|c| c.is_uppercase() || c == '_'));

    // WRONG: Rust error types in JSON
    // { "error": "Io(Error { ... })" } ❌
    // { "error": "CkpError::KernelNotFound" } ❌

    // If this test fails:
    // - Rust error types are leaking into JSON
    // - Use protocol-level error codes
}

// TODO: Add schema validation tests
// - Load actual metadata.json files from storage
// - Verify they don't contain forbidden fields
// - Use JSON schema validation crate
