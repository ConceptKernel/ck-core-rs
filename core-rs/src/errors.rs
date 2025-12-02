//! Error types for CKP Core

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CkpError {
    #[error("URN parsing error: {0}")]
    UrnParse(String),

    #[error("URN validation error: {0}")]
    UrnValidation(String),

    #[error("Invalid URN format: {0}")]
    InvalidUrnFormat(String),

    #[error("Invalid stage: {0}")]
    InvalidStage(String),

    #[error("Invalid kernel name: {0}")]
    InvalidKernelName(String),

    #[error("Invalid version: {0}")]
    InvalidVersion(String),

    #[error("Invalid predicate: {0}")]
    InvalidPredicate(String),

    #[error("Invalid edge URN format: {0}")]
    InvalidEdgeUrn(String),

    #[error("Invalid agent URN format: {0}")]
    InvalidAgentUrn(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path error: {0}")]
    Path(String),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Ontology error: {0}")]
    Ontology(String),

    #[error("RBAC error: {0}")]
    Rbac(String),

    #[error("Edge routing error: {0}")]
    EdgeRouting(String),

    #[error("Edge already exists: {0}")]
    EdgeAlreadyExists(String),

    #[error("Process error: {0}")]
    Process(String),

    #[error("Process error: {0}")]
    ProcessError(String),

    #[error("Governor error: {0}")]
    Governor(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Project error: {0}")]
    ProjectError(String),

    #[error("Port error: {0}")]
    PortError(String),

    #[error("Port unavailable: {0}")]
    PortUnavailable(String),

    #[error("Project already registered: {0}")]
    ProjectAlreadyRegistered(String),

    #[error("Project not found")]
    ProjectNotFound,

    #[error("Kernel not found: {0}")]
    KernelNotFound(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    #[error("Regex error: {0}")]
    RegexError(String),

    #[error("Build error: {0}")]
    BuildError(String),
}

impl From<regex::Error> for CkpError {
    fn from(err: regex::Error) -> Self {
        CkpError::RegexError(err.to_string())
    }
}

impl From<crate::ontology::library::OntologyError> for CkpError {
    fn from(err: crate::ontology::library::OntologyError) -> Self {
        CkpError::Ontology(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CkpError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urn_parse_error_display() {
        let err = CkpError::UrnParse("invalid URN".to_string());
        let display = format!("{}", err);
        assert!(display.contains("URN parsing error"));
        assert!(display.contains("invalid URN"));
    }

    #[test]
    fn test_urn_validation_error_display() {
        let err = CkpError::UrnValidation("missing kernel name".to_string());
        let display = format!("{}", err);
        assert!(display.contains("URN validation error"));
        assert!(display.contains("missing kernel name"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: CkpError = io_err.into();

        match err {
            CkpError::Io(_) => {} // Success
            _ => panic!("Expected Io variant"),
        }
    }

    #[test]
    fn test_yaml_error_conversion() {
        // Create invalid YAML and parse
        let yaml = "invalid: yaml: content:";
        let result: std::result::Result<serde_json::Value, serde_yaml::Error> = serde_yaml::from_str(yaml);
        let yaml_err = result.unwrap_err();

        let err: CkpError = yaml_err.into();
        match err {
            CkpError::Yaml(_) => {} // Success
            _ => panic!("Expected Yaml variant"),
        }
    }

    #[test]
    fn test_json_error_conversion() {
        // Create invalid JSON and parse
        let json = "{invalid json}";
        let result: std::result::Result<serde_json::Value, serde_json::Error> = serde_json::from_str(json);
        let json_err = result.unwrap_err();

        let err: CkpError = json_err.into();
        match err {
            CkpError::Json(_) => {} // Success
            _ => panic!("Expected Json variant"),
        }
    }

    #[test]
    fn test_regex_error_conversion() {
        // Create invalid regex pattern
        let result = regex::Regex::new("[invalid");
        let regex_err = result.unwrap_err();

        let err: CkpError = regex_err.into();
        match err {
            CkpError::RegexError(_) => {} // Success
            _ => panic!("Expected RegexError variant"),
        }
    }

    #[test]
    fn test_file_not_found_error_display() {
        let err = CkpError::FileNotFound("config.yaml".to_string());
        let display = format!("{}", err);
        assert!(display.contains("File not found"));
        assert!(display.contains("config.yaml"));
    }

    #[test]
    fn test_kernel_not_found_error_display() {
        let err = CkpError::KernelNotFound("System.Missing".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Kernel not found"));
        assert!(display.contains("System.Missing"));
    }

    #[test]
    fn test_project_not_found_error_display() {
        let err = CkpError::ProjectNotFound;
        let display = format!("{}", err);
        assert_eq!(display, "Project not found");
    }

    #[test]
    fn test_error_debug_format() {
        let err = CkpError::PortError("port 8080 in use".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("PortError"));
        assert!(debug.contains("port 8080 in use"));
    }

    #[test]
    fn test_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<CkpError>();
    }

    #[test]
    fn test_error_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<CkpError>();
    }

    #[test]
    fn test_result_type_alias() {
        // Verify Result<T> type alias works correctly
        let ok_result: Result<String> = Ok("success".to_string());
        assert!(ok_result.is_ok());
        assert_eq!(ok_result.unwrap(), "success");

        let err_result: Result<String> = Err(CkpError::FileNotFound("test".to_string()));
        assert!(err_result.is_err());
    }

    #[test]
    fn test_multiple_error_variants_have_unique_messages() {
        let errors = vec![
            CkpError::UrnParse("urn_parse".to_string()),
            CkpError::UrnValidation("urn_validation".to_string()),
            CkpError::InvalidUrnFormat("invalid_format".to_string()),
            CkpError::FileNotFound("not_found".to_string()),
            CkpError::KernelNotFound("kernel_not_found".to_string()),
        ];

        // Each error should have distinct message
        let messages: Vec<String> = errors.iter().map(|e| format!("{}", e)).collect();

        assert!(messages[0].contains("URN parsing error"));
        assert!(messages[1].contains("URN validation error"));
        assert!(messages[2].contains("Invalid URN format"));
        assert!(messages[3].contains("File not found"));
        assert!(messages[4].contains("Kernel not found"));
    }

    #[test]
    fn test_edge_routing_errors() {
        let err = CkpError::EdgeRouting("invalid route".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Edge routing error"));
        assert!(display.contains("invalid route"));

        let err2 = CkpError::EdgeAlreadyExists("ckp://A->B".to_string());
        let display2 = format!("{}", err2);
        assert!(display2.contains("Edge already exists"));
        assert!(display2.contains("ckp://A->B"));
    }

    #[test]
    fn test_validation_and_parse_errors() {
        let val_err = CkpError::ValidationError("invalid data".to_string());
        let parse_err = CkpError::ParseError("parse failed".to_string());

        assert!(format!("{}", val_err).contains("Validation error"));
        assert!(format!("{}", parse_err).contains("Parse error"));
    }

    #[test]
    fn test_rbac_and_governor_errors() {
        let rbac_err = CkpError::Rbac("permission denied".to_string());
        let gov_err = CkpError::Governor("queue error".to_string());

        assert!(format!("{}", rbac_err).contains("RBAC error"));
        assert!(format!("{}", gov_err).contains("Governor error"));
    }
}
