/// Workflow validation and cycle detection using SPARQL
///
/// Implements pure SPARQL-based validation to detect circular references,
/// verify kernel dependencies, and classify cycle types as intentional loops
/// vs problematic cycles.

use crate::ontology::{OntologyLibrary, OntologyError};
use crate::workflow::{WorkflowValidation, WorkflowCycle, CycleType};
use std::collections::{HashMap, HashSet};

/// Validate workflow structure using SPARQL queries
pub fn validate_workflow_structure(
    library: &OntologyLibrary,
    workflow_urn: &str,
) -> Result<WorkflowValidation, OntologyError> {
    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // 1. Detect cycles
    let cycles = detect_cycles_via_sparql(library, workflow_urn)?;

    // 2. Check for problematic cycles
    let problematic_cycles: Vec<_> = cycles.iter()
        .filter(|c| !c.is_intentional)
        .collect();

    if !problematic_cycles.is_empty() {
        for cycle in &problematic_cycles {
            warnings.push(format!(
                "Problematic cycle detected: {:?} (no clear exit condition)",
                cycle.kernels
            ));
        }
    }

    // 3. Verify all referenced kernels exist
    let missing_kernels = verify_kernel_dependencies(library, workflow_urn)?;

    if !missing_kernels.is_empty() {
        for kernel in &missing_kernels {
            errors.push(format!("Missing kernel dependency: {}", kernel));
        }
    }

    // 4. Validate edge predicates
    let invalid_predicates = validate_edge_predicates(library, workflow_urn)?;

    if !invalid_predicates.is_empty() {
        for predicate in &invalid_predicates {
            errors.push(format!("Invalid edge predicate: {}", predicate));
        }
    }

    // 5. Check for orphaned kernels (no incoming or outgoing edges)
    let orphaned = find_orphaned_kernels(library, workflow_urn)?;

    if !orphaned.is_empty() {
        for kernel in &orphaned {
            warnings.push(format!("Orphaned kernel (not connected): {}", kernel));
        }
    }

    let is_valid = errors.is_empty();

    Ok(WorkflowValidation {
        is_valid,
        cycles,
        missing_kernels,
        invalid_predicates,
        warnings,
        errors,
    })
}

/// Detect cycles in workflow using SPARQL property paths
pub fn detect_cycles_via_sparql(
    library: &OntologyLibrary,
    workflow_urn: &str,
) -> Result<Vec<WorkflowCycle>, OntologyError> {
    // Query all edges for this workflow
    let edges_query = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT ?source ?target ?trigger
WHERE {{
    <{}> ckpw:hasEdge ?edge .

    ?edge rdf:type ckpw:WorkflowEdge ;
          ckpw:edgeSource ?source ;
          ckpw:edgeTarget ?target .

    OPTIONAL {{ ?edge ckpw:edgeTrigger ?trigger }}
}}
"#, workflow_urn);

    let results = library.query_sparql(&edges_query)?;

    // Build adjacency list
    let mut graph: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for row in results {
        let source = row.get("source").cloned().unwrap_or_default();
        let target = row.get("target").cloned().unwrap_or_default();
        let trigger = row.get("trigger").cloned().unwrap_or_default();

        graph.entry(source.clone())
            .or_insert_with(Vec::new)
            .push((target, trigger));
    }

    // Detect cycles using DFS
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();
    let mut cycles_raw = Vec::new();

    for node in graph.keys() {
        if !visited.contains(node) {
            let mut path = Vec::new();
            dfs_detect_cycles(
                node,
                &graph,
                &mut visited,
                &mut rec_stack,
                &mut path,
                &mut cycles_raw,
            );
        }
    }

    // Classify and deduplicate cycles
    let mut cycles = Vec::new();
    let mut seen_cycles = HashSet::new();

    for cycle_kernels in cycles_raw {
        // Create canonical representation (sorted) for deduplication
        let mut canonical = cycle_kernels.clone();
        canonical.sort();
        let canonical_key = canonical.join("|");

        if !seen_cycles.contains(&canonical_key) {
            seen_cycles.insert(canonical_key);

            // Classify cycle type
            let (is_intentional, cycle_type, has_exit_condition) =
                classify_cycle(&cycle_kernels, &graph);

            cycles.push(WorkflowCycle {
                kernels: cycle_kernels,
                is_intentional,
                has_exit_condition,
                cycle_type,
            });
        }
    }

    Ok(cycles)
}

/// DFS-based cycle detection
fn dfs_detect_cycles(
    node: &String,
    graph: &HashMap<String, Vec<(String, String)>>,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
    cycles: &mut Vec<Vec<String>>,
) {
    visited.insert(node.clone());
    rec_stack.insert(node.clone());
    path.push(node.clone());

    if let Some(neighbors) = graph.get(node) {
        for (neighbor, _trigger) in neighbors {
            if !visited.contains(neighbor) {
                dfs_detect_cycles(neighbor, graph, visited, rec_stack, path, cycles);
            } else if rec_stack.contains(neighbor) {
                // Found cycle - extract it
                if let Some(cycle_start) = path.iter().position(|n| n == neighbor) {
                    let cycle = path[cycle_start..].to_vec();
                    cycles.push(cycle);
                }
            }
        }
    }

    path.pop();
    rec_stack.remove(node);
}

/// Classify cycle as intentional loop or problematic
fn classify_cycle(
    kernels: &[String],
    graph: &HashMap<String, Vec<(String, String)>>,
) -> (bool, CycleType, bool) {
    // Check for intentional closed-loop verification pattern
    // Pattern: Validator -> ... -> Wss -> Validator (with 5+ nodes)
    let has_validator = kernels.iter().any(|k| k.contains("Validator"));
    let has_wss = kernels.iter().any(|k| k.contains("Wss"));

    if has_validator && has_wss && kernels.len() >= 5 {
        // Check for exit conditions in triggers
        let has_exit = check_exit_conditions(kernels, graph);
        return (true, CycleType::ClosedLoopVerification, has_exit);
    }

    // Check for simple request-response pattern
    // Pattern: A -> B -> A (2 nodes)
    if kernels.len() == 2 {
        let has_exit = check_exit_conditions(kernels, graph);
        return (true, CycleType::RequestResponse, has_exit);
    }

    // Otherwise, problematic cycle
    (false, CycleType::Problematic, false)
}

/// Check if cycle has exit conditions in edge triggers
fn check_exit_conditions(
    kernels: &[String],
    graph: &HashMap<String, Vec<(String, String)>>,
) -> bool {
    for kernel in kernels {
        if let Some(neighbors) = graph.get(kernel) {
            for (target, trigger) in neighbors {
                if kernels.contains(target) {
                    // Check if trigger has conditional logic
                    if trigger.contains("when") || trigger.contains("if") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Verify all referenced kernels exist in System.Registry
fn verify_kernel_dependencies(
    library: &OntologyLibrary,
    workflow_urn: &str,
) -> Result<Vec<String>, OntologyError> {
    // Query all kernels referenced in workflow
    let referenced_query = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT DISTINCT ?kernel
WHERE {{
    <{}> ckpw:hasEdge ?edge .

    {{
        ?edge ckpw:edgeSource ?kernel .
    }} UNION {{
        ?edge ckpw:edgeTarget ?kernel .
    }}
}}
"#, workflow_urn);

    let referenced = library.query_sparql(&referenced_query)?;

    // Query all kernels that actually exist
    let existing_query = r#"
PREFIX ckp: <https://conceptkernel.org/ontology#>
PREFIX bfo: <http://purl.obolibrary.org/obo/BFO_>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT DISTINCT ?kernel
WHERE {
    ?kernel rdf:type bfo:0000040 ;
            rdf:type ckp:Kernel .
}
"#;

    let existing = library.query_sparql(existing_query)?;

    let existing_set: HashSet<String> = existing.iter()
        .filter_map(|row| row.get("kernel").cloned())
        .collect();

    let mut missing = Vec::new();

    for row in referenced {
        if let Some(kernel) = row.get("kernel") {
            if !existing_set.contains(kernel) {
                missing.push(kernel.clone());
            }
        }
    }

    Ok(missing)
}

/// Validate edge predicates against workflow ontology
fn validate_edge_predicates(
    library: &OntologyLibrary,
    workflow_urn: &str,
) -> Result<Vec<String>, OntologyError> {
    // Query predicates used in workflow
    let used_query = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>

SELECT DISTINCT ?predicate
WHERE {{
    <{}> ckpw:hasEdge ?edge .
    ?edge ckpw:edgePredicate ?predicate .
}}
"#, workflow_urn);

    let used = library.query_sparql(&used_query)?;

    // Query valid predicates defined in ontology
    let valid_query = r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>
PREFIX owl: <http://www.w3.org/2002/07/owl#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

SELECT DISTINCT ?predicate
WHERE {
    ?predicate rdf:type owl:ObjectProperty ;
               rdfs:subPropertyOf ckp:relatesTo .
    FILTER(STRSTARTS(STR(?predicate), "https://conceptkernel.org/ontology/workflow#"))
}
"#;

    let valid = library.query_sparql(valid_query)?;

    let valid_set: HashSet<String> = valid.iter()
        .filter_map(|row| row.get("predicate").cloned())
        .collect();

    let mut invalid = Vec::new();

    for row in used {
        if let Some(predicate) = row.get("predicate") {
            if !valid_set.contains(predicate) {
                invalid.push(predicate.clone());
            }
        }
    }

    Ok(invalid)
}

/// Find orphaned kernels (not connected to workflow)
fn find_orphaned_kernels(
    library: &OntologyLibrary,
    workflow_urn: &str,
) -> Result<Vec<String>, OntologyError> {
    // Query all kernels referenced in workflow
    let connected_query = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>

SELECT DISTINCT ?kernel
WHERE {{
    <{}> ckpw:hasEdge ?edge .

    {{
        ?edge ckpw:edgeSource ?kernel .
    }} UNION {{
        ?edge ckpw:edgeTarget ?kernel .
    }}
}}
"#, workflow_urn);

    let connected = library.query_sparql(&connected_query)?;

    let connected_set: HashSet<String> = connected.iter()
        .filter_map(|row| row.get("kernel").cloned())
        .collect();

    // Query all kernels declared in workflow
    let declared_query = format!(r#"
PREFIX ckpw: <https://conceptkernel.org/ontology/workflow#>

SELECT DISTINCT ?kernel
WHERE {{
    <{}> ckpw:hasKernel ?kernel .
}}
"#, workflow_urn);

    let declared = library.query_sparql(&declared_query)?;

    let mut orphaned = Vec::new();

    for row in declared {
        if let Some(kernel) = row.get("kernel") {
            if !connected_set.contains(kernel) {
                orphaned.push(kernel.clone());
            }
        }
    }

    Ok(orphaned)
}
