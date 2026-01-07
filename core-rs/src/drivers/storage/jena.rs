//! JenaStorage - Apache Jena Fuseki RDF triple store backend
//!
//! Provides StorageDriver implementation using Apache Jena Fuseki for
//! RDF-based ontology storage and SPARQL querying.
//!
//! ## Features
//!
//! - **RDF Native**: Stores ontologies as RDF triples (Turtle format)
//! - **SPARQL Queries**: Query ontologies using SPARQL 1.1
//! - **Reasoning**: Optional inference and reasoning support
//! - **Small Footprint**: Stateless containers (no filesystem needed)
//! - **HTTP Protocol**: RESTful API for all operations
//!
//! ## Jena Fuseki Setup
//!
//! ```bash
//! # Run Jena Fuseki in Docker
//! docker run -p 3030:3030 stain/jena-fuseki
//!
//! # Create dataset "ck"
//! curl -X POST http://localhost:3030/$/datasets \
//!   -d 'dbName=ck&dbType=tdb2'
//! ```

use crate::drivers::traits::{
    ExecutionMode, JobFile, JobHandle, ResourceRequirements, StorageDriver, StorageLocation,
    StorageEvent, StorageEventStream,
    ToolDefinition, ToolResponse,
};
use crate::errors::{CkpError, Result};
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose};
use futures::stream::StreamExt;
use reqwest::{Client, header::{HeaderMap, HeaderValue, AUTHORIZATION}};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use tokio::sync::broadcast;

/// JenaStorage - Apache Jena Fuseki RDF storage backend
///
/// Uses Jena Fuseki for RDF triple store operations.
/// Ideal for small footprint deployments with SPARQL reasoning.
///
/// # Configuration
///
/// ```yaml
/// storage:
///   type: jena
///   fuseki_url: http://fuseki:3030
///   dataset: ck
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use ckp_core::drivers::JenaStorage;
///
/// let storage = JenaStorage::new(
///     "http://localhost:3030".to_string(),
///     "ck".to_string()
/// );
///
/// // Load ontology as RDF
/// let ttl = storage.load_ontology("MyKernel").await?;
/// println!("Ontology: {}", ttl);
/// ```
pub struct JenaStorage {
    /// Jena Fuseki base URL
    fuseki_url: String,

    /// Dataset name
    dataset: String,

    /// HTTP client
    client: Client,

    /// Storage event broadcaster
    event_tx: broadcast::Sender<StorageEvent>,
}

// Manual Debug implementation to skip client field
impl std::fmt::Debug for JenaStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JenaStorage")
            .field("fuseki_url", &self.fuseki_url)
            .field("dataset", &self.dataset)
            .field("client", &"<reqwest::Client>")
            .finish()
    }
}

impl JenaStorage {
    /// Create new JenaStorage without authentication
    ///
    /// # Arguments
    ///
    /// * `fuseki_url` - Jena Fuseki base URL (e.g., "http://localhost:3030")
    /// * `dataset` - Dataset name (e.g., "ck")
    ///
    /// # Security Warning
    ///
    /// This creates an unauthenticated client. Use `new_with_auth()` for production.
    pub fn new(fuseki_url: String, dataset: String) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            fuseki_url,
            dataset,
            client: Client::new(),
            event_tx,
        }
    }

    /// Create new JenaStorage with HTTP Basic Authentication
    ///
    /// # Arguments
    ///
    /// * `fuseki_url` - Jena Fuseki base URL (e.g., "http://localhost:3030")
    /// * `dataset` - Dataset name (e.g., "ck")
    /// * `username` - Fuseki username (e.g., "ckp_system")
    /// * `password` - Fuseki password (from environment variable)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ckp_core::drivers::JenaStorage;
    ///
    /// let jena = JenaStorage::new_with_auth(
    ///     "http://jena-fuseki:3030".to_string(),
    ///     "conceptkernel".to_string(),
    ///     "ckp_system".to_string(),
    ///     std::env::var("JENA_PASSWORD").unwrap()
    /// );
    /// ```
    ///
    /// # Security
    ///
    /// - Uses HTTP Basic Authentication
    /// - Credentials sent with every request
    /// - Requires Fuseki configured with Apache Shiro
    pub fn new_with_auth(
        fuseki_url: String,
        dataset: String,
        username: String,
        password: String,
    ) -> Self {
        // Create HTTP Basic Auth header
        let auth_value = format!("{}:{}", username, password);
        let auth_header = format!("Basic {}", general_purpose::STANDARD.encode(auth_value));

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth_header).expect("Invalid auth header")
        );

        let client = Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build HTTP client");

        let (event_tx, _) = broadcast::channel(100);

        Self {
            fuseki_url,
            dataset,
            client,
            event_tx,
        }
    }

    /// Create persistent TDB2 dataset
    ///
    /// Creates a new dataset with persistent TDB2 storage (NOT in-memory).
    /// This ensures data survives server restarts.
    ///
    /// # Arguments
    ///
    /// * `dataset_name` - Name for the new dataset
    ///
    /// # Returns
    ///
    /// Ok if dataset created successfully
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let storage = JenaStorage::new("http://localhost:3030".to_string(), "ck".to_string());
    /// storage.create_persistent_dataset("ck-persistent").await?;
    /// ```
    /// Upload TTL content to a named graph using Graph Store HTTP Protocol
    ///
    /// # Arguments
    ///
    /// * `graph_uri` - Named graph URI (e.g., "https://conceptkernel.org/ontology/core")
    /// * `ttl_content` - Turtle/TTL format RDF content
    ///
    /// # Returns
    ///
    /// Ok if upload succeeded
    pub async fn upload_ttl_to_graph(&self, graph_uri: &str, ttl_content: &str) -> Result<()> {
        // Use Graph Store HTTP Protocol: PUT /{dataset}?graph={graph_uri}
        let url = format!("{}/{}?graph={}",
            self.fuseki_url,
            self.dataset,
            urlencoding::encode(graph_uri)
        );

        let response = self.client
            .put(&url)
            .header("Content-Type", "text/turtle; charset=utf-8")
            .body(ttl_content.to_string())
            .send()
            .await
            .map_err(|e| CkpError::IoError(format!("Failed to upload TTL: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body: String = response.text().await.unwrap_or_default();
            return Err(CkpError::Http(format!(
                "TTL upload failed: {} - {}",
                status, error_body
            )));
        }

        Ok(())
    }

    /// Save TTL data to a named graph using GSP (Graph Store Protocol)
    /// This is the proven working method based on test-edge-roundtrip.py
    pub async fn save_ttl_to_graph(&self, ttl_data: &str, graph_uri: &str) -> Result<()> {
        let data_url = format!("{}/{}/data", self.fuseki_url, self.dataset);

        let response = self.client
            .put(&data_url)
            .query(&[("graph", graph_uri)])
            .header("Content-Type", "text/turtle")
            .body(ttl_data.to_string())
            .send()
            .await
            .map_err(|e| CkpError::IoError(format!("TTL save request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read response".to_string());
            return Err(CkpError::IoError(format!(
                "TTL save to graph failed ({}): {}",
                status, body
            )));
        }

        Ok(())
    }

    /// Execute SPARQL UPDATE query (INSERT, DELETE, etc.)
    pub async fn execute_sparql_update(&self, sparql_update: &str) -> Result<()> {
        // Try /update endpoint first (standard SPARQL 1.1 Update)
        let update_url = format!("{}/{}/update", self.fuseki_url, self.dataset);

        let response = self.client
            .post(&update_url)
            .header("Content-Type", "application/sparql-update")
            .body(sparql_update.to_string())
            .send()
            .await
            .map_err(|e| CkpError::IoError(format!("SPARQL UPDATE request failed: {}", e)))?;

        // If /update endpoint not available (HTTP 405), try base dataset endpoint
        // Send UPDATE directly in body like queries (not form-encoded)
        if response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
            eprintln!("[JenaStorage] /update endpoint not available, trying base dataset endpoint");
            let base_url = format!("{}/{}", self.fuseki_url, self.dataset);

            let base_response = self.client
                .post(&base_url)
                .header("Content-Type", "application/sparql-update")
                .body(sparql_update.to_string())
                .send()
                .await
                .map_err(|e| CkpError::IoError(format!("SPARQL UPDATE via base endpoint failed: {}", e)))?;

            if !base_response.status().is_success() {
                let status = base_response.status();
                let body = base_response.text().await.unwrap_or_else(|_| "Unable to read response".to_string());
                return Err(CkpError::IoError(format!(
                    "SPARQL UPDATE via base endpoint failed ({}): {}",
                    status, body
                )));
            }

            return Ok(());
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read response".to_string());
            return Err(CkpError::IoError(format!(
                "SPARQL UPDATE failed ({}): {}",
                status, body
            )));
        }

        Ok(())
    }

    /// Execute SPARQL SELECT query and return JSON results
    pub async fn execute_sparql_query(&self, sparql_query: &str) -> Result<serde_json::Value> {
        // Fuseki with empty endpoints uses dataset root for all operations
        // Content-Type header determines the operation type
        let query_url = format!("{}/{}", self.fuseki_url, self.dataset);

        let response = self.client
            .post(&query_url)
            .header("Content-Type", "application/sparql-query")
            .header("Accept", "application/sparql-results+json")
            .body(sparql_query.to_string())
            .send()
            .await
            .map_err(|e| CkpError::IoError(format!("SPARQL query request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read response".to_string());
            return Err(CkpError::IoError(format!(
                "SPARQL query failed ({}): {}",
                status, body
            )));
        }

        let json = response.json::<serde_json::Value>().await
            .map_err(|e| CkpError::IoError(format!("Failed to parse SPARQL results: {}", e)))?;

        Ok(json)
    }

    /// Alias for execute_sparql_query for consistency with DisklessGovernor
    pub async fn query_sparql_json(&self, sparql_query: &str) -> Result<serde_json::Value> {
        self.execute_sparql_query(sparql_query).await
    }

    pub async fn create_persistent_dataset(&self, dataset_name: &str) -> Result<()> {
        let create_url = format!("{}/$/datasets", self.fuseki_url);

        let params = [
            ("dbName", dataset_name),
            ("dbType", "tdb2"), // TDB2 = persistent storage
        ];

        let response = self
            .client
            .post(&create_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to create dataset: {}", e)))?;

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "Failed to create dataset: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        Ok(())
    }

    /// Check if dataset exists
    pub async fn dataset_exists(&self, dataset_name: &str) -> Result<bool> {
        let list_url = format!("{}/$/datasets", self.fuseki_url);

        let response = self
            .client
            .get(&list_url)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to list datasets: {}", e)))?;

        if !response.status().is_success() {
            return Ok(false);
        }

        let json: JsonValue = response
            .json()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to parse JSON: {}", e)))?;

        // Parse dataset list
        if let Some(datasets) = json.get("datasets").and_then(|d| d.as_array()) {
            for dataset in datasets {
                if let Some(ds_name) = dataset.get("ds.name").and_then(|n| n.as_str()) {
                    if ds_name.trim_start_matches('/') == dataset_name {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    /// Ensure dataset exists (create if not)
    ///
    /// Idempotent operation - safe to call multiple times.
    pub async fn ensure_dataset(&self, dataset_name: &str) -> Result<()> {
        if !self.dataset_exists(dataset_name).await? {
            self.create_persistent_dataset(dataset_name).await?;
        }
        Ok(())
    }

    // ========================================================================
    // ONTOLOGY LOADING & REASONING (v1.3.20)
    // ========================================================================

    /// Load CKP protocol ontologies into Fuseki
    ///
    /// Loads the ConceptKernel Protocol ontologies for semantic validation:
    /// - conceptkernel-bfo-base.ttl (Kernel, Edge, Instance classes)
    /// - conceptkernel-relations.ttl (SWRL rules, inference)
    /// - ck-predicates.v1.3.16.ttl (Edge predicates)
    ///
    /// These ontologies define the semantic integrity rules that ALL
    /// kernels, edges, and instances must conform to.
    ///
    /// # Arguments
    ///
    /// * `ontology_dir` - Path to directory containing .ttl files (e.g., "/concepts/.ontology")
    ///
    /// # Returns
    ///
    /// Ok if ontologies loaded successfully
    pub async fn load_protocol_ontologies(&self, ontology_dir: &std::path::Path) -> Result<()> {
        let ontology_files = vec![
            "conceptkernel-bfo-base.ttl",
            "conceptkernel-relations.ttl",
            "conceptkernel-processes.ttl",
            "conceptkernel-rbac.ttl",
            "conceptkernel-workflow.ttl",
        ];

        let graph_uri = "http://conceptkernel.org/ontology/protocol";

        for filename in ontology_files {
            let ontology_path = ontology_dir.join(filename);

            if !ontology_path.exists() {
                eprintln!("[JenaStorage] Warning: Ontology file not found: {:?}", ontology_path);
                continue;
            }

            eprintln!("[JenaStorage] Loading ontology: {}", filename);

            let ttl_content = tokio::fs::read_to_string(&ontology_path)
                .await
                .map_err(|e| CkpError::IoError(format!("Failed to read ontology: {}", e)))?;

            // Upload to named graph
            let response = self
                .client
                .post(&self.data_endpoint())
                .query(&[("graph", graph_uri)])
                .header("Content-Type", "text/turtle")
                .body(ttl_content)
                .send()
                .await
                .map_err(|e| CkpError::Http(format!("Failed to load ontology: {}", e)))?;

            if !response.status().is_success() {
                return Err(CkpError::Http(format!(
                    "Failed to load ontology {}: {}",
                    filename,
                    response.status()
                )));
            }

            eprintln!("[JenaStorage] ✓ Loaded {}", filename);
        }

        Ok(())
    }

    /// Validate edge authorization using SPARQL
    ///
    /// Queries the RDF store to check if an edge is authorized according
    /// to the target kernel's queue_contract.
    ///
    /// # Arguments
    ///
    /// * `source` - Source kernel name
    /// * `target` - Target kernel name
    /// * `predicate` - Edge predicate (e.g., "PRODUCES")
    ///
    /// # Returns
    ///
    /// Ok(true) if edge is authorized, Ok(false) if not, Err on query failure
    ///
    /// # SPARQL Query
    ///
    /// ```sparql
    /// ASK WHERE {
    ///   ?edge a ckp:Edge ;
    ///         ckp:source "Source" ;
    ///         ckp:target "Target" ;
    ///         ckp:predicate "PRODUCES" ;
    ///         ckp:isAuthorized true .
    /// }
    /// ```
    pub async fn validate_edge_authorization(
        &self,
        source: &str,
        target: &str,
        predicate: &str,
    ) -> Result<bool> {
        let sparql = format!(
            r#"PREFIX ckp: <http://conceptkernel.org/ontology#>
ASK WHERE {{
  ?edge a ckp:Edge ;
        ckp:source "{source}" ;
        ckp:target "{target}" ;
        ckp:predicate "{predicate}" ;
        ckp:isAuthorized true .
}}"#,
            source = source,
            target = target,
            predicate = predicate
        );

        let response = self
            .client
            .post(&self.query_endpoint())
            .header("Content-Type", "application/sparql-query")
            .header("Accept", "application/sparql-results+json")
            .body(sparql)
            .send()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to execute ASK query: {}", e)))?;

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "ASK query failed: {}",
                response.status()
            )));
        }

        let result: JsonValue = response
            .json()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to parse ASK result: {}", e)))?;

        // ASK query returns {"boolean": true/false}
        Ok(result
            .get("boolean")
            .and_then(|b| b.as_bool())
            .unwrap_or(false))
    }

    /// Validate kernel ontology against CKP protocol rules
    ///
    /// Checks if a kernel's ontology conforms to ConceptKernel Protocol requirements:
    /// - Valid apiVersion (conceptkernel/v1)
    /// - Valid kind (Ontology)
    /// - Valid kernel type (node:cold, node:hot, etc.)
    /// - Valid predicates in notification_contract
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Kernel name
    /// * `ontology_ttl` - Ontology in Turtle/YAML format
    ///
    /// # Returns
    ///
    /// Ok(Vec<String>) with validation errors (empty = valid)
    pub async fn validate_kernel_ontology(
        &self,
        kernel_name: &str,
        ontology_ttl: &str,
    ) -> Result<Vec<String>> {
        let mut errors = Vec::new();

        // Parse as YAML (for now - TODO: convert to pure RDF)
        let ontology_yaml = match serde_yaml::from_str::<serde_yaml::Value>(ontology_ttl) {
            Ok(yaml) => yaml,
            Err(e) => {
                errors.push(format!("Invalid ontology format: {}", e));
                return Ok(errors);
            }
        };

        // Validate apiVersion
        if let Some(api_version) = ontology_yaml.get("apiVersion").and_then(|v| v.as_str()) {
            if !api_version.starts_with("conceptkernel/") {
                errors.push(format!(
                    "Invalid apiVersion: {}. Must be 'conceptkernel/v1'",
                    api_version
                ));
            }
        } else {
            errors.push("Missing apiVersion field".to_string());
        }

        // Validate kind
        if let Some(kind) = ontology_yaml.get("kind").and_then(|v| v.as_str()) {
            if kind != "Ontology" {
                errors.push(format!("Invalid kind: {}. Must be 'Ontology'", kind));
            }
        } else {
            errors.push("Missing kind field".to_string());
        }

        // Validate metadata.name matches kernel_name
        if let Some(metadata) = ontology_yaml.get("metadata") {
            if let Some(name) = metadata.get("name").and_then(|v| v.as_str()) {
                let expected_name = format!("ckp://{}", kernel_name);
                if name != expected_name {
                    errors.push(format!(
                        "Name mismatch: expected '{}', found '{}'",
                        expected_name, name
                    ));
                }
            } else {
                errors.push("Missing metadata.name field".to_string());
            }

            // Validate kernel type exists
            if metadata.get("type").is_none() {
                errors.push("Missing metadata.type field".to_string());
            }
        } else {
            errors.push("Missing metadata section".to_string());
        }

        Ok(errors)
    }

    /// Query for unauthorized edges (validation check)
    ///
    /// Finds all edges in the RDF store that are NOT authorized by their
    /// target kernels. This is a critical integrity violation.
    ///
    /// # Returns
    ///
    /// Vec of (source, target, predicate) tuples for unauthorized edges
    pub async fn find_unauthorized_edges(&self) -> Result<Vec<(String, String, String)>> {
        let sparql = r#"PREFIX ckp: <http://conceptkernel.org/ontology#>
SELECT ?source ?target ?predicate
WHERE {
  ?edge a ckp:Edge ;
        ckp:source ?source ;
        ckp:target ?target ;
        ckp:predicate ?predicate ;
        ckp:isAuthorized false .
}"#;

        let result = self.query(sparql).await?;

        let mut unauthorized = Vec::new();

        if let Some(bindings) = result
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
        {
            for binding in bindings {
                let source = binding
                    .get("source")
                    .and_then(|s| s.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let target = binding
                    .get("target")
                    .and_then(|t| t.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let predicate = binding
                    .get("predicate")
                    .and_then(|p| p.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                unauthorized.push((source, target, predicate));
            }
        }

        Ok(unauthorized)
    }

    // ========================================================================
    // SHACL VALIDATION (v1.3.20)
    // ========================================================================
    // SHACL (Shapes Constraint Language) provides comprehensive triple validation
    // against shapes defined in conceptkernel-shapes.ttl

    /// Load SHACL shapes from file into Jena Fuseki
    ///
    /// Loads SHACL shapes into a dedicated graph for validation.
    ///
    /// # Arguments
    ///
    /// * `shapes_path` - Path to SHACL shapes file (e.g., conceptkernel-shapes.ttl)
    ///
    /// # Graph
    ///
    /// Shapes loaded into: `http://conceptkernel.org/shapes`
    pub async fn load_shacl_shapes(&self, shapes_path: &std::path::Path) -> Result<()> {
        let shapes_ttl = tokio::fs::read_to_string(shapes_path).await?;
        let graph_uri = "http://conceptkernel.org/shapes";

        let response = self
            .client
            .post(&self.data_endpoint())
            .query(&[("graph", graph_uri)])
            .header("Content-Type", "text/turtle")
            .body(shapes_ttl)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "Failed to load SHACL shapes: HTTP {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// Validate RDF graph using SHACL shapes
    ///
    /// Runs SHACL validation against a specific graph and returns whether it conforms.
    ///
    /// # Arguments
    ///
    /// * `graph_uri` - Graph to validate (e.g., "http://conceptkernel.org/edges")
    ///
    /// # Returns
    ///
    /// `Ok(true)` if graph conforms to all SHACL constraints
    /// `Ok(false)` if validation violations exist
    ///
    /// # Example
    ///
    /// ```rust
    /// let conforms = jena.validate_graph_with_shacl("http://conceptkernel.org/edges").await?;
    /// if !conforms {
    ///     let violations = jena.get_all_shacl_violations().await?;
    ///     eprintln!("Validation failed: {} violations", violations.len());
    /// }
    /// ```
    pub async fn validate_graph_with_shacl(&self, graph_uri: &str) -> Result<bool> {
        // SHACL validation query using Apache Jena's SHACL support
        // This uses SPARQL with SHACL vocabulary to check constraints
        let sparql = format!(
            r#"PREFIX sh: <http://www.w3.org/ns/shacl#>
ASK WHERE {{
  GRAPH <{graph_uri}> {{
    ?s ?p ?o .
  }}
  GRAPH <http://conceptkernel.org/shapes> {{
    ?shape a sh:NodeShape .
  }}
  # Check if ANY validation result exists
  # (Jena SHACL creates sh:ValidationResult for violations)
  FILTER NOT EXISTS {{
    ?result a sh:ValidationResult ;
            sh:focusNode ?s ;
            sh:resultSeverity sh:Violation .
  }}
}}"#,
            graph_uri = graph_uri
        );

        let response = self
            .client
            .post(&self.query_endpoint())
            .header("Content-Type", "application/sparql-query")
            .header("Accept", "application/sparql-results+json")
            .body(sparql)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "SHACL validation query failed: HTTP {}",
                response.status()
            )));
        }

        let result: JsonValue = response.json().await?;

        // ASK query returns {"boolean": true/false}
        Ok(result
            .get("boolean")
            .and_then(|b| b.as_bool())
            .unwrap_or(false))
    }

    /// Get all SHACL validation violations across all graphs
    ///
    /// Retrieves detailed report of ALL SHACL constraint violations.
    ///
    /// # Returns
    ///
    /// Vec of violation messages (human-readable)
    ///
    /// # Example Output
    ///
    /// ```
    /// [
    ///   "Edge must have exactly one source Kernel (ckp://System.BadKernel)",
    ///   "Kernel must have apiVersion matching 'conceptkernel/v1.3.20' format",
    ///   "Edge can ONLY be created by Governor (Data Sovereignty)"
    /// ]
    /// ```
    pub async fn get_all_shacl_violations(&self) -> Result<Vec<String>> {
        // Query for all sh:ValidationResult instances
        let sparql = r#"PREFIX sh: <http://www.w3.org/ns/shacl#>
SELECT ?focusNode ?message ?value
WHERE {
  ?result a sh:ValidationResult ;
          sh:focusNode ?focusNode ;
          sh:resultMessage ?message ;
          sh:resultSeverity sh:Violation .
  OPTIONAL { ?result sh:value ?value }
}
ORDER BY ?focusNode"#;

        let response = self
            .client
            .post(&self.query_endpoint())
            .header("Content-Type", "application/sparql-query")
            .header("Accept", "application/sparql-results+json")
            .body(sparql)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "SHACL violations query failed: HTTP {}",
                response.status()
            )));
        }

        let result: JsonValue = response.json().await?;
        let mut violations = Vec::new();

        if let Some(bindings) = result
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
        {
            for binding in bindings {
                let focus_node = binding
                    .get("focusNode")
                    .and_then(|f| f.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                let message = binding
                    .get("message")
                    .and_then(|m| m.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("No message");

                let value = binding
                    .get("value")
                    .and_then(|val| val.get("value"))
                    .and_then(|v| v.as_str())
                    .map(|v| format!(" (value: {})", v))
                    .unwrap_or_default();

                violations.push(format!("{} - {}{}", focus_node, message, value));
            }
        }

        Ok(violations)
    }

    /// Validate kernel instance before admission
    ///
    /// This is the CRITICAL data sovereignty check: "No semantic compatibility? No admission."
    ///
    /// # Validation Steps
    ///
    /// 1. Check kernel conforms to ckp:Kernel shape
    /// 2. Check all edges are authorized
    /// 3. Check instance provenance is traceable
    /// 4. Check no protocol ontology violations
    ///
    /// # Returns
    ///
    /// `Ok(Vec<String>)` with violation messages (empty = valid, can admit)
    ///
    /// # Usage in Governor
    ///
    /// ```rust
    /// let violations = jena.validate_kernel_admission(kernel_name, ontology).await?;
    /// if !violations.is_empty() {
    ///     eprintln!("❌ Semantic validation FAILED - cannot admit kernel");
    ///     for violation in violations {
    ///         eprintln!("  - {}", violation);
    ///     }
    ///     // Archive job with reason: "Semantic validation failed"
    ///     return;
    /// }
    /// eprintln!("✅ Semantic validation PASSED - kernel admitted");
    /// ```
    pub async fn validate_kernel_admission(
        &self,
        kernel_name: &str,
        _ontology_ttl: &str,
    ) -> Result<Vec<String>> {
        let mut violations = Vec::new();

        // Step 1: Check kernel exists and conforms to ckp:Kernel shape
        let kernel_uri = format!("ckp://{}", kernel_name);
        let sparql_kernel_check = format!(
            r#"PREFIX ckp: <http://conceptkernel.org/ontology#>
ASK WHERE {{
  <{kernel_uri}> a ckp:Kernel ;
                 ckp:kernelName ?name ;
                 ckp:apiVersion ?version ;
                 ckp:kind "ConceptKernel" .
}}"#,
            kernel_uri = kernel_uri
        );

        let response = self
            .client
            .post(&self.query_endpoint())
            .header("Content-Type", "application/sparql-query")
            .header("Accept", "application/sparql-results+json")
            .body(sparql_kernel_check)
            .send()
            .await?;

        let result: JsonValue = response.json().await?;
        let kernel_exists = result
            .get("boolean")
            .and_then(|b| b.as_bool())
            .unwrap_or(false);

        if !kernel_exists {
            violations.push(format!(
                "Kernel {} does not conform to ckp:Kernel shape (missing required properties)",
                kernel_name
            ));
        }

        // Step 2: Check all edges involving this kernel are authorized
        let unauthorized = self.find_unauthorized_edges().await?;
        for (source, target, predicate) in unauthorized {
            if source.contains(kernel_name) || target.contains(kernel_name) {
                violations.push(format!(
                    "Unauthorized edge: {} --[{}]--> {} (Data Sovereignty violation)",
                    source, predicate, target
                ));
            }
        }

        // Step 3: Get SHACL violations for this kernel
        let all_shacl_violations = self.get_all_shacl_violations().await?;
        for violation in all_shacl_violations {
            if violation.contains(kernel_name) || violation.contains(&kernel_uri) {
                violations.push(violation);
            }
        }

        Ok(violations)
    }

    // ========================================================================
    // EDGE METADATA STORAGE (v1.3.20)
    // ========================================================================

    /// Save edge metadata as RDF triples (structured format)
    ///
    /// Stores edge routing metadata in RDF format for semantic queries.
    /// Uses the EdgeMetadata structure for type-safe edge management.
    ///
    /// # Arguments
    ///
    /// * `edge_metadata` - Edge metadata structure
    ///
    /// # RDF Format
    ///
    /// ```turtle
    /// @prefix ckp: <https://conceptkernel.org/ontology#> .
    /// @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
    ///
    /// <ckp://Edge#Connection-Source-to-Target-PRODUCES:v1.3.20>
    ///   a ckp:EdgeConnection ;
    ///   ckp:hasPredicate ckp:Edge-PRODUCES ;
    ///   ckp:hasSource ckp:Kernel-Source ;
    ///   ckp:hasTarget ckp:Kernel-Target ;
    ///   ckp:version "v1.3.20"^^xsd:string ;
    ///   ckp:createdAt "2025-12-19T10:00:00.000Z"^^xsd:dateTime ;
    ///   ckp:status "active"^^xsd:string .
    /// ```
    pub async fn save_edge_metadata_structured(
        &self,
        edge_metadata: &crate::drivers::EdgeMetadata,
    ) -> Result<()> {
        use crate::drivers::EdgeMetadata;

        let graph_uri = format!("ckp://edges/{}/{}-to-{}",
            edge_metadata.predicate,
            edge_metadata.source,
            edge_metadata.target
        );

        // Convert to Turtle with full entity graph for System.Discovery compatibility
        let ttl = format!(
            r#"@prefix ckp: <https://conceptkernel.org/ontology#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

# Edge entity
<{urn}> a ckp:EdgeConnection ;
    ckp:hasURN "{urn}" ;
    ckp:hasPredicate ckp:Edge-{predicate} ;
    ckp:hasSource ckp:Kernel-{source} ;
    ckp:hasTarget ckp:Kernel-{target} ;
    ckp:version "{version}"^^xsd:string ;
    ckp:createdAt "{created_at}"^^xsd:dateTime ;
    ckp:status "active"^^xsd:string .

# Source kernel entity
ckp:Kernel-{source}
    ckp:hasName "{source}" .

# Target kernel entity
ckp:Kernel-{target}
    ckp:hasName "{target}" .

# Predicate entity
ckp:Edge-{predicate}
    ckp:predicateName "{predicate}" .
"#,
            urn = edge_metadata.urn,
            predicate = edge_metadata.predicate,
            source = edge_metadata.source,
            target = edge_metadata.target,
            version = edge_metadata.version,
            created_at = edge_metadata.created_at,
        );

        // Upload to Jena
        let response = self.client
            .post(&self.data_endpoint())
            .query(&[("graph", &graph_uri)])
            .header("Content-Type", "text/turtle")
            .body(ttl)
            .send()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to save edge metadata: {}", e)))?;

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "Jena returned error: {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// Save edge metadata as RDF triples (legacy method)
    ///
    /// Stores edge routing metadata in RDF format for semantic queries.
    ///
    /// # Arguments
    ///
    /// * `source` - Source kernel name
    /// * `target` - Target kernel name
    /// * `predicate` - Edge predicate (e.g., "PRODUCES")
    /// * `metadata` - Additional metadata as JSON
    ///
    /// # RDF Format
    ///
    /// ```turtle
    /// @prefix ckp: <http://conceptkernel.org/> .
    /// @prefix edge: <http://conceptkernel.org/edge/> .
    ///
    /// edge:PRODUCES.Source-to-Target
    ///   a ckp:Edge ;
    ///   ckp:source "Source" ;
    ///   ckp:target "Target" ;
    ///   ckp:predicate "PRODUCES" ;
    ///   ckp:created "2025-12-18T10:00:00Z" .
    /// ```
    pub async fn save_edge_metadata(
        &self,
        source: &str,
        target: &str,
        predicate: &str,
        metadata: &JsonValue,
    ) -> Result<()> {
        use crate::drivers::EdgeMetadata;

        // Convert to EdgeMetadata structure
        let edge_urn = format!("ckp://Edge#Connection-{}-to-{}-{}:v1.3.20", source, target, predicate);
        let default_created = chrono::Utc::now().to_rfc3339();
        let created_at = metadata.get("created")
            .and_then(|c| c.as_str())
            .unwrap_or(&default_created)
            .to_string();
        let version = metadata.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("v1.3.20")
            .to_string();

        let edge_metadata = EdgeMetadata {
            api_version: "conceptkernel/v1".to_string(),
            kind: "EdgeConnection".to_string(),
            urn: edge_urn,
            created_at,
            predicate: predicate.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            version,
        };

        // Use structured method
        self.save_edge_metadata_structured(&edge_metadata).await
    }

    /// Query edges by predicate (structured format)
    ///
    /// Returns all EdgeMetadata structures with the given predicate using SPARQL.
    ///
    /// # Arguments
    ///
    /// * `predicate` - Edge predicate to filter by
    ///
    /// # Returns
    ///
    /// Vec of EdgeMetadata structures
    pub async fn query_edges_by_predicate_structured(&self, predicate: &str) -> Result<Vec<crate::drivers::EdgeMetadata>> {
        use crate::drivers::EdgeMetadata;

        let sparql = format!(
            r#"PREFIX ckp: <https://conceptkernel.org/ontology#>

SELECT ?urn ?source ?target ?version ?created_at
WHERE {{
  ?urn a ckp:EdgeConnection ;
       ckp:hasPredicate ckp:Edge-{predicate} ;
       ckp:hasSource ?sourceUrn ;
       ckp:hasTarget ?targetUrn ;
       ckp:version ?version ;
       ckp:createdAt ?created_at .

  BIND(REPLACE(STR(?sourceUrn), ".*Kernel-", "") AS ?source)
  BIND(REPLACE(STR(?targetUrn), ".*Kernel-", "") AS ?target)
}}
ORDER BY ?created_at
"#,
            predicate = predicate
        );

        let response = self.client
            .post(&self.query_endpoint())
            .header("Content-Type", "application/sparql-query")
            .header("Accept", "application/sparql-results+json")
            .body(sparql)
            .send()
            .await
            .map_err(|e| CkpError::Http(format!("SPARQL query failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "Jena query error: {}",
                response.status()
            )));
        }

        let results: serde_json::Value = response
            .json()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to parse SPARQL results: {}", e)))?;

        // Parse results into EdgeMetadata
        let bindings = results["results"]["bindings"].as_array()
            .ok_or_else(|| CkpError::Http("Invalid SPARQL results format".to_string()))?;

        let mut edges = Vec::new();
        for binding in bindings {
            let urn = binding["urn"]["value"].as_str()
                .ok_or_else(|| CkpError::Http("Missing URN in results".to_string()))?;
            let source = binding["source"]["value"].as_str()
                .ok_or_else(|| CkpError::Http("Missing source in results".to_string()))?;
            let target = binding["target"]["value"].as_str()
                .ok_or_else(|| CkpError::Http("Missing target in results".to_string()))?;
            let version = binding["version"]["value"].as_str()
                .ok_or_else(|| CkpError::Http("Missing version in results".to_string()))?;
            let created_at = binding["created_at"]["value"].as_str()
                .ok_or_else(|| CkpError::Http("Missing created_at in results".to_string()))?;

            edges.push(EdgeMetadata {
                api_version: "conceptkernel/v1".to_string(),
                kind: "EdgeConnection".to_string(),
                urn: urn.to_string(),
                created_at: created_at.to_string(),
                predicate: predicate.to_string(),
                source: source.to_string(),
                target: target.to_string(),
                version: version.to_string(),
            });
        }

        Ok(edges)
    }

    /// Query edges by predicate (legacy format)
    ///
    /// Returns all edges with the given predicate using SPARQL.
    ///
    /// # Arguments
    ///
    /// * `predicate` - Edge predicate to filter by
    ///
    /// # Returns
    ///
    /// Vec of (source, target) pairs
    pub async fn query_edges_by_predicate(&self, predicate: &str) -> Result<Vec<(String, String)>> {
        // Use structured method and convert to tuples
        let edges = self.query_edges_by_predicate_structured(predicate).await?;
        Ok(edges.into_iter().map(|e| (e.source, e.target)).collect())
    }

    /// Query all edges for a target kernel
    ///
    /// Returns all incoming edges to a target kernel.
    pub async fn query_edges_to_target(&self, target: &str) -> Result<Vec<(String, String)>> {
        let sparql = format!(
            r#"PREFIX ckp: <http://conceptkernel.org/>
SELECT ?source ?predicate
WHERE {{
  ?edge a ckp:Edge ;
        ckp:target "{target}" ;
        ckp:source ?source ;
        ckp:predicate ?predicate .
}}"#,
            target = target
        );

        let result = self.query(&sparql).await?;

        let mut edges = Vec::new();

        if let Some(bindings) = result
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
        {
            for binding in bindings {
                let source = binding
                    .get("source")
                    .and_then(|s| s.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let predicate = binding
                    .get("predicate")
                    .and_then(|p| p.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                if !source.is_empty() && !predicate.is_empty() {
                    edges.push((predicate, source));
                }
            }
        }

        Ok(edges)
    }

    /// Get SPARQL query endpoint
    fn query_endpoint(&self) -> String {
        format!("{}/{}/query", self.fuseki_url, self.dataset)
    }

    /// Get SPARQL update endpoint
    fn update_endpoint(&self) -> String {
        format!("{}/{}/update", self.fuseki_url, self.dataset)
    }

    /// Get data endpoint for graph operations
    fn data_endpoint(&self) -> String {
        format!("{}/{}/data", self.fuseki_url, self.dataset)
    }

    /// Execute SPARQL query
    async fn query(&self, sparql: &str) -> Result<JsonValue> {
        let response = self
            .client
            .post(&self.query_endpoint())
            .header("Accept", "application/sparql-results+json")
            .form(&[("query", sparql)])
            .send()
            .await
            .map_err(|e| CkpError::Http(format!("SPARQL query failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "SPARQL query failed: {}",
                response.status()
            )));
        }

        let json = response
            .json::<JsonValue>()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to parse SPARQL results: {}", e)))?;

        Ok(json)
    }

    /// Execute SPARQL update
    async fn update(&self, sparql: &str) -> Result<()> {
        let response = self
            .client
            .post(&self.update_endpoint())
            .header("Content-Type", "application/sparql-update")
            .body(sparql.to_string())
            .send()
            .await
            .map_err(|e| CkpError::Http(format!("SPARQL update failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "SPARQL update failed: {}",
                response.status()
            )));
        }

        Ok(())
    }

    // ========================================================================
    // BFO 2020 OCCURRENT TRANSACTION STORAGE (v1.3.20)
    // ========================================================================
    // These methods store workflow execution events as BFO Occurrents (bfo:0000003)
    // for temporal reasoning and provenance tracking.

    /// Insert WorkflowExecution occurrent into RDF store
    ///
    /// Stores workflow execution as a BFO Occurrent entity with temporal properties.
    ///
    /// # Arguments
    ///
    /// * `workflow_name` - Name of the workflow being executed
    /// * `transaction_id` - Unique transaction identifier
    /// * `timestamp` - Execution timestamp
    ///
    /// # RDF Format (BFO 2020)
    ///
    /// ```turtle
    /// @prefix bfo: <http://purl.obolibrary.org/obo/> .
    /// @prefix ckp: <https://conceptkernel.org/ontology#> .
    /// @prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
    ///
    /// <ckp://WorkflowExecution/{transaction_id}>
    ///   a bfo:BFO_0000003 ;  # Occurrent
    ///   a ckp:WorkflowExecution ;
    ///   ckp:workflowName "{workflow_name}" ;
    ///   ckp:transactionId "{transaction_id}" ;
    ///   ckp:occurredAt "{timestamp}"^^xsd:dateTime .
    /// ```
    ///
    /// # Returns
    ///
    /// Ok if insertion successful
    pub async fn insert_workflow_execution_occurrent(
        &self,
        workflow_name: &str,
        transaction_id: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let urn = format!("ckp://WorkflowExecution/{}", transaction_id);
        let timestamp_str = timestamp.to_rfc3339();

        let sparql_insert = format!(
            r#"PREFIX bfo: <http://purl.obolibrary.org/obo/>
PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
  <{urn}> a bfo:BFO_0000003 ;  # Occurrent
          a ckp:WorkflowExecution ;
          ckp:workflowName "{workflow_name}" ;
          ckp:transactionId "{transaction_id}" ;
          ckp:occurredAt "{timestamp}"^^xsd:dateTime .
}}"#,
            urn = urn,
            workflow_name = workflow_name,
            transaction_id = transaction_id,
            timestamp = timestamp_str
        );

        self.execute_sparql_update(&sparql_insert).await
    }

    /// Insert KernelInvocation occurrent into RDF store
    ///
    /// Stores kernel invocation as a BFO Occurrent with execution details.
    ///
    /// # Arguments
    ///
    /// * `kernel_name` - Name of the invoked kernel
    /// * `transaction_id` - Associated transaction ID
    /// * `timestamp` - Invocation timestamp
    ///
    /// # RDF Format (BFO 2020)
    ///
    /// ```turtle
    /// @prefix bfo: <http://purl.obolibrary.org/obo/> .
    /// @prefix ckp: <https://conceptkernel.org/ontology#> .
    ///
    /// <ckp://KernelInvocation/{transaction_id}/{kernel_name}>
    ///   a bfo:BFO_0000003 ;  # Occurrent
    ///   a ckp:KernelInvocation ;
    ///   ckp:kernelName "{kernel_name}" ;
    ///   ckp:transactionId "{transaction_id}" ;
    ///   ckp:occurredAt "{timestamp}"^^xsd:dateTime .
    /// ```
    pub async fn insert_kernel_invocation_occurrent(
        &self,
        kernel_name: &str,
        transaction_id: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let urn = format!("ckp://KernelInvocation/{}/{}", transaction_id, kernel_name);
        let timestamp_str = timestamp.to_rfc3339();

        let sparql_insert = format!(
            r#"PREFIX bfo: <http://purl.obolibrary.org/obo/>
PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
  <{urn}> a bfo:BFO_0000003 ;  # Occurrent
          a ckp:KernelInvocation ;
          ckp:kernelName "{kernel_name}" ;
          ckp:transactionId "{transaction_id}" ;
          ckp:occurredAt "{timestamp}"^^xsd:dateTime .
}}"#,
            urn = urn,
            kernel_name = kernel_name,
            transaction_id = transaction_id,
            timestamp = timestamp_str
        );

        self.execute_sparql_update(&sparql_insert).await
    }

    /// Insert EdgeRouting occurrent into RDF store
    ///
    /// Stores edge routing event as a BFO Occurrent with source/target information.
    ///
    /// # Arguments
    ///
    /// * `source_kernel` - Source kernel name
    /// * `target_kernel` - Target kernel name
    /// * `predicate` - Edge predicate (e.g., "PRODUCES")
    /// * `transaction_id` - Associated transaction ID
    /// * `timestamp` - Routing timestamp
    ///
    /// # RDF Format (BFO 2020)
    ///
    /// ```turtle
    /// <ckp://EdgeRouting/{transaction_id}>
    ///   a bfo:BFO_0000003 ;  # Occurrent
    ///   a ckp:EdgeRouting ;
    ///   ckp:sourceKernel "{source}" ;
    ///   ckp:targetKernel "{target}" ;
    ///   ckp:predicate "{predicate}" ;
    ///   ckp:transactionId "{transaction_id}" ;
    ///   ckp:occurredAt "{timestamp}"^^xsd:dateTime .
    /// ```
    pub async fn insert_edge_routing_occurrent(
        &self,
        source_kernel: &str,
        target_kernel: &str,
        predicate: &str,
        transaction_id: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let urn = format!("ckp://EdgeRouting/{}", transaction_id);
        let timestamp_str = timestamp.to_rfc3339();

        let sparql_insert = format!(
            r#"PREFIX bfo: <http://purl.obolibrary.org/obo/>
PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
  <{urn}> a bfo:BFO_0000003 ;  # Occurrent
          a ckp:EdgeRouting ;
          ckp:sourceKernel "{source_kernel}" ;
          ckp:targetKernel "{target_kernel}" ;
          ckp:predicate "{predicate}" ;
          ckp:transactionId "{transaction_id}" ;
          ckp:occurredAt "{timestamp}"^^xsd:dateTime .
}}"#,
            urn = urn,
            source_kernel = source_kernel,
            target_kernel = target_kernel,
            predicate = predicate,
            transaction_id = transaction_id,
            timestamp = timestamp_str
        );

        self.execute_sparql_update(&sparql_insert).await
    }

    /// Query all transactions (WorkflowExecution, KernelInvocation, EdgeRouting)
    ///
    /// Returns all BFO Occurrent transactions ordered by timestamp.
    ///
    /// # Returns
    ///
    /// Vec of JSON objects containing transaction details
    ///
    /// # Example Output
    ///
    /// ```json
    /// [
    ///   {
    ///     "type": "WorkflowExecution",
    ///     "urn": "ckp://WorkflowExecution/tx-123",
    ///     "transactionId": "tx-123",
    ///     "workflowName": "bakery-flow",
    ///     "timestamp": "2025-01-04T10:00:00Z"
    ///   },
    ///   {
    ///     "type": "KernelInvocation",
    ///     "urn": "ckp://KernelInvocation/tx-123/System.Bakery",
    ///     "kernelName": "System.Bakery",
    ///     "transactionId": "tx-123",
    ///     "timestamp": "2025-01-04T10:00:01Z"
    ///   }
    /// ]
    /// ```
    pub async fn query_transactions(&self) -> Result<Vec<serde_json::Value>> {
        let sparql = r#"PREFIX bfo: <http://purl.obolibrary.org/obo/>
PREFIX ckp: <urn:ckp:>

SELECT ?urn ?type ?transactionId ?timestamp ?workflowUrn ?kernelName ?sourceKernel ?targetKernel ?predicate
WHERE {
  ?urn a bfo:BFO_0000003 ;
       ckp:transactionId ?transactionId ;
       ckp:timestamp ?timestamp .

  # Get specific type
  ?urn a ?typeClass .
  FILTER(?typeClass IN (ckp:WorkflowStart, ckp:KernelInvocation, ckp:EdgeRouting))

  BIND(REPLACE(STR(?typeClass), ".*:", "") AS ?type)

  # Optional fields based on type
  OPTIONAL { ?urn ckp:workflowUrn ?workflowUrn }
  OPTIONAL { ?urn ckp:kernelName ?kernelName }
  OPTIONAL { ?urn ckp:sourceKernel ?sourceKernel }
  OPTIONAL { ?urn ckp:targetKernel ?targetKernel }
  OPTIONAL { ?urn ckp:predicate ?predicate }
}
ORDER BY ?timestamp"#;

        let result = self.execute_sparql_query(sparql).await?;

        // Parse SPARQL results into JSON objects
        let bindings = result
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
            .ok_or_else(|| CkpError::SparqlError("Invalid SPARQL results".to_string()))?;

        let mut transactions = Vec::new();
        for binding in bindings {
            let mut tx = serde_json::Map::new();

            // Required fields
            if let Some(urn) = binding.get("urn").and_then(|u| u.get("value")).and_then(|v| v.as_str()) {
                tx.insert("urn".to_string(), serde_json::Value::String(urn.to_string()));
            }
            if let Some(tx_type) = binding.get("type").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
                tx.insert("type".to_string(), serde_json::Value::String(tx_type.to_string()));
            }
            if let Some(tx_id) = binding.get("transactionId").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
                tx.insert("transactionId".to_string(), serde_json::Value::String(tx_id.to_string()));
            }
            if let Some(timestamp) = binding.get("timestamp").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
                tx.insert("timestamp".to_string(), serde_json::Value::String(timestamp.to_string()));
            }

            // Optional fields
            if let Some(workflow) = binding.get("workflowUrn").and_then(|w| w.get("value")).and_then(|v| v.as_str()) {
                tx.insert("workflowUrn".to_string(), serde_json::Value::String(workflow.to_string()));
            }
            if let Some(kernel) = binding.get("kernelName").and_then(|k| k.get("value")).and_then(|v| v.as_str()) {
                tx.insert("kernelName".to_string(), serde_json::Value::String(kernel.to_string()));
            }
            if let Some(source) = binding.get("sourceKernel").and_then(|s| s.get("value")).and_then(|v| v.as_str()) {
                tx.insert("sourceKernel".to_string(), serde_json::Value::String(source.to_string()));
            }
            if let Some(target) = binding.get("targetKernel").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
                tx.insert("targetKernel".to_string(), serde_json::Value::String(target.to_string()));
            }
            if let Some(predicate) = binding.get("predicate").and_then(|p| p.get("value")).and_then(|v| v.as_str()) {
                tx.insert("predicate".to_string(), serde_json::Value::String(predicate.to_string()));
            }

            transactions.push(serde_json::Value::Object(tx));
        }

        Ok(transactions)
    }

    /// Query specific transaction by ID
    ///
    /// Returns transaction details for a specific transaction ID.
    ///
    /// # Arguments
    ///
    /// * `transaction_id` - Transaction ID to query
    ///
    /// # Returns
    ///
    /// Option<JsonValue> with transaction details (None if not found)
    pub async fn query_transaction_by_id(&self, transaction_id: &str) -> Result<Option<serde_json::Value>> {
        let sparql = format!(
            r#"PREFIX bfo: <http://purl.obolibrary.org/obo/>
PREFIX ckp: <https://conceptkernel.org/ontology#>

SELECT ?urn ?type ?timestamp ?workflowName ?kernelName ?sourceKernel ?targetKernel ?predicate
WHERE {{
  ?urn a bfo:BFO_0000003 ;
       ckp:transactionId "{transaction_id}" ;
       ckp:occurredAt ?timestamp .

  ?urn a ?typeClass .
  FILTER(?typeClass IN (ckp:WorkflowExecution, ckp:KernelInvocation, ckp:EdgeRouting))

  BIND(REPLACE(STR(?typeClass), ".*#", "") AS ?type)

  OPTIONAL {{ ?urn ckp:workflowName ?workflowName }}
  OPTIONAL {{ ?urn ckp:kernelName ?kernelName }}
  OPTIONAL {{ ?urn ckp:sourceKernel ?sourceKernel }}
  OPTIONAL {{ ?urn ckp:targetKernel ?targetKernel }}
  OPTIONAL {{ ?urn ckp:predicate ?predicate }}
}}"#,
            transaction_id = transaction_id
        );

        let result = self.execute_sparql_query(&sparql).await?;

        let bindings = result
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
            .ok_or_else(|| CkpError::SparqlError("Invalid SPARQL results".to_string()))?;

        if bindings.is_empty() {
            return Ok(None);
        }

        let binding = &bindings[0];
        let mut tx = serde_json::Map::new();

        tx.insert("transactionId".to_string(), serde_json::Value::String(transaction_id.to_string()));

        if let Some(urn) = binding.get("urn").and_then(|u| u.get("value")).and_then(|v| v.as_str()) {
            tx.insert("urn".to_string(), serde_json::Value::String(urn.to_string()));
        }
        if let Some(tx_type) = binding.get("type").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
            tx.insert("type".to_string(), serde_json::Value::String(tx_type.to_string()));
        }
        if let Some(timestamp) = binding.get("timestamp").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
            tx.insert("timestamp".to_string(), serde_json::Value::String(timestamp.to_string()));
        }
        if let Some(workflow) = binding.get("workflowName").and_then(|w| w.get("value")).and_then(|v| v.as_str()) {
            tx.insert("workflowName".to_string(), serde_json::Value::String(workflow.to_string()));
        }
        if let Some(kernel) = binding.get("kernelName").and_then(|k| k.get("value")).and_then(|v| v.as_str()) {
            tx.insert("kernelName".to_string(), serde_json::Value::String(kernel.to_string()));
        }
        if let Some(source) = binding.get("sourceKernel").and_then(|s| s.get("value")).and_then(|v| v.as_str()) {
            tx.insert("sourceKernel".to_string(), serde_json::Value::String(source.to_string()));
        }
        if let Some(target) = binding.get("targetKernel").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
            tx.insert("targetKernel".to_string(), serde_json::Value::String(target.to_string()));
        }
        if let Some(predicate) = binding.get("predicate").and_then(|p| p.get("value")).and_then(|v| v.as_str()) {
            tx.insert("predicate".to_string(), serde_json::Value::String(predicate.to_string()));
        }

        Ok(Some(serde_json::Value::Object(tx)))
    }

    /// Query transactions filtered by workflow name
    ///
    /// Returns all transactions associated with a specific workflow.
    ///
    /// # Arguments
    ///
    /// * `workflow_name` - Workflow name to filter by
    ///
    /// # Returns
    ///
    /// Vec of JSON objects containing transaction details
    pub async fn query_transactions_by_workflow(&self, workflow_urn: &str) -> Result<Vec<serde_json::Value>> {
        let sparql = format!(
            r#"PREFIX bfo: <http://purl.obolibrary.org/obo/>
PREFIX ckp: <urn:ckp:>

SELECT ?urn ?type ?transactionId ?timestamp ?workflowUrn ?kernelName ?sourceKernel ?targetKernel ?predicate
WHERE {{
  # First, find all transaction IDs for this workflow
  ?workflowExec a ckp:WorkflowStart ;
                ckp:workflowUrn "{workflow_urn}" ;
                ckp:transactionId ?transactionId .

  # Then, find all occurrents with those transaction IDs
  ?urn a bfo:BFO_0000003 ;
       ckp:transactionId ?transactionId ;
       ckp:timestamp ?timestamp .

  ?urn a ?typeClass .
  FILTER(?typeClass IN (ckp:WorkflowStart, ckp:KernelInvocation, ckp:EdgeRouting))

  BIND(REPLACE(STR(?typeClass), ".*:", "") AS ?type)

  OPTIONAL {{ ?urn ckp:workflowUrn ?workflowUrn }}
  OPTIONAL {{ ?urn ckp:kernelName ?kernelName }}
  OPTIONAL {{ ?urn ckp:sourceKernel ?sourceKernel }}
  OPTIONAL {{ ?urn ckp:targetKernel ?targetKernel }}
  OPTIONAL {{ ?urn ckp:predicate ?predicate }}
}}
ORDER BY ?timestamp"#,
            workflow_urn = workflow_urn
        );

        let result = self.execute_sparql_query(&sparql).await?;

        let bindings = result
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
            .ok_or_else(|| CkpError::SparqlError("Invalid SPARQL results".to_string()))?;

        let mut transactions = Vec::new();
        for binding in bindings {
            let mut tx = serde_json::Map::new();

            if let Some(urn) = binding.get("urn").and_then(|u| u.get("value")).and_then(|v| v.as_str()) {
                tx.insert("urn".to_string(), serde_json::Value::String(urn.to_string()));
            }
            if let Some(tx_type) = binding.get("type").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
                tx.insert("type".to_string(), serde_json::Value::String(tx_type.to_string()));
            }
            if let Some(tx_id) = binding.get("transactionId").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
                tx.insert("transactionId".to_string(), serde_json::Value::String(tx_id.to_string()));
            }
            if let Some(timestamp) = binding.get("timestamp").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
                tx.insert("timestamp".to_string(), serde_json::Value::String(timestamp.to_string()));
            }
            if let Some(workflow) = binding.get("workflowUrn").and_then(|w| w.get("value")).and_then(|v| v.as_str()) {
                tx.insert("workflowUrn".to_string(), serde_json::Value::String(workflow.to_string()));
            }
            if let Some(kernel) = binding.get("kernelName").and_then(|k| k.get("value")).and_then(|v| v.as_str()) {
                tx.insert("kernelName".to_string(), serde_json::Value::String(kernel.to_string()));
            }
            if let Some(source) = binding.get("sourceKernel").and_then(|s| s.get("value")).and_then(|v| v.as_str()) {
                tx.insert("sourceKernel".to_string(), serde_json::Value::String(source.to_string()));
            }
            if let Some(target) = binding.get("targetKernel").and_then(|t| t.get("value")).and_then(|v| v.as_str()) {
                tx.insert("targetKernel".to_string(), serde_json::Value::String(target.to_string()));
            }
            if let Some(predicate) = binding.get("predicate").and_then(|p| p.get("value")).and_then(|v| v.as_str()) {
                tx.insert("predicate".to_string(), serde_json::Value::String(predicate.to_string()));
            }

            transactions.push(serde_json::Value::Object(tx));
        }

        Ok(transactions)
    }

    /// Get named graph URI for kernel
    fn graph_uri(&self, kernel_name: &str) -> String {
        format!("ckp://{}/ontology", kernel_name)
    }

    /// Upload RDF to named graph
    async fn upload_rdf(&self, kernel_name: &str, ttl: &str) -> Result<()> {
        let graph_uri = self.graph_uri(kernel_name);

        let response = self
            .client
            .put(&self.data_endpoint())
            .query(&[("graph", &graph_uri)])
            .header("Content-Type", "text/turtle")
            .body(ttl.to_string())
            .send()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to upload RDF: {}", e)))?;

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "RDF upload failed: {}",
                response.status()
            )));
        }

        Ok(())
    }

    /// Download RDF from named graph
    async fn download_rdf(&self, kernel_name: &str) -> Result<String> {
        let graph_uri = self.graph_uri(kernel_name);

        let response = self
            .client
            .get(&self.data_endpoint())
            .query(&[("graph", &graph_uri)])
            .header("Accept", "text/turtle")
            .send()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to download RDF: {}", e)))?;

        if response.status() == 404 {
            return Err(CkpError::NotFound(format!(
                "Ontology not found for kernel {}",
                kernel_name
            )));
        }

        if !response.status().is_success() {
            return Err(CkpError::Http(format!(
                "RDF download failed: {}",
                response.status()
            )));
        }

        let ttl = response
            .text()
            .await
            .map_err(|e| CkpError::Http(format!("Failed to read RDF response: {}", e)))?;

        Ok(ttl)
    }
}

#[async_trait]
impl StorageDriver for JenaStorage {
    // ========================================================================
    // EXISTING METHODS (v1.3.19 - implemented via SPARQL)
    // ========================================================================

    async fn write_job(&self, _target_urn: &str, _job: JobFile) -> Result<String> {
        // Jobs are still stored in NATS or filesystem, not in RDF
        // JenaStorage is primarily for ontology storage
        Err(CkpError::NotImplemented(
            "write_job not supported by JenaStorage (use LocalStorage or NATS)".to_string(),
        ))
    }

    async fn read_jobs(&self, _kernel_name: &str) -> Result<Vec<JobHandle>> {
        Err(CkpError::NotImplemented(
            "read_jobs not supported by JenaStorage (use LocalStorage or NATS)".to_string(),
        ))
    }

    async fn archive_job(&self, _kernel_name: &str, _job: &JobHandle) -> Result<()> {
        Err(CkpError::NotImplemented(
            "archive_job not supported by JenaStorage (use LocalStorage or NATS)".to_string(),
        ))
    }

    async fn mint_storage_artifact(
        &self,
        kernel_name: &str,
        instance_id: &str,
        data: JsonValue,
    ) -> Result<String> {
        // Store instance as RDF triple
        let instance_uri = format!("ckp://{}/storage/{}", kernel_name, instance_id);

        // Convert JSON to simple triple
        let ttl = format!(
            r#"@prefix ckp: <ckp://> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

<{}> a ckp:StorageInstance ;
    ckp:kernel "{}" ;
    ckp:instanceId "{}" ;
    ckp:data "{}" .
"#,
            instance_uri,
            kernel_name,
            instance_id,
            serde_json::to_string(&data).map_err(|e| CkpError::Json(e))?
        );

        // Upload to Jena
        self.upload_rdf(&format!("{}/storage", kernel_name), &ttl)
            .await?;

        // Broadcast event
        let _ = self.event_tx.send(StorageEvent::InstanceCreated {
            kernel_name: kernel_name.to_string(),
            instance_urn: instance_uri.clone(),
        });

        Ok(instance_uri)
    }

    async fn record_transaction(
        &self,
        kernel_name: &str,
        transaction: JsonValue,
    ) -> Result<()> {
        // Store transaction as RDF triple
        let tx_id = transaction
            .get("txId")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let ttl = format!(
            r#"@prefix ckp: <ckp://> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

<ckp://{}/tx/{}> a ckp:Transaction ;
    ckp:kernel "{}" ;
    ckp:data "{}" ;
    ckp:timestamp "{}" .
"#,
            kernel_name,
            tx_id,
            kernel_name,
            serde_json::to_string(&transaction).map_err(|e| CkpError::Json(e))?,
            chrono::Utc::now().to_rfc3339()
        );

        self.upload_rdf(&format!("{}/transactions", kernel_name), &ttl)
            .await?;

        Ok(())
    }

    async fn resolve_urn(&self, urn: &str) -> Result<StorageLocation> {
        // URNs in Jena are just URIs
        Ok(StorageLocation::Urn(urn.to_string()))
    }

    async fn kernel_exists(&self, kernel_name: &str) -> Result<bool> {
        // Check if kernel graph exists by trying to download it
        match self.download_rdf(kernel_name).await {
            Ok(_) => Ok(true),
            Err(CkpError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    async fn get_edge_queue(
        &self,
        _kernel_name: &str,
        _source_kernel: &str,
    ) -> Result<StorageLocation> {
        // Edge queues not supported in Jena
        Err(CkpError::NotImplemented(
            "get_edge_queue not supported by JenaStorage".to_string(),
        ))
    }

    // ========================================================================
    // NEW METHODS (v1.3.20) - RDF-specific implementations
    // ========================================================================

    async fn load_ontology(&self, kernel_name: &str) -> Result<String> {
        // Download RDF from Jena
        self.download_rdf(kernel_name).await
    }

    async fn save_ontology(&self, kernel_name: &str, ttl: &str) -> Result<()> {
        // Upload RDF to Jena
        self.upload_rdf(kernel_name, ttl).await
    }

    async fn load_tool_definition(
        &self,
        kernel_name: &str,
        _tool_name: &str,
    ) -> Result<ToolDefinition> {
        // Load ontology and parse using SPARQL
        let sparql = format!(
            r#"
PREFIX ckp: <ckp://>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?type ?image ?timeout
WHERE {{
  GRAPH <{}> {{
    ?kernel ckp:type ?type .
    OPTIONAL {{ ?kernel ckp:containerImage ?image }}
    OPTIONAL {{ ?kernel ckp:timeout ?timeout }}
  }}
}}
"#,
            self.graph_uri(kernel_name)
        );

        let results = self.query(&sparql).await?;

        // Parse results
        let bindings = results
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
            .ok_or_else(|| CkpError::Config("Invalid SPARQL results".to_string()))?;

        if bindings.is_empty() {
            return Err(CkpError::NotFound(format!(
                "No tool definition found for kernel {}",
                kernel_name
            )));
        }

        let first = &bindings[0];
        let kernel_type = first
            .get("type")
            .and_then(|t| t.get("value"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| CkpError::Config("Missing kernel type".to_string()))?;

        let execution_mode = ExecutionMode::from_kernel_type(kernel_type)?;

        let container_image = first
            .get("image")
            .and_then(|i| i.get("value"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let timeout_seconds = first
            .get("timeout")
            .and_then(|t| t.get("value"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(300);

        Ok(ToolDefinition {
            name: kernel_name.to_string(),
            execution_mode,
            container_image,
            command: Vec::new(),
            args: Vec::new(),
            env_vars: HashMap::new(),
            resources: None,
            timeout_seconds,
        })
    }

    async fn save_result(
        &self,
        kernel_name: &str,
        job_id: &str,
        result: &ToolResponse,
    ) -> Result<()> {
        // Store result as RDF triple
        let ttl = format!(
            r#"@prefix ckp: <ckp://> .

<ckp://{}/results/{}> a ckp:ToolResult ;
    ckp:jobId "{}" ;
    ckp:status "{}" ;
    ckp:timestamp "{}" ;
    ckp:durationMs {} ;
    ckp:output "{}" .
"#,
            kernel_name,
            job_id,
            result.job_id,
            result.status,
            result.timestamp,
            result.duration_ms,
            serde_json::to_string(&result.output).map_err(|e| CkpError::Json(e))?
        );

        self.upload_rdf(&format!("{}/results", kernel_name), &ttl)
            .await?;

        Ok(())
    }

    async fn load_result(&self, kernel_name: &str, job_id: &str) -> Result<ToolResponse> {
        // Query result from Jena using SPARQL
        let sparql = format!(
            r#"
PREFIX ckp: <ckp://>

SELECT ?jobId ?status ?timestamp ?durationMs ?output
WHERE {{
  GRAPH <ckp://{}/results> {{
    ?result ckp:jobId "{}" ;
            ckp:status ?status ;
            ckp:timestamp ?timestamp ;
            ckp:durationMs ?durationMs ;
            ckp:output ?output .
    BIND("{}" AS ?jobId)
  }}
}}
"#,
            kernel_name, job_id, job_id
        );

        let results = self.query(&sparql).await?;

        let bindings = results
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
            .ok_or_else(|| CkpError::Config("Invalid SPARQL results".to_string()))?;

        if bindings.is_empty() {
            return Err(CkpError::NotFound(format!(
                "Result not found for job {}",
                job_id
            )));
        }

        let first = &bindings[0];

        Ok(ToolResponse {
            job_id: job_id.to_string(),
            status: first
                .get("status")
                .and_then(|s| s.get("value"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            output: serde_json::from_str(
                first
                    .get("output")
                    .and_then(|o| o.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("{}"),
            )
            .unwrap_or(serde_json::json!({})),
            timestamp: first
                .get("timestamp")
                .and_then(|t| t.get("value"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            duration_ms: first
                .get("durationMs")
                .and_then(|d| d.get("value"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0),
            error: None,
        })
    }

    async fn list_edge_queues(&self, kernel_name: &str) -> Result<Vec<(String, usize)>> {
        // Query for all edges targeting this kernel
        let sparql = format!(
            r#"PREFIX ckp: <http://conceptkernel.org/>
SELECT ?source (COUNT(?instance) AS ?count)
WHERE {{
  ?edge a ckp:Edge ;
        ckp:target "{}" ;
        ckp:source ?source .
  OPTIONAL {{
    ?instance ckp:sourceKernel ?source ;
              ckp:targetKernel "{}" .
  }}
}}
GROUP BY ?source
"#,
            kernel_name, kernel_name
        );

        let result = self.query(&sparql).await?;

        let mut edges = Vec::new();

        if let Some(bindings) = result
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
        {
            for binding in bindings {
                let source = binding
                    .get("source")
                    .and_then(|s| s.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let count = binding
                    .get("count")
                    .and_then(|c| c.get("value"))
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(0);

                if !source.is_empty() {
                    edges.push((source, count));
                }
            }
        }

        Ok(edges)
    }

    async fn list_instances(&self, kernel_name: &str) -> Result<Vec<String>> {
        // Query for all instances for this kernel
        let sparql = format!(
            r#"PREFIX ckp: <ckp://>
SELECT ?instance ?instanceId
WHERE {{
  ?instance a ckp:StorageInstance ;
            ckp:kernel "{}" ;
            ckp:instanceId ?instanceId .
}}
"#,
            kernel_name
        );

        let result = self.query(&sparql).await?;

        let mut instances = Vec::new();

        if let Some(bindings) = result
            .get("results")
            .and_then(|r| r.get("bindings"))
            .and_then(|b| b.as_array())
        {
            for binding in bindings {
                if let Some(instance_urn) = binding
                    .get("instance")
                    .and_then(|i| i.get("value"))
                    .and_then(|v| v.as_str())
                {
                    instances.push(instance_urn.to_string());
                }
            }
        }

        Ok(instances)
    }

    async fn subscribe_storage_events(&self) -> Result<StorageEventStream> {
        // For Jena, we poll for new instances (Jena doesn't have native event streaming)
        let rx = self.event_tx.subscribe();

        let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
            .filter_map(|result| async move {
                match result {
                    Ok(event) => Some(event),
                    Err(_) => None,
                }
            });

        Ok(Box::pin(stream))
    }

    /// Execute SPARQL UPDATE query (trait override)
    ///
    /// Sends SPARQL UPDATE to Jena Fuseki endpoint
    async fn execute_sparql_update(&self, sparql_update: &str) -> Result<()> {
        // Inline implementation to avoid any recursion issues
        let update_url = format!("{}/{}", self.fuseki_url, self.dataset);

        let response = self.client
            .post(&update_url)
            .header("Content-Type", "application/sparql-update")
            .body(sparql_update.to_string())
            .send()
            .await
            .map_err(|e| CkpError::IoError(format!("SPARQL UPDATE request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read response".to_string());
            return Err(CkpError::IoError(format!(
                "SPARQL UPDATE failed ({}): {}",
                status, body
            )));
        }

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running Jena Fuseki server
    // Run: docker run -p 3030:3030 stain/jena-fuseki
    // Create dataset: curl -X POST http://localhost:3030/$/datasets -d 'dbName=ck&dbType=tdb2'

    #[tokio::test]
    #[ignore] // Requires Jena Fuseki server
    async fn test_jena_storage_save_load_ontology() {
        let storage = JenaStorage::new("http://localhost:3030".to_string(), "ck".to_string());

        let ttl = r#"@prefix ckp: <ckp://> .
<ckp://TestKernel> a ckp:Kernel ;
    ckp:type "node:cold" ;
    ckp:version "v1.0.0" .
"#;

        // Save ontology
        storage
            .save_ontology("TestKernel", ttl)
            .await
            .unwrap();

        // Load ontology
        let loaded = storage.load_ontology("TestKernel").await.unwrap();
        assert!(loaded.contains("TestKernel"));
        assert!(loaded.contains("node:cold"));
    }

    #[tokio::test]
    #[ignore] // Requires Jena Fuseki server
    async fn test_jena_storage_kernel_exists() {
        let storage = JenaStorage::new("http://localhost:3030".to_string(), "ck".to_string());

        // Should return false for non-existent kernel
        assert!(!storage.kernel_exists("NonExistentKernel").await.unwrap());
    }
}
