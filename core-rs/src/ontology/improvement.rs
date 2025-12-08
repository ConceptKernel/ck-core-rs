/// Self-Improvement API for ConceptKernel v1.3.18
///
/// Provides unified API for:
/// - Validating kernel ontologies
/// - Generating improvement recommendations
/// - Querying validation issues
/// - Submitting recommendations to consensus
/// - Triggering improvement processes via actions

use crate::ontology::{OntologyLibrary, OntologyError};
use serde::{Deserialize, Serialize};

/// Validation issue detected during ontology check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub kernel_name: String,
    pub severity: IssueSeverity,
    pub issue_type: IssueType,
    pub description: String,
    pub location: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueSeverity {
    HIGH,
    MEDIUM,
    LOW,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IssueType {
    MissingImport,
    MissingBfoAlignment,
    MissingCkpNamespace,
    InvalidStructure,
}

/// Improvement recommendation generated from validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementRecommendation {
    pub recommendation_id: String,
    pub kernel_name: String,
    pub priority: Priority,
    pub action_type: ActionType,
    pub description: String,
    pub estimated_impact: String,
    pub consensus_status: ConsensusStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Priority {
    CRITICAL,
    HIGH,
    MEDIUM,
    LOW,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    AddImport,
    AddBfoType,
    AddRole,
    AddFunction,
    Refactor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsensusStatus {
    Pending,
    Approved,
    Rejected,
}

/// Improvement process status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementProcess {
    pub process_urn: String,
    pub kernel_name: String,
    pub phase: ProcessPhase,
    pub issues: Vec<ValidationIssue>,
    pub recommendations: Vec<ImprovementRecommendation>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProcessPhase {
    Validation,
    Analysis,
    Recommendation,
    Consensus,
    Execution,
    Completed,
}

/// Unified improvement API
pub struct ImprovementAPI {
    library: OntologyLibrary,
}

impl ImprovementAPI {
    /// Create new improvement API with ontology library
    pub fn new(library: OntologyLibrary) -> Self {
        Self { library }
    }

    /// Query all validation issues via SPARQL
    pub fn query_all_issues(&self) -> Result<Vec<ValidationIssue>, OntologyError> {
        let query = r#"
PREFIX ckpi: <https://conceptkernel.org/ontology/improvement#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?issue ?kernel ?severity ?issueType ?description
WHERE {
    ?issue rdf:type ckpi:ValidationIssue ;
           ckpi:affectedKernel ?kernel ;
           ckpi:severity ?severity ;
           rdfs:label ?issueType ;
           rdfs:comment ?description .
}
ORDER BY DESC(?severity)
"#;

        let results = self.library.query_sparql(query)?;

        Ok(results.iter().map(|row| {
            ValidationIssue {
                kernel_name: row.get("kernel").cloned().unwrap_or_default(),
                severity: Self::parse_severity(row.get("severity").unwrap_or(&String::from("LOW"))),
                issue_type: Self::parse_issue_type(row.get("issueType").unwrap_or(&String::from("Unknown"))),
                description: row.get("description").cloned().unwrap_or_default(),
                location: String::new(),
            }
        }).collect())
    }

    /// Query all improvement recommendations via SPARQL
    pub fn query_all_recommendations(&self) -> Result<Vec<ImprovementRecommendation>, OntologyError> {
        let query = r#"
PREFIX ckpi: <https://conceptkernel.org/ontology/improvement#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?recommendation ?kernel ?priority ?actionType ?description ?impact ?status
WHERE {
    ?recommendation rdf:type ckpi:ImprovementRecommendation ;
                    ckpi:affectedKernel ?kernel ;
                    ckpi:priority ?priority ;
                    ckpi:actionType ?actionType ;
                    rdfs:comment ?description ;
                    ckpi:estimatedImpact ?impact .
    OPTIONAL { ?recommendation ckpi:consensusStatus ?status }
}
ORDER BY DESC(?priority)
"#;

        let results = self.library.query_sparql(query)?;

        Ok(results.iter().map(|row| {
            ImprovementRecommendation {
                recommendation_id: row.get("recommendation").cloned().unwrap_or_default(),
                kernel_name: row.get("kernel").cloned().unwrap_or_default(),
                priority: Self::parse_priority(row.get("priority").unwrap_or(&String::from("LOW"))),
                action_type: Self::parse_action_type(row.get("actionType").unwrap_or(&String::from("Unknown"))),
                description: row.get("description").cloned().unwrap_or_default(),
                estimated_impact: row.get("impact").cloned().unwrap_or_default(),
                consensus_status: Self::parse_consensus_status(row.get("status")),
                created_at: chrono::Utc::now().to_rfc3339(),
            }
        }).collect())
    }

    /// Query recommendations by status
    pub fn query_recommendations_by_status(&self, status: ConsensusStatus) -> Result<Vec<ImprovementRecommendation>, OntologyError> {
        let status_str = match status {
            ConsensusStatus::Pending => "PENDING",
            ConsensusStatus::Approved => "APPROVED",
            ConsensusStatus::Rejected => "REJECTED",
        };

        let query = format!(r#"
PREFIX ckpi: <https://conceptkernel.org/ontology/improvement#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?recommendation ?kernel ?priority ?actionType ?description ?impact
WHERE {{
    ?recommendation rdf:type ckpi:ImprovementRecommendation ;
                    ckpi:affectedKernel ?kernel ;
                    ckpi:priority ?priority ;
                    ckpi:actionType ?actionType ;
                    rdfs:comment ?description ;
                    ckpi:estimatedImpact ?impact ;
                    ckpi:consensusStatus "{}" .
}}
ORDER BY DESC(?priority)
"#, status_str);

        let results = self.library.query_sparql(&query)?;

        Ok(results.iter().map(|row| {
            ImprovementRecommendation {
                recommendation_id: row.get("recommendation").cloned().unwrap_or_default(),
                kernel_name: row.get("kernel").cloned().unwrap_or_default(),
                priority: Self::parse_priority(row.get("priority").unwrap_or(&String::from("LOW"))),
                action_type: Self::parse_action_type(row.get("actionType").unwrap_or(&String::from("Unknown"))),
                description: row.get("description").cloned().unwrap_or_default(),
                estimated_impact: row.get("impact").cloned().unwrap_or_default(),
                consensus_status: status.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
            }
        }).collect())
    }

    /// Query improvement processes
    pub fn query_improvement_processes(&self) -> Result<Vec<ImprovementProcess>, OntologyError> {
        let query = r#"
PREFIX ckpi: <https://conceptkernel.org/ontology/improvement#>
PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?process ?kernel ?processUrn
WHERE {
    ?process rdf:type ckpi:ImprovementProcess ;
             ckp:hasProcessUrn ?processUrn .
    OPTIONAL { ?process ckp:affectsKernel ?kernel }
}
"#;

        let results = self.library.query_sparql(query)?;

        Ok(results.iter().map(|row| {
            ImprovementProcess {
                process_urn: row.get("processUrn").cloned().unwrap_or_default(),
                kernel_name: row.get("kernel").cloned().unwrap_or_default(),
                phase: ProcessPhase::Validation,
                issues: Vec::new(),
                recommendations: Vec::new(),
                started_at: chrono::Utc::now().to_rfc3339(),
                completed_at: None,
            }
        }).collect())
    }

    // Helper parsers
    fn parse_severity(s: &str) -> IssueSeverity {
        match s.to_uppercase().as_str() {
            "HIGH" => IssueSeverity::HIGH,
            "MEDIUM" => IssueSeverity::MEDIUM,
            _ => IssueSeverity::LOW,
        }
    }

    fn parse_issue_type(s: &str) -> IssueType {
        if s.contains("Import") { IssueType::MissingImport }
        else if s.contains("BFO") { IssueType::MissingBfoAlignment }
        else if s.contains("namespace") { IssueType::MissingCkpNamespace }
        else { IssueType::InvalidStructure }
    }

    fn parse_priority(s: &str) -> Priority {
        match s.to_uppercase().as_str() {
            "CRITICAL" => Priority::CRITICAL,
            "HIGH" => Priority::HIGH,
            "MEDIUM" => Priority::MEDIUM,
            _ => Priority::LOW,
        }
    }

    fn parse_action_type(s: &str) -> ActionType {
        if s.contains("IMPORT") { ActionType::AddImport }
        else if s.contains("BFO") { ActionType::AddBfoType }
        else if s.contains("ROLE") { ActionType::AddRole }
        else if s.contains("FUNCTION") { ActionType::AddFunction }
        else { ActionType::Refactor }
    }

    fn parse_consensus_status(s: Option<&String>) -> ConsensusStatus {
        match s.map(|s| s.as_str()) {
            Some("APPROVED") => ConsensusStatus::Approved,
            Some("REJECTED") => ConsensusStatus::Rejected,
            _ => ConsensusStatus::Pending,
        }
    }
}

/// Action request payload for triggering improvement processes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerImprovementAction {
    pub action: String,  // "trigger-improvement"
    pub kernel_name: Option<String>,  // None = all kernels
    pub include_ai_analysis: bool,
    pub submit_to_consensus: bool,
}

/// Response from improvement action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementActionResponse {
    pub process_urn: String,
    pub issues_detected: usize,
    pub recommendations_generated: usize,
    pub submitted_to_consensus: bool,
    pub consensus_proposals: Vec<String>,
}
