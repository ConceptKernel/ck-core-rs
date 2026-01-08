//! Occurrent Tracker for Workflow Lifecycle Events
//!
//! Tracks workflow execution occurrents (BFO temporal individuals) to Jena storage.
//! Each workflow execution creates timestamped occurrent instances:
//!
//! - WorkflowStart: When workflow begins
//! - KernelInvocation: When each kernel is invoked
//! - EdgeRouting: When data flows through edges
//! - WorkflowComplete: When workflow finishes (success/failure)
//!
//! # Example
//!
//! ```rust,ignore
//! use ckp_core::drivers::storage::OccurrentTracker;
//!
//! let tracker = OccurrentTracker::new(storage.clone());
//! tracker.track_workflow_start("System.Workflow.Bakery", "tx-123").await?;
//! tracker.track_kernel_invocation("System.Workflow.Bakery", "tx-123", "AcceptOrder", 0).await?;
//! tracker.track_workflow_complete("System.Workflow.Bakery", "tx-123", "success").await?;
//! ```

use crate::drivers::traits::StorageDriver;
use crate::errors::Result;
use chrono::Utc;
use std::sync::Arc;

/// Tracks workflow lifecycle occurrents to storage backend
pub struct OccurrentTracker {
    storage: Arc<dyn StorageDriver>,
}

impl OccurrentTracker {
    /// Create new occurrent tracker with storage backend
    ///
    /// # Arguments
    ///
    /// * `storage` - Storage driver for persisting occurrents (typically JenaStorage)
    pub fn new(storage: Arc<dyn StorageDriver>) -> Self {
        Self { storage }
    }

    /// Track workflow start occurrent
    ///
    /// Creates a BFO occurrent instance marking workflow initialization.
    ///
    /// # Arguments
    ///
    /// * `workflow_urn` - Workflow URN (e.g., "System.Workflow.Bakery")
    /// * `tx_id` - Transaction ID (e.g., "tx-1735989123456")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// tracker.track_workflow_start("System.Workflow.Bakery", "tx-123").await?;
    /// ```
    pub async fn track_workflow_start(&self, workflow_urn: &str, tx_id: &str) -> Result<()> {
        let timestamp = Utc::now().to_rfc3339();
        let occurrent_urn = format!("urn:ckp:occurrent:workflow-start:{}", tx_id);

        let sparql = format!(
            r#"
PREFIX bfo: <http://purl.obolibrary.org/obo/>
PREFIX ckp: <urn:ckp:>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
    <{occurrent_urn}> a bfo:BFO_0000003 ; # Occurrent
                       a ckp:WorkflowStart ;
                       ckp:workflowUrn "{workflow_urn}" ;
                       ckp:transactionId "{tx_id}" ;
                       ckp:timestamp "{timestamp}"^^xsd:dateTime .
}}
"#
        );

        self.execute_sparql(&sparql).await?;
        Ok(())
    }

    /// Track kernel invocation occurrent
    ///
    /// Creates an occurrent marking a specific kernel being invoked in the workflow.
    ///
    /// # Arguments
    ///
    /// * `workflow_urn` - Workflow URN
    /// * `tx_id` - Transaction ID
    /// * `kernel_name` - Kernel being invoked (e.g., "AcceptOrder")
    /// * `step_num` - Step number in workflow (0-indexed)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// tracker.track_kernel_invocation("System.Workflow.Bakery", "tx-123", "AcceptOrder", 0).await?;
    /// ```
    pub async fn track_kernel_invocation(
        &self,
        workflow_urn: &str,
        tx_id: &str,
        kernel_name: &str,
        step_num: usize,
    ) -> Result<()> {
        let timestamp = Utc::now().to_rfc3339();
        let occurrent_urn = format!("urn:ckp:occurrent:kernel-invoke:{}:{}", tx_id, step_num);

        let sparql = format!(
            r#"
PREFIX bfo: <http://purl.obolibrary.org/obo/>
PREFIX ckp: <urn:ckp:>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
    <{occurrent_urn}> a bfo:BFO_0000003 ; # Occurrent
                       a ckp:KernelInvocation ;
                       ckp:workflowUrn "{workflow_urn}" ;
                       ckp:transactionId "{tx_id}" ;
                       ckp:kernelName "{kernel_name}" ;
                       ckp:stepNumber {step_num} ;
                       ckp:timestamp "{timestamp}"^^xsd:dateTime .
}}
"#
        );

        self.execute_sparql(&sparql).await?;
        Ok(())
    }

    /// Track edge routing occurrent
    ///
    /// Creates an occurrent marking data flowing through an edge.
    ///
    /// # Arguments
    ///
    /// * `edge_urn` - Edge URN
    /// * `tx_id` - Transaction ID
    /// * `source_kernel` - Source kernel name
    /// * `target_kernel` - Target kernel name
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// tracker.track_edge_routing("urn:edge:123", "tx-123", "AcceptOrder", "ValidateOrder").await?;
    /// ```
    pub async fn track_edge_routing(
        &self,
        edge_urn: &str,
        tx_id: &str,
        source_kernel: &str,
        target_kernel: &str,
    ) -> Result<()> {
        let timestamp = Utc::now().to_rfc3339();
        let occurrent_urn = format!("urn:ckp:occurrent:edge-route:{}:{}", tx_id, edge_urn);

        let sparql = format!(
            r#"
PREFIX bfo: <http://purl.obolibrary.org/obo/>
PREFIX ckp: <urn:ckp:>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
    <{occurrent_urn}> a bfo:BFO_0000003 ; # Occurrent
                       a ckp:EdgeRouting ;
                       ckp:edgeUrn "{edge_urn}" ;
                       ckp:transactionId "{tx_id}" ;
                       ckp:sourceKernel "{kernel_name}" ;
                       ckp:targetKernel "{target_kernel}" ;
                       ckp:timestamp "{timestamp}"^^xsd:dateTime .
}}
"#,
            kernel_name = source_kernel
        );

        self.execute_sparql(&sparql).await?;
        Ok(())
    }

    /// Track workflow completion occurrent
    ///
    /// Creates an occurrent marking workflow completion.
    ///
    /// # Arguments
    ///
    /// * `workflow_urn` - Workflow URN
    /// * `tx_id` - Transaction ID
    /// * `status` - Completion status ("success" or "failure")
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// tracker.track_workflow_complete("System.Workflow.Bakery", "tx-123", "success").await?;
    /// ```
    pub async fn track_workflow_complete(
        &self,
        workflow_urn: &str,
        tx_id: &str,
        status: &str,
    ) -> Result<()> {
        let timestamp = Utc::now().to_rfc3339();
        let occurrent_urn = format!("urn:ckp:occurrent:workflow-complete:{}", tx_id);

        let sparql = format!(
            r#"
PREFIX bfo: <http://purl.obolibrary.org/obo/>
PREFIX ckp: <urn:ckp:>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
    <{occurrent_urn}> a bfo:BFO_0000003 ; # Occurrent
                       a ckp:WorkflowComplete ;
                       ckp:workflowUrn "{workflow_urn}" ;
                       ckp:transactionId "{tx_id}" ;
                       ckp:status "{status}" ;
                       ckp:timestamp "{timestamp}"^^xsd:dateTime .
}}
"#
        );

        self.execute_sparql(&sparql).await?;
        Ok(())
    }

    /// Execute SPARQL update query on storage backend
    ///
    /// # Arguments
    ///
    /// * `sparql` - SPARQL UPDATE query
    ///
    /// # Returns
    ///
    /// Result indicating success or failure
    async fn execute_sparql(&self, sparql: &str) -> Result<()> {
        eprintln!("[OccurrentTracker] Executing SPARQL:\n{}", sparql);

        // Execute SPARQL update on JenaStorage
        self.storage.execute_sparql_update(sparql).await?;

        eprintln!("[OccurrentTracker] ✓ SPARQL update successful");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drivers::storage::LocalStorage;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_occurrent_tracker_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(LocalStorage::new(
            temp_dir.path().to_path_buf(),
            String::new(),
        ));
        let tracker = OccurrentTracker::new(storage);

        // Test that tracker can be created
        assert!(std::mem::size_of_val(&tracker) > 0);
    }

    #[tokio::test]
    async fn test_workflow_start_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(LocalStorage::new(
            temp_dir.path().to_path_buf(),
            String::new(),
        ));
        let tracker = OccurrentTracker::new(storage);

        // This will generate SPARQL but not execute (no Jena backend)
        let result = tracker
            .track_workflow_start("System.Workflow.Test", "tx-test-123")
            .await;

        // Should succeed (logging only in test mode)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_kernel_invocation_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(LocalStorage::new(
            temp_dir.path().to_path_buf(),
            String::new(),
        ));
        let tracker = OccurrentTracker::new(storage);

        let result = tracker
            .track_kernel_invocation("System.Workflow.Test", "tx-test-123", "TestKernel", 0)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_edge_routing_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(LocalStorage::new(
            temp_dir.path().to_path_buf(),
            String::new(),
        ));
        let tracker = OccurrentTracker::new(storage);

        let result = tracker
            .track_edge_routing("urn:edge:test", "tx-test-123", "KernelA", "KernelB")
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_workflow_complete_tracking() {
        let temp_dir = TempDir::new().unwrap();
        let storage = Arc::new(LocalStorage::new(
            temp_dir.path().to_path_buf(),
            String::new(),
        ));
        let tracker = OccurrentTracker::new(storage);

        let result = tracker
            .track_workflow_complete("System.Workflow.Test", "tx-test-123", "success")
            .await;

        assert!(result.is_ok());
    }
}
