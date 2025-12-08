//! URN parsing and validation module
//!
//! Universal Resource Names (URNs) for ConceptKernel v1.3.12
//!
//! ## URN Formats
//!
//! **Kernel URN:**
//! ```text
//! ckp://Domain.Concept:version#stage/path
//! ckp://Recipes.BakeCake:v0.1#storage/tx-123.inst
//! ```
//!
//! **Edge URN:**
//! ```text
//! ckp://Edge.PREDICATE.Source-to-Target:version
//! ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12
//! ```

mod ckdl_parser;
mod resolver;
mod validator;

pub use ckdl_parser::{
    CkdlParser, CkdlDocument, ExternDeclaration, KernelDeclaration, EdgeDeclaration
};
pub use resolver::{UrnResolver, ParsedUrn, ParsedEdgeUrn, ParsedAgentUrn, AgentType, ParsedQueryUrn, ParsedQueryUrnV2};
pub use validator::UrnValidator;

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: UrnValidator is exported and accessible
    ///
    /// Verifies that UrnValidator is part of the public API and can be used
    /// to validate URNs according to URN.v1.3.16.DRAFT-03 specification.
    ///
    /// DRAFT-03 URN patterns:
    /// - Kernel: `ckp://Kernel-Name:version`
    /// - Edge: `ckp://Edge.PREDICATE.Source-to-Target:version`
    /// - Process: `ckp://Process#{Type}-tx_{timestamp}_{hash}` (future)
    /// - Agent: `ckp://Agent/user:{username}` or `ckp://Agent/process:{KernelName}` (future)
    /// - Role: `ckp://Role/{role-name}` (future)
    /// - Proof: `ckp://Instance#Proof-{Type}-{Date}-{Hash}` (future)
    #[test]
    fn test_urn_validator_is_exported() {
        // Verify UrnValidator type is accessible via public API
        // Compilation success means export works correctly

        // Test kernel URN validation (DRAFT-03 section: Kernel Identity)
        let result = UrnValidator::validate("ckp://Recipes.BakeCake:v0.1");
        assert!(result.valid, "Kernel URN should be valid");

        // Test edge URN validation (DRAFT-03 section: Edge Identity)
        let result = UrnValidator::validate_edge_urn(
            "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12"
        );
        assert!(result.valid, "Edge URN should be valid");

        // Test kernel name validation
        assert!(UrnValidator::is_valid_kernel_name("System.Gateway.HTTP"));
        assert!(!UrnValidator::is_valid_kernel_name(".Invalid"));

        // Test version validation
        assert!(UrnValidator::is_valid_version("v1.3.16"));
        assert!(!UrnValidator::is_valid_version("invalid"));

        // Test stage validation
        assert!(UrnValidator::is_valid_stage("inbox"));
        assert!(UrnValidator::is_valid_stage("storage"));

        // Test predicate validation
        assert!(UrnValidator::is_valid_predicate("PRODUCES"));
        assert!(UrnValidator::is_valid_predicate("REQUIRES"));
    }

    /// Test: UrnResolver is exported and accessible
    ///
    /// Verifies that UrnResolver is part of the public API and can be used
    /// to parse and resolve URNs according to URN.v1.3.16.DRAFT-03 specification.
    #[test]
    fn test_urn_resolver_is_exported() {
        // Verify UrnResolver type is accessible via public API
        // Compilation success means export works correctly

        // Test kernel URN parsing (DRAFT-03 section: Kernel Identity)
        let parsed = UrnResolver::parse("ckp://Recipes.BakeCake:v0.1#storage/tx-123.inst").unwrap();
        assert_eq!(parsed.kernel, "Recipes.BakeCake");
        assert_eq!(parsed.version, "v0.1");
        assert_eq!(parsed.stage, Some("storage".to_string()));
        assert_eq!(parsed.path, Some("tx-123.inst".to_string()));

        // Test edge URN parsing with version (edge_versioning: true)
        let parsed_edge = UrnResolver::parse_edge_urn(
            "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12"
        ).unwrap();
        assert_eq!(parsed_edge.predicate, "PRODUCES");
        assert_eq!(parsed_edge.source, "MixIngredients");
        assert_eq!(parsed_edge.target, "BakeCake");
        assert_eq!(parsed_edge.version, Some("v1.3.12".to_string()));
        // With version: edge_dir includes version
        assert_eq!(parsed_edge.edge_dir, "PRODUCES.MixIngredients-to-BakeCake:v1.3.12");
        assert_eq!(parsed_edge.queue_path, "queue/edges/PRODUCES.MixIngredients-to-BakeCake:v1.3.12");

        // Test edge URN parsing without version (edge_versioning: false)
        let parsed_edge_no_ver = UrnResolver::parse_edge_urn(
            "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake"
        ).unwrap();
        assert_eq!(parsed_edge_no_ver.predicate, "PRODUCES");
        assert_eq!(parsed_edge_no_ver.source, "MixIngredients");
        assert_eq!(parsed_edge_no_ver.target, "BakeCake");
        assert_eq!(parsed_edge_no_ver.version, None);
        // Without version: edge_dir has no version
        assert_eq!(parsed_edge_no_ver.edge_dir, "PRODUCES.MixIngredients-to-BakeCake");
        assert_eq!(parsed_edge_no_ver.queue_path, "queue/edges/PRODUCES.MixIngredients-to-BakeCake");

        // Test URN type detection
        assert!(UrnResolver::is_kernel_urn("ckp://Recipes.BakeCake:v0.1"));
        assert!(UrnResolver::is_edge_urn("ckp://Edge.PRODUCES.A-to-B:v1.0"));

        // Test URN building
        let components = ParsedUrn {
            kernel: "Test.Kernel".to_string(),
            version: "v1.0".to_string(),
            stage: Some("inbox".to_string()),
            path: None,
        };
        let urn = UrnResolver::build(&components);
        assert_eq!(urn, "ckp://Test.Kernel:v1.0#inbox");
    }

    /// Test: ParsedUrn and ParsedEdgeUrn types are exported
    ///
    /// Verifies that parsed URN data structures are accessible and can be
    /// constructed/used by external code.
    #[test]
    fn test_parsed_urn_types_are_exported() {
        // Verify ParsedUrn is accessible and can be constructed
        let parsed_urn = ParsedUrn {
            kernel: "System.Consensus".to_string(),
            version: "v1.3.16".to_string(),
            stage: Some("storage".to_string()),
            path: Some("proof-123.inst".to_string()),
        };

        // Verify all fields are accessible
        assert_eq!(parsed_urn.kernel, "System.Consensus");
        assert_eq!(parsed_urn.version, "v1.3.16");
        assert_eq!(parsed_urn.stage, Some("storage".to_string()));
        assert_eq!(parsed_urn.path, Some("proof-123.inst".to_string()));

        // Verify ParsedEdgeUrn is accessible and can be constructed (with version)
        let parsed_edge = ParsedEdgeUrn {
            predicate: "PRODUCES".to_string(),
            source: "MixIngredients".to_string(),
            target: "BakeCake".to_string(),
            version: Some("v1.3.12".to_string()),
            queue_path: "queue/edges/PRODUCES.MixIngredients-to-BakeCake:v1.3.12".to_string(),
            edge_dir: "PRODUCES.MixIngredients-to-BakeCake:v1.3.12".to_string(),
        };

        // Verify all fields are accessible
        assert_eq!(parsed_edge.predicate, "PRODUCES");
        assert_eq!(parsed_edge.source, "MixIngredients");
        assert_eq!(parsed_edge.target, "BakeCake");
        assert_eq!(parsed_edge.version, Some("v1.3.12".to_string()));
        assert_eq!(parsed_edge.queue_path, "queue/edges/PRODUCES.MixIngredients-to-BakeCake:v1.3.12");
        assert_eq!(parsed_edge.edge_dir, "PRODUCES.MixIngredients-to-BakeCake:v1.3.12");

        // Verify Clone and PartialEq traits work
        let cloned_urn = parsed_urn.clone();
        assert_eq!(parsed_urn, cloned_urn);

        let cloned_edge = parsed_edge.clone();
        assert_eq!(parsed_edge, cloned_edge);
    }

    /// Test: URN patterns align with DRAFT-03 specification
    ///
    /// This test documents the URN patterns from URN.v1.3.16.DRAFT-03 and
    /// verifies that current implementation handles Kernel and Edge URNs correctly.
    ///
    /// IMPLEMENTED (v1.3.16):
    /// - ✅ Kernel URNs: `ckp://Kernel-Name:version#stage/path`
    /// - ✅ Edge URNs: `ckp://Edge.PREDICATE.Source-to-Target:version`
    ///
    /// FUTURE (documented in DRAFT-03, not yet implemented):
    /// - ⏳ Process URNs: `ckp://Process#{Type}-tx_{timestamp}_{hash}`
    /// - ⏳ Agent URNs: `ckp://Agent/user:{username}`, `ckp://Agent/process:{KernelName}`
    /// - ⏳ Role URNs: `ckp://Role/{role-name}`
    /// - ⏳ Proof URNs: `ckp://Instance#Proof-{Type}-{Date}-{Hash}`
    /// - ⏳ Consensus URNs: `ckp://Consensus#{Type}-{Subject}-{Date}`
    #[test]
    fn test_urn_patterns_draft03_alignment() {
        // ✅ IMPLEMENTED: Kernel URN pattern (DRAFT-03 Section 1)
        let kernel_urns = vec![
            "ckp://Recipes.BakeCake:v0.1",
            "ckp://System.Consensus:v1.3.16",
            "ckp://ConceptKernel.LLM.Claude:v0.1",
            "ckp://System.Gateway.HTTP:v1.3.12#inbox",
            "ckp://System.Registry:v1.0#storage/tx-456.inst",
        ];

        for urn in kernel_urns {
            let result = UrnValidator::validate(urn);
            assert!(result.valid, "Kernel URN should be valid: {}", urn);

            let parsed = UrnResolver::parse(urn).unwrap();
            assert!(parsed.kernel.len() > 0);
            assert!(parsed.version.len() > 0);
        }

        // ✅ IMPLEMENTED: Edge URN pattern (DRAFT-03 Section 5)
        let edge_urns = vec![
            "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12",
            "ckp://Edge.REQUIRES.Source-to-Target:v1.0",
            "ckp://Edge.VALIDATES.A-to-B:v0.1",
            "ckp://Edge.INFLUENCES.X-to-Y:v2.0",
        ];

        for edge_urn in edge_urns {
            let result = UrnValidator::validate_edge_urn(edge_urn);
            assert!(result.valid, "Edge URN should be valid: {}", edge_urn);

            let parsed = UrnResolver::parse_edge_urn(edge_urn).unwrap();
            assert!(parsed.predicate.len() > 0);
            assert!(parsed.source.len() > 0);
            assert!(parsed.target.len() > 0);
        }

        // ⏳ FUTURE: Process URN pattern (DRAFT-03 Section 2)
        // Format: ckp://Process#{Type}-tx_{timestamp}_{hash}
        // Examples:
        //   - ckp://Process#Invocation-tx_20251128_100000_abc123
        //   - ckp://Process#EdgeRoute-tx_20251128_100001_def456
        //   - ckp://Process#ProposeMapping-tx_20251128_100002_ghi789
        //
        // TODO: Implement ProcessUrnValidator and ProcessUrnResolver when needed

        // ⏳ FUTURE: Agent URN pattern (DRAFT-03 Section 3)
        // User format: ckp://Agent/user:{username}
        // Process format: ckp://Agent/process:{KernelName}
        // Examples:
        //   - ckp://Agent/user:conceptkernel
        //   - ckp://Agent/user:alice
        //   - ckp://Agent/process:ConceptKernel.LLM.Claude
        //   - ckp://Agent/process:System.Governor.Consensus
        //
        // TODO: Implement AgentUrnValidator and AgentUrnResolver when needed

        // ⏳ FUTURE: Role URN pattern (DRAFT-03 Section 4)
        // Format: ckp://Role/{role-name}
        // Examples:
        //   - ckp://Role/system-admin
        //   - ckp://Role/ontology-curator
        //   - ckp://Role/edge-manager
        //   - ckp://Role/query-user
        //
        // TODO: Implement RoleUrnValidator when RBAC system is complete

        // ⏳ FUTURE: Proof URN pattern (DRAFT-03 Section 5)
        // Format: ckp://Instance#Proof-{Type}-{Date}-{Hash}
        // Examples:
        //   - ckp://Instance#Proof-EdgeCreation-20251128-abc123
        //   - ckp://Instance#Proof-BfoMapping-DELEGATES-20251128-def456
        //   - ckp://Instance#Proof-ConsensusVote-20251128-jkl012
        //
        // TODO: Implement ProofUrnValidator when System.Proof kernel is complete
    }

    /// Test: URN module provides comprehensive validation
    ///
    /// Verifies that the URN module exports provide all necessary functionality
    /// for URN validation, parsing, and resolution.
    #[test]
    fn test_urn_module_completeness() {
        // Verify comprehensive validation capabilities
        fn accepts_validator(_: fn(&str) -> crate::urn::validator::ValidationResult) {}
        accepts_validator(UrnValidator::validate);
        accepts_validator(UrnValidator::validate_kernel_urn);
        accepts_validator(UrnValidator::validate_edge_urn);

        // Verify helper validation functions
        fn accepts_name_validator(_: fn(&str) -> bool) {}
        accepts_name_validator(UrnValidator::is_valid_kernel_name);
        accepts_name_validator(UrnValidator::is_valid_version);
        accepts_name_validator(UrnValidator::is_valid_stage);
        accepts_name_validator(UrnValidator::is_valid_predicate);

        // Verify Result-based validation
        fn accepts_result_validator(_: fn(&str) -> crate::errors::Result<()>) {}
        accepts_result_validator(UrnValidator::assert_valid);
        accepts_result_validator(UrnValidator::assert_valid_edge_urn);

        // Verify parsing capabilities
        fn accepts_parser(_: fn(&str) -> crate::errors::Result<ParsedUrn>) {}
        accepts_parser(UrnResolver::parse);

        fn accepts_edge_parser(_: fn(&str) -> crate::errors::Result<ParsedEdgeUrn>) {}
        accepts_edge_parser(UrnResolver::parse_edge_urn);

        // Verify URN building
        fn accepts_builder(_: fn(&ParsedUrn) -> String) {}
        accepts_builder(UrnResolver::build);

        // Verify URN type detection
        fn accepts_type_checker(_: fn(&str) -> bool) {}
        accepts_type_checker(UrnResolver::is_kernel_urn);
        accepts_type_checker(UrnResolver::is_edge_urn);

        // If this test compiles, all functions are properly exported
    }
}
