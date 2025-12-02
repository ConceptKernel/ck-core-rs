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
