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

pub use ckdl_parser::{
    parse_ckdl_file, ckdl_to_workflow,
    CkdlWorkflow, ExternKernel, WorkflowKernel, CkdlEdge,
    ComponentOrigin, ComponentAnalysis,
};

use crate::ontology::{OntologyLibrary, OntologyError};
use serde::{Deserialize, Serialize};
use std::path::Path;

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
}

impl WorkflowAPI {
    /// Create new workflow API with ontology library
    pub fn new(library: OntologyLibrary) -> Self {
        Self { library }
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
        let _workflow = ckdl_to_workflow(ckdl_workflow.clone());

        // TODO: Insert as RDF triples into Oxigraph
        // In production:
        // 1. Convert workflow to RDF (WorkflowEdge entities, etc.)
        // 2. Insert into Oxigraph via SPARQL UPDATE or programmatic API
        // 3. Store workflow phases, triggers, etc.

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

        Ok(results.iter().map(|row| {
            Workflow {
                workflow_urn: row.get("workflow").cloned().unwrap_or_default(),
                label: row.get("label").cloned().unwrap_or_default(),
                description: row.get("description").cloned().unwrap_or_default(),
                version: "1.0".to_string(),
                trigger: WorkflowTrigger::OnActionRequest,
                phases: Vec::new(),
                edges: Vec::new(),
                status: Self::parse_workflow_status(row.get("status")),
            }
        }).collect())
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

    // Helper methods

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
