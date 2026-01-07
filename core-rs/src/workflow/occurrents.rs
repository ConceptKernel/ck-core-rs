//! Workflow Lifecycle Occurrent Tracking
//!
//! This module centralizes all occurrent creation logic for workflow execution tracking.
//! Occurrents represent temporal instances of processes and events in the CKP ontology.

use crate::drivers::JenaStorage;
use crate::errors::Result;
use std::sync::Arc;
use chrono::Utc;

/// Tracks workflow lifecycle occurrents including executions, kernel invocations,
/// edge routing events, and completion statuses.
pub struct OccurrentTracker {
    storage: Arc<JenaStorage>,
}

impl OccurrentTracker {
    /// Creates a new OccurrentTracker with the given Jena storage driver
    pub fn new(storage: Arc<JenaStorage>) -> Self {
        Self { storage }
    }

    /// Tracks the start of a workflow execution
    ///
    /// Creates a WorkflowExecution occurrent with URN pattern:
    /// `ckp://Process#WorkflowExecution-{workflow}-{tx_id}`
    ///
    /// # Arguments
    /// * `workflow_name` - Name of the workflow being executed
    /// * `tx_id` - Unique transaction identifier
    pub async fn track_workflow_start(&self, workflow_name: &str, tx_id: &str) -> Result<()> {
        let urn = format!("ckp://Process#WorkflowExecution-{}-{}", workflow_name, tx_id);
        let graph_uri = format!("ckp://occurrents/workflow/{}", tx_id);
        let timestamp = Utc::now().to_rfc3339();

        let sparql = format!(
            r#"PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
  GRAPH <{graph_uri}> {{
    <{urn}> a bfo:0000003 ;
        a ckp:WorkflowExecution ;
        ckp:hasURN "{urn}" ;
        ckp:workflowName "{workflow_name}" ;
        ckp:transactionId "{tx_id}" ;
        ckp:startTime "{timestamp}"^^xsd:dateTime ;
        ckp:status "running" .
  }}
}}
"#,
            graph_uri = graph_uri,
            urn = urn,
            workflow_name = workflow_name,
            tx_id = tx_id,
            timestamp = timestamp
        );

        self.storage.execute_sparql_update(&sparql).await?;
        Ok(())
    }

    /// Tracks a kernel invocation within a workflow
    ///
    /// Creates a KernelInvocation occurrent with URN pattern:
    /// `ckp://Process#Invocation-{kernel}-{tx_id}-{step}`
    ///
    /// # Arguments
    /// * `workflow_name` - Name of the parent workflow
    /// * `tx_id` - Transaction identifier
    /// * `kernel_name` - Name of the kernel being invoked
    /// * `step` - Step number in the workflow execution
    pub async fn track_kernel_invocation(
        &self,
        workflow_name: &str,
        tx_id: &str,
        kernel_name: &str,
        step: usize,
    ) -> Result<()> {
        let invocation_urn = format!("ckp://Process#Invocation-{}-{}-{}", kernel_name, tx_id, step);
        let workflow_urn = format!("ckp://Process#WorkflowExecution-{}-{}", workflow_name, tx_id);
        let graph_uri = format!("ckp://occurrents/workflow/{}", tx_id);
        let timestamp = Utc::now().to_rfc3339();

        let sparql = format!(
            r#"PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
  GRAPH <{graph_uri}> {{
    <{invocation_urn}> a bfo:0000003 ;
        a ckp:KernelInvocation ;
        ckp:hasURN "{invocation_urn}" ;
        ckp:kernelName "{kernel_name}" ;
        ckp:transactionId "{tx_id}" ;
        ckp:stepNumber {step} ;
        ckp:invocationTime "{timestamp}"^^xsd:dateTime ;
        ckp:partOfWorkflow <{workflow_urn}> .
  }}
}}
"#,
            graph_uri = graph_uri,
            invocation_urn = invocation_urn,
            kernel_name = kernel_name,
            tx_id = tx_id,
            step = step,
            timestamp = timestamp,
            workflow_urn = workflow_urn
        );

        self.storage.execute_sparql_update(&sparql).await?;
        Ok(())
    }

    /// Tracks an edge routing event
    ///
    /// Creates an EdgeRouting occurrent with URN pattern:
    /// `ckp://Process#EdgeRouting-{edge}-{tx_id}`
    ///
    /// # Arguments
    /// * `edge_urn` - URN of the edge being routed
    /// * `tx_id` - Transaction identifier
    /// * `source` - Source kernel name
    /// * `target` - Target kernel name
    pub async fn track_edge_routing(
        &self,
        edge_urn: &str,
        tx_id: &str,
        source: &str,
        target: &str,
    ) -> Result<()> {
        // Extract edge identifier from URN for routing URN
        let edge_id = edge_urn
            .split('#')
            .last()
            .unwrap_or("unknown");

        let routing_urn = format!("ckp://Process#EdgeRouting-{}-{}", edge_id, tx_id);
        let graph_uri = format!("ckp://occurrents/edge/{}", edge_id);
        let timestamp = Utc::now().to_rfc3339();

        let sparql = format!(
            r#"PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

INSERT DATA {{
  GRAPH <{graph_uri}> {{
    <{routing_urn}> a bfo:0000003 ;
        a ckp:EdgeRouting ;
        ckp:hasURN "{routing_urn}" ;
        ckp:routedEdge <{edge_urn}> ;
        ckp:transactionId "{tx_id}" ;
        ckp:sourceKernel "{source}" ;
        ckp:targetKernel "{target}" ;
        ckp:routingTime "{timestamp}"^^xsd:dateTime .
  }}
}}
"#,
            graph_uri = graph_uri,
            routing_urn = routing_urn,
            edge_urn = edge_urn,
            tx_id = tx_id,
            source = source,
            target = target,
            timestamp = timestamp
        );

        self.storage.execute_sparql_update(&sparql).await?;
        Ok(())
    }

    /// Tracks workflow completion
    ///
    /// Updates the WorkflowExecution occurrent with completion time and final status.
    ///
    /// # Arguments
    /// * `workflow_name` - Name of the workflow
    /// * `tx_id` - Transaction identifier
    /// * `status` - Final status (e.g., "completed", "failed", "cancelled")
    pub async fn track_workflow_complete(
        &self,
        workflow_name: &str,
        tx_id: &str,
        status: &str,
    ) -> Result<()> {
        let urn = format!("ckp://Process#WorkflowExecution-{}-{}", workflow_name, tx_id);
        let graph_uri = format!("ckp://occurrents/workflow/{}", tx_id);
        let timestamp = Utc::now().to_rfc3339();

        let sparql = format!(
            r#"PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

DELETE {{
  GRAPH <{graph_uri}> {{
    <{urn}> ckp:status ?oldStatus .
  }}
}}
INSERT {{
  GRAPH <{graph_uri}> {{
    <{urn}> ckp:status "{status}" ;
        ckp:endTime "{timestamp}"^^xsd:dateTime .
  }}
}}
WHERE {{
  GRAPH <{graph_uri}> {{
    <{urn}> ckp:status ?oldStatus .
  }}
}}
"#,
            graph_uri = graph_uri,
            urn = urn,
            status = status,
            timestamp = timestamp
        );

        self.storage.execute_sparql_update(&sparql).await?;
        Ok(())
    }

    /// Retrieves the current status of a workflow execution
    ///
    /// # Arguments
    /// * `workflow_name` - Name of the workflow
    /// * `tx_id` - Transaction identifier
    ///
    /// # Returns
    /// The current status string if found, None otherwise
    pub async fn get_workflow_status(
        &self,
        workflow_name: &str,
        tx_id: &str,
    ) -> Result<Option<String>> {
        let urn = format!("ckp://Process#WorkflowExecution-{}-{}", workflow_name, tx_id);
        let graph_uri = format!("ckp://occurrents/workflow/{}", tx_id);

        let sparql = format!(
            r#"PREFIX ckp: <https://conceptkernel.org/ontology#>

SELECT ?status
WHERE {{
  GRAPH <{graph_uri}> {{
    <{urn}> ckp:status ?status .
  }}
}}
"#,
            graph_uri = graph_uri,
            urn = urn
        );

        let results = self.storage.execute_sparql_query(&sparql).await?;

        // Parse JSON results to extract status
        if let Some(bindings) = results["results"]["bindings"].as_array() {
            if let Some(first_binding) = bindings.first() {
                if let Some(status_value) = first_binding["status"]["value"].as_str() {
                    return Ok(Some(status_value.to_string()));
                }
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drivers::JenaStorage;
    use std::sync::Arc;

    fn create_test_tracker() -> OccurrentTracker {
        let fuseki_url = "http://localhost:3030".to_string(); // Will fail without Fuseki running
        let dataset = "test_occurrents".to_string();

        // Create JenaStorage instance (will error in actual tests without Fuseki running)
        let storage = JenaStorage::new(fuseki_url, dataset);

        OccurrentTracker::new(Arc::new(storage))
    }

    #[tokio::test]
    #[ignore] // Requires Fuseki server running
    async fn test_track_workflow_start() {
        let tracker = create_test_tracker();

        let result = tracker.track_workflow_start("test_workflow", "tx_123").await;
        assert!(result.is_ok(), "Failed to track workflow start: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore] // Requires Fuseki server running
    async fn test_track_kernel_invocation() {
        let tracker = create_test_tracker();

        // First start the workflow
        tracker.track_workflow_start("test_workflow", "tx_123").await.unwrap();

        // Then track kernel invocation
        let result = tracker.track_kernel_invocation(
            "test_workflow",
            "tx_123",
            "test_kernel",
            1
        ).await;

        assert!(result.is_ok(), "Failed to track kernel invocation: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore] // Requires Fuseki server running
    async fn test_track_edge_routing() {
        let tracker = create_test_tracker();

        let result = tracker.track_edge_routing(
            "ckp://Edge#test_edge",
            "tx_123",
            "source_kernel",
            "target_kernel"
        ).await;

        assert!(result.is_ok(), "Failed to track edge routing: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore] // Requires Fuseki server running
    async fn test_track_workflow_complete() {
        let tracker = create_test_tracker();

        // Start workflow first
        tracker.track_workflow_start("test_workflow", "tx_123").await.unwrap();

        // Complete it
        let result = tracker.track_workflow_complete(
            "test_workflow",
            "tx_123",
            "completed"
        ).await;

        assert!(result.is_ok(), "Failed to track workflow completion: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore] // Requires Fuseki server running
    async fn test_complete_workflow_lifecycle() {
        let tracker = create_test_tracker();
        let workflow_name = "bakery_workflow";
        let tx_id = "tx_456";

        // Start workflow
        tracker.track_workflow_start(workflow_name, tx_id).await.unwrap();

        // Track multiple kernel invocations
        tracker.track_kernel_invocation(workflow_name, tx_id, "ingredient_kernel", 1).await.unwrap();
        tracker.track_kernel_invocation(workflow_name, tx_id, "mixing_kernel", 2).await.unwrap();
        tracker.track_kernel_invocation(workflow_name, tx_id, "baking_kernel", 3).await.unwrap();

        // Track edge routing
        tracker.track_edge_routing(
            "ckp://Edge#ingredient_to_mixing",
            tx_id,
            "ingredient_kernel",
            "mixing_kernel"
        ).await.unwrap();

        tracker.track_edge_routing(
            "ckp://Edge#mixing_to_baking",
            tx_id,
            "mixing_kernel",
            "baking_kernel"
        ).await.unwrap();

        // Complete workflow
        let result = tracker.track_workflow_complete(workflow_name, tx_id, "completed").await;
        assert!(result.is_ok(), "Failed complete workflow lifecycle: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore] // Requires Fuseki server running
    async fn test_workflow_failure_tracking() {
        let tracker = create_test_tracker();

        tracker.track_workflow_start("failing_workflow", "tx_789").await.unwrap();
        tracker.track_kernel_invocation("failing_workflow", "tx_789", "error_kernel", 1).await.unwrap();

        let result = tracker.track_workflow_complete(
            "failing_workflow",
            "tx_789",
            "failed"
        ).await;

        assert!(result.is_ok(), "Failed to track workflow failure: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore] // Requires Fuseki server running
    async fn test_multiple_workflows_same_name() {
        let tracker = create_test_tracker();

        // Track two different executions of the same workflow
        tracker.track_workflow_start("parallel_workflow", "tx_001").await.unwrap();
        tracker.track_workflow_start("parallel_workflow", "tx_002").await.unwrap();

        tracker.track_kernel_invocation("parallel_workflow", "tx_001", "kernel_a", 1).await.unwrap();
        tracker.track_kernel_invocation("parallel_workflow", "tx_002", "kernel_b", 1).await.unwrap();

        // Complete one
        tracker.track_workflow_complete("parallel_workflow", "tx_001", "completed").await.unwrap();

        // The other should still be running (this is implicit, we just verify no errors)
        let result = tracker.track_kernel_invocation("parallel_workflow", "tx_002", "kernel_c", 2).await;
        assert!(result.is_ok(), "Failed to continue second workflow: {:?}", result.err());
    }

    #[tokio::test]
    #[ignore] // Requires Fuseki server running
    async fn test_edge_routing_with_complex_urn() {
        let tracker = create_test_tracker();

        let complex_edge_urn = "ckp://Edge#workflow.step1.output-to-step2.input";
        let result = tracker.track_edge_routing(
            complex_edge_urn,
            "tx_complex",
            "step1_kernel",
            "step2_kernel"
        ).await;

        assert!(result.is_ok(), "Failed to track edge routing with complex URN: {:?}", result.err());
    }
}
