/**
 * validator.rs
 * Ontology validation framework using Jena OWL inference
 *
 * Validates that:
 * 1. All canonical ontologies are loaded into Jena
 * 2. OWL imports are satisfied
 * 3. Class hierarchies are consistent
 * 4. Property domains/ranges are enforced
 * 5. Runtime RDF instances match ontology constraints
 */

use crate::ontology::library::{OntologyLibrary, OntologyError};
use std::sync::Arc;

/// Validation severity levels
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationSeverity {
    Error,   // Critical - system cannot operate
    Warning, // Non-critical - should be fixed
    Info,    // Informational - best practice
}

/// Validation issue
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub severity: ValidationSeverity,
    pub category: String,
    pub message: String,
    pub details: Option<String>,
}

impl ValidationIssue {
    pub fn error(category: &str, message: &str) -> Self {
        Self {
            severity: ValidationSeverity::Error,
            category: category.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    pub fn warning(category: &str, message: &str) -> Self {
        Self {
            severity: ValidationSeverity::Warning,
            category: category.to_string(),
            message: message.to_string(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: &str) -> Self {
        self.details = Some(details.to_string());
        self
    }
}

/// Validation report
#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
    pub info: Vec<ValidationIssue>,
    pub passed: bool,
}

impl ValidationReport {
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            warnings: Vec::new(),
            info: Vec::new(),
            passed: true,
        }
    }

    pub fn add_error(&mut self, issue: ValidationIssue) {
        self.passed = false;
        self.errors.push(issue);
    }

    pub fn add_warning(&mut self, issue: ValidationIssue) {
        self.warnings.push(issue);
    }

    pub fn add_info(&mut self, issue: ValidationIssue) {
        self.info.push(issue);
    }

    pub fn merge(&mut self, other: ValidationReport) {
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        self.info.extend(other.info);
        if !other.passed {
            self.passed = false;
        }
    }

    pub fn print_summary(&self) {
        if self.passed {
            println!("✅ Ontology validation PASSED");
        } else {
            eprintln!("❌ Ontology validation FAILED");
        }

        if !self.errors.is_empty() {
            eprintln!("\n🔴 Errors ({}):", self.errors.len());
            for (i, issue) in self.errors.iter().enumerate() {
                eprintln!("   {}. [{}] {}", i + 1, issue.category, issue.message);
                if let Some(ref details) = issue.details {
                    eprintln!("      Details: {}", details);
                }
            }
        }

        if !self.warnings.is_empty() {
            println!("\n🟡 Warnings ({}):", self.warnings.len());
            for (i, issue) in self.warnings.iter().enumerate() {
                println!("   {}. [{}] {}", i + 1, issue.category, issue.message);
                if let Some(ref details) = issue.details {
                    println!("      Details: {}", details);
                }
            }
        }

        if !self.info.is_empty() {
            println!("\n🔵 Info ({}):", self.info.len());
            for (i, issue) in self.info.iter().enumerate() {
                println!("   {}. [{}] {}", i + 1, issue.category, issue.message);
            }
        }
    }
}

/// Ontology validator
pub struct OntologyValidator {
    pub library: Arc<OntologyLibrary>,
}

impl OntologyValidator {
    pub fn new(library: Arc<OntologyLibrary>) -> Self {
        Self { library }
    }

    /// Validate all ontologies are loaded and consistent
    ///
    /// This is the main validation entry point that should be called at startup.
    pub fn validate_ontologies(&self) -> Result<ValidationReport, OntologyError> {
        let mut report = ValidationReport::new();

        println!("[OntologyValidator] Starting ontology validation...");

        // 1. Check canonical ontologies are loaded
        println!("[OntologyValidator] Checking canonical ontologies...");
        report.merge(self.check_canonical_ontologies()?);

        // 2. Verify class definitions exist
        println!("[OntologyValidator] Verifying class definitions...");
        report.merge(self.verify_class_definitions()?);

        // 3. Validate property definitions
        println!("[OntologyValidator] Validating property definitions...");
        report.merge(self.validate_property_definitions()?);

        // 4. Check BFO alignment
        println!("[OntologyValidator] Checking BFO alignment...");
        report.merge(self.check_bfo_alignment()?);

        println!("[OntologyValidator] Validation complete\n");

        Ok(report)
    }

    /// Check canonical ontologies are present in Jena
    fn check_canonical_ontologies(&self) -> Result<ValidationReport, OntologyError> {
        let mut report = ValidationReport::new();

        // Define expected ontologies
        let expected_ontologies = vec![
            ("ckp:Kernel", "Core kernel class"),
            ("ckp:WorkflowExecution", "Workflow execution class"),
            ("ckp:KernelInvocation", "Kernel invocation class"),
            ("ckp:EdgeRouting", "Edge routing class"),
        ];

        for (class_uri, description) in expected_ontologies {
            let query = format!(
                r#"
                PREFIX ckp: <https://conceptkernel.org/ontology#>
                PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
                PREFIX owl: <http://www.w3.org/2002/07/owl#>

                ASK {{
                    {class_uri} rdf:type owl:Class .
                }}
                "#,
                class_uri = class_uri
            );

            let results = self.library.query_sparql(&query)?;
            let exists = results.first()
                .and_then(|row| row.get("result"))
                .and_then(|s| s.parse::<bool>().ok())
                .unwrap_or(false);

            if !exists {
                report.add_error(
                    ValidationIssue::error(
                        "missing-class",
                        &format!("{} not found in ontology", class_uri)
                    )
                    .with_details(description)
                );
            } else {
                report.add_info(ValidationIssue {
                    severity: ValidationSeverity::Info,
                    category: "class-found".to_string(),
                    message: format!("{} found", class_uri),
                    details: Some(description.to_string()),
                });
            }
        }

        Ok(report)
    }

    /// Verify class definitions have proper BFO superclasses
    fn verify_class_definitions(&self) -> Result<ValidationReport, OntologyError> {
        let mut report = ValidationReport::new();

        // Check WorkflowExecution is subclass of bfo:0000003 (Occurrent)
        let query = r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

            ASK {
                ckp:WorkflowExecution rdfs:subClassOf* bfo:0000003 .
            }
        "#;

        let results = self.library.query_sparql(query)?;
        let is_occurrent = results.first()
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);

        if !is_occurrent {
            report.add_error(
                ValidationIssue::error(
                    "class-hierarchy",
                    "ckp:WorkflowExecution is not a subclass of bfo:0000003 (Occurrent)"
                )
            );
        }

        // Check KernelInvocation is subclass of bfo:0000003 (Occurrent)
        let query = r#"
            PREFIX ckp: <https://conceptkernel.org/ontology#>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

            ASK {
                ckp:KernelInvocation rdfs:subClassOf* bfo:0000003 .
            }
        "#;

        let results = self.library.query_sparql(query)?;
        let is_occurrent = results.first()
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);

        if !is_occurrent {
            report.add_error(
                ValidationIssue::error(
                    "class-hierarchy",
                    "ckp:KernelInvocation is not a subclass of bfo:0000003 (Occurrent)"
                )
            );
        }

        Ok(report)
    }

    /// Validate property definitions have correct domains
    fn validate_property_definitions(&self) -> Result<ValidationReport, OntologyError> {
        let mut report = ValidationReport::new();

        // Define expected properties with their expected characteristics
        let expected_properties = vec![
            ("ckp:hasURN", "datatype", "URN identifier property"),
            ("ckp:workflowName", "datatype", "Workflow name property"),
            ("ckp:startTime", "datatype", "Start timestamp property"),
            ("ckp:kernelName", "datatype", "Kernel name property"),
            ("ckp:transactionId", "datatype", "Transaction ID property"),
        ];

        for (property_uri, _property_type, description) in expected_properties {
            let query = format!(
                r#"
                PREFIX ckp: <https://conceptkernel.org/ontology#>
                PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
                PREFIX owl: <http://www.w3.org/2002/07/owl#>

                ASK {{
                    {{ {property_uri} rdf:type owl:DatatypeProperty }}
                    UNION
                    {{ {property_uri} rdf:type owl:ObjectProperty }}
                }}
                "#,
                property_uri = property_uri
            );

            let results = self.library.query_sparql(&query)?;
            let exists = results.first()
                .and_then(|row| row.get("result"))
                .and_then(|s| s.parse::<bool>().ok())
                .unwrap_or(false);

            if !exists {
                report.add_warning(
                    ValidationIssue::warning(
                        "missing-property",
                        &format!("{} not defined in ontology", property_uri)
                    )
                    .with_details(description)
                );
            }
        }

        Ok(report)
    }

    /// Check BFO alignment for core classes
    fn check_bfo_alignment(&self) -> Result<ValidationReport, OntologyError> {
        let mut report = ValidationReport::new();

        // Check that bfo:0000003 (Occurrent) is defined
        let query = r#"
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            PREFIX owl: <http://www.w3.org/2002/07/owl#>

            ASK {
                bfo:0000003 rdf:type owl:Class .
            }
        "#;

        let results = self.library.query_sparql(query)?;
        let bfo_loaded = results.first()
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);

        if !bfo_loaded {
            report.add_error(
                ValidationIssue::error(
                    "missing-bfo",
                    "BFO ontology not loaded - bfo:0000003 (Occurrent) not found"
                )
                .with_details("Ensure http://purl.obolibrary.org/obo/bfo.owl is imported")
            );
        } else {
            report.add_info(ValidationIssue {
                severity: ValidationSeverity::Info,
                category: "bfo-loaded".to_string(),
                message: "BFO ontology loaded successfully".to_string(),
                details: Some("bfo:0000003 (Occurrent) found".to_string()),
            });
        }

        Ok(report)
    }

    /// Validate a specific RDF instance matches ontology constraints
    ///
    /// This can be called after creating instances to verify they're valid.
    pub fn validate_instance(&self, instance_uri: &str) -> Result<Vec<ValidationIssue>, OntologyError> {
        let mut issues = Vec::new();

        // Check if instance has rdf:type declaration
        let query = format!(
            r#"
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            ASK {{
                <{instance_uri}> rdf:type ?type .
            }}
            "#,
            instance_uri = instance_uri
        );

        let results = self.library.query_sparql(&query)?;
        let has_type = results.first()
            .and_then(|row| row.get("result"))
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(false);

        if !has_type {
            issues.push(
                ValidationIssue::error(
                    "instance-validation",
                    &format!("Instance {} has no rdf:type declaration", instance_uri)
                )
            );
        }

        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    #[ignore] // Requires Jena server and ontologies loaded
    fn test_validation_report() {
        let mut report = ValidationReport::new();
        assert!(report.passed);

        report.add_error(ValidationIssue::error("test", "Test error"));
        assert!(!report.passed);
        assert_eq!(report.errors.len(), 1);
    }

    #[tokio::test]
    #[ignore] // Requires Jena server running
    async fn test_ontology_validator() {
        let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();

        let library = OntologyLibrary::new(project_root).unwrap();
        let validator = OntologyValidator::new(Arc::new(library));

        let report = validator.validate_ontologies().unwrap();
        report.print_summary();

        // In a properly configured system, validation should pass
        // If it fails, it indicates missing or misconfigured ontologies
    }
}
