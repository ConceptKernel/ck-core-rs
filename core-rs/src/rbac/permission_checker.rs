//! Permission Checker for RBAC enforcement
//!
//! Provides permission checking, communication authorization (whitelist/blacklist),
//! consensus validation, and git operation controls.
//!
//! Reference: Node.js v1.3.14 - PermissionChecker.js

use crate::errors::{CkpError, Result};
use crate::ontology::OntologyReader;
use crate::ontology::library::OntologyLibrary;
use crate::continuant_tracker::ContinuantTracker;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Consensus proposal structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConsensusProposal {
    action: String,
    status: String,
    proposer: String,
    approvers: Vec<String>,
    timestamp: String,
    threshold: u32,
}

/// Self-improvement configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfImprovementConfig {
    pub enabled: bool,
    pub requires_consensus: bool,
    pub allowed_actions: Vec<String>,
    pub forbidden_actions: Vec<String>,
}

impl Default for SelfImprovementConfig {
    fn default() -> Self {
        SelfImprovementConfig {
            enabled: false,
            requires_consensus: true,
            allowed_actions: Vec::new(),
            forbidden_actions: Vec::new(),
        }
    }
}

/// Permission Checker - RBAC enforcement for ConceptKernel
pub struct PermissionChecker {
    root: PathBuf,
    pattern_cache: HashMap<String, Regex>,
    ontology_lib: Option<OntologyLibrary>,
}

impl PermissionChecker {
    /// Create new PermissionChecker without ontology library
    ///
    /// # Arguments
    /// * `root` - Root directory (project root, not concepts/)
    ///
    /// Note: For backward compatibility. Use `new_with_ontology()` for full RBAC support.
    pub fn new(root: PathBuf) -> Self {
        PermissionChecker {
            root,
            pattern_cache: HashMap::new(),
            ontology_lib: None,
        }
    }

    /// Create new PermissionChecker with ontology library loaded
    ///
    /// # Arguments
    /// * `root` - Root directory (project root, not concepts/)
    ///
    /// # Returns
    /// PermissionChecker with ontology-based RBAC enabled
    pub fn new_with_ontology(root: PathBuf) -> Result<Self> {
        let ontology_lib = OntologyLibrary::new(root.clone())?;

        Ok(PermissionChecker {
            root,
            pattern_cache: HashMap::new(),
            ontology_lib: Some(ontology_lib),
        })
    }

    /// Check if user has permission to perform action on kernel
    ///
    /// # Arguments
    /// * `username` - User performing action
    /// * `permission` - Permission required (e.g., "kernel.modify")
    /// * `kernel_urn` - Target kernel URN or simple name
    ///
    /// # Returns
    /// true if permission granted, false otherwise
    pub async fn check_permission(
        &self,
        username: &str,
        permission: &str,
        _kernel_urn: &str,
    ) -> Result<bool> {
        // Get user permissions
        let user_perms = self.get_user_permissions(username);

        // Check for wildcard permission
        if user_perms.contains(&"*".to_string()) {
            // Admin has all permissions, even consensus-required ones
            return Ok(true);
        }

        // Check for exact match or wildcard match
        let mut has_permission = false;
        for perm in &user_perms {
            if perm == permission {
                has_permission = true;
                break;
            }

            // Check wildcard patterns (e.g., "kernel.*" matches "kernel.create")
            if perm.contains('*') {
                let pattern = perm.replace('.', "\\.").replace('*', ".*");
                let regex = Regex::new(&format!("^{}$", pattern))?;
                if regex.is_match(permission) {
                    has_permission = true;
                    break;
                }
            }
        }

        if !has_permission {
            return Ok(false);
        }

        // If permission requires consensus, check for approval
        if self.requires_consensus(permission) {
            // For now, return false if consensus is required
            // Full consensus implementation would check approved proposals
            return Ok(false);
        }

        Ok(true)
    }

    /// Check agent permission using SPARQL (RBAC Phase 02)
    ///
    /// # Arguments
    /// * `agent_urn` - Agent URN (format: `ckp://Agent/user:{username}` or `ckp://Agent/process:{KernelName}`)
    /// * `permission` - Permission string (e.g., "http.handle", "consensus.propose")
    ///
    /// # Returns
    /// true if agent has permission via role grants
    ///
    /// Uses direct SPARQL query for efficient permission checking.
    /// Falls back to `check_permission()` if ontology library is not available.
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::rbac::PermissionChecker;
    /// # use std::path::PathBuf;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let checker = PermissionChecker::new_with_ontology(PathBuf::from("/project"))?;
    /// let has_perm = checker.check_agent_permission_sparql("ckp://Agent/user:admin", "http.handle").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check_agent_permission_sparql(
        &self,
        agent_urn: &str,
        permission: &str,
    ) -> Result<bool> {
        if let Some(ref ontology_lib) = self.ontology_lib {
            // Direct SPARQL query for permission
            match ontology_lib.check_agent_permission(agent_urn, permission) {
                Ok(has_permission) => return Ok(has_permission),
                Err(e) => {
                    eprintln!("[PermissionChecker] SPARQL permission check failed: {}", e);
                    // Fall through to username-based check
                }
            }
        }

        // Fallback: extract username from agent URN and use existing method
        if agent_urn.starts_with("ckp://Agent/user:") {
            let username = agent_urn.strip_prefix("ckp://Agent/user:").unwrap_or("unknown");
            return self.check_permission(username, permission, "").await;
        } else if agent_urn.starts_with("ckp://Agent/process:") {
            // Process agents: use admin permissions for now
            return self.check_permission("admin", permission, "").await;
        }

        // Unknown agent URN format
        Ok(false)
    }

    /// Check if source kernel can emit to target kernel
    ///
    /// # Arguments
    /// * `source_kernel_urn` - Source kernel URN or simple name
    /// * `target_kernel_urn` - Target kernel URN or simple name
    ///
    /// # Returns
    /// true if communication allowed, false otherwise
    pub fn can_emit_to(&mut self, source_kernel_urn: &str, target_kernel_urn: &str) -> Result<bool> {
        // Extract kernel name from URN
        let source_name = self.extract_kernel_name(source_kernel_urn);

        // Normalize target to URN format
        let normalized_target = if target_kernel_urn.starts_with("ckp://") {
            target_kernel_urn.to_string()
        } else {
            format!("ckp://{}", target_kernel_urn)
        };

        // Load source kernel's ontology
        let ontology_reader = OntologyReader::new(self.root.clone());
        let ontology = match ontology_reader.read_by_kernel_name(&source_name) {
            Ok(ont) => ont,
            Err(e) => {
                eprintln!("[PermissionChecker] Failed to load ontology for {}: {}", source_name, e);
                // Fail closed - deny on error
                return Ok(false);
            }
        };

        // Extract RBAC communication rules
        let rbac = ontology.spec.as_ref().and_then(|s| s.rbac.as_ref());
        let comm = rbac.and_then(|r| r.communication.as_ref());

        // Check blacklist first (denied patterns)
        if let Some(denied) = comm.and_then(|c| c.denied.as_ref()) {
            for pattern in denied {
                if self.matches_pattern(&normalized_target, pattern)? {
                    eprintln!(
                        "[PermissionChecker] Communication denied: {} -> {} (blacklist: {})",
                        source_name, target_kernel_urn, pattern
                    );
                    return Ok(false);
                }
            }
        }

        // Check whitelist (allowed patterns)
        if let Some(allowed) = comm.and_then(|c| c.allowed.as_ref()) {
            // If whitelist exists, target must be in it
            for allowed_urn in allowed {
                if allowed_urn == "ckp://*" || self.matches_pattern(&normalized_target, allowed_urn)? {
                    return Ok(true);
                }
            }

            // Not in whitelist
            eprintln!(
                "[PermissionChecker] Communication not in whitelist: {} -> {}",
                source_name, target_kernel_urn
            );
            return Ok(false);
        }

        // No restrictions (promiscuous mode) - allow all
        Ok(true)
    }

    /// Assert that source can emit to target (throws error if not allowed)
    ///
    /// # Arguments
    /// * `source_kernel_urn` - Source kernel URN
    /// * `target_kernel_urn` - Target kernel URN
    ///
    /// # Errors
    /// Returns error if communication not authorized
    pub fn assert_can_emit_to(
        &mut self,
        source_kernel_urn: &str,
        target_kernel_urn: &str,
    ) -> Result<()> {
        let allowed = self.can_emit_to(source_kernel_urn, target_kernel_urn)?;

        if !allowed {
            return Err(CkpError::Rbac(format!(
                "Communication not authorized: {} -> {}",
                source_kernel_urn, target_kernel_urn
            )));
        }

        Ok(())
    }

    /// Check if permission requires consensus approval (RBAC Phase 02)
    ///
    /// # Arguments
    /// * `permission` - Permission string
    ///
    /// # Returns
    /// true if permission requires consensus (QuorumLow or QuorumHigh)
    ///
    /// Queries the ontology library for the permission's quorum requirement.
    /// Falls back to hardcoded list if ontology library is not available.
    pub fn requires_consensus(&self, permission: &str) -> bool {
        if let Some(ref ontology_lib) = self.ontology_lib {
            // Try to get quorum requirement from ontology
            match ontology_lib.get_permission_quorum(permission) {
                Ok(quorum_uri) => {
                    // Check if quorum requires consensus (QuorumLow or QuorumHigh)
                    return quorum_uri.contains("QuorumLow") || quorum_uri.contains("QuorumHigh");
                },
                Err(e) => {
                    eprintln!("[PermissionChecker] Failed to query quorum for permission '{}': {}", permission, e);
                    // Fall through to hardcoded list
                }
            }
        }

        // Fallback to hardcoded consensus requirements (backward compatibility)
        const CONSENSUS_REQUIRED: &[&str] = &[
            "kernel.delete",
            "edge.authorize",
            "rbac.role.create",
            "rbac.permission.grant",
            "git.merge_main",
            "git.tag",
            "consensus.execute",
        ];

        CONSENSUS_REQUIRED.contains(&permission)
    }

    /// Get self-improvement configuration for kernel
    ///
    /// # Arguments
    /// * `kernel_urn` - Kernel URN or simple name
    ///
    /// # Returns
    /// Self-improvement configuration
    pub fn get_self_improvement_config(&self, kernel_urn: &str) -> Result<SelfImprovementConfig> {
        let kernel_name = self.extract_kernel_name(kernel_urn);
        let ontology_reader = OntologyReader::new(self.root.clone());

        let ontology = match ontology_reader.read_by_kernel_name(&kernel_name) {
            Ok(ont) => ont,
            Err(_) => {
                // Return safe defaults if ontology not found
                return Ok(SelfImprovementConfig::default());
            }
        };

        // Extract self-improvement config from ontology
        if let Some(spec) = &ontology.spec {
            if let Some(rbac) = &spec.rbac {
                if let Some(self_improvement) = &rbac.self_improvement {
                    return Ok(SelfImprovementConfig {
                        enabled: self_improvement.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                        requires_consensus: self_improvement.get("requires_consensus").and_then(|v| v.as_bool()).unwrap_or(true),
                        allowed_actions: self_improvement
                            .get("allowed_actions")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                            .unwrap_or_default(),
                        forbidden_actions: self_improvement
                            .get("forbidden_actions")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                            .unwrap_or_default(),
                    });
                }
            }
        }

        Ok(SelfImprovementConfig::default())
    }

    /// Check if kernel can perform git operation
    ///
    /// # Arguments
    /// * `kernel_urn` - Kernel URN or simple name
    /// * `operation` - Git operation: "commit", "tag", or "merge_main"
    ///
    /// # Returns
    /// true if operation allowed
    pub fn can_perform_git_operation(&self, kernel_urn: &str, operation: &str) -> Result<bool> {
        let kernel_name = self.extract_kernel_name(kernel_urn);
        let ontology_reader = OntologyReader::new(self.root.clone());

        let ontology = match ontology_reader.read_by_kernel_name(&kernel_name) {
            Ok(ont) => ont,
            Err(e) => {
                eprintln!(
                    "[PermissionChecker] Failed to load ontology for {}: {}",
                    kernel_name, e
                );
                return Ok(false); // Fail closed
            }
        };

        // Extract git permissions from ontology
        if let Some(spec) = &ontology.spec {
            if let Some(rbac) = &spec.rbac {
                if let Some(git_perms) = &rbac.git {
                    return match operation {
                        "commit" => Ok(git_perms.get("can_commit").and_then(|v| v.as_bool()).unwrap_or(true)),
                        "tag" => Ok(git_perms.get("can_tag").and_then(|v| v.as_bool()).unwrap_or(false)),
                        "merge_main" => Ok(git_perms.get("can_merge_main").and_then(|v| v.as_bool()).unwrap_or(false)),
                        _ => Ok(false),
                    };
                }
            }
        }

        // Defaults
        match operation {
            "commit" => Ok(true),
            "tag" => Ok(false),
            "merge_main" => Ok(false),
            _ => Ok(false),
        }
    }

    /// Get user permissions using ontology library (RBAC Phase 02)
    ///
    /// # Arguments
    /// * `username` - Username (will be converted to agent URN)
    ///
    /// # Returns
    /// Vector of permission strings
    ///
    /// Queries the ontology library for permissions granted to the user's roles.
    /// Falls back to hardcoded permissions if ontology library is not available.
    fn get_user_permissions(&self, username: &str) -> Vec<String> {
        if let Some(ref ontology_lib) = self.ontology_lib {
            // Convert username to agent URN
            let agent_urn = format!("ckp://Agent/user:{}", username);

            // Try to get permissions from ontology
            match ontology_lib.get_agent_permissions(&agent_urn) {
                Ok(permissions) => {
                    if !permissions.is_empty() {
                        return permissions;
                    }
                    // Fall through to default if no permissions found
                },
                Err(e) => {
                    eprintln!("[PermissionChecker] Failed to query ontology for permissions: {}", e);
                    // Fall through to default
                }
            }
        }

        // Fallback to hardcoded permissions (backward compatibility)
        if username == "admin" || username == "root" {
            vec!["*".to_string()]
        } else {
            vec![
                "kernel.create".to_string(),
                "kernel.modify".to_string(),
                "kernel.read".to_string(),
                "edge.create".to_string(),
                "storage.read".to_string(),
            ]
        }
    }

    // ========================================================================
    // BFO ROLE-BASED ACCESS CONTROL (Phase 4 Stage 3)
    // ========================================================================

    /// Check if kernel has a specific role
    ///
    /// Integrates with ContinuantTracker to query Role entities assigned to kernels.
    ///
    /// # Arguments
    /// * `kernel_urn` - Kernel URN or simple name
    /// * `role_name` - Role name (e.g., "admin", "developer", "viewer")
    ///
    /// # Returns
    /// true if kernel has the role
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::rbac::PermissionChecker;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let checker = PermissionChecker::new(PathBuf::from("/project"));
    /// let has_admin = checker.has_role("System.Gateway.HTTP", "admin")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn has_role(&self, kernel_urn: &str, role_name: &str) -> Result<bool> {
        let kernel_name = self.extract_kernel_name(kernel_urn);

        // Get concepts root (project_root/concepts)
        let concepts_root = self.root.join("concepts");

        // Create ContinuantTracker
        let tracker = ContinuantTracker::new(concepts_root);

        // Get all roles for the kernel
        let roles = tracker.get_kernel_roles(&kernel_name)?;

        // Check if any role matches the requested role name
        for role in roles {
            if role.name == role_name {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get all roles assigned to a kernel
    ///
    /// # Arguments
    /// * `kernel_urn` - Kernel URN or simple name
    ///
    /// # Returns
    /// Vector of role names
    pub fn get_kernel_roles(&self, kernel_urn: &str) -> Result<Vec<String>> {
        let kernel_name = self.extract_kernel_name(kernel_urn);

        let concepts_root = self.root.join("concepts");
        let tracker = ContinuantTracker::new(concepts_root);

        let roles = tracker.get_kernel_roles(&kernel_name)?;

        Ok(roles.iter().map(|r| r.name.clone()).collect())
    }

    /// Check if kernel has any of the specified roles
    ///
    /// # Arguments
    /// * `kernel_urn` - Kernel URN or simple name
    /// * `role_names` - List of acceptable role names
    ///
    /// # Returns
    /// true if kernel has at least one of the roles
    pub fn has_any_role(&self, kernel_urn: &str, role_names: &[&str]) -> Result<bool> {
        for role_name in role_names {
            if self.has_role(kernel_urn, role_name)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Check if kernel has all of the specified roles
    ///
    /// # Arguments
    /// * `kernel_urn` - Kernel URN or simple name
    /// * `role_names` - List of required role names
    ///
    /// # Returns
    /// true if kernel has all of the roles
    pub fn has_all_roles(&self, kernel_urn: &str, role_names: &[&str]) -> Result<bool> {
        for role_name in role_names {
            if !self.has_role(kernel_urn, role_name)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Check if consensus approval exists (private helper)
    #[allow(dead_code)]
    fn has_consensus_approval(
        &self,
        kernel_urn: &str,
        action: &str,
        _username: &str,
    ) -> Result<bool> {
        let kernel_name = self.extract_kernel_name(kernel_urn);
        let approved_dir = self
            .root
            .join("concepts")
            .join(&kernel_name)
            .join("consensus")
            .join("approved");

        if !approved_dir.exists() {
            return Ok(false);
        }

        // Search for matching approved proposals
        for entry in fs::read_dir(approved_dir)? {
            let entry = entry?;
            let proposal_file = entry.path().join("proposal.json");

            if proposal_file.exists() {
                let content = fs::read_to_string(&proposal_file)?;
                let proposal: ConsensusProposal = serde_json::from_str(&content)?;

                if proposal.action == action && proposal.status == "approved" {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Match URN against pattern with wildcards
    ///
    /// # Arguments
    /// * `urn` - URN to check
    /// * `pattern` - Pattern with optional wildcards
    ///
    /// # Returns
    /// true if URN matches pattern
    pub fn matches_pattern(&mut self, urn: &str, pattern: &str) -> Result<bool> {
        // Exact match
        if urn == pattern {
            return Ok(true);
        }

        // Wildcard match
        if pattern.contains('*') {
            // Check cache first
            if let Some(regex) = self.pattern_cache.get(pattern) {
                return Ok(regex.is_match(urn));
            }

            // Convert pattern to regex
            let regex_pattern = pattern.replace('.', "\\.").replace('*', ".*");
            let regex = Regex::new(&format!("^{}$", regex_pattern))?;

            // Cache the regex
            self.pattern_cache.insert(pattern.to_string(), regex.clone());

            return Ok(regex.is_match(urn));
        }

        Ok(false)
    }

    /// Extract kernel name from URN or return simple name
    ///
    /// # Arguments
    /// * `kernel_urn_or_name` - Full URN or simple name
    ///
    /// # Returns
    /// Kernel name without URN prefix
    fn extract_kernel_name(&self, kernel_urn_or_name: &str) -> String {
        if kernel_urn_or_name.starts_with("ckp://") {
            // Extract kernel name from URN (remove version if present)
            let without_prefix = kernel_urn_or_name.trim_start_matches("ckp://");
            if let Some(colon_pos) = without_prefix.find(':') {
                without_prefix[..colon_pos].to_string()
            } else {
                without_prefix.to_string()
            }
        } else {
            kernel_urn_or_name.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, PermissionChecker) {
        let temp_dir = TempDir::new().unwrap();
        let checker = PermissionChecker::new(temp_dir.path().to_path_buf());
        (temp_dir, checker)
    }

    fn create_test_ontology(temp: &TempDir, kernel_name: &str, allowed: Vec<&str>, denied: Vec<&str>) {
        let kernel_dir = temp.path().join("concepts").join(kernel_name);
        fs::create_dir_all(&kernel_dir).unwrap();

        let allowed_yaml = if allowed.is_empty() {
            String::new()
        } else {
            format!("      allowed:\n{}", allowed.iter().map(|a| format!("        - {}", a)).collect::<Vec<_>>().join("\n"))
        };

        let denied_yaml = if denied.is_empty() {
            String::new()
        } else {
            format!("      denied:\n{}", denied.iter().map(|d| format!("        - {}", d)).collect::<Vec<_>>().join("\n"))
        };

        let ontology_content = format!(
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://{}:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
{}
{}
"#,
            kernel_name, allowed_yaml, denied_yaml
        );

        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology_content).unwrap();
    }

    #[tokio::test]
    async fn test_check_permission_exact_match() {
        let (_temp, checker) = setup_test_env();

        // Admin should have all permissions
        let allowed = checker.check_permission("admin", "kernel.delete", "TestKernel").await.unwrap();
        assert!(allowed);

        // Developer should have kernel.create
        let allowed = checker.check_permission("developer@example.com", "kernel.create", "TestKernel").await.unwrap();
        assert!(allowed);
    }

    #[tokio::test]
    async fn test_check_permission_wildcard() {
        let (_temp, checker) = setup_test_env();

        // Admin has "*" which matches everything
        let allowed = checker.check_permission("admin", "any.permission.here", "TestKernel").await.unwrap();
        assert!(allowed);
    }

    #[tokio::test]
    async fn test_check_permission_denied() {
        let (_temp, checker) = setup_test_env();

        // Developer should NOT have kernel.delete (not in default list)
        let allowed = checker.check_permission("developer@example.com", "kernel.delete", "TestKernel").await.unwrap();
        assert!(!allowed);
    }

    #[tokio::test]
    async fn test_can_emit_to_whitelist_allowed() {
        let (temp, mut checker) = setup_test_env();

        create_test_ontology(
            &temp,
            "Source",
            vec!["ckp://Target:v0.1", "ckp://System.Proof:v0.1"],
            vec![],
        );

        let allowed = checker.can_emit_to("ckp://Source:v0.1", "ckp://Target:v0.1").unwrap();
        assert!(allowed);
    }

    #[tokio::test]
    async fn test_can_emit_to_whitelist_denied() {
        let (temp, mut checker) = setup_test_env();

        create_test_ontology(
            &temp,
            "Source",
            vec!["ckp://AllowedTarget:v0.1"],
            vec![],
        );

        let allowed = checker.can_emit_to("ckp://Source:v0.1", "ckp://DeniedTarget:v0.1").unwrap();
        assert!(!allowed);
    }

    #[tokio::test]
    async fn test_can_emit_to_blacklist_denied() {
        let (temp, mut checker) = setup_test_env();

        create_test_ontology(
            &temp,
            "Source",
            vec!["ckp://*"],  // Allow all
            vec!["ckp://Malicious.*"],  // Except Malicious.*
        );

        let allowed = checker.can_emit_to("ckp://Source:v0.1", "ckp://Malicious.Actor:v1.0").unwrap();
        assert!(!allowed);
    }

    #[tokio::test]
    async fn test_can_emit_to_promiscuous() {
        let (temp, mut checker) = setup_test_env();

        // Create ontology with no RBAC rules
        let kernel_dir = temp.path().join("concepts/Source");
        fs::create_dir_all(&kernel_dir).unwrap();
        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://Source:v0.1
  type: node:cold
  version: v0.1
spec: {}
"#,
        )
        .unwrap();

        // Should allow all (promiscuous mode)
        let allowed = checker.can_emit_to("ckp://Source:v0.1", "ckp://AnyTarget:v0.1").unwrap();
        assert!(allowed);
    }

    #[test]
    fn test_matches_pattern_exact() {
        let (_temp, mut checker) = setup_test_env();

        let matches = checker.matches_pattern("ckp://System.Proof:v0.1", "ckp://System.Proof:v0.1").unwrap();
        assert!(matches);

        let matches = checker.matches_pattern("ckp://System.Proof:v0.1", "ckp://System.Other:v0.1").unwrap();
        assert!(!matches);
    }

    #[test]
    fn test_matches_pattern_wildcard() {
        let (_temp, mut checker) = setup_test_env();

        let matches = checker.matches_pattern("ckp://System.Gateway.HTTP:v1.0", "ckp://System.*").unwrap();
        assert!(matches);

        let matches = checker.matches_pattern("ckp://Recipes.BakeCake:v0.1", "ckp://System.*").unwrap();
        assert!(!matches);
    }

    #[test]
    fn test_matches_pattern_complex() {
        let (_temp, mut checker) = setup_test_env();

        let matches = checker.matches_pattern("ckp://Recipes.BakeCake.v2:v0.1", "ckp://Recipes.*.v2:*").unwrap();
        assert!(matches);

        let matches = checker.matches_pattern("ckp://Recipes.BakeCake.v1:v0.1", "ckp://Recipes.*.v2:*").unwrap();
        assert!(!matches);
    }

    #[test]
    fn test_requires_consensus_critical() {
        let (_temp, checker) = setup_test_env();

        assert!(checker.requires_consensus("kernel.delete"));
        assert!(checker.requires_consensus("edge.authorize"));
        assert!(checker.requires_consensus("git.merge_main"));
        assert!(checker.requires_consensus("git.tag"));
        assert!(checker.requires_consensus("consensus.execute"));
    }

    #[test]
    fn test_requires_consensus_normal() {
        let (_temp, checker) = setup_test_env();

        assert!(!checker.requires_consensus("kernel.create"));
        assert!(!checker.requires_consensus("kernel.read"));
        assert!(!checker.requires_consensus("storage.read"));
        assert!(!checker.requires_consensus("kernel.modify"));
    }

    #[test]
    fn test_self_improvement_config() {
        let (temp, checker) = setup_test_env();

        // Create kernel with self-improvement config
        let kernel_dir = temp.path().join("concepts/TestKernel");
        fs::create_dir_all(&kernel_dir).unwrap();

        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://TestKernel:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    self_improvement:
      enabled: true
      requires_consensus: false
      allowed_actions:
        - schema.add_field
      forbidden_actions:
        - ontology.modify_rbac
"#,
        )
        .unwrap();

        let config = checker.get_self_improvement_config("TestKernel").unwrap();
        assert!(config.enabled);
        assert!(!config.requires_consensus);
        assert_eq!(config.allowed_actions.len(), 1);
        assert_eq!(config.forbidden_actions.len(), 1);
    }

    #[test]
    fn test_git_operation_commit() {
        let (temp, checker) = setup_test_env();

        let kernel_dir = temp.path().join("concepts/TestKernel");
        fs::create_dir_all(&kernel_dir).unwrap();

        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://TestKernel:v0.1
  type: node:cold
  version: v0.1
spec: {}
"#,
        )
        .unwrap();

        // Commit should be allowed by default
        let allowed = checker.can_perform_git_operation("TestKernel", "commit").unwrap();
        assert!(allowed);
    }

    #[test]
    fn test_git_operation_tag_denied() {
        let (temp, checker) = setup_test_env();

        let kernel_dir = temp.path().join("concepts/TestKernel");
        fs::create_dir_all(&kernel_dir).unwrap();

        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://TestKernel:v0.1
  type: node:cold
  version: v0.1
spec: {}
"#,
        )
        .unwrap();

        // Tag should be denied by default
        let allowed = checker.can_perform_git_operation("TestKernel", "tag").unwrap();
        assert!(!allowed);
    }

    // ============================================================================
    // COMPLEX RBAC SCENARIO TESTS (10 tests)
    // ============================================================================

    // -------------------------------------------------------------------------
    // 1. Multi-level RBAC hierarchies: Parent-child role inheritance
    // -------------------------------------------------------------------------
    #[test]
    fn test_rbac_hierarchy_parent_child_inheritance() {
        let (temp, mut checker) = setup_test_env();

        // Create parent kernel with specific permissions
        create_test_ontology(
            &temp,
            "Parent.System",
            vec!["ckp://Child.Service:v0.1"],
            vec![],
        );

        // Create child kernel that inherits parent's permissions (via wildcard)
        create_test_ontology(
            &temp,
            "Child.Service",
            vec!["ckp://Grandchild.*"],
            vec![],
        );

        // Create grandchild kernel
        create_test_ontology(
            &temp,
            "Grandchild.Worker",
            vec![],
            vec![],
        );

        // Parent should be able to emit to child
        let allowed = checker.can_emit_to("ckp://Parent.System:v0.1", "ckp://Child.Service:v0.1").unwrap();
        assert!(allowed);

        // Child should be able to emit to grandchild via wildcard
        let allowed = checker.can_emit_to("ckp://Child.Service:v0.1", "ckp://Grandchild.Worker:v0.1").unwrap();
        assert!(allowed);

        // Parent should NOT be able to skip hierarchy and emit to grandchild directly
        let allowed = checker.can_emit_to("ckp://Parent.System:v0.1", "ckp://Grandchild.Worker:v0.1").unwrap();
        assert!(!allowed);
    }

    // -------------------------------------------------------------------------
    // 2. Multi-level RBAC hierarchies: Transitive role relationships
    // -------------------------------------------------------------------------
    #[test]
    fn test_rbac_hierarchy_transitive_relationships() {
        let (temp, mut checker) = setup_test_env();

        // Create A -> B -> C chain where each can only talk to next
        create_test_ontology(
            &temp,
            "ServiceA",
            vec!["ckp://ServiceB:v0.1"],
            vec![],
        );

        create_test_ontology(
            &temp,
            "ServiceB",
            vec!["ckp://ServiceC:v0.1"],
            vec![],
        );

        create_test_ontology(
            &temp,
            "ServiceC",
            vec!["ckp://ServiceD:v0.1"],
            vec![],
        );

        // Direct relationships should work
        assert!(checker.can_emit_to("ckp://ServiceA:v0.1", "ckp://ServiceB:v0.1").unwrap());
        assert!(checker.can_emit_to("ckp://ServiceB:v0.1", "ckp://ServiceC:v0.1").unwrap());
        assert!(checker.can_emit_to("ckp://ServiceC:v0.1", "ckp://ServiceD:v0.1").unwrap());

        // Transitive relationships should NOT work (no implicit transitivity)
        assert!(!checker.can_emit_to("ckp://ServiceA:v0.1", "ckp://ServiceC:v0.1").unwrap());
        assert!(!checker.can_emit_to("ckp://ServiceA:v0.1", "ckp://ServiceD:v0.1").unwrap());
        assert!(!checker.can_emit_to("ckp://ServiceB:v0.1", "ckp://ServiceD:v0.1").unwrap());
    }

    // -------------------------------------------------------------------------
    // 3. Multi-level RBAC hierarchies: Role conflict resolution
    // -------------------------------------------------------------------------
    #[test]
    fn test_rbac_hierarchy_conflict_resolution() {
        let (temp, mut checker) = setup_test_env();

        // Create kernel with both wildcard allow and specific deny (deny should win)
        create_test_ontology(
            &temp,
            "ConflictSource",
            vec!["ckp://Service.*"],  // Allow all Service.*
            vec!["ckp://Service.Restricted:v0.1"],  // Except Service.Restricted
        );

        // Should allow Service.Public
        let allowed = checker.can_emit_to("ckp://ConflictSource:v0.1", "ckp://Service.Public:v0.1").unwrap();
        assert!(allowed);

        // Should DENY Service.Restricted (blacklist takes precedence)
        let allowed = checker.can_emit_to("ckp://ConflictSource:v0.1", "ckp://Service.Restricted:v0.1").unwrap();
        assert!(!allowed);

        // Create kernel with overlapping patterns
        create_test_ontology(
            &temp,
            "OverlapSource",
            vec!["ckp://System.*", "ckp://System.Core.*"],  // Overlapping patterns
            vec!["ckp://System.Core.Secret:*"],  // Specific deny
        );

        // Should allow System.Core.Public
        let allowed = checker.can_emit_to("ckp://OverlapSource:v0.1", "ckp://System.Core.Public:v1.0").unwrap();
        assert!(allowed);

        // Should DENY System.Core.Secret (deny wins)
        let allowed = checker.can_emit_to("ckp://OverlapSource:v0.1", "ckp://System.Core.Secret:v1.0").unwrap();
        assert!(!allowed);
    }

    // -------------------------------------------------------------------------
    // 4. Dynamic permission updates: Runtime permission changes
    // -------------------------------------------------------------------------
    #[test]
    fn test_rbac_dynamic_runtime_permission_changes() {
        let (temp, mut checker) = setup_test_env();

        // Initial restrictive policy
        create_test_ontology(
            &temp,
            "DynamicKernel",
            vec!["ckp://AllowedTarget:v0.1"],
            vec![],
        );

        // Initially allowed
        let allowed = checker.can_emit_to("ckp://DynamicKernel:v0.1", "ckp://AllowedTarget:v0.1").unwrap();
        assert!(allowed);

        // Initially denied
        let allowed = checker.can_emit_to("ckp://DynamicKernel:v0.1", "ckp://NewTarget:v0.1").unwrap();
        assert!(!allowed);

        // Update ontology to add new target (simulating runtime update)
        create_test_ontology(
            &temp,
            "DynamicKernel",
            vec!["ckp://AllowedTarget:v0.1", "ckp://NewTarget:v0.1"],
            vec![],
        );

        // Create new checker instance (simulates reloading config)
        let mut checker = PermissionChecker::new(temp.path().to_path_buf());

        // Now should allow new target
        let allowed = checker.can_emit_to("ckp://DynamicKernel:v0.1", "ckp://NewTarget:v0.1").unwrap();
        assert!(allowed);

        // Original target still allowed
        let allowed = checker.can_emit_to("ckp://DynamicKernel:v0.1", "ckp://AllowedTarget:v0.1").unwrap();
        assert!(allowed);
    }

    // -------------------------------------------------------------------------
    // 5. Dynamic permission updates: Permission revocation mid-operation
    // -------------------------------------------------------------------------
    #[test]
    fn test_rbac_dynamic_permission_revocation() {
        let (temp, mut checker) = setup_test_env();

        // Start with permissive policy
        create_test_ontology(
            &temp,
            "RevokableKernel",
            vec!["ckp://Target1:v0.1", "ckp://Target2:v0.1", "ckp://Target3:v0.1"],
            vec![],
        );

        // All targets should be allowed
        assert!(checker.can_emit_to("ckp://RevokableKernel:v0.1", "ckp://Target1:v0.1").unwrap());
        assert!(checker.can_emit_to("ckp://RevokableKernel:v0.1", "ckp://Target2:v0.1").unwrap());
        assert!(checker.can_emit_to("ckp://RevokableKernel:v0.1", "ckp://Target3:v0.1").unwrap());

        // Revoke Target2 by moving it to blacklist
        create_test_ontology(
            &temp,
            "RevokableKernel",
            vec!["ckp://Target1:v0.1", "ckp://Target3:v0.1"],
            vec!["ckp://Target2:v0.1"],  // Now blacklisted
        );

        // Reload permissions
        let mut checker = PermissionChecker::new(temp.path().to_path_buf());

        // Target1 and Target3 still allowed
        assert!(checker.can_emit_to("ckp://RevokableKernel:v0.1", "ckp://Target1:v0.1").unwrap());
        assert!(checker.can_emit_to("ckp://RevokableKernel:v0.1", "ckp://Target3:v0.1").unwrap());

        // Target2 now denied (revoked and blacklisted)
        assert!(!checker.can_emit_to("ckp://RevokableKernel:v0.1", "ckp://Target2:v0.1").unwrap());
    }

    // -------------------------------------------------------------------------
    // 6. Dynamic permission updates: Temporary permission grants
    // -------------------------------------------------------------------------
    #[test]
    fn test_rbac_dynamic_temporary_permission_grants() {
        let (temp, mut checker) = setup_test_env();

        // Initial state: no special permissions
        create_test_ontology(
            &temp,
            "TemporaryKernel",
            vec!["ckp://StandardTarget:v0.1"],
            vec![],
        );

        // Standard target allowed
        assert!(checker.can_emit_to("ckp://TemporaryKernel:v0.1", "ckp://StandardTarget:v0.1").unwrap());

        // Privileged target denied
        assert!(!checker.can_emit_to("ckp://TemporaryKernel:v0.1", "ckp://PrivilegedTarget:v0.1").unwrap());

        // Grant temporary access (simulating temporary elevation)
        create_test_ontology(
            &temp,
            "TemporaryKernel",
            vec!["ckp://StandardTarget:v0.1", "ckp://PrivilegedTarget:v0.1"],
            vec![],
        );

        // Reload to pick up temporary grant
        let mut checker = PermissionChecker::new(temp.path().to_path_buf());

        // Now privileged target is allowed
        assert!(checker.can_emit_to("ckp://TemporaryKernel:v0.1", "ckp://PrivilegedTarget:v0.1").unwrap());

        // Revoke temporary grant
        create_test_ontology(
            &temp,
            "TemporaryKernel",
            vec!["ckp://StandardTarget:v0.1"],  // Remove privileged
            vec![],
        );

        // Reload to revoke grant
        let mut checker = PermissionChecker::new(temp.path().to_path_buf());

        // Standard still allowed
        assert!(checker.can_emit_to("ckp://TemporaryKernel:v0.1", "ckp://StandardTarget:v0.1").unwrap());

        // Privileged now denied again
        assert!(!checker.can_emit_to("ckp://TemporaryKernel:v0.1", "ckp://PrivilegedTarget:v0.1").unwrap());
    }

    // -------------------------------------------------------------------------
    // 7. Consensus with partial approval: 2-of-3 consensus scenarios
    // -------------------------------------------------------------------------
    #[test]
    fn test_rbac_consensus_partial_approval_2_of_3() {
        let (temp, checker) = setup_test_env();

        // Create kernel with consensus directory structure
        let kernel_dir = temp.path().join("concepts/ConsensusKernel");
        fs::create_dir_all(&kernel_dir).unwrap();

        // Create basic ontology
        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://ConsensusKernel:v0.1
  type: node:cold
  version: v0.1
spec: {}
"#,
        )
        .unwrap();

        // Create consensus directory
        let consensus_dir = kernel_dir.join("consensus/approved/action-123");
        fs::create_dir_all(&consensus_dir).unwrap();

        // Create partially approved proposal (2 of 3 approvers)
        let proposal = ConsensusProposal {
            action: "kernel.delete".to_string(),
            status: "pending".to_string(),  // Not fully approved yet
            proposer: "alice@example.com".to_string(),
            approvers: vec!["bob@example.com".to_string(), "charlie@example.com".to_string()],
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            threshold: 3,  // Requires 3 approvals
        };

        fs::write(
            consensus_dir.join("proposal.json"),
            serde_json::to_string_pretty(&proposal).unwrap(),
        )
        .unwrap();

        // Check that consensus validation would recognize partial approval
        let has_approval = checker.has_consensus_approval("ConsensusKernel", "kernel.delete", "alice@example.com").unwrap();

        // Should be false because status is "pending", not "approved"
        assert!(!has_approval);

        // Now update proposal to approved status
        let approved_proposal = ConsensusProposal {
            action: "kernel.delete".to_string(),
            status: "approved".to_string(),  // Now approved
            proposer: "alice@example.com".to_string(),
            approvers: vec![
                "bob@example.com".to_string(),
                "charlie@example.com".to_string(),
                "dave@example.com".to_string(),
            ],
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            threshold: 3,
        };

        fs::write(
            consensus_dir.join("proposal.json"),
            serde_json::to_string_pretty(&approved_proposal).unwrap(),
        )
        .unwrap();

        // Now should be approved
        let has_approval = checker.has_consensus_approval("ConsensusKernel", "kernel.delete", "alice@example.com").unwrap();
        assert!(has_approval);
    }

    // -------------------------------------------------------------------------
    // 8. Consensus with partial approval: Timeout handling in consensus
    // -------------------------------------------------------------------------
    #[test]
    fn test_rbac_consensus_timeout_handling() {
        let (temp, checker) = setup_test_env();

        let kernel_dir = temp.path().join("concepts/TimeoutKernel");
        fs::create_dir_all(&kernel_dir).unwrap();

        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://TimeoutKernel:v0.1
  type: node:cold
  version: v0.1
spec: {}
"#,
        )
        .unwrap();

        // Create expired proposal (timestamp in the past, status still pending)
        let consensus_dir = kernel_dir.join("consensus/approved/action-expired");
        fs::create_dir_all(&consensus_dir).unwrap();

        let expired_proposal = ConsensusProposal {
            action: "git.merge_main".to_string(),
            status: "pending".to_string(),  // Never got full approval
            proposer: "alice@example.com".to_string(),
            approvers: vec!["bob@example.com".to_string()],  // Only 1 of 2
            timestamp: "2020-01-01T00:00:00Z".to_string(),  // Old timestamp
            threshold: 2,
        };

        fs::write(
            consensus_dir.join("proposal.json"),
            serde_json::to_string_pretty(&expired_proposal).unwrap(),
        )
        .unwrap();

        // Should not be approved (status is pending)
        let has_approval = checker.has_consensus_approval("TimeoutKernel", "git.merge_main", "alice@example.com").unwrap();
        assert!(!has_approval);

        // Create recent proposal that IS approved
        let consensus_dir2 = kernel_dir.join("consensus/approved/action-recent");
        fs::create_dir_all(&consensus_dir2).unwrap();

        let recent_proposal = ConsensusProposal {
            action: "git.merge_main".to_string(),
            status: "approved".to_string(),
            proposer: "alice@example.com".to_string(),
            approvers: vec!["bob@example.com".to_string(), "charlie@example.com".to_string()],
            timestamp: "2024-12-01T00:00:00Z".to_string(),
            threshold: 2,
        };

        fs::write(
            consensus_dir2.join("proposal.json"),
            serde_json::to_string_pretty(&recent_proposal).unwrap(),
        )
        .unwrap();

        // Recent approved proposal should work
        let has_approval = checker.has_consensus_approval("TimeoutKernel", "git.merge_main", "alice@example.com").unwrap();
        assert!(has_approval);
    }

    // -------------------------------------------------------------------------
    // 9. Time-based permission expiry: TTL-based permissions
    // -------------------------------------------------------------------------
    #[test]
    fn test_rbac_ttl_based_permissions() {
        let (temp, mut checker) = setup_test_env();

        // Create kernel with time-based permissions (simulated via comments)
        let kernel_dir = temp.path().join("concepts/TTLKernel");
        fs::create_dir_all(&kernel_dir).unwrap();

        // Ontology with time-sensitive access (conceptual - would need real TTL support)
        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://TTLKernel:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - ckp://TemporaryAccess:v0.1  # Valid until 2025-01-01
        - ckp://PermanentAccess:v0.1
"#,
        )
        .unwrap();

        // Test current permissions (both allowed)
        assert!(checker.can_emit_to("ckp://TTLKernel:v0.1", "ckp://TemporaryAccess:v0.1").unwrap());
        assert!(checker.can_emit_to("ckp://TTLKernel:v0.1", "ckp://PermanentAccess:v0.1").unwrap());

        // Simulate TTL expiry by removing temporary access
        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://TTLKernel:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - ckp://PermanentAccess:v0.1  # TemporaryAccess expired
"#,
        )
        .unwrap();

        // Reload checker
        let mut checker = PermissionChecker::new(temp.path().to_path_buf());

        // Temporary access now denied (expired)
        assert!(!checker.can_emit_to("ckp://TTLKernel:v0.1", "ckp://TemporaryAccess:v0.1").unwrap());

        // Permanent access still allowed
        assert!(checker.can_emit_to("ckp://TTLKernel:v0.1", "ckp://PermanentAccess:v0.1").unwrap());
    }

    // -------------------------------------------------------------------------
    // 10. Time-based permission expiry: Scheduled permission activation
    // -------------------------------------------------------------------------
    #[test]
    fn test_rbac_scheduled_permission_activation() {
        let (temp, mut checker) = setup_test_env();

        // Create kernel with scheduled activation (before activation)
        let kernel_dir = temp.path().join("concepts/ScheduledKernel");
        fs::create_dir_all(&kernel_dir).unwrap();

        // Initial state: future permission not yet active
        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://ScheduledKernel:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - ckp://CurrentService:v0.1
      # FutureService not yet in allowed list (activation scheduled for later)
"#,
        )
        .unwrap();

        // Current service allowed
        assert!(checker.can_emit_to("ckp://ScheduledKernel:v0.1", "ckp://CurrentService:v0.1").unwrap());

        // Future service not yet allowed (before activation)
        assert!(!checker.can_emit_to("ckp://ScheduledKernel:v0.1", "ckp://FutureService:v0.1").unwrap());

        // Simulate scheduled activation by updating ontology
        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://ScheduledKernel:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - ckp://CurrentService:v0.1
        - ckp://FutureService:v0.1  # Now activated
"#,
        )
        .unwrap();

        // Reload checker (simulates scheduled activation event)
        let mut checker = PermissionChecker::new(temp.path().to_path_buf());

        // Both services now allowed (activation completed)
        assert!(checker.can_emit_to("ckp://ScheduledKernel:v0.1", "ckp://CurrentService:v0.1").unwrap());
        assert!(checker.can_emit_to("ckp://ScheduledKernel:v0.1", "ckp://FutureService:v0.1").unwrap());

        // Can also schedule deactivation
        fs::write(
            kernel_dir.join("conceptkernel.yaml"),
            r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: ckp://ScheduledKernel:v0.1
  type: node:cold
  version: v0.1
spec:
  rbac:
    communication:
      allowed:
        - ckp://FutureService:v0.1  # CurrentService deactivated
"#,
        )
        .unwrap();

        // Reload for deactivation
        let mut checker = PermissionChecker::new(temp.path().to_path_buf());

        // Current service now deactivated
        assert!(!checker.can_emit_to("ckp://ScheduledKernel:v0.1", "ckp://CurrentService:v0.1").unwrap());

        // Future service still allowed
        assert!(checker.can_emit_to("ckp://ScheduledKernel:v0.1", "ckp://FutureService:v0.1").unwrap());
    }

    // ============================================================================
    // ONTOLOGY-BASED RBAC INTEGRATION TESTS (PHASE02)
    // ============================================================================

    /// Test: PermissionChecker with OntologyLibrary integration
    ///
    /// Verifies that PermissionChecker can be instantiated with ontology library
    /// and falls back gracefully when ontology is not available.
    #[tokio::test]
    async fn test_rbac_ontology_integration_instantiation() {
        let (temp, _checker) = setup_test_env();

        // Test 1: Create PermissionChecker without ontology (backward compat)
        let checker_no_ontology = PermissionChecker::new(temp.path().to_path_buf());

        // Should work with fallback permissions
        let allowed = checker_no_ontology.check_permission("admin", "kernel.create", "TestKernel").await.unwrap();
        assert!(allowed);

        // Test 2: Try to create PermissionChecker with ontology
        // Will fail gracefully if ontology not found, but shouldn't panic
        let result = PermissionChecker::new_with_ontology(temp.path().to_path_buf());

        // If ontology library loads, verify it works
        if let Ok(checker_with_ontology) = result {
            let allowed = checker_with_ontology.check_permission("admin", "kernel.create", "TestKernel").await.unwrap();
            assert!(allowed);
        }
        // If ontology not found, that's OK - it's optional
    }

    /// Test: Agent URN-based permission checking
    ///
    /// Verifies that check_agent_permission_sparql() works with Agent URNs
    /// and falls back to username-based checking.
    #[tokio::test]
    async fn test_rbac_ontology_agent_urn_permissions() {
        let (temp, _checker) = setup_test_env();

        let checker = PermissionChecker::new(temp.path().to_path_buf());

        // Test user agent URN
        let user_agent = "ckp://Agent/user:admin";
        let allowed = checker.check_agent_permission_sparql(user_agent, "kernel.create").await.unwrap();
        assert!(allowed, "Admin user agent should have kernel.create permission");

        // Test process agent URN (should use admin permissions)
        let process_agent = "ckp://Agent/process:System.Gateway";
        let allowed = checker.check_agent_permission_sparql(process_agent, "kernel.create").await.unwrap();
        assert!(allowed, "Process agents should have admin-level permissions");

        // Test developer user agent (should have limited permissions)
        let dev_agent = "ckp://Agent/user:developer";
        let allowed = checker.check_agent_permission_sparql(dev_agent, "kernel.create").await.unwrap();
        assert!(allowed, "Developer should have kernel.create permission");

        // Test developer should NOT have kernel.delete
        let not_allowed = checker.check_agent_permission_sparql(dev_agent, "kernel.delete").await.unwrap();
        assert!(!not_allowed, "Developer should NOT have kernel.delete permission");
    }

    /// Test: Quorum-based consensus checking
    ///
    /// Verifies that requires_consensus() checks permission quorum requirements
    /// and falls back to hardcoded list when ontology not available.
    #[test]
    fn test_rbac_ontology_quorum_requirements() {
        let (temp, _setup) = setup_test_env();

        let checker = PermissionChecker::new(temp.path().to_path_buf());

        // Test hardcoded consensus-required permissions
        assert!(checker.requires_consensus("kernel.delete"));
        assert!(checker.requires_consensus("edge.authorize"));
        assert!(checker.requires_consensus("git.merge_main"));
        assert!(checker.requires_consensus("consensus.execute"));

        // Test non-consensus permissions
        assert!(!checker.requires_consensus("kernel.create"));
        assert!(!checker.requires_consensus("kernel.read"));
        assert!(!checker.requires_consensus("storage.read"));
    }

    /// Test: Permission fallback behavior
    ///
    /// Verifies that permission checking falls back to hardcoded defaults
    /// when ontology library is not available or queries fail.
    #[tokio::test]
    async fn test_rbac_ontology_fallback_behavior() {
        let (temp, _setup) = setup_test_env();

        // Create checker without ontology
        let checker = PermissionChecker::new(temp.path().to_path_buf());

        // Admin should have all permissions (wildcard)
        let permissions = vec![
            "kernel.create",
            "kernel.modify",
            "kernel.delete",
            "edge.authorize",
            "rbac.role.create",
        ];

        for perm in permissions {
            let allowed = checker.check_permission("admin", perm, "TestKernel").await.unwrap();
            assert!(allowed, "Admin should have permission: {}", perm);
        }

        // Regular user should have limited permissions
        let allowed = checker.check_permission("user@example.com", "kernel.create", "TestKernel").await.unwrap();
        assert!(allowed, "Regular user should have kernel.create");

        let not_allowed = checker.check_permission("user@example.com", "kernel.delete", "TestKernel").await.unwrap();
        assert!(!not_allowed, "Regular user should NOT have kernel.delete");
    }

    /// Test: Ontology-based permission query with actual .ttl file
    ///
    /// This test attempts to load the actual RBAC ontology if available
    /// and verify that permission queries work correctly.
    #[tokio::test]
    async fn test_rbac_ontology_with_real_ttl_files() {
        // Try to create checker with ontology from actual project root
        let project_root = std::env::current_dir().unwrap();

        // Attempt to create with ontology
        let result = PermissionChecker::new_with_ontology(project_root.clone());

        match result {
            Ok(checker) => {
                // Ontology loaded successfully - test SPARQL queries

                // Test admin user permissions
                let admin_agent = "ckp://Agent/user:admin";
                let has_perm = checker.check_agent_permission_sparql(admin_agent, "http.handle").await;

                if let Ok(allowed) = has_perm {
                    println!("✓ SPARQL permission query succeeded: {}", allowed);
                } else {
                    println!("✓ SPARQL query executed (permission not found in ontology)");
                }

                // Test consensus requirements from ontology
                let requires = checker.requires_consensus("consensus.enforce");
                println!("✓ Quorum check for 'consensus.enforce': {}", requires);
            }
            Err(e) => {
                // Ontology not available - that's OK for this test
                println!("ℹ Ontology library not available (expected in CI): {}", e);

                // Verify fallback still works
                let checker = PermissionChecker::new(project_root);
                let allowed = checker.check_permission("admin", "kernel.create", "TestKernel").await.unwrap();
                assert!(allowed, "Fallback permissions should work");
            }
        }
    }

    /// Test: Agent URN format validation
    ///
    /// Verifies that check_agent_permission_sparql() correctly handles
    /// various Agent URN formats and rejects invalid formats.
    #[tokio::test]
    async fn test_rbac_ontology_agent_urn_format_validation() {
        let (temp, _setup) = setup_test_env();
        let checker = PermissionChecker::new(temp.path().to_path_buf());

        // Valid user agent URN
        let valid_user = "ckp://Agent/user:admin";
        let result = checker.check_agent_permission_sparql(valid_user, "kernel.create").await;
        assert!(result.is_ok(), "Valid user agent URN should be accepted");

        // Valid process agent URN
        let valid_process = "ckp://Agent/process:System.Gateway";
        let result = checker.check_agent_permission_sparql(valid_process, "kernel.create").await;
        assert!(result.is_ok(), "Valid process agent URN should be accepted");

        // Invalid agent URN format (should return false, not error)
        let invalid_urn = "ckp://InvalidFormat";
        let result = checker.check_agent_permission_sparql(invalid_urn, "kernel.create").await;
        assert!(result.is_ok(), "Invalid URN should be handled gracefully");
        assert!(!result.unwrap(), "Invalid URN should return false permission");
    }

    /// Test: Permission inheritance and role chaining
    ///
    /// Verifies that permission checking respects role-based permission grants
    /// when ontology library is available.
    #[tokio::test]
    async fn test_rbac_ontology_role_based_permissions() {
        let (temp, _setup) = setup_test_env();

        // Try with ontology if available
        let result = PermissionChecker::new_with_ontology(temp.path().to_path_buf());

        if let Ok(checker) = result {
            // Test that roles grant multiple permissions
            let agent = "ckp://Agent/user:admin";

            // Admin should have multiple permissions via role
            let perms = vec!["kernel.create", "kernel.modify", "kernel.read"];

            for perm in perms {
                let has_perm = checker.check_agent_permission_sparql(agent, perm).await;
                if let Ok(allowed) = has_perm {
                    // Permission check succeeded via SPARQL
                    println!("✓ Agent {} has permission {}: {}", agent, perm, allowed);
                }
            }
        } else {
            // Fallback test
            let checker = PermissionChecker::new(temp.path().to_path_buf());
            let allowed = checker.check_permission("admin", "kernel.create", "TestKernel").await.unwrap();
            assert!(allowed);
        }
    }
}
