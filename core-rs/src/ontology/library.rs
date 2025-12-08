/**
 * library.rs
 * RDF-based ontology library using standard URN resolution
 */

use oxigraph::store::Store;
use oxigraph::model::NamedNode;
use oxigraph::io::RdfFormat;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::urn::UrnResolver;
use crate::project::ProjectConfig;

#[derive(Error, Debug)]
pub enum OntologyError {
    #[error("Failed to load ontology: {0}")]
    LoadError(String),
    
    #[error("Failed to parse RDF: {0}")]
    ParseError(String),
    
    #[error("Ontology not found: {0}")]
    NotFound(String),
    
    #[error("Query error: {0}")]
    QueryError(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Oxigraph error: {0}")]
    StoreError(String),
    
    #[error("URN resolution error: {0}")]
    UrnError(String),
}

/// Metadata for a kernel role
#[derive(Debug, Clone)]
pub struct RoleMetadata {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub bearer_urn: String,
    pub context: String,
}

/// Metadata for a kernel function
#[derive(Debug, Clone)]
pub struct FunctionMetadata {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub realized_by_urn: String,
    pub capabilities: Vec<String>,
}

/// Metadata for a kernel entity
#[derive(Debug, Clone)]
pub struct KernelMetadata {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub urn: String,
    pub kernel_type: String,
    pub version: String,
}

pub struct OntologyLibrary {
    store: Store,
    pub project_root: PathBuf,
    kernel_graphs: HashMap<String, String>,
}

impl OntologyLibrary {
    /// Create and load ontology library from .ckproject
    pub fn new(project_root: PathBuf) -> Result<Self, OntologyError> {
        let store = Store::new()
            .map_err(|e| OntologyError::StoreError(e.to_string()))?;
        
        let mut library = Self {
            store,
            project_root: project_root.clone(),
            kernel_graphs: HashMap::new(),
        };
        
        // Load core ontologies from .ckproject
        library.load_from_project(&project_root)?;
        
        Ok(library)
    }
    
    /// Load ontologies defined in .ckproject
    fn load_from_project(&mut self, project_root: &Path) -> Result<(), OntologyError> {
        let ckproject_path = project_root.join(".ckproject");

        if !ckproject_path.exists() {
            eprintln!("[OntologyLibrary] ⚠️  WARNING: .ckproject not found at {}", ckproject_path.display());
            eprintln!("[OntologyLibrary]    Canonical ontologies will not be loaded");
            return Ok(()); // Optional
        }

        let config = ProjectConfig::load(&ckproject_path)
            .map_err(|e| OntologyError::LoadError(e.to_string()))?;

        if let Some(ontology_config) = config.spec.ontology {
            eprintln!("[OntologyLibrary] Loading canonical ontologies from .ckproject...");

            // Load core ontologies - support both file:// and URN formats
            if let Err(e) = self.load_ontology_reference(&ontology_config.core, "https://conceptkernel.org/ontology/core") {
                eprintln!("[OntologyLibrary] ⚠️  WARNING: Failed to load core ontology: {}", e);
            }

            if let Err(e) = self.load_ontology_reference(&ontology_config.bfo, "http://purl.obolibrary.org/obo/bfo.owl") {
                eprintln!("[OntologyLibrary] ⚠️  WARNING: Failed to load BFO ontology: {}", e);
            }

            if let Err(e) = self.load_ontology_reference(&ontology_config.predicates, "https://conceptkernel.org/ontology/predicates") {
                eprintln!("[OntologyLibrary] ⚠️  WARNING: Failed to load predicates ontology: {}", e);
            }

            // Load optional ontologies
            if let Some(processes) = ontology_config.processes.as_ref() {
                if let Err(e) = self.load_ontology_reference(processes, "https://conceptkernel.org/ontology/processes") {
                    eprintln!("[OntologyLibrary] ⚠️  WARNING: Failed to load processes ontology: {}", e);
                }
            }

            if let Some(rbac) = ontology_config.rbac.as_ref() {
                if let Err(e) = self.load_ontology_reference(rbac, "https://conceptkernel.org/ontology/rbac") {
                    eprintln!("[OntologyLibrary] ⚠️  WARNING: Failed to load RBAC ontology: {}", e);
                }
            }

            if let Some(improvement) = ontology_config.improvement.as_ref() {
                if let Err(e) = self.load_ontology_reference(improvement, "https://conceptkernel.org/ontology/improvement") {
                    eprintln!("[OntologyLibrary] ⚠️  WARNING: Failed to load self-improvement ontology: {}", e);
                }
            }

            if let Some(workflow) = ontology_config.workflow.as_ref() {
                if let Err(e) = self.load_ontology_reference(workflow, "https://conceptkernel.org/ontology/workflow") {
                    eprintln!("[OntologyLibrary] ⚠️  WARNING: Failed to load workflow ontology: {}", e);
                }
            }

            eprintln!("[OntologyLibrary] ✓ Canonical ontologies loaded from .ckproject");
        } else {
            eprintln!("[OntologyLibrary] ⚠️  WARNING: No spec.ontology section in .ckproject");
            eprintln!("[OntologyLibrary]    Canonical ontologies from /concepts/.ontology/ will not be loaded");
            eprintln!("[OntologyLibrary]    Add spec.ontology section referencing canonical ontology files");
        }

        Ok(())
    }

    /// Load ontology from file:// or ckp:// reference
    fn load_ontology_reference(&mut self, reference: &str, graph_uri: &str) -> Result<(), OntologyError> {
        if reference.starts_with("file://") {
            // Local file reference
            let path_str = reference.strip_prefix("file://").unwrap();
            let path = if path_str.starts_with("./") {
                self.project_root.join(path_str.strip_prefix("./").unwrap())
            } else {
                PathBuf::from(path_str)
            };

            if !path.exists() {
                return Err(OntologyError::NotFound(format!(
                    "Ontology file not found: {} (resolved to {})",
                    reference, path.display()
                )));
            }

            self.load_ontology_file(&path, graph_uri)
        } else if reference.starts_with("ckp://") {
            // URN reference - delegate to existing URN loader
            self.load_ontology_urn(reference, graph_uri)
        } else if reference.starts_with("http://") || reference.starts_with("https://") {
            // HTTP reference - for BFO, just note it (don't try to fetch)
            eprintln!("[OntologyLibrary]    Skipping remote ontology: {}", reference);
            Ok(())
        } else {
            Err(OntologyError::LoadError(format!(
                "Unsupported ontology reference format: {} (expected file://, ckp://, or http(s)://)",
                reference
            )))
        }
    }
    
    /// Load ontology via URN resolution
    ///
    /// URN: ckp://ConceptKernel.Ontology:v1.3.16#storage/ck-predicates
    /// Resolves to: concepts/ConceptKernel.Ontology/storage/ck-predicates.v1.3.16.ttl
    fn load_ontology_urn(&mut self, urn: &str, graph_uri: &str) -> Result<(), OntologyError> {
        eprintln!("[OntologyLibrary] Resolving URN: {}", urn);

        // Parse URN (format: ckp://kernel:version#stage/path)
        let parsed = UrnResolver::parse(urn)
            .map_err(|e| OntologyError::UrnError(e.to_string()))?;

        eprintln!("[OntologyLibrary] Parsed - kernel: {}, stage: {}, path: {:?}, version: {}",
            parsed.kernel,
            parsed.stage.as_ref().unwrap_or(&"none".to_string()),
            parsed.path,
            parsed.version
        );

        // Build filesystem path from parsed components
        // Use UrnResolver to get base path: concepts/{kernel}/{stage}
        let base_path = UrnResolver::resolve_to_path(urn, &self.project_root.join("concepts"))
            .map_err(|e| OntologyError::UrnError(e.to_string()))?;

        // The base_path includes stage but not the filename yet
        // We need to append .v{version}.ttl to the final path component
        if let Some(path_str) = parsed.path {
            // Remove the path from base_path and re-add with version
            let parent = base_path.parent().ok_or_else(|| {
                OntologyError::LoadError(format!("Invalid URN path resolution: {}", urn))
            })?;

            let version = parsed.version.trim_start_matches('v');
            let filename = format!("{}.v{}.ttl", path_str, version);
            let full_path = parent.join(filename);

            eprintln!("[OntologyLibrary] Resolved path: {:?}", full_path);

            if !full_path.exists() {
                return Err(OntologyError::NotFound(format!(
                    "Ontology not found: {} (resolved to {:?})",
                    urn, full_path
                )));
            }

            self.load_ontology_file(&full_path, graph_uri)?;
        } else {
            return Err(OntologyError::LoadError(format!(
                "URN missing path component (filename): {}",
                urn
            )));
        }

        Ok(())
    }
    
    /// Load kernel's ontology from ontology.ttl
    pub fn load_kernel_ontology(&mut self, kernel_name: &str) -> Result<(), OntologyError> {
        let ontology_path = self.project_root
            .join("concepts")
            .join(kernel_name)
            .join("ontology.ttl");

        if !ontology_path.exists() {
            return Err(OntologyError::NotFound(format!(
                "No ontology.ttl found for kernel: {}",
                kernel_name
            )));
        }

        // Validate ontology imports before loading
        self.validate_kernel_ontology_imports(kernel_name, &ontology_path)?;

        let graph_uri = format!(
            "https://conceptkernel.org/ontology/{}",
            kernel_name.to_lowercase().replace(".", "-")
        );

        self.load_ontology_file(&ontology_path, &graph_uri)?;
        self.kernel_graphs.insert(kernel_name.to_string(), graph_uri);

        Ok(())
    }

    /// Validate that kernel ontology properly imports canonical ontologies
    fn validate_kernel_ontology_imports(&self, kernel_name: &str, ontology_path: &Path) -> Result<(), OntologyError> {
        use std::fs;

        let content = fs::read_to_string(ontology_path)
            .map_err(|e| OntologyError::LoadError(format!("Failed to read ontology.ttl: {}", e)))?;

        let mut warnings = Vec::new();

        // Check for required owl:imports declarations
        if !content.contains("owl:imports <https://conceptkernel.org/ontology/core>") &&
           !content.contains("owl:imports <http://conceptkernel.org/ontology/core>") {
            warnings.push("Missing owl:imports for ConceptKernel core ontology");
        }

        if !content.contains("owl:imports <http://purl.obolibrary.org/obo/bfo.owl>") {
            warnings.push("Missing owl:imports for BFO (Basic Formal Ontology)");
        }

        // Check for BFO class usage
        let uses_bfo = content.contains("bfo:0000040") || // MaterialEntity
                       content.contains("bfo:0000023") || // Role
                       content.contains("bfo:0000034") || // Function
                       content.contains("bfo:0000016");   // Disposition

        if !uses_bfo {
            warnings.push("No BFO class usage detected - ontology may not be properly aligned");
        }

        // Check for ckp: namespace usage
        let uses_ckp = content.contains("ckp:Kernel") ||
                       content.contains("ckp:bearer") ||
                       content.contains("ckp:realizedBy");

        if !uses_ckp {
            warnings.push("No ConceptKernel namespace (ckp:) usage detected - ontology may not be properly aligned");
        }

        // Emit warnings
        if !warnings.is_empty() {
            eprintln!("[OntologyLibrary] ⚠️  WARNING: Kernel '{}' ontology validation issues:", kernel_name);
            for warning in &warnings {
                eprintln!("[OntologyLibrary]    • {}", warning);
            }
            eprintln!("[OntologyLibrary]    Location: {}", ontology_path.display());
            eprintln!("[OntologyLibrary]    Each kernel ontology.ttl should import canonical ontologies:");
            eprintln!("[OntologyLibrary]      owl:imports <https://conceptkernel.org/ontology/core> ;");
            eprintln!("[OntologyLibrary]      owl:imports <http://purl.obolibrary.org/obo/bfo.owl> ;");
        }

        Ok(())
    }
    
    fn load_ontology_file(&mut self, path: &Path, graph_uri: &str) -> Result<(), OntologyError> {
        eprintln!("[OntologyLibrary] Loading ontology file: {:?}", path);
        eprintln!("[OntologyLibrary] Graph URI: {}", graph_uri);

        if !path.exists() {
            return Err(OntologyError::NotFound(format!("File not found: {:?}", path)));
        }

        if path.is_dir() {
            return Err(OntologyError::LoadError(format!("Path is a directory: {:?}", path)));
        }

        let content = fs::read_to_string(path)?;
        
        let _graph_name = NamedNode::new(graph_uri)
            .map_err(|e| OntologyError::ParseError(e.to_string()))?;
        
        self.store
            .load_from_reader(
                RdfFormat::Turtle,
                content.as_bytes(),
            )
            .map_err(|e| OntologyError::LoadError(e.to_string()))?;
        
        Ok(())
    }
    
    /// Get edge predicate for edge name
    /// 
    /// Example: "REQUIRES" → "ckp:requires"
    pub fn get_edge_predicate(&self, edge_name: &str) -> Result<String, OntologyError> {
        let query = format!(
            r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>
            
            SELECT ?predicate
            FROM <https://conceptkernel.org/ontology/predicates>
            WHERE {{
                ?mapping ckp:edgeName "{}" ;
                         ckp:predicate ?predicate .
            }}
            "#,
            edge_name
        );
        
        let results = self.query_sparql(&query)?;
        
        results.first()
            .and_then(|row| row.get("predicate"))
            .map(|s| s.to_string())
            .ok_or_else(|| OntologyError::NotFound(format!(
                "No predicate mapping for edge: {}",
                edge_name
            )))
    }
    
    /// Get kernel classes
    pub fn get_kernel_classes(&self, kernel_name: &str) -> Result<Vec<String>, OntologyError> {
        let graph_uri = self.kernel_graphs.get(kernel_name)
            .ok_or_else(|| OntologyError::NotFound(format!(
                "Kernel ontology not loaded: {}",
                kernel_name
            )))?;
        
        let query = format!(
            r#"
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            PREFIX owl: <http://www.w3.org/2002/07/owl#>
            
            SELECT ?class
            FROM <{}>
            WHERE {{
                ?class rdf:type owl:Class .
            }}
            "#,
            graph_uri
        );
        
        let results = self.query_sparql(&query)?;
        
        Ok(results.iter()
            .filter_map(|row| row.get("class").map(|s| s.to_string()))
            .collect())
    }

    // ========================================================================
    // KERNEL ENTITY QUERIES (Phase 1)
    // ========================================================================

    /// Get kernel URN
    ///
    /// Returns the canonical URN for a kernel entity.
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name (e.g., "System.Gateway")
    ///
    /// # Returns
    /// Kernel URN in format `ckp://Continuant#Kernel-{name}`
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut lib = OntologyLibrary::new(PathBuf::from("/project"))?;
    /// lib.load_kernel_ontology("System.Gateway")?;
    /// let urn = lib.get_kernel_urn("System.Gateway")?;
    /// assert_eq!(urn, "ckp://Continuant#Kernel-System.Gateway");
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_kernel_urn(&self, kernel_name: &str) -> Result<String, OntologyError> {
        Ok(format!("ckp://Continuant#Kernel-{}", kernel_name))
    }

    /// Get roles for a kernel
    ///
    /// Returns all BFO Role entities (bfo:0000023) that the kernel bears.
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name (e.g., "System.Gateway")
    ///
    /// # Returns
    /// Vector of RoleMetadata with role information
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut lib = OntologyLibrary::new(PathBuf::from("/project"))?;
    /// lib.load_kernel_ontology("System.Gateway")?;
    /// let roles = lib.get_kernel_roles("System.Gateway")?;
    /// assert_eq!(roles.len(), 2); // HTTP Ingress, Request Router
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_kernel_roles(&self, kernel_name: &str) -> Result<Vec<RoleMetadata>, OntologyError> {
        let graph_uri = self.kernel_graphs.get(kernel_name)
            .ok_or_else(|| OntologyError::NotFound(format!(
                "Kernel ontology not loaded: {}",
                kernel_name
            )))?;

        let kernel_uri = format!("https://conceptkernel.org/kernel/{}", kernel_name);

        let query = format!(
            r#"
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX ckp: <https://conceptkernel.org/ontology#>
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

            SELECT ?role ?label ?comment ?context
            FROM <{}>
            WHERE {{
                ?role rdf:type bfo:0000023 ;
                      ckp:bearer <{}> ;
                      rdfs:label ?label ;
                      rdfs:comment ?comment ;
                      ckp:roleContext ?context .
            }}
            "#,
            graph_uri, kernel_uri
        );

        let results = self.query_sparql(&query)?;

        Ok(results.iter()
            .filter_map(|row| {
                Some(RoleMetadata {
                    uri: row.get("role")?.to_string(),
                    name: row.get("label")?.to_string(),
                    description: row.get("comment")?.to_string(),
                    bearer_urn: self.get_kernel_urn(kernel_name).ok()?,
                    context: row.get("context")?.to_string(),
                })
            })
            .collect())
    }

    /// Get functions for a kernel
    ///
    /// Returns all BFO Function entities (bfo:0000034) that the kernel realizes.
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name (e.g., "System.Gateway")
    ///
    /// # Returns
    /// Vector of FunctionMetadata with function information
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut lib = OntologyLibrary::new(PathBuf::from("/project"))?;
    /// lib.load_kernel_ontology("System.Gateway")?;
    /// let functions = lib.get_kernel_functions("System.Gateway")?;
    /// assert!(functions[0].capabilities.contains(&"ingress-routing".to_string()));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_kernel_functions(&self, kernel_name: &str) -> Result<Vec<FunctionMetadata>, OntologyError> {
        let graph_uri = self.kernel_graphs.get(kernel_name)
            .ok_or_else(|| OntologyError::NotFound(format!(
                "Kernel ontology not loaded: {}",
                kernel_name
            )))?;

        let kernel_uri = format!("https://conceptkernel.org/kernel/{}", kernel_name);

        let query = format!(
            r#"
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX ckp: <https://conceptkernel.org/ontology#>
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

            SELECT ?function ?label ?comment ?capability
            FROM <{}>
            WHERE {{
                ?function rdf:type bfo:0000034 ;
                          ckp:realizedBy <{}> ;
                          rdfs:label ?label ;
                          rdfs:comment ?comment ;
                          ckp:capability ?capability .
            }}
            "#,
            graph_uri, kernel_uri
        );

        let results = self.query_sparql(&query)?;

        // Group capabilities by function URI
        let mut functions_map: HashMap<String, FunctionMetadata> = HashMap::new();

        for row in results {
            let uri = match row.get("function") {
                Some(u) => u.to_string(),
                None => continue,
            };

            let capability = match row.get("capability") {
                Some(c) => c.to_string(),
                None => continue,
            };

            functions_map.entry(uri.clone())
                .and_modify(|f| f.capabilities.push(capability.clone()))
                .or_insert_with(|| {
                    FunctionMetadata {
                        uri: uri.clone(),
                        name: row.get("label").map(|s| s.to_string()).unwrap_or_default(),
                        description: row.get("comment").map(|s| s.to_string()).unwrap_or_default(),
                        realized_by_urn: self.get_kernel_urn(kernel_name).unwrap_or_default(),
                        capabilities: vec![capability],
                    }
                });
        }

        Ok(functions_map.into_values().collect())
    }

    /// Get full metadata for a kernel entity
    ///
    /// Returns complete metadata including name, description, URN, type, version.
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name (e.g., "System.Gateway")
    ///
    /// # Returns
    /// KernelMetadata with kernel information
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut lib = OntologyLibrary::new(PathBuf::from("/project"))?;
    /// lib.load_kernel_ontology("System.Gateway")?;
    /// let metadata = lib.get_kernel_metadata("System.Gateway")?;
    /// assert_eq!(metadata.kernel_type, "rust:hot");
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_kernel_metadata(&self, kernel_name: &str) -> Result<KernelMetadata, OntologyError> {
        let graph_uri = self.kernel_graphs.get(kernel_name)
            .ok_or_else(|| OntologyError::NotFound(format!(
                "Kernel ontology not loaded: {}",
                kernel_name
            )))?;

        let kernel_uri = format!("https://conceptkernel.org/kernel/{}", kernel_name);

        let query = format!(
            r#"
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX ckp: <https://conceptkernel.org/ontology#>
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

            SELECT ?label ?comment ?urn ?type ?version
            FROM <{}>
            WHERE {{
                <{}> rdf:type bfo:0000040 ;
                     rdf:type ckp:Kernel ;
                     rdfs:label ?label ;
                     rdfs:comment ?comment ;
                     ckp:hasUrn ?urn ;
                     ckp:kernelType ?type ;
                     ckp:version ?version .
            }}
            "#,
            graph_uri, kernel_uri
        );

        let results = self.query_sparql(&query)?;

        results.first()
            .ok_or_else(|| OntologyError::NotFound(format!(
                "No kernel metadata found for: {}",
                kernel_name
            )))
            .and_then(|row| {
                Ok(KernelMetadata {
                    uri: kernel_uri,
                    name: row.get("label")
                        .ok_or_else(|| OntologyError::QueryError("Missing label".to_string()))?
                        .to_string(),
                    description: row.get("comment")
                        .ok_or_else(|| OntologyError::QueryError("Missing comment".to_string()))?
                        .to_string(),
                    urn: row.get("urn")
                        .ok_or_else(|| OntologyError::QueryError("Missing URN".to_string()))?
                        .to_string(),
                    kernel_type: row.get("type")
                        .ok_or_else(|| OntologyError::QueryError("Missing type".to_string()))?
                        .to_string(),
                    version: row.get("version")
                        .ok_or_else(|| OntologyError::QueryError("Missing version".to_string()))?
                        .to_string(),
                })
            })
    }

    /// Execute SPARQL query
    pub fn query_sparql(&self, query: &str) -> Result<Vec<HashMap<String, String>>, OntologyError> {
        use oxigraph::sparql::QueryResults;
        
        let results = self.store
            .query(query)
            .map_err(|e| OntologyError::QueryError(e.to_string()))?;
        
        match results {
            QueryResults::Solutions(solutions) => {
                let mut rows = Vec::new();
                
                for solution in solutions {
                    let solution = solution
                        .map_err(|e| OntologyError::QueryError(e.to_string()))?;
                    
                    let mut row = HashMap::new();
                    
                    for (var, term) in solution.iter() {
                        row.insert(var.as_str().to_string(), term.to_string());
                    }
                    
                    rows.push(row);
                }
                
                Ok(rows)
            }
            QueryResults::Boolean(result) => {
                let mut row = HashMap::new();
                row.insert("result".to_string(), result.to_string());
                Ok(vec![row])
            }
            QueryResults::Graph(_) => {
                Err(OntologyError::QueryError(
                    "Graph queries not yet supported".to_string()
                ))
            }
        }
    }
    
    /// Check if class is BFO Occurrent (temporal entity)
    pub fn is_temporal_entity(&self, class_uri: &str) -> Result<bool, OntologyError> {
        let query = format!(
            r#"
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>

            ASK {{
                <{}> rdfs:subClassOf* bfo:0000003 .
            }}
            "#,
            class_uri
        );

        let results = self.query_sparql(&query)?;

        Ok(results.get(0)
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false))
    }

    // ========================================================================
    // BFO VALIDATION QUERIES (Phase 4 Stage 3)
    // ========================================================================

    /// Check if class is BFO Continuant (persistent entity)
    ///
    /// Continuants are entities that persist through time (kernels, agents, etc.)
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let library = OntologyLibrary::new(PathBuf::from("/project"))?;
    /// let is_continuant = library.is_continuant("https://conceptkernel.org/ontology#Kernel")?;
    /// assert!(is_continuant); // ckp:Kernel is subclass of bfo:Continuant
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_continuant(&self, class_uri: &str) -> Result<bool, OntologyError> {
        let query = format!(
            r#"
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>

            ASK {{
                <{}> rdfs:subClassOf* bfo:0000002 .
            }}
            "#,
            class_uri
        );

        let results = self.query_sparql(&query)?;

        Ok(results.get(0)
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false))
    }

    /// Check if class is BFO Occurrent (temporal entity)
    ///
    /// Alias for `is_temporal_entity()` with BFO nomenclature
    ///
    /// Occurrents are entities that occur in time (processes, events, etc.)
    pub fn is_occurrent(&self, class_uri: &str) -> Result<bool, OntologyError> {
        self.is_temporal_entity(class_uri)
    }

    /// Check if class is BFO Material Entity
    ///
    /// Material Entities are independent continuants (kernels, physical objects)
    pub fn is_material_entity(&self, class_uri: &str) -> Result<bool, OntologyError> {
        let query = format!(
            r#"
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>

            ASK {{
                <{}> rdfs:subClassOf* bfo:0000040 .
            }}
            "#,
            class_uri
        );

        let results = self.query_sparql(&query)?;

        Ok(results.get(0)
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false))
    }

    /// Check if class is BFO Process
    ///
    /// Processes are occurrents that unfold over time with temporal parts
    pub fn is_process(&self, class_uri: &str) -> Result<bool, OntologyError> {
        let query = format!(
            r#"
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>

            ASK {{
                <{}> rdfs:subClassOf* bfo:0000015 .
            }}
            "#,
            class_uri
        );

        let results = self.query_sparql(&query)?;

        Ok(results.get(0)
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false))
    }

    /// Get BFO classification of a class
    ///
    /// Returns the top-level BFO category (Continuant, Occurrent, or None)
    pub fn get_bfo_classification(&self, class_uri: &str) -> Result<String, OntologyError> {
        if self.is_continuant(class_uri)? {
            Ok("Continuant".to_string())
        } else if self.is_occurrent(class_uri)? {
            Ok("Occurrent".to_string())
        } else {
            Ok("None".to_string())
        }
    }

    pub fn loaded_kernels(&self) -> Vec<String> {
        self.kernel_graphs.keys().cloned().collect()
    }

    // ========================================================================
    // TEMPORAL REASONING QUERIES (Phase 4 Stage 3)
    // ========================================================================

    /// Check if process1 temporally precedes process2
    ///
    /// Uses BFO temporal relations to check if process1 ends before process2 starts.
    /// Requires timestamp metadata in process storage.
    ///
    /// # Arguments
    /// * `process1_uri` - First process URI
    /// * `process2_uri` - Second process URI
    ///
    /// # Returns
    /// true if process1 temporally precedes process2
    pub fn process_precedes(&self, process1_uri: &str, process2_uri: &str) -> Result<bool, OntologyError> {
        let query = format!(
            r#"
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX ckp: <https://conceptkernel.org/ontology#>

            ASK {{
                <{}> ckp:endTime ?end1 .
                <{}> ckp:startTime ?start2 .
                FILTER (?end1 < ?start2)
            }}
            "#,
            process1_uri, process2_uri
        );

        let results = self.query_sparql(&query)?;

        Ok(results.get(0)
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false))
    }

    /// Check if two processes temporally overlap
    ///
    /// Returns true if the processes have any temporal intersection.
    ///
    /// # Arguments
    /// * `process1_uri` - First process URI
    /// * `process2_uri` - Second process URI
    ///
    /// # Returns
    /// true if processes overlap in time
    pub fn processes_overlap(&self, process1_uri: &str, process2_uri: &str) -> Result<bool, OntologyError> {
        let query = format!(
            r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>

            ASK {{
                <{}> ckp:startTime ?start1 ;
                     ckp:endTime ?end1 .
                <{}> ckp:startTime ?start2 ;
                     ckp:endTime ?end2 .

                # Overlap if: start1 <= end2 AND start2 <= end1
                FILTER (?start1 <= ?end2 && ?start2 <= ?end1)
            }}
            "#,
            process1_uri, process2_uri
        );

        let results = self.query_sparql(&query)?;

        Ok(results.get(0)
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false))
    }

    /// Check if process1 occurs during process2
    ///
    /// Returns true if process1's timespan is entirely within process2's timespan.
    ///
    /// # Arguments
    /// * `process1_uri` - Inner process URI
    /// * `process2_uri` - Outer process URI
    ///
    /// # Returns
    /// true if process1 occurs during process2
    pub fn process_during(&self, process1_uri: &str, process2_uri: &str) -> Result<bool, OntologyError> {
        let query = format!(
            r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>

            ASK {{
                <{}> ckp:startTime ?start1 ;
                     ckp:endTime ?end1 .
                <{}> ckp:startTime ?start2 ;
                     ckp:endTime ?end2 .

                # During if: start2 <= start1 AND end1 <= end2
                FILTER (?start2 <= ?start1 && ?end1 <= ?end2)
            }}
            "#,
            process1_uri, process2_uri
        );

        let results = self.query_sparql(&query)?;

        Ok(results.get(0)
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false))
    }

    /// Get all processes that temporally overlap with a given process
    ///
    /// Returns URIs of all processes that have temporal intersection.
    ///
    /// # Arguments
    /// * `process_uri` - Process URI to check
    /// * `graph_uri` - Named graph to query (e.g., kernel-specific graph)
    ///
    /// # Returns
    /// Vector of overlapping process URIs
    pub fn get_overlapping_processes(&self, process_uri: &str, graph_uri: &str) -> Result<Vec<String>, OntologyError> {
        let query = format!(
            r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?process
            FROM <{}>
            WHERE {{
                <{}> ckp:startTime ?start1 ;
                     ckp:endTime ?end1 .

                ?process rdf:type ckp:Process ;
                         ckp:startTime ?start2 ;
                         ckp:endTime ?end2 .

                # Overlap condition
                FILTER (?start1 <= ?end2 && ?start2 <= ?end1)

                # Exclude self
                FILTER (?process != <{}>)
            }}
            "#,
            graph_uri, process_uri, process_uri
        );

        let results = self.query_sparql(&query)?;

        Ok(results.iter()
            .filter_map(|row| row.get("process").map(|s| s.to_string()))
            .collect())
    }

    /// Get temporal ordering of processes
    ///
    /// Returns processes sorted by start time.
    ///
    /// # Arguments
    /// * `graph_uri` - Named graph to query
    ///
    /// # Returns
    /// Vector of (process_uri, start_time) tuples sorted chronologically
    pub fn get_process_timeline(&self, graph_uri: &str) -> Result<Vec<(String, String)>, OntologyError> {
        let query = format!(
            r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?process ?startTime
            FROM <{}>
            WHERE {{
                ?process rdf:type ckp:Process ;
                         ckp:startTime ?startTime .
            }}
            ORDER BY ?startTime
            "#,
            graph_uri
        );

        let results = self.query_sparql(&query)?;

        Ok(results.iter()
            .filter_map(|row| {
                let process = row.get("process")?.to_string();
                let start_time = row.get("startTime")?.to_string();
                Some((process, start_time))
            })
            .collect())
    }

    // ========================================================================
    // PERMISSION QUERIES (Phase 2 - RBAC)
    // ========================================================================

    /// Get all roles assigned to an agent
    ///
    /// Queries the ontology for roles linked to the agent via `ckp:hasRole`.
    ///
    /// # Arguments
    /// * `agent_urn` - Agent URN (format: `ckp://Agent/user:{username}` or `ckp://Agent/process:{KernelName}`)
    ///
    /// # Returns
    /// Vector of role URNs
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let library = OntologyLibrary::new(PathBuf::from("/project"))?;
    /// let roles = library.get_agent_roles("ckp://Agent/user:admin")?;
    /// for role in roles {
    ///     println!("Role: {}", role);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_agent_roles(&self, agent_urn: &str) -> Result<Vec<String>, OntologyError> {
        let query = format!(
            r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>

            SELECT ?role
            WHERE {{
                <{agent_urn}> ckp:hasRole ?role .
            }}
            "#,
            agent_urn = agent_urn
        );

        let results = self.query_sparql(&query)?;

        Ok(results.iter()
            .filter_map(|row| row.get("role").map(|s| s.to_string()))
            .collect())
    }

    /// Get all permissions granted by a role
    ///
    /// Queries the ontology for permissions linked to the role via `ckp:grants`.
    ///
    /// # Arguments
    /// * `role_urn` - Role URN (e.g., `ckp://Role/system-admin`)
    ///
    /// # Returns
    /// Vector of permission strings in dot notation (e.g., `["http.handle", "kernel.route"]`)
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let library = OntologyLibrary::new(PathBuf::from("/project"))?;
    /// let perms = library.get_role_permissions("ckp://Role/system-admin")?;
    /// for perm in perms {
    ///     println!("Permission: {}", perm);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_role_permissions(&self, role_urn: &str) -> Result<Vec<String>, OntologyError> {
        let query = format!(
            r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>

            SELECT ?permString
            WHERE {{
                <{role_urn}> ckp:grants ?permission .
                ?permission ckp:permissionString ?permString .
            }}
            "#,
            role_urn = role_urn
        );

        let results = self.query_sparql(&query)?;

        Ok(results.iter()
            .filter_map(|row| row.get("permString").map(|s| s.to_string()))
            .collect())
    }

    /// Check if an agent has a specific permission
    ///
    /// Performs transitive permission check: agent → role → permission.
    /// Uses SPARQL ASK query to verify the agent has the permission through any of their roles.
    ///
    /// # Arguments
    /// * `agent_urn` - Agent URN (format: `ckp://Agent/user:{username}` or `ckp://Agent/process:{KernelName}`)
    /// * `permission` - Permission string in dot notation (e.g., `"http.handle"`)
    ///
    /// # Returns
    /// `true` if agent has permission through any role, `false` otherwise
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let library = OntologyLibrary::new(PathBuf::from("/project"))?;
    /// let has_perm = library.check_agent_permission(
    ///     "ckp://Agent/user:admin",
    ///     "http.handle"
    /// )?;
    /// if has_perm {
    ///     println!("Agent has permission");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn check_agent_permission(&self, agent_urn: &str, permission: &str) -> Result<bool, OntologyError> {
        let query = format!(
            r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            ASK {{
                <{agent_urn}> ckp:hasRole ?role .
                ?role rdf:type bfo:0000023 .
                ?role ckp:grants ?perm .
                ?perm ckp:permissionString "{permission}" .
            }}
            "#,
            agent_urn = agent_urn,
            permission = permission
        );

        // Execute ASK query using Oxigraph store directly
        use oxigraph::sparql::QueryResults;

        let query_obj = oxigraph::sparql::Query::parse(&query, None)
            .map_err(|e| OntologyError::QueryError(e.to_string()))?;

        let result = self.store.query(query_obj)
            .map_err(|e| OntologyError::QueryError(e.to_string()))?;

        match result {
            QueryResults::Boolean(b) => Ok(b),
            _ => Ok(false), // ASK should always return boolean
        }
    }

    /// Get quorum level required for a permission
    ///
    /// Queries the ontology for the quorum requirement of a permission.
    ///
    /// # Arguments
    /// * `permission` - Permission string in dot notation (e.g., `"consensus.enforce"`)
    ///
    /// # Returns
    /// Quorum level URI (e.g., `"https://conceptkernel.org/ontology#QuorumHigh"`)
    ///
    /// # Errors
    /// Returns `OntologyError::NotFound` if permission or quorum level not found
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let library = OntologyLibrary::new(PathBuf::from("/project"))?;
    /// let quorum = library.get_permission_quorum("consensus.enforce")?;
    /// println!("Quorum level: {}", quorum);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_permission_quorum(&self, permission: &str) -> Result<String, OntologyError> {
        let query = format!(
            r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>

            SELECT ?quorum
            WHERE {{
                ?perm ckp:permissionString "{permission}" .
                ?perm ckp:requiresQuorum ?quorum .
            }}
            "#,
            permission = permission
        );

        let results = self.query_sparql(&query)?;

        results.first()
            .and_then(|row| row.get("quorum"))
            .map(|s| s.to_string())
            .ok_or_else(|| OntologyError::NotFound(format!(
                "No quorum level found for permission: {}",
                permission
            )))
    }

    /// Get all permissions for an agent (transitive via roles)
    ///
    /// Convenience method that combines get_agent_roles() and get_role_permissions()
    /// to return all permissions an agent has through all their roles.
    ///
    /// # Arguments
    /// * `agent_urn` - Agent URN (format: `ckp://Agent/user:{username}` or `ckp://Agent/process:{KernelName}`)
    ///
    /// # Returns
    /// Vector of permission strings (deduplicated)
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let library = OntologyLibrary::new(PathBuf::from("/project"))?;
    /// let perms = library.get_agent_permissions("ckp://Agent/user:admin")?;
    /// for perm in perms {
    ///     println!("Permission: {}", perm);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_agent_permissions(&self, agent_urn: &str) -> Result<Vec<String>, OntologyError> {
        let roles = self.get_agent_roles(agent_urn)?;

        let mut permissions = std::collections::HashSet::new();
        for role in roles {
            let role_perms = self.get_role_permissions(&role)?;
            permissions.extend(role_perms);
        }

        Ok(permissions.into_iter().collect())
    }

    // ========================================================================
    // GENERIC QUERY API (v2)
    // ========================================================================

    /// Generic query method that works across all kernels
    ///
    /// This method makes queries generic since all kernels share the same BFO
    /// ontological foundation. It supports both kernel-scoped and global queries.
    ///
    /// # Query Patterns
    ///
    /// 1. Kernel-scoped: `ckp://{Kernel}:{Version}/{Resource}?params`
    /// 2. Global query: `ckp://{Resource}?params`
    /// 3. Backward compat: `ckp://{Kernel}?view={resource}&params`
    ///
    /// # Supported Resources
    ///
    /// All BFO Occurrents:
    /// - `Process` - All processes (BFO:0000015)
    /// - `Workflow` - Workflow processes
    /// - `ImprovementProcess` - Self-improvement processes
    /// - `ConsensusProcess` - Consensus governance processes
    /// - `WorkflowPhase` - Temporal parts of workflows
    ///
    /// # Arguments
    ///
    /// * `query_urn` - Query URN in one of the supported patterns
    ///
    /// # Returns
    ///
    /// Vector of result rows (HashMap<String, String>)
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::OntologyLibrary;
    /// use std::path::PathBuf;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let library = OntologyLibrary::new(PathBuf::from("."))?;
    ///
    /// // Kernel-scoped query
    /// let results = library.query_generic("ckp://System.Gateway:v1.0/Process?limit=20")?;
    ///
    /// // Global query (all kernels)
    /// let results = library.query_generic("ckp://Process?limit=100")?;
    ///
    /// for row in results {
    ///     println!("Process: {}", row.get("process").unwrap_or(&"".to_string()));
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn query_generic(&self, query_urn: &str) -> Result<Vec<std::collections::HashMap<String, String>>, OntologyError> {
        use crate::urn::UrnResolver;

        // Parse query URN using v2 parser
        let parsed = UrnResolver::parse_query_urn_v2(query_urn)
            .map_err(|e| OntologyError::ParseError(format!("Invalid query URN: {}", e)))?;

        // Build SPARQL query based on resource type
        let sparql_query = match parsed.resource.as_str() {
            "Process" => self.build_process_query(&parsed)?,
            "Workflow" => self.build_workflow_query(&parsed)?,
            "ImprovementProcess" => self.build_improvement_process_query(&parsed)?,
            "ConsensusProcess" => self.build_consensus_process_query(&parsed)?,
            "WorkflowPhase" => self.build_workflow_phase_query(&parsed)?,
            _ => {
                return Err(OntologyError::ParseError(format!(
                    "Unsupported resource type: {}. Supported: Process, Workflow, ImprovementProcess, ConsensusProcess, WorkflowPhase",
                    parsed.resource
                )));
            }
        };

        self.query_sparql(&sparql_query)
    }

    /// Build SPARQL query for Process resources
    fn build_process_query(&self, parsed: &crate::urn::ParsedQueryUrnV2) -> Result<String, OntologyError> {
        let limit = parsed.params.get("limit").map(|s| s.as_str()).unwrap_or("100");
        let order = parsed.params.get("order").map(|s| s.as_str()).unwrap_or("desc");

        let kernel_filter = if let Some(ref kernel) = parsed.kernel {
            format!("FILTER(CONTAINS(STR(?kernel), \"{}\"))", kernel)
        } else {
            String::new()
        };

        Ok(format!(
            r#"
PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?process ?kernel ?timestamp ?type
WHERE {{
    ?process rdf:type bfo:0000015 ;
             ckp:processUrn ?urn ;
             ckp:kernel ?kernel ;
             ckp:timestamp ?timestamp .
    OPTIONAL {{ ?process rdf:type ?type }}
    {}
}}
ORDER BY {}(?timestamp)
LIMIT {}
"#,
            kernel_filter,
            if order == "desc" { "DESC" } else { "ASC" },
            limit
        ))
    }

    /// Build SPARQL query for Workflow resources
    fn build_workflow_query(&self, parsed: &crate::urn::ParsedQueryUrnV2) -> Result<String, OntologyError> {
        let limit = parsed.params.get("limit").map(|s| s.as_str()).unwrap_or("50");

        let status_filter = if let Some(status) = parsed.params.get("status") {
            format!("FILTER(?status = \"{}\")", status)
        } else {
            String::new()
        };

        Ok(format!(
            r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?workflow ?label ?description ?status
WHERE {{
    ?workflow rdf:type ckpw:Workflow ;
              ckpw:workflowLabel ?label ;
              ckpw:workflowDescription ?description .
    OPTIONAL {{ ?workflow ckpw:workflowStatus ?status }}
    {}
}}
ORDER BY ?label
LIMIT {}
"#,
            status_filter, limit
        ))
    }

    /// Build SPARQL query for ImprovementProcess resources
    fn build_improvement_process_query(&self, parsed: &crate::urn::ParsedQueryUrnV2) -> Result<String, OntologyError> {
        let limit = parsed.params.get("limit").map(|s| s.as_str()).unwrap_or("50");

        let kernel_filter = if let Some(ref kernel) = parsed.kernel {
            format!("FILTER(CONTAINS(STR(?kernel), \"{}\"))", kernel)
        } else {
            String::new()
        };

        Ok(format!(
            r#"
PREFIX ckpi: <https://conceptkernel.org/ontology/improvement#>
PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?process ?kernel ?phase ?timestamp
WHERE {{
    ?process rdf:type ckpi:ImprovementProcess ;
             rdfs:subClassOf bfo:0000015 ;
             ckp:kernel ?kernel ;
             ckp:timestamp ?timestamp .
    OPTIONAL {{ ?process ckpi:currentPhase ?phase }}
    {}
}}
ORDER BY DESC(?timestamp)
LIMIT {}
"#,
            kernel_filter, limit
        ))
    }

    /// Build SPARQL query for ConsensusProcess resources
    fn build_consensus_process_query(&self, parsed: &crate::urn::ParsedQueryUrnV2) -> Result<String, OntologyError> {
        let limit = parsed.params.get("limit").map(|s| s.as_str()).unwrap_or("50");

        Ok(format!(
            r#"
PREFIX ckpc: <https://conceptkernel.org/ontology/consensus#>
PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?process ?proposal ?status ?quorum
WHERE {{
    ?process rdf:type ckpc:ConsensusProcess ;
             rdfs:subClassOf bfo:0000015 ;
             ckpc:proposal ?proposal .
    OPTIONAL {{ ?process ckpc:status ?status }}
    OPTIONAL {{ ?process ckpc:quorum ?quorum }}
}}
ORDER BY DESC(?process)
LIMIT {}
"#,
            limit
        ))
    }

    /// Build SPARQL query for WorkflowPhase resources
    fn build_workflow_phase_query(&self, parsed: &crate::urn::ParsedQueryUrnV2) -> Result<String, OntologyError> {
        let limit = parsed.params.get("limit").map(|s| s.as_str()).unwrap_or("50");

        let workflow_filter = if let Some(workflow) = parsed.params.get("workflow") {
            format!("FILTER(CONTAINS(STR(?workflow), \"{}\"))", workflow)
        } else {
            String::new()
        };

        Ok(format!(
            r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?phase ?workflow ?phaseName ?status ?kernel
WHERE {{
    ?phase rdf:type ckpw:WorkflowPhase ;
           ckpw:phaseName ?phaseName ;
           ckpw:kernelUrn ?kernel .
    ?workflow ckpw:hasPhase ?phase .
    OPTIONAL {{ ?phase ckpw:phaseStatus ?status }}
    {}
}}
ORDER BY ?workflow ?phaseName
LIMIT {}
"#,
            workflow_filter, limit
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_create_library() {
        let temp_dir = TempDir::new().unwrap();
        let lib = OntologyLibrary::new(temp_dir.path().to_path_buf());
        assert!(lib.is_ok());
    }
}
