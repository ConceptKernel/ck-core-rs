/**
 * ckdl_parser.rs
 * CKDL (Concept Kernel Definition Language) parser
 * Version: v2.4-phase2 (CORE.URN v1.3.16 Compliance)
 *
 * Parses CKDL files containing:
 * - EXTERN declarations (upstream dependencies)
 * - KERNEL definitions (kernel URNs with metadata)
 * - EDGE definitions (predicates connecting kernels)
 */

use std::fs;
use std::path::Path;
use crate::errors::{CkpError, Result};
use crate::urn::UrnValidator;

/// URN string type
pub type Urn = String;

/// CKDL file representation
#[derive(Debug, Clone)]
pub struct CkdlDocument {
    pub version: String,
    pub domain: String,
    pub externs: Vec<ExternDeclaration>,
    pub kernels: Vec<KernelDeclaration>,
    pub edges: Vec<EdgeDeclaration>,
}

/// EXTERN declaration
#[derive(Debug, Clone)]
pub struct ExternDeclaration {
    pub urn: Urn,
    pub category: String, // e.g., "Intelligence & Ontology", "Governance & Routing"
}

/// KERNEL declaration
#[derive(Debug, Clone)]
pub struct KernelDeclaration {
    pub urn: Urn,
    pub kernel_type: Option<String>, // e.g., "python:hot", "rust:cold"
    pub port: Option<u16>,
}

/// EDGE declaration
#[derive(Debug, Clone)]
pub struct EdgeDeclaration {
    pub urn: Urn,
    pub predicate: String,
    pub source: Urn,
    pub target: Urn,
}

pub struct CkdlParser;

impl CkdlParser {
    /// Parse CKDL file
    pub fn parse_file(path: &Path) -> Result<CkdlDocument> {
        let content = fs::read_to_string(path)
            .map_err(|e| CkpError::IoError(format!("Failed to read CKDL file: {}", e)))?;

        Self::parse(&content)
    }

    /// Parse CKDL content
    pub fn parse(content: &str) -> Result<CkdlDocument> {
        let mut version = String::from("unknown");
        let mut domain = String::from("unknown");
        let mut externs = Vec::new();
        let mut kernels = Vec::new();
        let mut edges = Vec::new();
        let mut current_category = String::new();
        let mut current_kernel: Option<KernelDeclaration> = None;

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                // Extract metadata from comments
                if line.starts_with("# Version:") {
                    version = line.split(':').nth(1).unwrap_or("unknown").trim().to_string();
                } else if line.starts_with("# Domain:") {
                    domain = line.split(':').nth(1).unwrap_or("unknown").trim().to_string();
                } else if line.starts_with("# >") {
                    current_category = line[3..].trim().to_string();
                }
                continue;
            }

            // Parse declarations
            if line.starts_with("EXTERN ") {
                let urn_str = line[7..].trim();
                if UrnValidator::validate(urn_str).valid {
                    externs.push(ExternDeclaration {
                        urn: urn_str.to_string(),
                        category: current_category.clone(),
                    });
                } else {
                    eprintln!("Warning: Invalid EXTERN URN '{}'", urn_str);
                }
            } else if line.starts_with("KERNEL ") {
                // Save previous kernel if any
                if let Some(kernel) = current_kernel.take() {
                    kernels.push(kernel);
                }

                let urn_str = line[7..].trim();
                if UrnValidator::validate(urn_str).valid {
                    current_kernel = Some(KernelDeclaration {
                        urn: urn_str.to_string(),
                        kernel_type: None,
                        port: None,
                    });
                } else {
                    eprintln!("Warning: Invalid KERNEL URN '{}'", urn_str);
                }
            } else if line.starts_with("TYPE:") {
                if let Some(ref mut kernel) = current_kernel {
                    kernel.kernel_type = Some(line[5..].trim().to_string());
                }
            } else if line.starts_with("PORT:") {
                if let Some(ref mut kernel) = current_kernel {
                    if let Ok(port) = line[5..].trim().parse::<u16>() {
                        kernel.port = Some(port);
                    }
                }
            } else if line.starts_with("EDGE ") {
                // Save previous kernel if any
                if let Some(kernel) = current_kernel.take() {
                    kernels.push(kernel);
                }

                let urn_str = line[5..].trim();
                match Self::parse_edge(urn_str) {
                    Ok(edge) => edges.push(edge),
                    Err(e) => eprintln!("Warning: Failed to parse EDGE: {} - Error: {}", urn_str, e),
                }
            }
        }

        // Save final kernel if any
        if let Some(kernel) = current_kernel {
            kernels.push(kernel);
        }

        Ok(CkdlDocument {
            version,
            domain,
            externs,
            kernels,
            edges,
        })
    }

    /// Parse EDGE declaration
    /// Formats:
    /// - With version: `ckp://Edge.{PREDICATE}.{Source}-to-{Target}:{version}`
    /// - Without version: `ckp://Edge.{PREDICATE}.{Source}-to-{Target}`
    fn parse_edge(urn_str: &str) -> Result<EdgeDeclaration> {
        use crate::urn::UrnResolver;

        // Use UrnResolver to parse the edge URN
        let parsed = UrnResolver::parse_edge_urn(urn_str)?;

        // Validate edge URN
        if !UrnValidator::validate_edge_urn(urn_str).valid {
            return Err(CkpError::ParseError(format!("Invalid EDGE URN: {}", urn_str)));
        }

        // Construct source and target URNs (without versions for simplicity)
        let source_str = format!("ckp://{}", parsed.source);
        let target_str = format!("ckp://{}", parsed.target);

        Ok(EdgeDeclaration {
            urn: urn_str.to_string(),
            predicate: parsed.predicate,
            source: source_str,
            target: target_str,
        })
    }
}

impl CkdlDocument {
    /// Get all kernel URNs
    pub fn get_kernel_urns(&self) -> Vec<&Urn> {
        self.kernels.iter().map(|k| &k.urn).collect()
    }

    /// Get all extern URNs
    pub fn get_extern_urns(&self) -> Vec<&Urn> {
        self.externs.iter().map(|e| &e.urn).collect()
    }

    /// Get all dependencies (externs + edges)
    pub fn get_all_dependencies(&self) -> Vec<&Urn> {
        let mut deps = self.get_extern_urns();
        deps.extend(self.edges.iter().map(|e| &e.source));
        deps.extend(self.edges.iter().map(|e| &e.target));
        deps
    }

    /// Find kernel by URN
    pub fn find_kernel(&self, urn: &Urn) -> Option<&KernelDeclaration> {
        self.kernels.iter().find(|k| &k.urn == urn)
    }

    /// Find edges by source
    pub fn find_edges_from(&self, source: &Urn) -> Vec<&EdgeDeclaration> {
        self.edges.iter().filter(|e| &e.source == source).collect()
    }

    /// Find edges by target
    pub fn find_edges_to(&self, target: &Urn) -> Vec<&EdgeDeclaration> {
        self.edges.iter().filter(|e| &e.target == target).collect()
    }

    /// Export to CKDL format
    pub fn to_ckdl(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!("# CKDL: Concept Kernel Definition Language\n"));
        output.push_str(&format!("# Version: {}\n", self.version));
        output.push_str(&format!("# Domain: {}\n\n", self.domain));

        // Externs
        if !self.externs.is_empty() {
            output.push_str("# --- 1. Upstream Dependencies ---\n");
            let mut categories: std::collections::HashMap<String, Vec<&ExternDeclaration>> = std::collections::HashMap::new();

            for ext in &self.externs {
                categories.entry(ext.category.clone())
                    .or_insert_with(Vec::new)
                    .push(ext);
            }

            for (category, exts) in categories {
                if !category.is_empty() {
                    output.push_str(&format!("\n# > {}\n", category));
                }
                for ext in exts {
                    output.push_str(&format!("EXTERN {}\n", ext.urn.to_string()));
                }
            }
            output.push('\n');
        }

        // Kernels
        if !self.kernels.is_empty() {
            output.push_str("# --- 2. Kernel Definitions ---\n\n");
            for kernel in &self.kernels {
                output.push_str(&format!("KERNEL {}\n", kernel.urn.to_string()));
                if let Some(ref ktype) = kernel.kernel_type {
                    output.push_str(&format!("  TYPE: {}\n", ktype));
                }
                if let Some(port) = kernel.port {
                    output.push_str(&format!("  PORT: {}\n", port));
                }
                output.push('\n');
            }
        }

        // Edges
        if !self.edges.is_empty() {
            output.push_str("# --- 3. Edge Definitions ---\n\n");
            for edge in &self.edges {
                output.push_str(&format!("EDGE {}\n", edge.urn.to_string()));
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_extern() {
        let ckdl = r#"
# > Intelligence & Ontology
EXTERN ckp://ConceptKernel.LLM.Claude:v0.1
EXTERN ckp://ConceptKernel.Ontology:v1.0
"#;

        let doc = CkdlParser::parse(ckdl).unwrap();
        assert_eq!(doc.externs.len(), 2);
        assert_eq!(doc.externs[0].category, "Intelligence & Ontology");
    }

    #[test]
    fn test_parse_kernel() {
        let ckdl = r#"
KERNEL ckp://Com.NeuxScience.GameDispatch.Waterfall:v0.1
  TYPE: python:hot
  PORT: 3013
"#;

        let doc = CkdlParser::parse(ckdl).unwrap();
        assert_eq!(doc.kernels.len(), 1);
        assert_eq!(doc.kernels[0].kernel_type, Some("python:hot".to_string()));
        assert_eq!(doc.kernels[0].port, Some(3013));
    }

    #[test]
    fn test_parse_edge() {
        // Test with version (edge_versioning: true)
        let ckdl = r#"
EDGE ckp://Edge.LINKS_IDENTITY.Com.NeuxScience.Participant-to-System.Oidc.User:v1.0
"#;

        let doc = CkdlParser::parse(ckdl).unwrap();
        assert_eq!(doc.edges.len(), 1);
        assert_eq!(doc.edges[0].predicate, "LINKS_IDENTITY");

        // Test without version (edge_versioning: false)
        let ckdl_no_ver = r#"
EDGE ckp://Edge.PRODUCES.MixIngredients-to-BakeCake
"#;

        let doc_no_ver = CkdlParser::parse(ckdl_no_ver).unwrap();
        assert_eq!(doc_no_ver.edges.len(), 1);
        assert_eq!(doc_no_ver.edges[0].predicate, "PRODUCES");
    }
}
