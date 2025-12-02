//! URN resolver for parsing and resolving ckp:// URIs to filesystem paths

use crate::errors::{CkpError, Result};
use regex::Regex;
use std::path::{Path, PathBuf};

/// Parsed kernel URN components
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedUrn {
    pub kernel: String,
    pub version: String,
    pub stage: Option<String>,
    pub path: Option<String>,
}

/// Parsed edge URN components
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedEdgeUrn {
    pub predicate: String,
    pub source: String,
    pub target: String,
    pub version: String,
    pub queue_path: String,
}

/// Agent type for parsed Agent URNs
#[derive(Debug, Clone, PartialEq)]
pub enum AgentType {
    User(String),       // User agent: ckp://Agent/user:{username}
    Process(String),    // Process agent: ckp://Agent/process:{KernelName}
}

/// Parsed agent URN components
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedAgentUrn {
    pub agent_type: AgentType,
    pub identifier: String,  // username or kernel name
}

/// URN resolver for parsing and resolving ckp:// URIs
pub struct UrnResolver;

impl UrnResolver {
    /// Parse ckp:// URN into components
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// let parsed = UrnResolver::parse("ckp://Recipes.BakeCake:v0.1#storage/tx-123.inst").unwrap();
    /// assert_eq!(parsed.kernel, "Recipes.BakeCake");
    /// assert_eq!(parsed.version, "v0.1");
    /// assert_eq!(parsed.stage, Some("storage".to_string()));
    /// assert_eq!(parsed.path, Some("tx-123.inst".to_string()));
    /// ```
    pub fn parse(urn: &str) -> Result<ParsedUrn> {
        if urn.is_empty() {
            return Err(CkpError::UrnParse("URN must be a non-empty string".to_string()));
        }

        // URN regex: ckp://[kernel]:[version]#[stage]/[path]
        // Groups: (kernel)(version)(stage?)(path?)
        let re = Regex::new(r"^ckp://([^:]+):([^#]+)(?:#([^/]+)(?:/(.+))?)?$")
            .map_err(|e| CkpError::UrnParse(format!("Regex error: {}", e)))?;

        let caps = re
            .captures(urn)
            .ok_or_else(|| CkpError::InvalidUrnFormat(urn.to_string()))?;

        Ok(ParsedUrn {
            kernel: caps.get(1).unwrap().as_str().to_string(),
            version: caps.get(2).unwrap().as_str().to_string(),
            stage: caps.get(3).map(|m| m.as_str().to_string()),
            path: caps.get(4).map(|m| m.as_str().to_string()),
        })
    }

    /// Resolve URN to absolute filesystem path
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    /// use std::path::Path;
    ///
    /// let path = UrnResolver::resolve_to_path(
    ///     "ckp://Recipes.BakeCake:v0.1#inbox",
    ///     Path::new("/test/concepts")
    /// ).unwrap();
    /// assert_eq!(path, Path::new("/test/concepts/Recipes.BakeCake/queue/inbox"));
    /// ```
    pub fn resolve_to_path(urn: &str, concepts_root: &Path) -> Result<PathBuf> {
        let parsed = Self::parse(urn)?;
        let base_path = concepts_root.join(&parsed.kernel);

        // If no stage specified, return kernel root
        if parsed.stage.is_none() {
            return Ok(base_path);
        }

        let stage = parsed.stage.as_ref().unwrap();

        // Map stage shortcuts to actual filesystem paths
        let stage_path = match stage.as_str() {
            "inbox" => "queue/inbox",
            "staging" => "queue/staging",
            "ready" => "queue/ready",
            "storage" => "storage",
            "archive" => "archive",
            "tx" => "tx",
            "consensus" => "consensus",
            "edges" => "queue/edges",
            _ => {
                return Err(CkpError::InvalidStage(format!(
                    "Unknown stage: {}. Valid stages: inbox, staging, ready, storage, archive, tx, consensus, edges",
                    stage
                )));
            }
        };

        let mut full_path = base_path.join(stage_path);

        // Append specific path if provided
        if let Some(ref urn_path) = parsed.path {
            full_path = full_path.join(urn_path);
        }

        Ok(full_path)
    }

    /// Build URN from components
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::{UrnResolver, ParsedUrn};
    ///
    /// let urn = UrnResolver::build(&ParsedUrn {
    ///     kernel: "Recipes.BakeCake".to_string(),
    ///     version: "v0.1".to_string(),
    ///     stage: Some("storage".to_string()),
    ///     path: Some("tx-123.inst".to_string()),
    /// });
    /// assert_eq!(urn, "ckp://Recipes.BakeCake:v0.1#storage/tx-123.inst");
    /// ```
    pub fn build(components: &ParsedUrn) -> String {
        let mut urn = format!("ckp://{}:{}", components.kernel, components.version);

        if let Some(ref stage) = components.stage {
            urn.push('#');
            urn.push_str(stage);

            if let Some(ref path) = components.path {
                urn.push('/');
                urn.push_str(path);
            }
        }

        urn
    }

    /// Resolve stage shortcut to full URN
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// let urn = UrnResolver::resolve_stage("ckp://Recipes.BakeCake:v0.1", "inbox").unwrap();
    /// assert_eq!(urn, "ckp://Recipes.BakeCake:v0.1#inbox");
    /// ```
    pub fn resolve_stage(urn: &str, stage: &str) -> Result<String> {
        let parsed = Self::parse(urn)?;
        Ok(format!("ckp://{}:{}#{}", parsed.kernel, parsed.version, stage))
    }

    /// Check if URN is an edge URN
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// assert!(UrnResolver::is_edge_urn("ckp://Edge.PRODUCES.A-to-B:v1.0"));
    /// assert!(!UrnResolver::is_edge_urn("ckp://Recipes.BakeCake:v0.1"));
    /// ```
    pub fn is_edge_urn(urn: &str) -> bool {
        urn.starts_with("ckp://Edge.")
    }

    /// Check if URN is a kernel URN
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// assert!(UrnResolver::is_kernel_urn("ckp://Recipes.BakeCake:v0.1"));
    /// assert!(!UrnResolver::is_kernel_urn("ckp://Edge.PRODUCES.A-to-B:v1.0"));
    /// ```
    pub fn is_kernel_urn(urn: &str) -> bool {
        urn.starts_with("ckp://") && !urn.starts_with("ckp://Edge.")
    }

    /// Parse edge URN into components
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// let parsed = UrnResolver::parse_edge_urn(
    ///     "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12"
    /// ).unwrap();
    /// assert_eq!(parsed.predicate, "PRODUCES");
    /// assert_eq!(parsed.source, "MixIngredients");
    /// assert_eq!(parsed.target, "BakeCake");
    /// assert_eq!(parsed.version, "v1.3.12");
    /// assert_eq!(parsed.queue_path, "queue/edges/PRODUCES.MixIngredients");
    /// ```
    pub fn parse_edge_urn(edge_urn: &str) -> Result<ParsedEdgeUrn> {
        if edge_urn.is_empty() {
            return Err(CkpError::UrnParse("Edge URN must be a non-empty string".to_string()));
        }

        // Edge URN regex: ckp://Edge.[PREDICATE].[Source]-to-[Target]:[version]
        // Groups: (predicate)(source)(target)(version)
        let re = Regex::new(r"^ckp://Edge\.([^.]+)\.([^-]+)-to-([^:]+):(.+)$")
            .map_err(|e| CkpError::UrnParse(format!("Regex error: {}", e)))?;

        let caps = re
            .captures(edge_urn)
            .ok_or_else(|| CkpError::InvalidEdgeUrn(edge_urn.to_string()))?;

        let predicate = caps.get(1).unwrap().as_str().to_string();
        let source = caps.get(2).unwrap().as_str().to_string();
        let target = caps.get(3).unwrap().as_str().to_string();
        let version = caps.get(4).unwrap().as_str().to_string();

        // Derive queue path from predicate and source
        let queue_path = format!("queue/edges/{}.{}", predicate, source);

        Ok(ParsedEdgeUrn {
            predicate,
            source,
            target,
            version,
            queue_path,
        })
    }

    /// Extract transaction ID from URN path
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// let tx_id = UrnResolver::extract_tx_id(
    ///     "ckp://Recipes.BakeCake:v0.1#storage/tx-123.inst"
    /// ).unwrap();
    /// assert_eq!(tx_id, "tx-123");
    /// ```
    pub fn extract_tx_id(urn: &str) -> Option<String> {
        let parsed = Self::parse(urn).ok()?;
        let urn_path = parsed.path?;

        // Extract filename from path
        let path = Path::new(&urn_path);
        let filename = path.file_name()?.to_str()?;

        // Extract txId from filename (match anything before the file extension)
        let re = Regex::new(r"^([^.]+)").ok()?;
        let caps = re.captures(filename)?;

        Some(caps.get(1)?.as_str().to_string())
    }

    /// Normalize kernel name (remove version if present)
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// assert_eq!(
    ///     UrnResolver::normalize_kernel_name("ckp://Recipes.BakeCake:v0.1"),
    ///     "Recipes.BakeCake"
    /// );
    /// assert_eq!(
    ///     UrnResolver::normalize_kernel_name("Recipes.BakeCake"),
    ///     "Recipes.BakeCake"
    /// );
    /// ```
    pub fn normalize_kernel_name(kernel_name_or_urn: &str) -> String {
        if kernel_name_or_urn.starts_with("ckp://") {
            if let Ok(parsed) = Self::parse(kernel_name_or_urn) {
                return parsed.kernel;
            }
        }
        kernel_name_or_urn.to_string()
    }

    /// Check if URN is an agent URN
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// assert!(UrnResolver::is_agent_urn("ckp://Agent/user:{username}"));
    /// assert!(UrnResolver::is_agent_urn("ckp://Agent/process:System.Gateway"));
    /// assert!(!UrnResolver::is_agent_urn("ckp://Recipes.BakeCake:v0.1"));
    /// ```
    pub fn is_agent_urn(urn: &str) -> bool {
        urn.starts_with("ckp://Agent/")
    }

    /// Parse agent URN into components
    ///
    /// Format: `ckp://Agent/user:{username}` or `ckp://Agent/process:{KernelName}`
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::{UrnResolver, AgentType};
    ///
    /// // Parse user agent (pattern: user:{username})
    /// let user_agent = UrnResolver::parse_agent_urn("ckp://Agent/user:admin").unwrap();
    /// assert_eq!(user_agent.identifier, "admin");
    /// match user_agent.agent_type {
    ///     AgentType::User(username) => assert_eq!(username, "admin"),
    ///     _ => panic!("Expected User agent type"),
    /// }
    ///
    /// // Parse process agent (pattern: process:{KernelName})
    /// let process_agent = UrnResolver::parse_agent_urn(
    ///     "ckp://Agent/process:System.Gateway"
    /// ).unwrap();
    /// assert_eq!(process_agent.identifier, "System.Gateway");
    /// match process_agent.agent_type {
    ///     AgentType::Process(kernel) => assert_eq!(kernel, "System.Gateway"),
    ///     _ => panic!("Expected Process agent type"),
    /// }
    /// ```
    pub fn parse_agent_urn(agent_urn: &str) -> Result<ParsedAgentUrn> {
        if agent_urn.is_empty() {
            return Err(CkpError::UrnParse("Agent URN must be a non-empty string".to_string()));
        }

        // Agent URN regex: ckp://Agent/(user|process):{identifier}
        // Groups: (type)(identifier)
        let re = Regex::new(r"^ckp://Agent/(user|process):(.+)$")
            .map_err(|e| CkpError::UrnParse(format!("Regex error: {}", e)))?;

        let caps = re
            .captures(agent_urn)
            .ok_or_else(|| CkpError::InvalidAgentUrn(agent_urn.to_string()))?;

        let agent_type_str = caps.get(1).unwrap().as_str();
        let identifier = caps.get(2).unwrap().as_str().to_string();

        let agent_type = match agent_type_str {
            "user" => AgentType::User(identifier.clone()),
            "process" => AgentType::Process(identifier.clone()),
            _ => {
                return Err(CkpError::InvalidAgentUrn(format!(
                    "Unknown agent type: {}. Valid types: user, process",
                    agent_type_str
                )))
            }
        };

        Ok(ParsedAgentUrn {
            agent_type,
            identifier,
        })
    }

    /// Build agent URN from components
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::{UrnResolver, ParsedAgentUrn, AgentType};
    ///
    /// // Build user agent URN
    /// let user_urn = UrnResolver::build_agent_urn(&ParsedAgentUrn {
    ///     agent_type: AgentType::User("admin".to_string()),
    ///     identifier: "admin".to_string(),
    /// });
    /// assert_eq!(user_urn, "ckp://Agent/user:admin");
    ///
    /// // Build process agent URN
    /// let process_urn = UrnResolver::build_agent_urn(&ParsedAgentUrn {
    ///     agent_type: AgentType::Process("System.Gateway".to_string()),
    ///     identifier: "System.Gateway".to_string(),
    /// });
    /// assert_eq!(process_urn, "ckp://Agent/process:System.Gateway");
    /// ```
    pub fn build_agent_urn(components: &ParsedAgentUrn) -> String {
        match &components.agent_type {
            AgentType::User(username) => format!("ckp://Agent/user:{}", username),
            AgentType::Process(kernel) => format!("ckp://Agent/process:{}", kernel),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_kernel_urn() {
        let urn = "ckp://Recipes.BakeCake:v0.1";
        let parsed = UrnResolver::parse(urn).unwrap();
        assert_eq!(parsed.kernel, "Recipes.BakeCake");
        assert_eq!(parsed.version, "v0.1");
        assert!(parsed.stage.is_none());
        assert!(parsed.path.is_none());
    }

    #[test]
    fn test_parse_urn_with_stage() {
        let urn = "ckp://Recipes.BakeCake:v0.1#storage";
        let parsed = UrnResolver::parse(urn).unwrap();
        assert_eq!(parsed.kernel, "Recipes.BakeCake");
        assert_eq!(parsed.version, "v0.1");
        assert_eq!(parsed.stage, Some("storage".to_string()));
        assert!(parsed.path.is_none());
    }

    #[test]
    fn test_parse_urn_with_stage_and_path() {
        let urn = "ckp://Recipes.BakeCake:v0.1#storage/tx-123.inst";
        let parsed = UrnResolver::parse(urn).unwrap();
        assert_eq!(parsed.kernel, "Recipes.BakeCake");
        assert_eq!(parsed.version, "v0.1");
        assert_eq!(parsed.stage, Some("storage".to_string()));
        assert_eq!(parsed.path, Some("tx-123.inst".to_string()));
    }

    #[test]
    fn test_parse_invalid_urn() {
        let result = UrnResolver::parse("invalid-urn");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_to_path_inbox() {
        let urn = "ckp://Recipes.BakeCake:v0.1#inbox";
        let root = Path::new("/test/concepts");
        let path = UrnResolver::resolve_to_path(urn, root).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/test/concepts/Recipes.BakeCake/queue/inbox")
        );
    }

    #[test]
    fn test_resolve_to_path_storage() {
        let urn = "ckp://Recipes.BakeCake:v0.1#storage/tx-123.inst";
        let root = Path::new("/test/concepts");
        let path = UrnResolver::resolve_to_path(urn, root).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/test/concepts/Recipes.BakeCake/storage/tx-123.inst")
        );
    }

    #[test]
    fn test_build_urn() {
        let components = ParsedUrn {
            kernel: "Recipes.BakeCake".to_string(),
            version: "v0.1".to_string(),
            stage: Some("storage".to_string()),
            path: Some("tx-123.inst".to_string()),
        };
        let urn = UrnResolver::build(&components);
        assert_eq!(urn, "ckp://Recipes.BakeCake:v0.1#storage/tx-123.inst");
    }

    #[test]
    fn test_is_edge_urn() {
        assert!(UrnResolver::is_edge_urn("ckp://Edge.PRODUCES.A-to-B:v1.0"));
        assert!(!UrnResolver::is_edge_urn("ckp://Recipes.BakeCake:v0.1"));
    }

    #[test]
    fn test_is_kernel_urn() {
        assert!(UrnResolver::is_kernel_urn("ckp://Recipes.BakeCake:v0.1"));
        assert!(!UrnResolver::is_kernel_urn("ckp://Edge.PRODUCES.A-to-B:v1.0"));
    }

    #[test]
    fn test_parse_edge_urn() {
        let edge_urn = "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12";
        let parsed = UrnResolver::parse_edge_urn(edge_urn).unwrap();
        assert_eq!(parsed.predicate, "PRODUCES");
        assert_eq!(parsed.source, "MixIngredients");
        assert_eq!(parsed.target, "BakeCake");
        assert_eq!(parsed.version, "v1.3.12");
        assert_eq!(parsed.queue_path, "queue/edges/PRODUCES.MixIngredients");
    }

    #[test]
    fn test_extract_tx_id() {
        let urn = "ckp://Recipes.BakeCake:v0.1#storage/tx-123.inst";
        let tx_id = UrnResolver::extract_tx_id(urn).unwrap();
        assert_eq!(tx_id, "tx-123");
    }

    #[test]
    fn test_normalize_kernel_name() {
        assert_eq!(
            UrnResolver::normalize_kernel_name("ckp://Recipes.BakeCake:v0.1"),
            "Recipes.BakeCake"
        );
        assert_eq!(
            UrnResolver::normalize_kernel_name("Recipes.BakeCake"),
            "Recipes.BakeCake"
        );
    }

    // NEW TESTS - Test Parity with Node.js

    /// Test: parse() - complex kernel name with dots
    /// Node.js equivalent: UrnResolver.test.js:63
    #[test]
    fn test_parse_complex_kernel_name() {
        let urn = "ckp://System.Gateway.HTTP:v1.3.12";
        let parsed = UrnResolver::parse(urn).unwrap();
        assert_eq!(parsed.kernel, "System.Gateway.HTTP");
        assert_eq!(parsed.version, "v1.3.12");
        assert!(parsed.stage.is_none());
        assert!(parsed.path.is_none());
    }

    /// Test: parse() - URN with edges stage
    /// Node.js equivalent: UrnResolver.test.js:72
    #[test]
    fn test_parse_urn_with_edges_stage() {
        let urn = "ckp://Recipes.BakeCake:v0.1#edges/PRODUCES.MixIngredients";
        let parsed = UrnResolver::parse(urn).unwrap();
        assert_eq!(parsed.kernel, "Recipes.BakeCake");
        assert_eq!(parsed.version, "v0.1");
        assert_eq!(parsed.stage, Some("edges".to_string()));
        assert_eq!(parsed.path, Some("PRODUCES.MixIngredients".to_string()));
    }

    /// Test: parseEdgeUrn() - complex kernel names with dots
    /// Node.js equivalent: UrnResolver.test.js:104
    #[test]
    fn test_parse_edge_urn_complex_names() {
        let edge_urn = "ckp://Edge.REQUIRES.System.Gateway.HTTP-to-System.Registry:v0.1";
        let parsed = UrnResolver::parse_edge_urn(edge_urn).unwrap();
        assert_eq!(parsed.predicate, "REQUIRES");
        assert_eq!(parsed.source, "System.Gateway.HTTP");
        assert_eq!(parsed.target, "System.Registry");
        assert_eq!(parsed.version, "v0.1");
        assert_eq!(parsed.queue_path, "queue/edges/REQUIRES.System.Gateway.HTTP");
    }

    /// Test: parseEdgeUrn() - invalid edge URN throws error
    /// Node.js equivalent: UrnResolver.test.js:115
    #[test]
    fn test_parse_edge_urn_invalid() {
        let invalid_edge_urn = "ckp://NotAnEdge:v0.1";
        let result = UrnResolver::parse_edge_urn(invalid_edge_urn);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Invalid edge URN"));
        }
    }

    /// Test: resolveToPath() - edges stage with path
    /// Node.js equivalent: UrnResolver.test.js:148
    #[test]
    fn test_resolve_to_path_edges() {
        let urn = "ckp://Recipes.BakeCake:v0.1#edges/PRODUCES.MixIngredients";
        let root = Path::new("/test/concepts");
        let path = UrnResolver::resolve_to_path(urn, root).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/test/concepts/Recipes.BakeCake/queue/edges/PRODUCES.MixIngredients")
        );
    }

    /// Test: resolveToPath() - tx stage
    /// Node.js equivalent: UrnResolver.test.js:156
    #[test]
    fn test_resolve_to_path_tx() {
        let urn = "ckp://Recipes.BakeCake:v0.1#tx/12345.tx";
        let root = Path::new("/test/concepts");
        let path = UrnResolver::resolve_to_path(urn, root).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/test/concepts/Recipes.BakeCake/tx/12345.tx")
        );
    }

    /// Test: resolveToPath() - invalid stage throws error
    /// Node.js equivalent: UrnResolver.test.js:164
    #[test]
    fn test_resolve_to_path_invalid_stage() {
        let urn = "ckp://Recipes.BakeCake:v0.1#invalid_stage";
        let result = UrnResolver::resolve_to_path(urn, Path::new("/test/concepts"));
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Unknown stage"));
        }
    }

    /// Test: resolveStage() - replace existing stage
    /// Node.js equivalent: UrnResolver.test.js:214
    #[test]
    fn test_resolve_stage_replace() {
        let base_urn = "ckp://Recipes.BakeCake:v0.1#storage";
        let result = UrnResolver::resolve_stage(base_urn, "inbox").unwrap();
        assert_eq!(result, "ckp://Recipes.BakeCake:v0.1#inbox");
    }

    // === Windows-Specific Path Handling Tests (2 tests) ===

    /// Test: Windows path separators (backslashes) are properly handled
    ///
    /// On Windows, paths use backslashes (\) instead of forward slashes (/).
    /// This test ensures that URN resolution works correctly with Windows paths.
    #[test]
    #[cfg(target_os = "windows")]
    fn test_urn_resolver_windows_paths() {
        use std::path::MAIN_SEPARATOR;

        let urn = "ckp://Recipes.BakeCake:v0.1#inbox";
        let root = Path::new("C:\\test\\concepts");
        let path = UrnResolver::resolve_to_path(urn, root).unwrap();

        // Verify path uses Windows separators
        let path_str = path.to_string_lossy();
        assert!(
            path_str.contains(MAIN_SEPARATOR),
            "Path should use Windows backslash separators: {}",
            path_str
        );

        // Verify correct resolution
        assert!(
            path_str.ends_with("queue\\inbox"),
            "Path should end with queue\\inbox: {}",
            path_str
        );

        // Verify kernel name in path
        assert!(
            path_str.contains("Recipes.BakeCake"),
            "Path should contain kernel name: {}",
            path_str
        );
    }

    /// Test: Windows drive letters (C:\, D:\, etc.) are properly handled
    ///
    /// Windows uses drive letters for absolute paths (C:\, D:\, etc.).
    /// This test ensures that URN resolution works with drive-letter paths
    /// and handles case-insensitivity correctly.
    #[test]
    #[cfg(target_os = "windows")]
    fn test_urn_resolver_windows_drive_letters() {
        // Test with C: drive
        let urn_c = "ckp://System.Registry:v1.0#storage/tx-456.inst";
        let root_c = Path::new("C:\\Program Files\\ConceptKernel");
        let path_c = UrnResolver::resolve_to_path(urn_c, root_c).unwrap();

        let path_c_str = path_c.to_string_lossy();
        assert!(
            path_c_str.starts_with("C:"),
            "Path should start with C: drive: {}",
            path_c_str
        );

        // Test with D: drive
        let urn_d = "ckp://Data.Pipeline:v0.5#staging";
        let root_d = Path::new("D:\\Data\\concepts");
        let path_d = UrnResolver::resolve_to_path(urn_d, root_d).unwrap();

        let path_d_str = path_d.to_string_lossy();
        assert!(
            path_d_str.starts_with("D:"),
            "Path should start with D: drive: {}",
            path_d_str
        );

        // Test case insensitivity (Windows paths are case-insensitive)
        let urn_case = "ckp://System.Registry:v1.0#storage";
        let root_lower = Path::new("c:\\test\\concepts");
        let root_upper = Path::new("C:\\test\\concepts");

        let path_lower = UrnResolver::resolve_to_path(urn_case, root_lower).unwrap();
        let path_upper = UrnResolver::resolve_to_path(urn_case, root_upper).unwrap();

        // Paths should be functionally equivalent (same components)
        assert_eq!(
            path_lower.components().count(),
            path_upper.components().count(),
            "Paths with different case drives should have same structure"
        );
    }

    /// Test: Unix-only test for forward slashes (excluded on Windows)
    ///
    /// This test ensures that on Unix systems, forward slashes are used
    /// correctly. It's excluded from Windows builds to prevent false failures.
    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_urn_resolver_unix_forward_slashes() {
        let urn = "ckp://Recipes.BakeCake:v0.1#inbox";
        let root = Path::new("/test/concepts");
        let path = UrnResolver::resolve_to_path(urn, root).unwrap();

        let path_str = path.to_string_lossy();
        assert!(
            path_str.contains('/'),
            "Unix paths should use forward slashes: {}",
            path_str
        );
        assert!(
            path_str.ends_with("queue/inbox"),
            "Path should end with queue/inbox: {}",
            path_str
        );
    }
}
