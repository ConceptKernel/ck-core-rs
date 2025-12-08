//! URN validation for ConceptKernel v1.3.12

use crate::errors::{CkpError, Result};
use crate::urn::resolver::UrnResolver;
use regex::Regex;

/// Validation result containing errors if invalid
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
        }
    }

    pub fn with_error(error: String) -> Self {
        Self {
            valid: false,
            errors: vec![error],
        }
    }

    pub fn add_error(&mut self, error: String) {
        self.valid = false;
        self.errors.push(error);
    }

    pub fn merge(&mut self, other: ValidationResult) {
        if !other.valid {
            self.valid = false;
            self.errors.extend(other.errors);
        }
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// URN validator for ConceptKernel
pub struct UrnValidator;

impl UrnValidator {
    /// Validate a kernel or edge URN
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnValidator;
    ///
    /// let result = UrnValidator::validate("ckp://Recipes.BakeCake:v0.1");
    /// assert!(result.valid);
    /// assert!(result.errors.is_empty());
    ///
    /// let result = UrnValidator::validate("invalid-urn");
    /// assert!(!result.valid);
    /// assert!(!result.errors.is_empty());
    /// ```
    pub fn validate(urn: &str) -> ValidationResult {
        // Check if URN is provided
        if urn.is_empty() {
            return ValidationResult::with_error("URN must be a non-empty string".to_string());
        }

        // Check protocol
        if !urn.starts_with("ckp://") {
            return ValidationResult::with_error(format!(
                "Invalid protocol. URN must start with 'ckp://', got: {}",
                urn
            ));
        }

        // Determine if edge or kernel URN
        if UrnResolver::is_edge_urn(urn) {
            Self::validate_edge_urn(urn)
        } else {
            Self::validate_kernel_urn(urn)
        }
    }

    /// Validate a kernel URN
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnValidator;
    ///
    /// let result = UrnValidator::validate_kernel_urn("ckp://Recipes.BakeCake:v0.1");
    /// assert!(result.valid);
    /// ```
    pub fn validate_kernel_urn(urn: &str) -> ValidationResult {
        let mut result = ValidationResult::new();

        match UrnResolver::parse(urn) {
            Ok(parsed) => {
                // Validate kernel name
                if !Self::is_valid_kernel_name(&parsed.kernel) {
                    result.add_error(format!(
                        "Invalid kernel name: {}. Must contain only letters, numbers, dots, and hyphens.",
                        parsed.kernel
                    ));
                }

                // Validate version
                if !Self::is_valid_version(&parsed.version) {
                    result.add_error(format!(
                        "Invalid version format: {}. Expected format: v[major].[minor] (e.g., v0.1, v1.3.12)",
                        parsed.version
                    ));
                }

                // Validate stage if present
                if let Some(ref stage) = parsed.stage {
                    if !Self::is_valid_stage(stage) {
                        result.add_error(format!(
                            "Invalid stage: {}. Valid stages: inbox, staging, ready, storage, archive, tx, consensus, edges",
                            stage
                        ));
                    }
                }
            }
            Err(e) => {
                result.add_error(e.to_string());
            }
        }

        result
    }

    /// Validate an edge URN
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnValidator;
    ///
    /// let result = UrnValidator::validate_edge_urn(
    ///     "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12"
    /// );
    /// assert!(result.valid);
    /// ```
    pub fn validate_edge_urn(edge_urn: &str) -> ValidationResult {
        let mut result = ValidationResult::new();

        match UrnResolver::parse_edge_urn(edge_urn) {
            Err(e) => {
                result.add_error(format!("Failed to parse edge URN: {}", e));
                return result;
            }
            Ok(parsed) => {
                // Validate predicate
                if !Self::is_valid_predicate(&parsed.predicate) {
                    result.add_error(format!(
                        "Invalid predicate: {}. Valid predicates: PRODUCES, REQUIRES, VALIDATES, INFLUENCES, TRANSFORMS, LLM_ASSIST, ANNOUNCES, LINKS_IDENTITY",
                        parsed.predicate
                    ));
                }

                // Validate source kernel name
                if !Self::is_valid_kernel_name(&parsed.source) {
                    result.add_error(format!("Invalid source kernel name: {}", parsed.source));
                }

                // Validate target kernel name
                if !Self::is_valid_kernel_name(&parsed.target) {
                    result.add_error(format!("Invalid target kernel name: {}", parsed.target));
                }

                // Validate version (if present)
                if let Some(ref version) = parsed.version {
                    if !Self::is_valid_version(version) {
                        result.add_error(format!("Invalid version format: {}", version));
                    }
                }
            }
            Err(e) => {
                result.add_error(e.to_string());
            }
        }

        result
    }

    /// Check if kernel name is valid
    ///
    /// Rules:
    /// - Can contain: letters, numbers, dots, hyphens
    /// - Cannot start or end with dot or hyphen
    /// - Must be at least 1 character
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnValidator;
    ///
    /// assert!(UrnValidator::is_valid_kernel_name("Recipes.BakeCake"));
    /// assert!(UrnValidator::is_valid_kernel_name("System.Gateway.HTTP"));
    /// assert!(!UrnValidator::is_valid_kernel_name(".Invalid"));
    /// assert!(!UrnValidator::is_valid_kernel_name("Invalid-"));
    /// ```
    pub fn is_valid_kernel_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }

        // Regex: must start with letter, can contain alphanumeric, dots, hyphens (not at start/end)
        let re = Regex::new(r"^[a-zA-Z]+([.-]?[a-zA-Z0-9]+)*$").unwrap();
        re.is_match(name)
    }

    /// Check if version format is valid
    ///
    /// Valid formats:
    /// - v0.1
    /// - v1.0
    /// - v1.3.12
    /// - v2.0.0
    /// - Also accepts without 'v' prefix: 0.1, 1.0, etc.
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnValidator;
    ///
    /// assert!(UrnValidator::is_valid_version("v0.1"));
    /// assert!(UrnValidator::is_valid_version("v1.3.12"));
    /// assert!(UrnValidator::is_valid_version("0.1"));
    /// assert!(!UrnValidator::is_valid_version("invalid"));
    /// ```
    pub fn is_valid_version(version: &str) -> bool {
        if version.is_empty() {
            return false;
        }

        // Regex: (v)?[major].[minor](.[patch])? - accepts both with and without 'v' prefix
        let re = Regex::new(r"^v?\d+\.\d+(\.\d+)?$").unwrap();
        re.is_match(version)
    }

    /// Check if stage name is valid
    ///
    /// Valid stages: inbox, staging, ready, storage, archive, tx, consensus, edges
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnValidator;
    ///
    /// assert!(UrnValidator::is_valid_stage("inbox"));
    /// assert!(UrnValidator::is_valid_stage("storage"));
    /// assert!(!UrnValidator::is_valid_stage("invalid"));
    /// ```
    pub fn is_valid_stage(stage: &str) -> bool {
        matches!(
            stage,
            "inbox" | "staging" | "ready" | "storage" | "archive" | "tx" | "consensus" | "edges"
        )
    }

    /// Check if predicate is valid
    ///
    /// Valid predicates: PRODUCES, REQUIRES, VALIDATES, INFLUENCES, TRANSFORMS, LLM_ASSIST, ANNOUNCES, LINKS_IDENTITY
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnValidator;
    ///
    /// assert!(UrnValidator::is_valid_predicate("PRODUCES"));
    /// assert!(UrnValidator::is_valid_predicate("VALIDATES"));
    /// assert!(!UrnValidator::is_valid_predicate("INVALID"));
    /// ```
    pub fn is_valid_predicate(predicate: &str) -> bool {
        matches!(
            predicate,
            "PRODUCES" | "REQUIRES" | "VALIDATES" | "INFLUENCES" | "TRANSFORMS" | "LLM_ASSIST" | "ANNOUNCES" | "LINKS_IDENTITY"
        )
    }

    /// Validate URN and return error if invalid
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnValidator;
    ///
    /// assert!(UrnValidator::assert_valid("ckp://Recipes.BakeCake:v0.1").is_ok());
    /// assert!(UrnValidator::assert_valid("invalid").is_err());
    /// ```
    pub fn assert_valid(urn: &str) -> Result<()> {
        let result = Self::validate(urn);

        if !result.valid {
            return Err(CkpError::UrnValidation(result.errors.join(", ")));
        }

        Ok(())
    }

    /// Validate edge URN and return error if invalid
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnValidator;
    ///
    /// let result = UrnValidator::assert_valid_edge_urn(
    ///     "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12"
    /// );
    /// assert!(result.is_ok());
    /// ```
    pub fn assert_valid_edge_urn(edge_urn: &str) -> Result<()> {
        let result = Self::validate_edge_urn(edge_urn);

        if !result.valid {
            return Err(CkpError::UrnValidation(result.errors.join(", ")));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_kernel_urn() {
        let result = UrnValidator::validate("ckp://Recipes.BakeCake:v0.1");
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_invalid_urn() {
        let result = UrnValidator::validate("invalid-urn");
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_validate_empty_urn() {
        let result = UrnValidator::validate("");
        assert!(!result.valid);
    }

    #[test]
    fn test_validate_valid_edge_urn() {
        let result = UrnValidator::validate_edge_urn(
            "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12"
        );
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validate_invalid_edge_urn() {
        let result = UrnValidator::validate_edge_urn("ckp://Edge.INVALID:v1.0");
        assert!(!result.valid);
    }

    #[test]
    fn test_is_valid_kernel_name() {
        assert!(UrnValidator::is_valid_kernel_name("Recipes.BakeCake"));
        assert!(UrnValidator::is_valid_kernel_name("System.Gateway.HTTP"));
        assert!(UrnValidator::is_valid_kernel_name("Simple"));
        assert!(!UrnValidator::is_valid_kernel_name(".Invalid"));
        assert!(!UrnValidator::is_valid_kernel_name("Invalid-"));
        assert!(!UrnValidator::is_valid_kernel_name(""));
    }

    #[test]
    fn test_is_valid_version() {
        assert!(UrnValidator::is_valid_version("v0.1"));
        assert!(UrnValidator::is_valid_version("v1.3.12"));
        assert!(UrnValidator::is_valid_version("0.1"));
        assert!(UrnValidator::is_valid_version("1.0"));
        assert!(!UrnValidator::is_valid_version("invalid"));
        assert!(!UrnValidator::is_valid_version(""));
    }

    #[test]
    fn test_is_valid_stage() {
        assert!(UrnValidator::is_valid_stage("inbox"));
        assert!(UrnValidator::is_valid_stage("storage"));
        assert!(UrnValidator::is_valid_stage("edges"));
        assert!(!UrnValidator::is_valid_stage("invalid"));
    }

    #[test]
    fn test_is_valid_predicate() {
        assert!(UrnValidator::is_valid_predicate("PRODUCES"));
        assert!(UrnValidator::is_valid_predicate("VALIDATES"));
        assert!(UrnValidator::is_valid_predicate("LLM_ASSIST"));
        assert!(!UrnValidator::is_valid_predicate("INVALID"));
    }

    #[test]
    fn test_assert_valid() {
        assert!(UrnValidator::assert_valid("ckp://Recipes.BakeCake:v0.1").is_ok());
        assert!(UrnValidator::assert_valid("invalid").is_err());
    }

    #[test]
    fn test_assert_valid_edge_urn() {
        assert!(UrnValidator::assert_valid_edge_urn(
            "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12"
        ).is_ok());
        assert!(UrnValidator::assert_valid_edge_urn("ckp://Edge.INVALID:v1.0").is_err());
    }

    // NEW TESTS - Test Parity with Node.js

    /// Test: validate() - valid URN with stage
    /// Node.js equivalent: UrnValidator.test.js:38
    #[test]
    fn test_validate_urn_with_stage() {
        let result = UrnValidator::validate("ckp://Recipes.BakeCake:v0.1#inbox");
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    /// Test: validate() - valid URN with stage and path
    /// Node.js equivalent: UrnValidator.test.js:47
    #[test]
    fn test_validate_urn_with_stage_and_path() {
        let result = UrnValidator::validate("ckp://Recipes.BakeCake:v0.1#storage/123.inst");
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    /// Test: validate() - invalid protocol
    /// Node.js equivalent: UrnValidator.test.js:56
    #[test]
    fn test_validate_invalid_protocol() {
        let result = UrnValidator::validate("http://Recipes.BakeCake:v0.1");
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("protocol") || e.contains("format")));
    }

    /// Test: validate() - missing version
    /// Node.js equivalent: UrnValidator.test.js:65
    #[test]
    fn test_validate_missing_version() {
        let result = UrnValidator::validate("ckp://Recipes.BakeCake");
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    /// Test: validate() - invalid kernel name starts with number
    /// Node.js equivalent: UrnValidator.test.js:74
    #[test]
    fn test_validate_kernel_starts_with_number() {
        let result = UrnValidator::validate("ckp://123Invalid:v0.1");
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("kernel name")));
    }

    /// Test: validate() - invalid kernel name special chars
    /// Node.js equivalent: UrnValidator.test.js:83
    #[test]
    fn test_validate_kernel_special_chars() {
        let result = UrnValidator::validate("ckp://Invalid@Name:v0.1");
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("kernel name")));
    }

    /// Test: validate() - invalid version format
    /// Node.js equivalent: UrnValidator.test.js:92
    #[test]
    fn test_validate_invalid_version_format() {
        let result = UrnValidator::validate("ckp://Recipes.BakeCake:invalid");
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("version")));
    }

    /// Test: validate() - invalid stage name
    /// Node.js equivalent: UrnValidator.test.js:101
    #[test]
    fn test_validate_invalid_stage() {
        let result = UrnValidator::validate("ckp://Recipes.BakeCake:v0.1#invalid_stage");
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("stage")));
    }

    /// Test: validateEdgeUrn() - valid REQUIRES predicate
    /// Node.js equivalent: UrnValidator.test.js:119
    #[test]
    fn test_validate_edge_urn_requires_predicate() {
        let result = UrnValidator::validate_edge_urn("ckp://Edge.REQUIRES.Source-to-Target:v0.1");
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    /// Test: validateEdgeUrn() - invalid predicate
    /// Node.js equivalent: UrnValidator.test.js:127
    #[test]
    fn test_validate_edge_urn_invalid_predicate() {
        let result = UrnValidator::validate_edge_urn("ckp://Edge.INVALID_PRED.Source-to-Target:v0.1");
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("predicate")));
    }

    /// Test: validateEdgeUrn() - missing Edge prefix
    /// Node.js equivalent: UrnValidator.test.js:136
    #[test]
    fn test_validate_edge_urn_missing_prefix() {
        let result = UrnValidator::validate_edge_urn("ckp://PRODUCES.Source-to-Target:v0.1");
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    /// Test: validateEdgeUrn() - invalid source kernel
    /// Node.js equivalent: UrnValidator.test.js:145
    #[test]
    fn test_validate_edge_urn_invalid_source() {
        let result = UrnValidator::validate_edge_urn("ckp://Edge.PRODUCES.123Invalid-to-Target:v0.1");
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("source")));
    }

    /// Test: validateEdgeUrn() - invalid target kernel
    /// Node.js equivalent: UrnValidator.test.js:154
    #[test]
    fn test_validate_edge_urn_invalid_target() {
        let result = UrnValidator::validate_edge_urn("ckp://Edge.PRODUCES.Source-to-456Invalid:v0.1");
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("target")));
    }

    // isValidKernelName() comprehensive tests

    /// Test: isValidKernelName() - valid simple name
    /// Node.js equivalent: UrnValidator.test.js:163
    #[test]
    fn test_is_valid_kernel_name_simple() {
        assert!(UrnValidator::is_valid_kernel_name("BakeCake"));
    }

    /// Test: isValidKernelName() - valid dotted name
    /// Node.js equivalent: UrnValidator.test.js:168
    #[test]
    fn test_is_valid_kernel_name_dotted() {
        assert!(UrnValidator::is_valid_kernel_name("Recipes.BakeCake"));
    }

    /// Test: isValidKernelName() - valid multi-dotted name
    /// Node.js equivalent: UrnValidator.test.js:173
    #[test]
    fn test_is_valid_kernel_name_multi_dotted() {
        assert!(UrnValidator::is_valid_kernel_name("System.Gateway.HTTP"));
    }

    /// Test: isValidKernelName() - valid with dashes
    /// Node.js equivalent: UrnValidator.test.js:178
    #[test]
    fn test_is_valid_kernel_name_with_dashes() {
        assert!(UrnValidator::is_valid_kernel_name("Multi-Word-Kernel"));
    }

    /// Test: isValidKernelName() - invalid starts with number
    /// Node.js equivalent: UrnValidator.test.js:183
    #[test]
    fn test_is_valid_kernel_name_starts_with_number() {
        assert!(!UrnValidator::is_valid_kernel_name("123Invalid"));
    }

    /// Test: isValidKernelName() - invalid special chars
    /// Node.js equivalent: UrnValidator.test.js:188
    #[test]
    fn test_is_valid_kernel_name_special_chars() {
        assert!(!UrnValidator::is_valid_kernel_name("Invalid@Name"));
    }

    /// Test: isValidKernelName() - invalid starts with dot
    /// Node.js equivalent: UrnValidator.test.js:193
    #[test]
    fn test_is_valid_kernel_name_starts_with_dot() {
        assert!(!UrnValidator::is_valid_kernel_name(".InvalidName"));
    }

    /// Test: isValidKernelName() - invalid ends with dot
    /// Node.js equivalent: UrnValidator.test.js:198
    #[test]
    fn test_is_valid_kernel_name_ends_with_dot() {
        assert!(!UrnValidator::is_valid_kernel_name("InvalidName."));
    }

    // isValidVersion() comprehensive tests

    /// Test: isValidVersion() - valid v-prefix versions
    /// Node.js equivalent: UrnValidator.test.js:203
    #[test]
    fn test_is_valid_version_v_prefix() {
        assert!(UrnValidator::is_valid_version("v0.1"));
        assert!(UrnValidator::is_valid_version("v1.0.0"));
        assert!(UrnValidator::is_valid_version("v1.3.12"));
    }

    /// Test: isValidVersion() - valid semver without v
    /// Node.js equivalent: UrnValidator.test.js:210
    #[test]
    fn test_is_valid_version_no_v_prefix() {
        assert!(UrnValidator::is_valid_version("0.1.0"));
        assert!(UrnValidator::is_valid_version("1.3.12"));
    }

    /// Test: isValidVersion() - invalid versions
    /// Node.js equivalent: UrnValidator.test.js:216
    #[test]
    fn test_is_valid_version_invalid_formats() {
        assert!(!UrnValidator::is_valid_version("invalid"));
        assert!(!UrnValidator::is_valid_version("v.1"));
        assert!(!UrnValidator::is_valid_version("1"));
    }

    // isValidStage() comprehensive tests

    /// Test: isValidStage() - all valid stages
    /// Node.js equivalent: UrnValidator.test.js:223
    #[test]
    fn test_is_valid_stage_all_valid() {
        let valid_stages = ["inbox", "staging", "ready", "storage", "archive", "tx", "edges"];
        for stage in &valid_stages {
            assert!(UrnValidator::is_valid_stage(stage), "{} should be valid", stage);
        }
    }

    /// Test: isValidStage() - invalid stages
    /// Node.js equivalent: UrnValidator.test.js:232
    #[test]
    fn test_is_valid_stage_invalid() {
        assert!(!UrnValidator::is_valid_stage("invalid"));
        assert!(!UrnValidator::is_valid_stage("INBOX")); // case sensitive
        assert!(!UrnValidator::is_valid_stage("outbox"));
    }

    // isValidPredicate() comprehensive tests

    /// Test: isValidPredicate() - all valid predicates
    /// Node.js equivalent: UrnValidator.test.js:239
    #[test]
    fn test_is_valid_predicate_all_valid() {
        let valid_predicates = ["PRODUCES", "REQUIRES", "INFLUENCES", "TRANSFORMS"];
        for pred in &valid_predicates {
            assert!(UrnValidator::is_valid_predicate(pred), "{} should be valid", pred);
        }
    }

    /// Test: isValidPredicate() - invalid predicates
    /// Node.js equivalent: UrnValidator.test.js:248
    #[test]
    fn test_is_valid_predicate_invalid() {
        assert!(!UrnValidator::is_valid_predicate("INVALID"));
        assert!(!UrnValidator::is_valid_predicate("produces")); // case sensitive
        assert!(!UrnValidator::is_valid_predicate("CREATES"));
    }
}
