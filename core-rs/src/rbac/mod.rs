//! RBAC (Role-Based Access Control) module
//!
//! Provides permission checking, communication authorization,
//! and consensus validation for ConceptKernel.
//!
//! Reference: Node.js v1.3.14 - PermissionChecker.js

pub mod permission_checker;

pub use permission_checker::{PermissionChecker, SelfImprovementConfig};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Test: PermissionChecker export is accessible
    ///
    /// Verifies that PermissionChecker type is exported and can be constructed
    /// for RBAC permission validation and consensus checking.
    #[test]
    fn test_permission_checker_export() {
        // Verify PermissionChecker type is accessible
        fn accepts_permission_checker(_: PermissionChecker) {}

        let checker = PermissionChecker::new(PathBuf::from("/tmp/test"));

        accepts_permission_checker(checker);

        // If this compiles, export is correct
    }

    /// Test: SelfImprovementConfig export is accessible
    ///
    /// Verifies that SelfImprovementConfig struct is exported and can be used
    /// for configuring self-improvement consensus requirements.
    #[test]
    fn test_self_improvement_config_export() {
        // Verify SelfImprovementConfig type is accessible
        fn accepts_config(_: SelfImprovementConfig) {}

        let config = SelfImprovementConfig {
            enabled: true,
            requires_consensus: true,
            allowed_actions: vec!["modify_kernel".to_string()],
            forbidden_actions: vec!["delete_all".to_string()],
        };

        accepts_config(config);

        // If this compiles, export is correct
    }
}
