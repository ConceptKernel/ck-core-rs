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
///
/// Supports two formats based on `edge_versioning` flag in .ckproject:
///
/// **When edge_versioning = false (default):**
/// - URN: `ckp://Edge.{PREDICATE}.{Source}-to-{Target}` (no version)
/// - Folder: `PREDICATE.Source-to-Target`
///
/// **When edge_versioning = true:**
/// - URN: `ckp://Edge.{PREDICATE}.{Source}-to-{Target}:{version}`
/// - Folder: `PREDICATE.Source-to-Target:{version}`
///
/// Example:
/// - `ckp://Edge.PRODUCES.ConceptKernel.LLM.Fabric-to-System.Wss` (no version)
/// - `ckp://Edge.PRODUCES.ConceptKernel.LLM.Fabric-to-System.Wss:v1.3.19` (with version)
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedEdgeUrn {
    pub predicate: String,
    pub source: String,
    pub target: String,
    pub version: Option<String>,  // None when edge_versioning: false
    pub queue_path: String,       // queue/edges/{edge_dir}
    pub edge_dir: String,         // PREDICATE.Source-to-Target(:version)?
}

impl ParsedEdgeUrn {
    /// Generate edge directory name based on versioning flag
    ///
    /// - edge_versioning = false: `PREDICATE.Source-to-Target`
    /// - edge_versioning = true: `PREDICATE.Source-to-Target:version`
    pub fn get_edge_dir(&self) -> String {
        if let Some(ref version) = self.version {
            format!("{}.{}-to-{}:{}", self.predicate, self.source, self.target, version)
        } else {
            format!("{}.{}-to-{}", self.predicate, self.source, self.target)
        }
    }

    /// Generate queue path
    pub fn get_queue_path(&self) -> String {
        format!("queue/edges/{}", self.get_edge_dir())
    }
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

/// Parsed query URN with query parameters (v1 - legacy)
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedQueryUrn {
    pub resource: String,  // e.g., "Process"
    pub fragment: Option<String>,  // e.g., "tx-123" from "Process#tx-123"
    pub params: std::collections::HashMap<String, String>,
}

/// Enhanced parsed query URN with kernel namespace (v2)
///
/// Supports hierarchical URN schema that reflects BFO ontology:
/// - `ckp://{Kernel}:{Version}/{Resource}?params` - Kernel-scoped query
/// - `ckp://{Resource}?params` - Global query across all kernels
///
/// # Examples
///
/// ```
/// use ckp_core::UrnResolver;
///
/// // Kernel-scoped query
/// let parsed = UrnResolver::parse_query_urn_v2(
///     "ckp://System.Gateway:v1.0/Process?limit=20"
/// ).unwrap();
/// assert_eq!(parsed.kernel, Some("System.Gateway".to_string()));
/// assert_eq!(parsed.version, Some("v1.0".to_string()));
/// assert_eq!(parsed.resource, "Process");
///
/// // Global query
/// let parsed = UrnResolver::parse_query_urn_v2(
///     "ckp://Process?limit=20"
/// ).unwrap();
/// assert_eq!(parsed.kernel, None);
/// assert_eq!(parsed.resource, "Process");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedQueryUrnV2 {
    pub kernel: Option<String>,        // Extracted from path namespace
    pub version: Option<String>,       // Kernel version
    pub resource: String,               // Process, Workflow, ImprovementProcess, etc.
    pub params: std::collections::HashMap<String, String>, // Query parameters
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

    /// Parse edge URN with optional version
    ///
    /// Supports two formats based on `edge_versioning` flag:
    ///
    /// **When edge_versioning = false (default):**
    /// - Format: `ckp://Edge.{PREDICATE}.{Source}-to-{Target}` (no version)
    ///
    /// **When edge_versioning = true:**
    /// - Format: `ckp://Edge.{PREDICATE}.{Source}-to-{Target}:{version}`
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// // Without version (edge_versioning: false)
    /// let parsed = UrnResolver::parse_edge_urn(
    ///     "ckp://Edge.PRODUCES.ConceptKernel.LLM.Fabric-to-System.Wss"
    /// ).unwrap();
    /// assert_eq!(parsed.predicate, "PRODUCES");
    /// assert_eq!(parsed.source, "ConceptKernel.LLM.Fabric");
    /// assert_eq!(parsed.target, "System.Wss");
    /// assert_eq!(parsed.version, None);
    /// assert_eq!(parsed.edge_dir, "PRODUCES.ConceptKernel.LLM.Fabric-to-System.Wss");
    ///
    /// // With version (edge_versioning: true)
    /// let parsed = UrnResolver::parse_edge_urn(
    ///     "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12"
    /// ).unwrap();
    /// assert_eq!(parsed.predicate, "PRODUCES");
    /// assert_eq!(parsed.source, "MixIngredients");
    /// assert_eq!(parsed.target, "BakeCake");
    /// assert_eq!(parsed.version, Some("v1.3.12".to_string()));
    /// assert_eq!(parsed.edge_dir, "PRODUCES.MixIngredients-to-BakeCake:v1.3.12");
    /// ```
    pub fn parse_edge_urn(edge_urn: &str) -> Result<ParsedEdgeUrn> {
        if edge_urn.is_empty() {
            return Err(CkpError::UrnParse("Edge URN must be a non-empty string".to_string()));
        }

        // Pattern: ckp://Edge.{PREDICATE}.{Source}-to-{Target}(:{version})?
        // Uses non-greedy matching to handle dots in kernel names
        let re = Regex::new(r"^ckp://Edge\.([^.]+)\.(.+?)-to-(.+?)(?::(.+))?$")
            .map_err(|e| CkpError::UrnParse(format!("Regex error: {}", e)))?;

        if let Some(caps) = re.captures(edge_urn) {
            let predicate = caps.get(1).unwrap().as_str().to_string();
            let source = caps.get(2).unwrap().as_str().to_string();
            let target = caps.get(3).unwrap().as_str().to_string();
            let version = caps.get(4).map(|m| m.as_str().to_string());

            // Generate paths based on whether version is present
            let edge_dir = if let Some(ref v) = version {
                format!("{}.{}-to-{}:{}", predicate, source, target, v)
            } else {
                format!("{}.{}-to-{}", predicate, source, target)
            };
            let queue_path = format!("queue/edges/{}", edge_dir);

            return Ok(ParsedEdgeUrn {
                predicate,
                source,
                target,
                version,
                queue_path,
                edge_dir,
            });
        }

        // Pattern didn't match
        Err(CkpError::InvalidEdgeUrn(format!(
            "Invalid edge URN format: {}. Expected 'ckp://Edge.PREDICATE.Source-to-Target' or 'ckp://Edge.PREDICATE.Source-to-Target:version'",
            edge_urn
        )))
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

    /// Parse query URN with query parameters (v1 - legacy)
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// let parsed = UrnResolver::parse_query_urn("ckp://Process?limit=10&order=desc").unwrap();
    /// assert_eq!(parsed.resource, "Process");
    /// assert_eq!(parsed.params.get("limit"), Some(&"10".to_string()));
    /// assert_eq!(parsed.params.get("order"), Some(&"desc".to_string()));
    /// ```
    pub fn parse_query_urn(urn: &str) -> Result<ParsedQueryUrn> {
        if !urn.starts_with("ckp://") {
            return Err(CkpError::UrnParse("Query URN must start with ckp://".to_string()));
        }

        // Split at '?' to separate resource from query params
        let parts: Vec<&str> = urn["ckp://".len()..].splitn(2, '?').collect();
        let resource_with_fragment = parts[0];

        // Split at '#' to separate resource type from fragment (e.g., Process#tx-123)
        let fragment_parts: Vec<&str> = resource_with_fragment.splitn(2, '#').collect();
        let resource = fragment_parts[0].to_string();
        let fragment = if fragment_parts.len() > 1 {
            Some(fragment_parts[1].to_string())
        } else {
            None
        };

        let mut params = std::collections::HashMap::new();
        if parts.len() > 1 {
            // Parse query parameters
            for param_pair in parts[1].split('&') {
                let kv: Vec<&str> = param_pair.splitn(2, '=').collect();
                if kv.len() == 2 {
                    params.insert(kv[0].to_string(), kv[1].to_string());
                }
            }
        }

        Ok(ParsedQueryUrn { resource, fragment, params })
    }

    /// Parse enhanced query URN with kernel namespace (v2)
    ///
    /// Supports multiple patterns:
    /// 1. Kernel-scoped: `ckp://{Kernel}:{Version}/{Resource}?params`
    /// 2. Global query: `ckp://{Resource}?params`
    /// 3. Kernel-as-resource (backward compat): `ckp://{Kernel}?view={resource}&params`
    ///
    /// This makes queries generic across all kernels since they share the same
    /// BFO ontological foundation. The kernel becomes a namespace/scope for queries.
    ///
    /// # Examples
    ///
    /// ```
    /// use ckp_core::UrnResolver;
    ///
    /// // Pattern 1: Kernel-scoped query (preferred)
    /// let parsed = UrnResolver::parse_query_urn_v2(
    ///     "ckp://System.Gateway:v1.0/Process?limit=20&order=desc"
    /// ).unwrap();
    /// assert_eq!(parsed.kernel, Some("System.Gateway".to_string()));
    /// assert_eq!(parsed.version, Some("v1.0".to_string()));
    /// assert_eq!(parsed.resource, "Process");
    /// assert_eq!(parsed.params.get("limit"), Some(&"20".to_string()));
    ///
    /// // Pattern 2: Global query (all kernels)
    /// let parsed = UrnResolver::parse_query_urn_v2(
    ///     "ckp://Process?limit=20"
    /// ).unwrap();
    /// assert_eq!(parsed.kernel, None);
    /// assert_eq!(parsed.resource, "Process");
    ///
    /// // Pattern 3: Kernel-as-resource (backward compatible)
    /// let parsed = UrnResolver::parse_query_urn_v2(
    ///     "ckp://System.Gateway?view=Process&limit=20"
    /// ).unwrap();
    /// assert_eq!(parsed.kernel, Some("System.Gateway".to_string()));
    /// assert_eq!(parsed.resource, "Process");
    /// ```
    pub fn parse_query_urn_v2(urn: &str) -> Result<ParsedQueryUrnV2> {
        if !urn.starts_with("ckp://") {
            return Err(CkpError::UrnParse("Query URN must start with ckp://".to_string()));
        }

        let content = &urn["ckp://".len()..];

        // Check if this is Pattern 1: ckp://{Kernel}:{Version}/{Resource}?params
        if content.contains(':') && content.contains('/') {
            let colon_pos = content.find(':').unwrap();
            let slash_pos = content.find('/').unwrap();

            if slash_pos > colon_pos {
                // This is kernel-scoped format
                let kernel = content[..colon_pos].to_string();
                let version_and_rest = &content[colon_pos + 1..];

                let version = version_and_rest[..slash_pos - colon_pos - 1].to_string();
                let resource_and_params = &version_and_rest[slash_pos - colon_pos..];

                // Parse resource and params
                let parts: Vec<&str> = resource_and_params.splitn(2, '?').collect();
                let resource = parts[0].to_string();

                let mut params = std::collections::HashMap::new();
                if parts.len() > 1 {
                    for param_pair in parts[1].split('&') {
                        let kv: Vec<&str> = param_pair.splitn(2, '=').collect();
                        if kv.len() == 2 {
                            params.insert(kv[0].to_string(), kv[1].to_string());
                        }
                    }
                }

                return Ok(ParsedQueryUrnV2 {
                    kernel: Some(kernel),
                    version: Some(version),
                    resource,
                    params,
                });
            }
        }

        // Parse as either Pattern 2 (global) or Pattern 3 (kernel-as-resource)
        let parts: Vec<&str> = content.splitn(2, '?').collect();
        let first_part = parts[0].to_string();

        let mut params = std::collections::HashMap::new();
        if parts.len() > 1 {
            for param_pair in parts[1].split('&') {
                let kv: Vec<&str> = param_pair.splitn(2, '=').collect();
                if kv.len() == 2 {
                    params.insert(kv[0].to_string(), kv[1].to_string());
                }
            }
        }

        // Check if 'view' param exists (Pattern 3: kernel-as-resource)
        if let Some(view) = params.get("view") {
            // Pattern 3: ckp://{Kernel}?view={resource}&params
            let resource = view.clone();
            params.remove("view");

            // Extract version if present in kernel name
            let (kernel, version) = if first_part.contains(':') {
                let parts: Vec<&str> = first_part.split(':').collect();
                (parts[0].to_string(), Some(parts[1].to_string()))
            } else {
                (first_part, None)
            };

            Ok(ParsedQueryUrnV2 {
                kernel: Some(kernel),
                version,
                resource,
                params,
            })
        } else {
            // Pattern 2: ckp://{Resource}?params (global query)
            Ok(ParsedQueryUrnV2 {
                kernel: None,
                version: None,
                resource: first_part,
                params,
            })
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
        // Test with version (edge_versioning: true)
        let edge_urn = "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake:v1.3.12";
        let parsed = UrnResolver::parse_edge_urn(edge_urn).unwrap();
        assert_eq!(parsed.predicate, "PRODUCES");
        assert_eq!(parsed.source, "MixIngredients");
        assert_eq!(parsed.target, "BakeCake");
        assert_eq!(parsed.version, Some("v1.3.12".to_string()));
        assert_eq!(parsed.edge_dir, "PRODUCES.MixIngredients-to-BakeCake:v1.3.12");
        assert_eq!(parsed.queue_path, "queue/edges/PRODUCES.MixIngredients-to-BakeCake:v1.3.12");

        // Test without version (edge_versioning: false)
        let edge_urn_no_ver = "ckp://Edge.PRODUCES.MixIngredients-to-BakeCake";
        let parsed_no_ver = UrnResolver::parse_edge_urn(edge_urn_no_ver).unwrap();
        assert_eq!(parsed_no_ver.predicate, "PRODUCES");
        assert_eq!(parsed_no_ver.source, "MixIngredients");
        assert_eq!(parsed_no_ver.target, "BakeCake");
        assert_eq!(parsed_no_ver.version, None);
        assert_eq!(parsed_no_ver.edge_dir, "PRODUCES.MixIngredients-to-BakeCake");
        assert_eq!(parsed_no_ver.queue_path, "queue/edges/PRODUCES.MixIngredients-to-BakeCake");
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
        // With version
        let edge_urn = "ckp://Edge.REQUIRES.System.Gateway.HTTP-to-System.Registry:v0.1";
        let parsed = UrnResolver::parse_edge_urn(edge_urn).unwrap();
        assert_eq!(parsed.predicate, "REQUIRES");
        assert_eq!(parsed.source, "System.Gateway.HTTP");
        assert_eq!(parsed.target, "System.Registry");
        assert_eq!(parsed.version, Some("v0.1".to_string()));
        assert_eq!(parsed.edge_dir, "REQUIRES.System.Gateway.HTTP-to-System.Registry:v0.1");
        assert_eq!(parsed.queue_path, "queue/edges/REQUIRES.System.Gateway.HTTP-to-System.Registry:v0.1");

        // Without version
        let edge_urn_no_ver = "ckp://Edge.REQUIRES.System.Gateway.HTTP-to-System.Registry";
        let parsed_no_ver = UrnResolver::parse_edge_urn(edge_urn_no_ver).unwrap();
        assert_eq!(parsed_no_ver.predicate, "REQUIRES");
        assert_eq!(parsed_no_ver.source, "System.Gateway.HTTP");
        assert_eq!(parsed_no_ver.target, "System.Registry");
        assert_eq!(parsed_no_ver.version, None);
        assert_eq!(parsed_no_ver.edge_dir, "REQUIRES.System.Gateway.HTTP-to-System.Registry");
        assert_eq!(parsed_no_ver.queue_path, "queue/edges/REQUIRES.System.Gateway.HTTP-to-System.Registry");
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

    // === Enhanced Query URN v2 Tests ===

    /// Test: Parse kernel-scoped query (Pattern 1)
    #[test]
    fn test_parse_query_urn_v2_kernel_scoped() {
        let urn = "ckp://System.Gateway:v1.0/Process?limit=20&order=desc";
        let parsed = UrnResolver::parse_query_urn_v2(urn).unwrap();

        assert_eq!(parsed.kernel, Some("System.Gateway".to_string()));
        assert_eq!(parsed.version, Some("v1.0".to_string()));
        assert_eq!(parsed.resource, "Process");
        assert_eq!(parsed.params.get("limit"), Some(&"20".to_string()));
        assert_eq!(parsed.params.get("order"), Some(&"desc".to_string()));
    }

    /// Test: Parse global query (Pattern 2)
    #[test]
    fn test_parse_query_urn_v2_global_query() {
        let urn = "ckp://Process?limit=20&status=completed";
        let parsed = UrnResolver::parse_query_urn_v2(urn).unwrap();

        assert_eq!(parsed.kernel, None);
        assert_eq!(parsed.version, None);
        assert_eq!(parsed.resource, "Process");
        assert_eq!(parsed.params.get("limit"), Some(&"20".to_string()));
        assert_eq!(parsed.params.get("status"), Some(&"completed".to_string()));
    }

    /// Test: Parse kernel-as-resource (Pattern 3)
    #[test]
    fn test_parse_query_urn_v2_kernel_as_resource() {
        let urn = "ckp://System.Gateway:v1.0?view=Process&limit=20";
        let parsed = UrnResolver::parse_query_urn_v2(urn).unwrap();

        assert_eq!(parsed.kernel, Some("System.Gateway".to_string()));
        assert_eq!(parsed.version, Some("v1.0".to_string()));
        assert_eq!(parsed.resource, "Process");
        assert_eq!(parsed.params.get("limit"), Some(&"20".to_string()));
        assert!(!parsed.params.contains_key("view")); // view param removed
    }

    /// Test: Parse kernel-scoped without params
    #[test]
    fn test_parse_query_urn_v2_no_params() {
        let urn = "ckp://System.Gateway:v1.0/Process";
        let parsed = UrnResolver::parse_query_urn_v2(urn).unwrap();

        assert_eq!(parsed.kernel, Some("System.Gateway".to_string()));
        assert_eq!(parsed.version, Some("v1.0".to_string()));
        assert_eq!(parsed.resource, "Process");
        assert!(parsed.params.is_empty());
    }

    /// Test: Parse complex kernel name with dots
    #[test]
    fn test_parse_query_urn_v2_complex_kernel_name() {
        let urn = "ckp://System.Gateway.HTTP:v1.3.12/Process?limit=10";
        let parsed = UrnResolver::parse_query_urn_v2(urn).unwrap();

        assert_eq!(parsed.kernel, Some("System.Gateway.HTTP".to_string()));
        assert_eq!(parsed.version, Some("v1.3.12".to_string()));
        assert_eq!(parsed.resource, "Process");
        assert_eq!(parsed.params.get("limit"), Some(&"10".to_string()));
    }

    /// Test: Parse different resource types (BFO Occurrents)
    #[test]
    fn test_parse_query_urn_v2_different_resources() {
        let test_cases = vec![
            ("ckp://System.Gateway:v1.0/Process?limit=20", "Process"),
            ("ckp://System.Workflow:v1.0/Workflow?status=active", "Workflow"),
            ("ckp://System.Improvement:v1.0/ImprovementProcess?phase=analysis", "ImprovementProcess"),
            ("ckp://System.Consensus:v1.0/ConsensusProcess?quorum=majority", "ConsensusProcess"),
            ("ckp://System.Workflow:v1.0/WorkflowPhase?workflow=self-improvement", "WorkflowPhase"),
        ];

        for (urn, expected_resource) in test_cases {
            let parsed = UrnResolver::parse_query_urn_v2(urn).unwrap();
            assert_eq!(parsed.resource, expected_resource);
            assert!(parsed.kernel.is_some());
            assert!(parsed.version.is_some());
        }
    }

    /// Test: Global queries work for all resource types
    #[test]
    fn test_parse_query_urn_v2_global_all_resources() {
        let test_cases = vec![
            "ckp://Process?limit=100",
            "ckp://Workflow?status=active",
            "ckp://ImprovementProcess?kernel=System.Gateway",
            "ckp://ConsensusProcess?proposal_type=improvement",
            "ckp://WorkflowPhase?status=in_progress",
        ];

        for urn in test_cases {
            let parsed = UrnResolver::parse_query_urn_v2(urn).unwrap();
            assert_eq!(parsed.kernel, None);
            assert_eq!(parsed.version, None);
            assert!(!parsed.resource.is_empty());
        }
    }

    /// Test: Invalid URN format errors
    #[test]
    fn test_parse_query_urn_v2_invalid() {
        let invalid_urns = vec![
            "not-a-urn",
            "http://example.com",
            "ckp:invalid",
        ];

        for urn in invalid_urns {
            let result = UrnResolver::parse_query_urn_v2(urn);
            assert!(result.is_err(), "Expected error for: {}", urn);
        }
    }

    /// Test: Kernel without version (backward compat)
    #[test]
    fn test_parse_query_urn_v2_kernel_no_version() {
        let urn = "ckp://System.Gateway?view=Process&limit=20";
        let parsed = UrnResolver::parse_query_urn_v2(urn).unwrap();

        assert_eq!(parsed.kernel, Some("System.Gateway".to_string()));
        assert_eq!(parsed.version, None);
        assert_eq!(parsed.resource, "Process");
        assert_eq!(parsed.params.get("limit"), Some(&"20".to_string()));
    }

    /// Test: Query params with special characters
    #[test]
    fn test_parse_query_urn_v2_special_params() {
        let urn = "ckp://System.Gateway:v1.0/Process?kernel=System.Gateway&timestamp_from=2025-12-01&timestamp_to=2025-12-31";
        let parsed = UrnResolver::parse_query_urn_v2(urn).unwrap();

        assert_eq!(parsed.kernel, Some("System.Gateway".to_string()));
        assert_eq!(parsed.resource, "Process");
        assert_eq!(parsed.params.get("kernel"), Some(&"System.Gateway".to_string()));
        assert_eq!(parsed.params.get("timestamp_from"), Some(&"2025-12-01".to_string()));
        assert_eq!(parsed.params.get("timestamp_to"), Some(&"2025-12-31".to_string()));
    }
}
