/**
 * generator.rs
 * Automatic ontology.ttl generation for forked/created kernels
 * Uses kernel-entity-template.ttl and inherits roles/functions from source
 */

use std::fs;
use std::path::{Path, PathBuf};
use crate::ontology::library::{RoleMetadata, FunctionMetadata};
use crate::errors::{CkpError, Result};
use chrono::Utc;

/// Source metadata for inheritance
#[derive(Debug, Clone)]
struct SourceMetadata {
    pub roles: Vec<RoleMetadata>,
    pub functions: Vec<FunctionMetadata>,
}

pub struct OntologyGenerator {
    project_root: PathBuf,
}

impl OntologyGenerator {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Generate ontology.ttl for a forked kernel
    ///
    /// # Arguments
    /// * `kernel_dir` - Path to kernel directory
    /// * `source_name` - Name of source kernel (for inheritance)
    /// * `new_name` - Name of new kernel
    /// * `evidence_id` - Optional evidence ID for provenance
    ///
    /// # Returns
    /// Path to generated ontology.ttl
    pub fn generate_from_template(
        &self,
        kernel_dir: &Path,
        source_name: Option<&str>,
        new_name: &str,
        evidence_id: Option<&str>,
    ) -> Result<PathBuf> {
        // 1. Read kernel-entity-template.ttl
        let template_path = self.project_root
            .join("concepts")
            .join(".ontology")
            .join("kernel-entity-template.ttl");

        if !template_path.exists() {
            return Err(CkpError::FileNotFound(format!(
                "Template not found: {}",
                template_path.display()
            )));
        }

        let template = fs::read_to_string(&template_path)
            .map_err(|e| CkpError::IoError(format!("Failed to read template: {}", e)))?;

        // 2. Parse conceptkernel.yaml to get metadata
        let config = self.read_kernel_config(kernel_dir)?;

        // 3. Try to load source ontology for role/function inheritance
        let source_metadata = if let Some(source) = source_name {
            self.try_load_source_metadata(source)
        } else {
            None
        };

        // 4. Generate ontology content with substitutions
        let ontology_content = self.apply_template(
            &template,
            new_name,
            &config,
            source_metadata.as_ref(),
            source_name,
            evidence_id,
        )?;

        // 5. Write ontology.ttl
        let ontology_path = kernel_dir.join("ontology.ttl");
        fs::write(&ontology_path, ontology_content)
            .map_err(|e| CkpError::IoError(format!("Failed to write ontology.ttl: {}", e)))?;

        println!("✓ Generated ontology.ttl for {}", new_name);
        if let Some(src) = source_name {
            if source_metadata.is_some() {
                println!("  └─ Inherited semantic metadata from {}", src);
            }
        }

        Ok(ontology_path)
    }

    /// Try to load source kernel metadata from ontology
    fn try_load_source_metadata(&self, _source_name: &str) -> Option<SourceMetadata> {
        // TODO: Parse source ontology.ttl to extract roles/functions
        // For now, return None (will implement SPARQL parsing in Phase 2)
        None
    }

    /// Apply template substitutions
    fn apply_template(
        &self,
        template: &str,
        kernel_name: &str,
        config: &KernelConfig,
        source_metadata: Option<&SourceMetadata>,
        source_name: Option<&str>,
        evidence_id: Option<&str>,
    ) -> Result<String> {
        let mut content = template.to_string();

        // Basic substitutions
        content = content.replace("{KERNEL_NAME}", kernel_name);
        content = content.replace("{KERNEL_LOWER}", &kernel_name.to_lowercase());
        content = content.replace(
            "{KERNEL_URN}",
            &format!("ckp://Continuant#Kernel-{}", kernel_name)
        );
        content = content.replace("{VERSION}", &config.version);
        content = content.replace("{KERNEL_TYPE}", &config.kernel_type);
        content = content.replace(
            "{KERNEL_DESCRIPTION}",
            &config.description.clone().unwrap_or_else(|| format!("{} kernel", kernel_name))
        );

        // Add provenance information
        let timestamp = Utc::now().to_rfc3339();
        let mut provenance = String::new();

        if let Some(source) = source_name {
            provenance.push_str(&format!(
                "  ckp:forkedFrom <https://conceptkernel.org/kernel/{}> ;\n",
                source
            ));
        }

        if let Some(eid) = evidence_id {
            provenance.push_str(&format!(
                "  ckp:createdByEvidence <evidence:{}> ;\n",
                eid
            ));
        }

        provenance.push_str(&format!(
            "  ckp:createdAt \"{}\"^^xsd:dateTime .",
            timestamp
        ));

        content = content.replace("{PROVENANCE}", &provenance);

        // Inherit roles and functions from source if available
        if let Some(source) = source_metadata {
            let roles = self.generate_role_triples(kernel_name, &source.roles);
            content = content.replace("{ROLES}", &roles);

            let functions = self.generate_function_triples(kernel_name, &source.functions);
            content = content.replace("{FUNCTIONS}", &functions);
        } else {
            // Generate default role and function based on kernel type
            let default_role = self.generate_default_role(kernel_name, &config.kernel_type);
            content = content.replace("{ROLES}", &default_role);

            let default_function = self.generate_default_function(kernel_name, &config.kernel_type);
            content = content.replace("{FUNCTIONS}", &default_function);
        }

        Ok(content)
    }

    /// Generate default role based on kernel type
    fn generate_default_role(&self, kernel_name: &str, kernel_type: &str) -> String {
        let (role_name, role_desc, context) = if kernel_type.contains("hot") {
            ("Service Provider", "Always-on service kernel", "service")
        } else if kernel_type.contains("cold") {
            ("Worker", "On-demand worker kernel", "computation")
        } else {
            ("Generic Kernel", "Generic kernel role", "general")
        };

        format!(
            r#"
<https://conceptkernel.org/kernel/{}/role/default>
  rdf:type bfo:0000023 ;  # BFO:Role
  rdfs:label "{}" ;
  rdfs:comment "{}" ;
  ckp:bearer <https://conceptkernel.org/kernel/{}> ;
  ckp:roleContext "{}" .
"#,
            kernel_name, role_name, role_desc, kernel_name, context
        )
    }

    /// Generate default function based on kernel type
    fn generate_default_function(&self, kernel_name: &str, kernel_type: &str) -> String {
        let (func_name, func_desc, capability) = if kernel_type.contains("rust") {
            ("Rust Processing", "High-performance Rust computation", "computation")
        } else if kernel_type.contains("node") {
            ("Node.js Processing", "JavaScript/Node.js computation", "javascript-execution")
        } else if kernel_type.contains("python") {
            ("Python Processing", "Python computation", "python-execution")
        } else {
            ("Generic Processing", "Generic kernel processing", "computation")
        };

        format!(
            r#"
<https://conceptkernel.org/kernel/{}/function/default>
  rdf:type bfo:0000034 ;  # BFO:Function
  rdfs:label "{}" ;
  rdfs:comment "{}" ;
  ckp:realizedBy <https://conceptkernel.org/kernel/{}> ;
  ckp:capability "{}" .
"#,
            kernel_name, func_name, func_desc, kernel_name, capability
        )
    }

    /// Generate RDF triples for roles
    fn generate_role_triples(&self, kernel_name: &str, roles: &[RoleMetadata]) -> String {
        let mut triples = String::new();

        for role in roles {
            triples.push_str(&format!(
                r#"
<https://conceptkernel.org/kernel/{}/role/{}>
  rdf:type bfo:0000023 ;  # BFO:Role
  rdfs:label "{}" ;
  rdfs:comment "{}" ;
  ckp:bearer <https://conceptkernel.org/kernel/{}> ;
  ckp:roleContext "{}" .
"#,
                kernel_name,
                role.name.to_lowercase().replace(" ", "-"),
                role.name,
                role.description,
                kernel_name,
                role.context
            ));
        }

        if triples.is_empty() {
            triples = "# No roles inherited\n".to_string();
        }

        triples
    }

    /// Generate RDF triples for functions
    fn generate_function_triples(&self, kernel_name: &str, functions: &[FunctionMetadata]) -> String {
        let mut triples = String::new();

        for func in functions {
            let capabilities = func.capabilities
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(" , ");

            triples.push_str(&format!(
                r#"
<https://conceptkernel.org/kernel/{}/function/{}>
  rdf:type bfo:0000034 ;  # BFO:Function
  rdfs:label "{}" ;
  rdfs:comment "{}" ;
  ckp:realizedBy <https://conceptkernel.org/kernel/{}> ;
  ckp:capability {} .
"#,
                kernel_name,
                func.name.to_lowercase().replace(" ", "-"),
                func.name,
                func.description,
                kernel_name,
                capabilities
            ));
        }

        if triples.is_empty() {
            triples = "# No functions inherited\n".to_string();
        }

        triples
    }

    /// Read kernel configuration from conceptkernel.yaml
    fn read_kernel_config(&self, kernel_dir: &Path) -> Result<KernelConfig> {
        let yaml_path = kernel_dir.join("conceptkernel.yaml");

        if !yaml_path.exists() {
            return Err(CkpError::FileNotFound(format!(
                "conceptkernel.yaml not found in {}",
                kernel_dir.display()
            )));
        }

        let yaml_content = fs::read_to_string(&yaml_path)
            .map_err(|e| CkpError::IoError(format!("Failed to read conceptkernel.yaml: {}", e)))?;

        let yaml: serde_yaml::Value = serde_yaml::from_str(&yaml_content)
            .map_err(|e| CkpError::ParseError(format!("Failed to parse YAML: {}", e)))?;

        // Extract metadata
        let metadata = yaml.get("metadata")
            .ok_or_else(|| CkpError::ParseError("Missing metadata section".to_string()))?;

        let config = KernelConfig {
            name: metadata.get("name")
                .and_then(|v: &serde_yaml::Value| v.as_str())
                .unwrap_or("Unknown")
                .to_string(),
            version: metadata.get("version")
                .and_then(|v: &serde_yaml::Value| v.as_str())
                .unwrap_or("0.1.0")
                .to_string(),
            kernel_type: metadata.get("type")
                .and_then(|v: &serde_yaml::Value| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            description: metadata.get("description")
                .and_then(|v: &serde_yaml::Value| v.as_str())
                .map(|s: &str| s.to_string()),
        };

        Ok(config)
    }
}

#[derive(Debug, Clone)]
struct KernelConfig {
    name: String,
    version: String,
    kernel_type: String,
    description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_generate_default_role() {
        let temp_dir = TempDir::new().unwrap();
        let generator = OntologyGenerator::new(temp_dir.path().to_path_buf());

        let role = generator.generate_default_role("Test.Kernel", "rust:hot");
        assert!(role.contains("Service Provider"));
        assert!(role.contains("Test.Kernel"));
    }

    #[test]
    fn test_generate_default_function() {
        let temp_dir = TempDir::new().unwrap();
        let generator = OntologyGenerator::new(temp_dir.path().to_path_buf());

        let func = generator.generate_default_function("Test.Kernel", "node:cold");
        assert!(func.contains("Node.js Processing"));
        assert!(func.contains("Test.Kernel"));
    }
}
