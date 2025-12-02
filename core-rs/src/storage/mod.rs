// storage/mod.rs - Storage subsystem

pub mod scanner;

pub use scanner::{InstanceScanner, InstanceSummary, InstanceDetail};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Test: InstanceScanner export is accessible
    ///
    /// Verifies that InstanceScanner type is exported and can be constructed
    /// for scanning storage directories and collecting CKI receipts.
    #[test]
    fn test_instance_scanner_export() {
        // Verify InstanceScanner type is accessible
        fn accepts_scanner(_: InstanceScanner) {}

        let scanner = InstanceScanner::new(
            PathBuf::from("/tmp/test"),
            "Test.Kernel".to_string()
        );

        accepts_scanner(scanner);

        // If this compiles, export is correct
    }

    /// Test: InstanceSummary export is accessible
    ///
    /// Verifies that InstanceSummary struct is exported and can be used
    /// to represent summarized instance data.
    #[test]
    fn test_instance_summary_export() {
        use chrono::Utc;

        // Verify InstanceSummary type is accessible
        fn accepts_summary(_: InstanceSummary) {}

        let summary = InstanceSummary {
            id: "tx-123".to_string(),
            name: "test-instance".to_string(),
            kernel: "Test.Kernel".to_string(),
            timestamp: Utc::now(),
        };

        accepts_summary(summary);

        // If this compiles, export is correct
    }

    /// Test: InstanceDetail export is accessible
    ///
    /// Verifies that InstanceDetail struct is exported and can be used
    /// to represent detailed instance data with full receipt contents.
    #[test]
    fn test_instance_detail_export() {
        use chrono::Utc;
        use serde_json::json;

        // Verify InstanceDetail type is accessible
        fn accepts_detail(_: InstanceDetail) {}

        let detail = InstanceDetail {
            id: "tx-123".to_string(),
            name: "test-instance".to_string(),
            kernel: "Test.Kernel".to_string(),
            timestamp: Utc::now(),
            action: Some("test_action".to_string()),
            success: Some(true),
            data: json!({
                "id": "tx-123",
                "kernel": "Test.Kernel",
                "inputs": [],
                "outputs": []
            }),
        };

        accepts_detail(detail);

        // If this compiles, export is correct
    }

    /// Test: All storage scanner functions are accessible
    ///
    /// Verifies that InstanceScanner provides all expected public methods
    /// for listing, counting, and describing instances.
    #[test]
    fn test_scanner_methods_accessible() {
        let scanner = InstanceScanner::new(
            PathBuf::from("/tmp/test"),
            "Test.Kernel".to_string()
        );

        // Verify method signatures exist and are accessible
        fn accepts_list(_: fn(&InstanceScanner, usize) -> crate::errors::Result<Vec<InstanceSummary>>) {}
        fn accepts_count(_: fn(&InstanceScanner) -> crate::errors::Result<usize>) {}
        fn accepts_describe(_: fn(&InstanceScanner, &str) -> crate::errors::Result<InstanceDetail>) {}

        accepts_list(InstanceScanner::list_instances);
        accepts_count(InstanceScanner::count_instances);
        accepts_describe(InstanceScanner::describe_instance);

        // If this compiles, all methods are properly exported
    }
}
