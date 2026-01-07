/// Workflow module for System.Workflow kernel
///
/// Provides public API for:
/// - Parsing CKDL (Concept Kernel Definition Language) files
/// - Storing workflow definitions in Oxigraph as RDF
/// - Detecting circular references using SPARQL
/// - Validating workflow structure
/// - Executing workflows by coordinating kernel actions
/// - Querying workflow status and history

pub mod validator;
pub mod ckdl_parser;
mod occurrents;

pub use ckdl_parser::{
    parse_ckdl_file, ckdl_to_workflow,
    CkdlWorkflow, ExternKernel, WorkflowKernel, CkdlEdge,
    ComponentOrigin, ComponentAnalysis,
};
pub use occurrents::OccurrentTracker;

use crate::ontology::{OntologyLibrary, OntologyError};
use crate::event_publisher::{KernelEventPublisher, KernelEventType};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// Workflow stored in System.Workflow kernel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub workflow_urn: String,
    pub label: String,
    pub description: String,
    pub version: String,
    pub trigger: WorkflowTrigger,
    pub phases: Vec<WorkflowPhase>,
    pub edges: Vec<WorkflowEdge>,
    pub status: WorkflowStatus,
}

/// Workflow execution phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowPhase {
    pub phase_name: String,
    pub kernel_urn: String,
    pub status: PhaseStatus,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}

/// Workflow edge defining kernel interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub edge_urn: String,
    pub source: String,
    pub target: String,
    pub predicate: String,
    pub trigger: String,
    pub action: Option<String>,
}

/// Workflow trigger condition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowTrigger {
    OnDaemonStartup,
    OnActionRequest,
    OnSchedule(String),  // e.g., "daily", "hourly"
    OnEvent(String),     // e.g., "kernel-registered", "issue-detected"
}

/// Workflow execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Blocked,
}

/// Phase execution status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhaseStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

/// Cycle detected in workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowCycle {
    pub kernels: Vec<String>,
    pub is_intentional: bool,
    pub has_exit_condition: bool,
    pub cycle_type: CycleType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CycleType {
    ClosedLoopVerification,  // Validator -> ... -> Wss -> Validator
    RequestResponse,         // A -> B -> A (simple feedback)
    Problematic,             // No clear exit condition
}

/// Validation result for workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowValidation {
    pub is_valid: bool,
    pub cycles: Vec<WorkflowCycle>,
    pub missing_kernels: Vec<String>,
    pub invalid_predicates: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Unified workflow API
pub struct WorkflowAPI {
    library: OntologyLibrary,
    event_publisher: Option<Arc<KernelEventPublisher>>,
}

impl WorkflowAPI {
    /// Create new workflow API with ontology library (without events)
    pub fn new(library: OntologyLibrary) -> Self {
        Self { library, event_publisher: None }
    }

    /// Create new workflow API with event publishing enabled
    pub async fn new_with_events(
        library: OntologyLibrary,
        nats_url: Option<&str>,
    ) -> Result<Self, OntologyError> {
        let event_publisher = if let Some(url) = nats_url {
            match KernelEventPublisher::new("System.Workflow".to_string(), Some(url)).await {
                Ok(publisher) => Some(Arc::new(publisher)),
                Err(e) => {
                    eprintln!("[WorkflowAPI] Failed to create event publisher: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self { library, event_publisher })
    }

    /// Load workflow from CKDL file
    ///
    /// Parses CKDL file and stores workflow definition in Oxigraph as RDF.
    /// Returns workflow URN for later querying and execution.
    ///
    /// # Example
    /// ```no_run
    /// use ckp_core::workflow::WorkflowAPI;
    /// use ckp_core::ontology::OntologyLibrary;
    /// use std::path::PathBuf;
    ///
    /// let library = OntologyLibrary::new(PathBuf::from("."))?;
    /// let mut workflow_api = WorkflowAPI::new(library);
    ///
    /// let workflow_urn = workflow_api.load_workflow_from_ckdl(
    ///     "workflows/self-improvement-cycle.ckdl"
    /// )?;
    ///
    /// println!("Loaded workflow: {}", workflow_urn);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn load_workflow_from_ckdl(&mut self, ckdl_path: impl AsRef<Path>) -> Result<String, OntologyError> {
        // Parse CKDL file with component origin analysis
        let ckdl_workflow = parse_ckdl_file(ckdl_path, &self.library.project_root)?;

        // Print component analysis
        eprintln!("[WorkflowAPI] Parsed CKDL workflow:");
        eprintln!("[WorkflowAPI]   URN: {}", ckdl_workflow.workflow_urn);
        eprintln!("[WorkflowAPI]   Label: {}", ckdl_workflow.label);
        eprintln!("[WorkflowAPI]   EXTERN kernels: {}", ckdl_workflow.analysis.total_extern);
        eprintln!("[WorkflowAPI]   Workflow kernels: {}", ckdl_workflow.analysis.total_workflow_kernels);
        eprintln!("[WorkflowAPI]   Edges: {}", ckdl_workflow.analysis.total_edges);
        eprintln!();
        eprintln!("[WorkflowAPI] Component Analysis:");
        eprintln!("[WorkflowAPI]   Forked kernels: {}", ckdl_workflow.analysis.forked_kernels.len());
        for k in &ckdl_workflow.analysis.forked_kernels {
            eprintln!("[WorkflowAPI]     ✓ {}", k);
        }
        eprintln!("[WorkflowAPI]   Brand new kernels: {}", ckdl_workflow.analysis.brand_new_kernels.len());
        for k in &ckdl_workflow.analysis.brand_new_kernels {
            eprintln!("[WorkflowAPI]     + {}", k);
        }
        eprintln!();

        // Convert to Workflow struct
        let workflow = ckdl_to_workflow(ckdl_workflow.clone());

        // ========================================================================
        // WORKFLOW PERSISTENCE (v1.3.20)
        // ========================================================================
        // Store workflow as RDF triples in Oxigraph for querying/listing

        eprintln!("[WorkflowAPI] Storing workflow in RDF store...");

        // Escape description for SPARQL (replace quotes, newlines, and Unicode arrows)
        let escaped_description = workflow.description
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "")
            .replace('→', "->")  // Replace Unicode arrow with ASCII
            .replace('←', "<-")  // Replace left arrow too
            .replace('↔', "<->");  // Bidirectional arrow

        // Build SPARQL INSERT for workflow metadata
        let workflow_insert = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

INSERT DATA {{
    <{workflow_urn}> rdf:type ckpw:Workflow ;
                     ckpw:workflowLabel "{label}" ;
                     ckpw:workflowDescription "{description}" ;
                     ckpw:workflowVersion "{version}" ;
                     ckpw:workflowStatus "{status}" .
}}
"#,
            workflow_urn = workflow.workflow_urn,
            label = workflow.label.replace('"', "\\\""),
            description = escaped_description,
            version = workflow.version,
            status = format!("{:?}", workflow.status),
        );

        // Execute workflow metadata insert
        self.library.execute_update(&workflow_insert)
            .map_err(|e| {
                eprintln!("[WorkflowAPI] Failed to store workflow metadata: {}", e);
                e
            })?;

        eprintln!("[WorkflowAPI] ✅ Workflow metadata stored");

        // Store workflow phases if any
        if !workflow.phases.is_empty() {
            eprintln!("[WorkflowAPI] Storing {} workflow phases...", workflow.phases.len());

            for (idx, phase) in workflow.phases.iter().enumerate() {
                // Create simple phase URN by extracting workflow name
                let workflow_name = workflow.workflow_urn
                    .trim_start_matches("ckp://")
                    .replace("#", "-")
                    .replace(":", "-")
                    .replace("/", "-");
                let phase_urn = format!("ckp://WorkflowPhase-{}-{}", workflow_name, idx);

                // Normalize kernel URN - ensure it starts with ckp:// and sanitize for SPARQL
                let mut kernel_urn = if !phase.kernel_urn.starts_with("ckp://") {
                    format!("ckp://{}", phase.kernel_urn)
                } else {
                    phase.kernel_urn.clone()
                };

                // Phase storage enabled - extract phase name from kernel URN
                eprintln!("[WorkflowAPI]   Storing phase {}: {}", idx, phase.kernel_urn);

                let phase_insert = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

INSERT DATA {{
    <{phase_urn}> rdf:type ckpw:WorkflowPhase ;
                  ckpw:phaseIndex {idx} ;
                  ckpw:phaseKernelUrn "{kernel_urn}" ;
                  ckpw:phaseStatus "{status}" .
}}
"#,
                    phase_urn = phase_urn,
                    idx = idx,
                    kernel_urn = kernel_urn.replace('"', "\\\""),
                    status = format!("{:?}", phase.status),
                );

                eprintln!("[WorkflowAPI] DEBUG SPARQL:\n{}", phase_insert);
                self.library.execute_update(&phase_insert)?;
            }
        }

        // Store workflow edges if any
        if !workflow.edges.is_empty() {
            eprintln!("[WorkflowAPI] Storing {} workflow edges...", workflow.edges.len());
            for (idx, edge) in workflow.edges.iter().enumerate() {
                // Create simple edge URN by extracting workflow name
                let workflow_name = workflow.workflow_urn
                    .trim_start_matches("ckp://")
                    .replace("#", "-")
                    .replace(":", "-")
                    .replace("/", "-");
                let edge_urn = format!("ckp://WorkflowEdge-{}-{}", workflow_name, idx);

                let edge_insert = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

INSERT DATA {{
    <{edge_urn}> rdf:type ckpw:WorkflowEdge ;
                 ckpw:edgeSourceUrn "{source}" ;
                 ckpw:edgePredicate "{predicate}" ;
                 ckpw:edgeTargetUrn "{target}" .
}}
"#,
                    edge_urn = edge_urn,
                    source = edge.source.replace('"', "\\\""),
                    predicate = edge.predicate.replace('"', "\\\""),
                    target = edge.target.replace('"', "\\\""),
                );

                self.library.execute_update(&edge_insert)?;

                // ALSO create actual EdgeConnection for routing (diskless mode)
                eprintln!("[WorkflowAPI]   DEBUG: edge.source = '{}'", edge.source);
                eprintln!("[WorkflowAPI]   DEBUG: edge.target = '{}'", edge.target);
                eprintln!("[WorkflowAPI]   DEBUG: edge.predicate = '{}'", edge.predicate);

                // Extract kernel names from URNs (ckp://Kernel.Name:v1.0 -> Kernel.Name)
                let source_name = edge.source
                    .trim_start_matches("ckp://")
                    .split(':')
                    .next()
                    .unwrap_or(&edge.source);
                let target_name = edge.target
                    .trim_start_matches("ckp://")
                    .split(':')
                    .next()
                    .unwrap_or(&edge.target);
                // Normalize predicate (ckp:produces -> PRODUCES)
                let predicate_name = edge.predicate
                    .trim_start_matches("ckp:")
                    .to_uppercase();

                eprintln!("[WorkflowAPI]   Creating EdgeConnection for routing: {} --{}-->  {}",
                    source_name, predicate_name, target_name);

                let edge_urn = format!("ckp://Edge#Connection-{}-to-{}-{}:v1.3.20",
                    source_name, target_name, predicate_name);
                let graph_uri = format!("ckp://edges/{}/{}-to-{}",
                    predicate_name, source_name, target_name);
                let timestamp = chrono::Utc::now().to_rfc3339();

                // Use TTL format with /data endpoint (proven working method)
                let edge_connection_ttl = format!(r#"@prefix ckp: <https://conceptkernel.org/ontology#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

<{urn}> a ckp:EdgeConnection ;
    ckp:hasURN "{urn}" ;
    ckp:hasPredicate ckp:Edge-{predicate} ;
    ckp:hasSource ckp:Kernel-{source} ;
    ckp:hasTarget ckp:Kernel-{target} ;
    ckp:version "1.3.20"^^xsd:string ;
    ckp:createdAt "{timestamp}"^^xsd:dateTime ;
    ckp:status "active"^^xsd:string .

ckp:Kernel-{source} ckp:hasName "{source}" .
ckp:Kernel-{target} ckp:hasName "{target}" .
ckp:Edge-{predicate} ckp:predicateName "{predicate}" .
"#,
                    urn = edge_urn,
                    predicate = predicate_name,
                    source = source_name,
                    target = target_name,
                    timestamp = timestamp,
                );

                eprintln!("[WorkflowAPI]   Saving EdgeConnection to graph: {}", graph_uri);

                self.library.save_ttl_to_graph(&edge_connection_ttl, &graph_uri)
                    .map_err(|e| {
                        eprintln!("[WorkflowAPI] ERROR: Failed to save EdgeConnection: {}", e);
                        e
                    })?;
                eprintln!("[WorkflowAPI]   ✓ EdgeConnection saved to Jena");
            }
        }

        eprintln!("[WorkflowAPI] ✅ Workflow persisted to RDF store");

        Ok(ckdl_workflow.workflow_urn)
    }

    /// Validate workflow structure and detect cycles
    ///
    /// Performs comprehensive validation:
    /// - Detects circular references using SPARQL
    /// - Classifies cycles as intentional vs problematic
    /// - Verifies all referenced kernels exist
    /// - Checks edge predicates are valid
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::workflow::WorkflowAPI;
    /// # use ckp_core::ontology::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # let library = OntologyLibrary::new(PathBuf::from("."))?;
    /// # let workflow_api = WorkflowAPI::new(library);
    /// let validation = workflow_api.validate_workflow("ckp://Process#SelfImprovementCycle:v1.3.18")?;
    ///
    /// if !validation.is_valid {
    ///     for error in validation.errors {
    ///         eprintln!("ERROR: {}", error);
    ///     }
    /// }
    ///
    /// for cycle in validation.cycles {
    ///     if !cycle.is_intentional {
    ///         eprintln!("WARNING: Problematic cycle detected: {:?}", cycle.kernels);
    ///     }
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn validate_workflow(&self, workflow_urn: &str) -> Result<WorkflowValidation, OntologyError> {
        validator::validate_workflow_structure(&self.library, workflow_urn)
    }

    /// Query all workflows stored in System.Workflow
    ///
    /// Returns list of all workflow instances with their current status.
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::workflow::WorkflowAPI;
    /// # use ckp_core::ontology::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # let library = OntologyLibrary::new(PathBuf::from("."))?;
    /// # let workflow_api = WorkflowAPI::new(library);
    /// let workflows = workflow_api.query_all_workflows()?;
    ///
    /// for workflow in workflows {
    ///     println!("{}: {} ({})", workflow.workflow_urn, workflow.label, workflow.status);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    /// Query workflow phases from RDF store
    pub fn query_workflow_phases(&self, workflow_urn: &str) -> Result<Vec<WorkflowPhase>, OntologyError> {
        // Extract workflow name for matching phase URNs (strip brackets first!)
        let workflow_name = workflow_urn
            .trim_start_matches("<")
            .trim_end_matches(">")
            .trim_start_matches("ckp://")
            .replace("#", "-")
            .replace(":", "-")
            .replace("/", "-");
        let phase_pattern = format!("ckp://WorkflowPhase-{}-", workflow_name);

        let query = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?phase ?idx ?kernel ?status
WHERE {{
    ?phase rdf:type ckpw:WorkflowPhase .
    OPTIONAL {{ ?phase ckpw:phaseIndex ?idx }}
    OPTIONAL {{ ?phase ckpw:phaseKernelUrn ?kernel }}
    OPTIONAL {{ ?phase ckpw:phaseStatus ?status }}
    FILTER (STRSTARTS(STR(?phase), "{}"))
}}
ORDER BY ?idx
"#, phase_pattern);

        let results = self.library.query_sparql(&query)?;

        Ok(results.iter().map(|row| {
            let kernel_urn = row.get("kernel").cloned().unwrap_or_default();
            WorkflowPhase {
                phase_name: kernel_urn.split(':').next().unwrap_or("").to_string(),
                kernel_urn,
                status: Self::parse_phase_status(row.get("status")),
                started_at: None,
                completed_at: None,
            }
        }).collect())
    }

    pub fn query_all_workflows(&self) -> Result<Vec<Workflow>, OntologyError> {
        let query = r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?workflow ?label ?description ?status
WHERE {
    ?workflow rdf:type ckpw:Workflow ;
              ckpw:workflowLabel ?label ;
              ckpw:workflowDescription ?description .
    OPTIONAL { ?workflow ckpw:workflowStatus ?status }
}
ORDER BY ?label
"#;

        let results = self.library.query_sparql(query)?;

        results.iter().map(|row| {
            let workflow_urn = row.get("workflow").cloned().unwrap_or_default();
            let phases = self.query_workflow_phases(&workflow_urn).unwrap_or_default();

            Ok(Workflow {
                workflow_urn,
                label: row.get("label").cloned().unwrap_or_default(),
                description: row.get("description").cloned().unwrap_or_default(),
                version: "1.0".to_string(),
                trigger: WorkflowTrigger::OnActionRequest,
                phases,
                edges: Vec::new(),
                status: Self::parse_workflow_status(row.get("status")),
            })
        }).collect()
    }

    fn parse_phase_status(status_str: Option<&String>) -> PhaseStatus {
        match status_str.map(|s| s.as_str()) {
            Some("Pending") => PhaseStatus::Pending,
            Some("InProgress") => PhaseStatus::InProgress,
            Some("Completed") => PhaseStatus::Completed,
            Some("Failed") => PhaseStatus::Failed,
            _ => PhaseStatus::Pending,
        }
    }

    /// Query workflow edges for specific workflow
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::workflow::WorkflowAPI;
    /// # use ckp_core::ontology::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # let library = OntologyLibrary::new(PathBuf::from("."))?;
    /// # let workflow_api = WorkflowAPI::new(library);
    /// let edges = workflow_api.query_workflow_edges("ckp://Process#SelfImprovementCycle:v1.3.18")?;
    ///
    /// for edge in edges {
    ///     println!("{} --[{}]--> {}", edge.source, edge.predicate, edge.target);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn query_workflow_edges(&self, workflow_urn: &str) -> Result<Vec<WorkflowEdge>, OntologyError> {
        let query = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?edge ?source ?target ?predicate ?trigger
WHERE {{
    <{}> ckpw:hasEdge ?edge .

    ?edge rdf:type ckpw:WorkflowEdge ;
          ckpw:edgeSource ?source ;
          ckpw:edgeTarget ?target ;
          ckpw:edgePredicate ?predicate .

    OPTIONAL {{ ?edge ckpw:edgeTrigger ?trigger }}
}}
ORDER BY ?source
"#, workflow_urn);

        let results = self.library.query_sparql(&query)?;

        Ok(results.iter().map(|row| {
            WorkflowEdge {
                edge_urn: row.get("edge").cloned().unwrap_or_default(),
                source: row.get("source").cloned().unwrap_or_default(),
                target: row.get("target").cloned().unwrap_or_default(),
                predicate: row.get("predicate").cloned().unwrap_or_default(),
                trigger: row.get("trigger").cloned().unwrap_or_default(),
                action: None,
            }
        }).collect())
    }

    /// Detect cycles in workflow using SPARQL
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::workflow::WorkflowAPI;
    /// # use ckp_core::ontology::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # let library = OntologyLibrary::new(PathBuf::from("."))?;
    /// # let workflow_api = WorkflowAPI::new(library);
    /// let cycles = workflow_api.detect_cycles("ckp://Process#SelfImprovementCycle:v1.3.18")?;
    ///
    /// for cycle in cycles {
    ///     if cycle.is_intentional {
    ///         println!("✓ Intentional loop: {:?}", cycle.kernels);
    ///     } else {
    ///         println!("⚠ Problematic cycle: {:?}", cycle.kernels);
    ///     }
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn detect_cycles(&self, workflow_urn: &str) -> Result<Vec<WorkflowCycle>, OntologyError> {
        validator::detect_cycles_via_sparql(&self.library, workflow_urn)
    }

    /// Update workflow phase status and publish event
    ///
    /// Updates the phase status in the RDF store and publishes a lifecycle event to NATS
    /// if event publishing is enabled.
    ///
    /// # Example
    /// ```no_run
    /// # use ckp_core::workflow::{WorkflowAPI, PhaseStatus};
    /// # use ckp_core::ontology::OntologyLibrary;
    /// # use std::path::PathBuf;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let library = OntologyLibrary::new(PathBuf::from("."))?;
    /// let workflow_api = WorkflowAPI::new_with_events(
    ///     library,
    ///     Some("nats://localhost:4222")
    /// ).await?;
    ///
    /// workflow_api.update_phase_status(
    ///     "ckp://Process#SelfImprovementCycle:v1.3.18",
    ///     "Validation",
    ///     PhaseStatus::InProgress,
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_phase_status(
        &self,
        workflow_urn: &str,
        phase_name: &str,
        new_status: PhaseStatus,
    ) -> Result<(), OntologyError> {
        // 1. Get kernel URN for this phase
        let kernel_urn = self.get_phase_kernel_urn(workflow_urn, phase_name)?;

        // 2. Update RDF triple store
        let update_query = self.build_phase_status_update(workflow_urn, phase_name, &new_status);
        self.library.execute_sparql_update(&update_query)?;

        // 3. Publish event to NATS
        if let Some(publisher) = &self.event_publisher {
            let (event_type, progress) = match new_status {
                PhaseStatus::Pending => (KernelEventType::PhasePending, 0),
                PhaseStatus::InProgress => (KernelEventType::PhaseInProgress, 50),
                PhaseStatus::Completed => (KernelEventType::PhaseCompleted, 100),
                PhaseStatus::Failed => (KernelEventType::PhaseFailed, 100),
                PhaseStatus::Skipped => (KernelEventType::PhaseSkipped, 100),
            };

            publisher.publish_phase_event(
                event_type,
                workflow_urn,
                phase_name,
                &kernel_urn,
                progress,
            ).await;
        }

        Ok(())
    }

    // Helper methods

    /// Get kernel URN for a specific workflow phase
    fn get_phase_kernel_urn(
        &self,
        workflow_urn: &str,
        phase_name: &str,
    ) -> Result<String, OntologyError> {
        let query = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>

SELECT ?kernel
WHERE {{
    <{workflow_urn}> ckpw:hasPhase ?phase .
    ?phase ckpw:phaseName "{phase_name}" ;
           ckpw:kernelUrn ?kernel .
}}
LIMIT 1
"#);

        let results = self.library.query_sparql(&query)?;
        results.get(0)
            .and_then(|row| row.get("kernel").cloned())
            .ok_or_else(|| OntologyError::QueryError(
                format!("Phase not found: {}", phase_name)
            ))
    }

    /// Build SPARQL UPDATE query for phase status change
    fn build_phase_status_update(
        &self,
        workflow_urn: &str,
        phase_name: &str,
        status: &PhaseStatus,
    ) -> String {
        let status_str = match status {
            PhaseStatus::Pending => "PENDING",
            PhaseStatus::InProgress => "IN_PROGRESS",
            PhaseStatus::Completed => "COMPLETED",
            PhaseStatus::Failed => "FAILED",
            PhaseStatus::Skipped => "SKIPPED",
        };

        format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>

DELETE {{
    ?phase ckpw:phaseStatus ?oldStatus .
}}
INSERT {{
    ?phase ckpw:phaseStatus "{status_str}" .
    ?phase ckpw:updatedAt "{timestamp}" .
}}
WHERE {{
    <{workflow_urn}> ckpw:hasPhase ?phase .
    ?phase ckpw:phaseName "{phase_name}" .
    OPTIONAL {{ ?phase ckpw:phaseStatus ?oldStatus }}
}}
"#,
            status_str = status_str,
            timestamp = chrono::Utc::now().to_rfc3339(),
            workflow_urn = workflow_urn,
            phase_name = phase_name
        )
    }

    fn parse_workflow_status(s: Option<&String>) -> WorkflowStatus {
        match s.map(|s| s.as_str()) {
            Some("IN_PROGRESS") => WorkflowStatus::InProgress,
            Some("COMPLETED") => WorkflowStatus::Completed,
            Some("FAILED") => WorkflowStatus::Failed,
            Some("BLOCKED") => WorkflowStatus::Blocked,
            _ => WorkflowStatus::Pending,
        }
    }
}
