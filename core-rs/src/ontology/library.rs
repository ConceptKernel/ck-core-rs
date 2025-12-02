/**
 * library.rs
 * RDF-based ontology library using standard URN resolution
 */

use oxigraph::store::Store;
use oxigraph::model::{NamedNode, GraphName};
use oxigraph::io::GraphFormat;
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
    project_root: PathBuf,
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
            return Ok(()); // Optional
        }
        
        let config = ProjectConfig::load(&ckproject_path)
            .map_err(|e| OntologyError::LoadError(e.to_string()))?;
        
        if let Some(ontology_config) = config.spec.ontology {
            // Load core ontologies using URN resolution
            self.load_ontology_urn(&ontology_config.core, "https://conceptkernel.org/ontology/core")?;
            self.load_ontology_urn(&ontology_config.bfo, "http://purl.obolibrary.org/obo/bfo.owl")?;
            self.load_ontology_urn(&ontology_config.predicates, "https://conceptkernel.org/ontology/predicates")?;
        }
        
        Ok(())
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
        
        let graph_uri = format!(
            "https://conceptkernel.org/ontology/{}",
            kernel_name.to_lowercase().replace(".", "-")
        );
        
        self.load_ontology_file(&ontology_path, &graph_uri)?;
        self.kernel_graphs.insert(kernel_name.to_string(), graph_uri);
        
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
        
        let graph_name = NamedNode::new(graph_uri)
            .map_err(|e| OntologyError::ParseError(e.to_string()))?;
        
        self.store
            .load_graph(
                content.as_bytes(),
                GraphFormat::Turtle,
                GraphName::NamedNode(graph_name),
                None,
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
