/**
 * query.rs
 * Query types and builders for SPARQL
 */

use std::collections::HashMap;

pub type QueryResult = HashMap<String, String>;

pub struct SparqlQuery {
    query: String,
}

impl SparqlQuery {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
        }
    }
    
    pub fn as_str(&self) -> &str {
        &self.query
    }
    
    /// Find edge predicate mapping
    pub fn edge_to_predicate(edge_name: &str) -> Self {
        Self::new(format!(
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
        ))
    }
    
    /// Find all classes in kernel ontology
    pub fn kernel_classes(graph_uri: &str) -> Self {
        Self::new(format!(
            r#"
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            PREFIX owl: <http://www.w3.org/2002/07/owl#>
            
            SELECT ?class ?label
            FROM <{}>
            WHERE {{
                ?class rdf:type owl:Class .
                OPTIONAL {{ ?class rdfs:label ?label }}
            }}
            "#,
            graph_uri
        ))
    }
    
    /// Check if class is temporal (BFO Occurrent)
    pub fn is_temporal(class_uri: &str) -> Self {
        Self::new(format!(
            r#"
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>

            ASK {{
                <{}> rdfs:subClassOf* bfo:0000003 .
            }}
            "#,
            class_uri
        ))
    }

    // =============================================================================
    // BFO Occurrent Queries (Temporal Processes) - using ckp:// URNs
    // =============================================================================

    /// Query occurrent processes by kernel (ckp://Process?kernel=X)
    /// Returns all process instances created by a specific kernel
    pub fn occurrents_by_kernel(kernel_urn: &str) -> Self {
        Self::new(format!(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?process ?txId ?timestamp ?exitCode
            WHERE {{
                ?process rdf:type bfo:0000015 ;  # BFO Process
                         ckp:kernel <{}> ;
                         ckp:txId ?txId ;
                         ckp:timestamp ?timestamp ;
                         ckp:exitCode ?exitCode .
            }}
            ORDER BY DESC(?timestamp)
            "#,
            kernel_urn
        ))
    }

    /// Query specific occurrent process by txId fragment (ckp://Process#txId)
    /// Returns single process instance with full details
    pub fn occurrent_by_id(tx_id: &str) -> Self {
        Self::new(format!(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?process ?kernel ?timestamp ?exitCode ?processUrn ?duration
            WHERE {{
                ?process rdf:type bfo:0000015 ;  # BFO Process
                         ckp:txId "{}" ;
                         ckp:kernel ?kernel ;
                         ckp:timestamp ?timestamp ;
                         ckp:exitCode ?exitCode ;
                         ckp:processUrn ?processUrn .
                OPTIONAL {{ ?process ckp:duration ?duration }}
            }}
            "#,
            tx_id
        ))
    }

    /// Query occurrent processes by time range
    /// Returns processes that occurred within a temporal boundary
    pub fn occurrents_by_timespan(start_iso: &str, end_iso: &str) -> Self {
        Self::new(format!(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
            PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

            SELECT ?process ?txId ?kernel ?timestamp ?exitCode
            WHERE {{
                ?process rdf:type bfo:0000015 ;
                         ckp:txId ?txId ;
                         ckp:kernel ?kernel ;
                         ckp:timestamp ?timestamp ;
                         ckp:exitCode ?exitCode .
                FILTER (?timestamp >= "{}"^^xsd:dateTime && ?timestamp <= "{}"^^xsd:dateTime)
            }}
            ORDER BY DESC(?timestamp)
            "#,
            start_iso, end_iso
        ))
    }

    /// Query failed occurrent processes (exitCode != 0)
    pub fn occurrents_failed(limit: usize) -> Self {
        Self::new(format!(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?process ?txId ?kernel ?timestamp ?exitCode
            WHERE {{
                ?process rdf:type bfo:0000015 ;
                         ckp:txId ?txId ;
                         ckp:kernel ?kernel ;
                         ckp:timestamp ?timestamp ;
                         ckp:exitCode ?exitCode .
                FILTER (?exitCode != 0)
            }}
            ORDER BY DESC(?timestamp)
            LIMIT {}
            "#,
            limit
        ))
    }

    // =============================================================================
    // BFO Continuant Queries (Persistent Entities) - using ckp:// URNs
    // =============================================================================

    /// Query continuant kernel instances (ckp://ConceptKernel.Name)
    /// Returns all kernels of a specific type as continuants
    pub fn continuants_by_type(kernel_type: &str) -> Self {
        Self::new(format!(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?kernel ?name ?version ?status
            WHERE {{
                ?kernel rdf:type bfo:0000002 ;  # BFO Continuant
                        rdf:type ckp:ConceptKernel ;
                        ckp:kernelType "{}" ;
                        ckp:name ?name ;
                        ckp:version ?version ;
                        ckp:status ?status .
            }}
            ORDER BY ?name
            "#,
            kernel_type
        ))
    }

    /// Query active continuant kernels (status=running)
    pub fn continuants_active() -> Self {
        Self::new(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?kernel ?name ?version ?pid ?port
            WHERE {
                ?kernel rdf:type bfo:0000002 ;  # BFO Continuant
                        rdf:type ckp:ConceptKernel ;
                        ckp:name ?name ;
                        ckp:status "running" ;
                        ckp:version ?version .
                OPTIONAL { ?kernel ckp:pid ?pid }
                OPTIONAL { ?kernel ckp:port ?port }
            }
            ORDER BY ?name
            "#
        )
    }

    /// Query continuant by URN (ckp://ConceptKernel.Name:version)
    pub fn continuant_by_urn(kernel_urn: &str) -> Self {
        Self::new(format!(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?kernel ?name ?version ?status ?config ?queue_contract ?notification_contract
            WHERE {{
                ?kernel ckp:urn <{}> ;
                        rdf:type bfo:0000002 ;  # BFO Continuant
                        ckp:name ?name ;
                        ckp:version ?version ;
                        ckp:status ?status .
                OPTIONAL {{ ?kernel ckp:config ?config }}
                OPTIONAL {{ ?kernel ckp:queueContract ?queue_contract }}
                OPTIONAL {{ ?kernel ckp:notificationContract ?notification_contract }}
            }}
            "#,
            kernel_urn
        ))
    }

    // =============================================================================
    // Edge and Node Queries - using ckp:// URNs
    // =============================================================================

    /// Query edges by predicate (ckp://Edge.PRODUCES.Source-to-Target)
    pub fn edges_by_predicate(predicate: &str) -> Self {
        Self::new(format!(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?edge ?source ?target ?queuePath ?instanceCount
            WHERE {{
                ?edge rdf:type ckp:Edge ;
                      ckp:predicate "{}" ;
                      ckp:source ?source ;
                      ckp:target ?target ;
                      ckp:queuePath ?queuePath .
                OPTIONAL {{ ?edge ckp:instanceCount ?instanceCount }}
            }}
            ORDER BY ?source ?target
            "#,
            predicate
        ))
    }

    /// Query all edges for a kernel (source or target)
    pub fn edges_for_kernel(kernel_name: &str) -> Self {
        Self::new(format!(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?edge ?predicate ?source ?target ?direction
            WHERE {{
                ?edge rdf:type ckp:Edge ;
                      ckp:predicate ?predicate ;
                      ckp:source ?source ;
                      ckp:target ?target .
                BIND(
                    IF(?source = "{}", "outgoing",
                    IF(?target = "{}", "incoming", "unknown"))
                    AS ?direction
                )
                FILTER(?source = "{}" || ?target = "{}")
            }}
            ORDER BY ?direction ?predicate
            "#,
            kernel_name, kernel_name, kernel_name, kernel_name
        ))
    }

    /// Query edge instances (messages in edge queues)
    pub fn edge_instances(edge_urn: &str, limit: usize) -> Self {
        Self::new(format!(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?instance ?txId ?timestamp ?from ?to ?payload
            WHERE {{
                ?instance rdf:type ckp:EdgeInstance ;
                          ckp:edge <{}> ;
                          ckp:txId ?txId ;
                          ckp:timestamp ?timestamp ;
                          ckp:from ?from ;
                          ckp:to ?to ;
                          ckp:payload ?payload .
            }}
            ORDER BY DESC(?timestamp)
            LIMIT {}
            "#,
            edge_urn, limit
        ))
    }

    // =============================================================================
    // Aggregation Queries - using ckp:// URNs
    // =============================================================================

    /// Count processes by kernel
    pub fn count_processes_by_kernel() -> Self {
        Self::new(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?kernel (COUNT(?process) AS ?processCount)
            WHERE {
                ?process rdf:type bfo:0000015 ;  # BFO Process
                         ckp:kernel ?kernel .
            }
            GROUP BY ?kernel
            ORDER BY DESC(?processCount)
            "#
        )
    }

    /// Count processes by exit code (success vs failure analysis)
    pub fn count_processes_by_exit_code() -> Self {
        Self::new(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?exitCode (COUNT(?process) AS ?count)
            WHERE {
                ?process rdf:type bfo:0000015 ;
                         ckp:exitCode ?exitCode .
            }
            GROUP BY ?exitCode
            ORDER BY ?exitCode
            "#
        )
    }

    /// Count edge instances by predicate
    pub fn count_edge_instances_by_predicate() -> Self {
        Self::new(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?predicate (COUNT(?instance) AS ?instanceCount)
            WHERE {
                ?edge rdf:type ckp:Edge ;
                      ckp:predicate ?predicate .
                ?instance ckp:edge ?edge .
            }
            GROUP BY ?predicate
            ORDER BY DESC(?instanceCount)
            "#
        )
    }

    /// Aggregate process duration statistics by kernel
    pub fn aggregate_duration_by_kernel() -> Self {
        Self::new(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?kernel
                   (COUNT(?process) AS ?totalProcesses)
                   (AVG(?duration) AS ?avgDuration)
                   (MIN(?duration) AS ?minDuration)
                   (MAX(?duration) AS ?maxDuration)
            WHERE {
                ?process rdf:type bfo:0000015 ;
                         ckp:kernel ?kernel ;
                         ckp:duration ?duration .
            }
            GROUP BY ?kernel
            ORDER BY DESC(?avgDuration)
            "#
        )
    }

    /// Count active kernels by type
    pub fn count_kernels_by_type() -> Self {
        Self::new(
            r#"
            PREFIX ckp: <ckp://>
            PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
            PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

            SELECT ?kernelType (COUNT(?kernel) AS ?kernelCount)
            WHERE {
                ?kernel rdf:type bfo:0000002 ;  # BFO Continuant
                        rdf:type ckp:ConceptKernel ;
                        ckp:kernelType ?kernelType ;
                        ckp:status "running" .
            }
            GROUP BY ?kernelType
            ORDER BY DESC(?kernelCount)
            "#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: SparqlQuery can be constructed with string
    #[test]
    fn test_sparql_query_new() {
        let query = SparqlQuery::new("SELECT * WHERE { ?s ?p ?o }");
        assert_eq!(query.as_str(), "SELECT * WHERE { ?s ?p ?o }");
    }

    /// Test: SparqlQuery can be constructed from String
    #[test]
    fn test_sparql_query_from_string() {
        let query_string = String::from("SELECT ?class WHERE { ?class a owl:Class }");
        let query = SparqlQuery::new(query_string);
        assert!(query.as_str().contains("SELECT ?class"));
    }

    /// Test: edge_to_predicate query construction
    #[test]
    fn test_edge_to_predicate_query() {
        let query = SparqlQuery::edge_to_predicate("PRODUCES");
        let query_str = query.as_str();

        // Verify query structure
        assert!(query_str.contains("PREFIX ckp:"));
        assert!(query_str.contains("SELECT ?predicate"));
        assert!(query_str.contains("PRODUCES"));
        assert!(query_str.contains("ckp:edgeName"));
        assert!(query_str.contains("ckp:predicate"));
    }

    /// Test: edge_to_predicate handles different edge names
    #[test]
    fn test_edge_to_predicate_various_names() {
        let predicates = vec!["PRODUCES", "REQUIRES", "VALIDATES", "INFLUENCES"];

        for pred in predicates {
            let query = SparqlQuery::edge_to_predicate(pred);
            let query_str = query.as_str();

            assert!(
                query_str.contains(pred),
                "Query should contain predicate name: {}",
                pred
            );
        }
    }

    /// Test: kernel_classes query construction
    #[test]
    fn test_kernel_classes_query() {
        let graph_uri = "https://conceptkernel.org/ontology/core";
        let query = SparqlQuery::kernel_classes(graph_uri);
        let query_str = query.as_str();

        // Verify query structure
        assert!(query_str.contains("PREFIX rdf:"));
        assert!(query_str.contains("PREFIX owl:"));
        assert!(query_str.contains("SELECT ?class ?label"));
        assert!(query_str.contains(graph_uri));
        assert!(query_str.contains("owl:Class"));
        assert!(query_str.contains("OPTIONAL"));
        assert!(query_str.contains("rdfs:label"));
    }

    /// Test: kernel_classes handles different graph URIs
    #[test]
    fn test_kernel_classes_various_graphs() {
        let graphs = vec![
            "https://conceptkernel.org/ontology/core",
            "https://example.com/ontology",
            "urn:x-myontology:graph1",
        ];

        for graph_uri in graphs {
            let query = SparqlQuery::kernel_classes(graph_uri);
            let query_str = query.as_str();

            assert!(
                query_str.contains(graph_uri),
                "Query should contain graph URI: {}",
                graph_uri
            );
        }
    }

    /// Test: is_temporal query construction
    #[test]
    fn test_is_temporal_query() {
        let class_uri = "http://purl.obolibrary.org/obo/BFO_0000015";
        let query = SparqlQuery::is_temporal(class_uri);
        let query_str = query.as_str();

        // Verify query structure
        assert!(query_str.contains("PREFIX rdfs:"));
        assert!(query_str.contains("PREFIX bfo:"));
        assert!(query_str.contains("ASK"));
        assert!(query_str.contains(class_uri));
        assert!(query_str.contains("rdfs:subClassOf*"));
        assert!(query_str.contains("bfo:0000003")); // BFO Occurrent
    }

    /// Test: is_temporal query uses ASK (not SELECT)
    #[test]
    fn test_is_temporal_uses_ask() {
        let query = SparqlQuery::is_temporal("http://example.com/Process");
        let query_str = query.as_str();

        assert!(query_str.contains("ASK"));
        assert!(!query_str.contains("SELECT"));
    }

    /// Test: query contains proper SPARQL structure
    #[test]
    fn test_query_sparql_structure() {
        let query = SparqlQuery::kernel_classes("http://example.com/graph");
        let query_str = query.as_str();

        // All SPARQL queries should have WHERE clause
        assert!(query_str.contains("WHERE"));

        // Should have proper bracing
        assert!(query_str.contains("{"));
        assert!(query_str.contains("}"));
    }

    /// Test: OPTIONAL clause handling in kernel_classes
    #[test]
    fn test_optional_clause_in_kernel_classes() {
        let query = SparqlQuery::kernel_classes("http://example.com/graph");
        let query_str = query.as_str();

        // kernel_classes uses OPTIONAL for label
        assert!(query_str.contains("OPTIONAL"));

        // The OPTIONAL clause should have proper structure
        let optional_start = query_str.find("OPTIONAL").unwrap();
        let after_optional = &query_str[optional_start..];
        assert!(after_optional.contains("{"));
        assert!(after_optional.contains("rdfs:label"));
    }

    /// Test: Query type alias
    #[test]
    fn test_query_result_type() {
        // QueryResult is a type alias for HashMap<String, String>
        let result: QueryResult = HashMap::new();
        assert_eq!(result.len(), 0);

        let mut result = QueryResult::new();
        result.insert("class".to_string(), "owl:Class".to_string());
        assert_eq!(result.get("class"), Some(&"owl:Class".to_string()));
    }

    /// Test: Empty query can be constructed
    #[test]
    fn test_empty_query() {
        let query = SparqlQuery::new("");
        assert_eq!(query.as_str(), "");
    }
}
