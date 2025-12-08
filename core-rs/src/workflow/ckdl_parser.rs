/// CKDL (Concept Kernel Definition Language) Parser
///
/// Parses CKDL workflow files and identifies:
/// - Which components are FORKED from existing kernels
/// - Which components are BRAND NEW concepts
/// - External dependencies (EXTERN)
/// - Workflow structure (edges, triggers, phases)

use crate::ontology::{OntologyLibrary, OntologyError, OntologyReader};
use crate::workflow::{Workflow, WorkflowPhase, WorkflowEdge, WorkflowTrigger, WorkflowStatus, PhaseStatus};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CkdlWorkflow {
    pub workflow_urn: String,
    pub label: String,
    pub description: String,
    pub trigger: String,
    pub quorum: Option<String>,
    pub extern_kernels: Vec<ExternKernel>,
    pub workflow_kernels: Vec<WorkflowKernel>,
    pub edges: Vec<CkdlEdge>,
    pub analysis: ComponentAnalysis,
}

#[derive(Debug, Clone)]
pub struct ExternKernel {
    pub urn: String,
    pub role: Option<String>,
    pub actions: Vec<String>,
    pub origin: ComponentOrigin,
}

#[derive(Debug, Clone)]
pub struct WorkflowKernel {
    pub urn: String,
    pub kernel_type: String,
    pub runtime: Option<String>,
    pub description: String,
    pub capabilities: Vec<String>,
    pub actions: Vec<String>,
    pub origin: ComponentOrigin,
}

#[derive(Debug, Clone)]
pub struct CkdlEdge {
    pub edge_urn: String,
    pub source: String,
    pub target: String,
    pub predicate: String,
    pub trigger: String,
    pub origin: ComponentOrigin,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComponentOrigin {
    /// Kernel exists in /concepts/ (forked from existing)
    Forked { kernel_path: String, version: String },
    /// Kernel declared in CKDL but doesn't exist yet (brand new)
    BrandNew,
    /// External kernel dependency (referenced but not defined)
    External,
}

#[derive(Debug, Clone)]
pub struct ComponentAnalysis {
    pub total_extern: usize,
    pub total_workflow_kernels: usize,
    pub total_edges: usize,
    pub forked_kernels: Vec<String>,
    pub brand_new_kernels: Vec<String>,
    pub external_dependencies: Vec<String>,
}

/// Parse CKDL workflow file
pub fn parse_ckdl_file(
    ckdl_path: impl AsRef<Path>,
    project_root: impl AsRef<Path>,
) -> Result<CkdlWorkflow, OntologyError> {
    let path = ckdl_path.as_ref();
    let project_root = project_root.as_ref();

    let content = fs::read_to_string(path)
        .map_err(|_e| OntologyError::LoadError("Failed to read CKDL file".to_string()))?;

    let mut workflow = CkdlWorkflow {
        workflow_urn: String::new(),
        label: String::new(),
        description: String::new(),
        trigger: String::new(),
        quorum: None,
        extern_kernels: Vec::new(),
        workflow_kernels: Vec::new(),
        edges: Vec::new(),
        analysis: ComponentAnalysis {
            total_extern: 0,
            total_workflow_kernels: 0,
            total_edges: 0,
            forked_kernels: Vec::new(),
            brand_new_kernels: Vec::new(),
            external_dependencies: Vec::new(),
        },
    };

    // Create ontology reader to check which kernels exist
    let reader = OntologyReader::new(project_root.to_path_buf());

    // Parse sections
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            i += 1;
            continue;
        }

        // Parse WORKFLOW declaration
        if line.starts_with("WORKFLOW ") {
            let (urn, metadata) = parse_workflow_header(&lines, i)?;
            workflow.workflow_urn = urn;
            workflow.label = metadata.get("label").cloned().unwrap_or_default();
            workflow.description = metadata.get("description").cloned().unwrap_or_default();
            workflow.trigger = metadata.get("trigger").cloned().unwrap_or_default();
            workflow.quorum = metadata.get("quorum").cloned();
            i += metadata.len() + 1;
            continue;
        }

        // Parse EXTERN declaration
        if line.starts_with("EXTERN ") {
            let (extern_kernel, lines_consumed) = parse_extern(&lines, i, &reader)?;
            workflow.extern_kernels.push(extern_kernel);
            i += lines_consumed;
            continue;
        }

        // Parse KERNEL declaration
        if line.starts_with("KERNEL ") {
            let (kernel, lines_consumed) = parse_kernel(&lines, i, &reader)?;
            workflow.workflow_kernels.push(kernel);
            i += lines_consumed;
            continue;
        }

        // Parse EDGE declaration
        if line.starts_with("EDGE ") {
            let (edge, lines_consumed) = parse_edge(&lines, i, &reader)?;
            workflow.edges.push(edge);
            i += lines_consumed;
            continue;
        }

        i += 1;
    }

    // Analyze component origins
    workflow.analysis = analyze_components(&workflow);

    Ok(workflow)
}

/// Parse WORKFLOW header
fn parse_workflow_header(lines: &[&str], start: usize) -> Result<(String, HashMap<String, String>), OntologyError> {
    let first_line = lines[start].trim();
    let urn = first_line.strip_prefix("WORKFLOW ")
        .ok_or_else(|| OntologyError::ParseError("Invalid WORKFLOW declaration".to_string()))?
        .trim()
        .to_string();

    let mut metadata = HashMap::new();
    let mut i = start + 1;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() || line.starts_with("EXTERN") || line.starts_with("KERNEL") || line.starts_with("EDGE") || line.starts_with('#') {
            break;
        }

        if let Some(label) = line.strip_prefix("LABEL:") {
            metadata.insert("label".to_string(), label.trim().trim_matches('"').to_string());
        } else if let Some(desc) = line.strip_prefix("DESCRIPTION:") {
            metadata.insert("description".to_string(), desc.trim().trim_matches('"').to_string());
        } else if let Some(trigger) = line.strip_prefix("TRIGGER:") {
            metadata.insert("trigger".to_string(), trigger.trim().trim_matches('"').to_string());
        } else if let Some(quorum) = line.strip_prefix("QUORUM:") {
            metadata.insert("quorum".to_string(), quorum.trim().trim_matches('"').to_string());
        }

        i += 1;
    }

    Ok((urn, metadata))
}

/// Parse EXTERN declaration
fn parse_extern(lines: &[&str], start: usize, reader: &OntologyReader) -> Result<(ExternKernel, usize), OntologyError> {
    let first_line = lines[start].trim();
    let urn = first_line.strip_prefix("EXTERN ")
        .ok_or_else(|| OntologyError::ParseError("Invalid EXTERN declaration".to_string()))?
        .trim()
        .to_string();

    let mut role = None;
    let mut actions = Vec::new();
    let mut i = start + 1;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() || !line.starts_with(' ') {
            break;
        }

        if let Some(r) = line.strip_prefix("ROLE:") {
            role = Some(r.trim().trim_matches('"').to_string());
        } else if let Some(a) = line.strip_prefix("ACTION:") {
            actions.push(a.trim().trim_matches('"').to_string());
        }

        i += 1;
    }

    // Check if kernel exists in /concepts/
    let origin = check_kernel_origin(&urn, reader);

    Ok((
        ExternKernel {
            urn,
            role,
            actions,
            origin,
        },
        i - start,
    ))
}

/// Parse KERNEL declaration
fn parse_kernel(lines: &[&str], start: usize, reader: &OntologyReader) -> Result<(WorkflowKernel, usize), OntologyError> {
    let first_line = lines[start].trim();
    let urn = first_line.strip_prefix("KERNEL ")
        .ok_or_else(|| OntologyError::ParseError("Invalid KERNEL declaration".to_string()))?
        .trim()
        .to_string();

    let mut kernel_type = String::new();
    let mut runtime = None;
    let mut description = String::new();
    let mut capabilities = Vec::new();
    let mut actions = Vec::new();
    let mut i = start + 1;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() || (!line.starts_with(' ') && !line.starts_with('\t')) {
            break;
        }

        if let Some(t) = line.strip_prefix("TYPE:") {
            kernel_type = t.trim().to_string();
        } else if let Some(r) = line.strip_prefix("RUNTIME:") {
            runtime = Some(r.trim().trim_matches('"').to_string());
        } else if let Some(d) = line.strip_prefix("DESCRIPTION:") {
            description = d.trim().trim_matches('"').to_string();
        } else if line.trim().starts_with("CAPABILITIES:") {
            i += 1;
            while i < lines.len() && lines[i].trim().starts_with('-') {
                let cap = lines[i].trim().strip_prefix('-').unwrap().trim().trim_matches('"');
                capabilities.push(cap.to_string());
                i += 1;
            }
            continue;
        } else if line.trim().starts_with("ACTIONS:") {
            i += 1;
            while i < lines.len() && lines[i].trim().starts_with('-') {
                let action = lines[i].trim().strip_prefix('-').unwrap().trim();
                actions.push(action.to_string());
                i += 1;
            }
            continue;
        }

        i += 1;
    }

    // Check if kernel exists in /concepts/
    let origin = check_kernel_origin(&urn, reader);

    Ok((
        WorkflowKernel {
            urn,
            kernel_type,
            runtime,
            description,
            capabilities,
            actions,
            origin,
        },
        i - start,
    ))
}

/// Parse EDGE declaration
fn parse_edge(lines: &[&str], start: usize, reader: &OntologyReader) -> Result<(CkdlEdge, usize), OntologyError> {
    let first_line = lines[start].trim();
    let edge_urn = first_line.strip_prefix("EDGE ")
        .ok_or_else(|| OntologyError::ParseError("Invalid EDGE declaration".to_string()))?
        .trim()
        .to_string();

    // Parse edge URN format: ckp://Edge.PREDICATE.Source-to-Target
    let parts: Vec<&str> = edge_urn.split('.').collect();
    let predicate = if parts.len() >= 2 {
        parts[1].to_string()
    } else {
        String::new()
    };

    let mut source = String::new();
    let mut target = String::new();
    let mut trigger = String::new();
    let mut i = start + 1;

    while i < lines.len() {
        let line = lines[i].trim();
        if line.is_empty() || (!line.starts_with(' ') && !line.starts_with('\t')) {
            break;
        }

        if let Some(_p) = line.strip_prefix("PREDICATE:") {
            // predicate already extracted from URN
        } else if let Some(t) = line.strip_prefix("TRIGGER:") {
            trigger = t.trim().trim_matches('"').to_string();
        }

        i += 1;
    }

    // Extract source and target from edge URN
    if let Some(last_part) = parts.last() {
        let source_target: Vec<&str> = last_part.split("-to-").collect();
        if source_target.len() == 2 {
            source = source_target[0].to_string();
            target = source_target[1].to_string();
        }
    }

    // Check if both source and target kernels exist
    let origin = check_edge_origin(&source, &target, reader);

    Ok((
        CkdlEdge {
            edge_urn,
            source,
            target,
            predicate,
            trigger,
            origin,
        },
        i - start,
    ))
}

/// Check if kernel exists in /concepts/ (forked) or is brand new
fn check_kernel_origin(urn: &str, reader: &OntologyReader) -> ComponentOrigin {
    // Extract kernel name from URN (format: ckp://Kernel.Name:version)
    if let Some(kernel_part) = urn.strip_prefix("ckp://") {
        let kernel_name = kernel_part.split(':').next().unwrap_or(kernel_part);

        // Check if kernel exists (reader.read expects Path, use check_if_exists instead)
        let kernel_path = PathBuf::from(kernel_name);
        match reader.read(&kernel_path) {
            Ok(_) => {
                // Kernel exists - it's forked
                let path_str = format!("concepts/{}/", kernel_name.replace('.', "/"));
                let version = kernel_part.split(':').nth(1).unwrap_or("unknown").to_string();
                ComponentOrigin::Forked {
                    kernel_path: path_str,
                    version,
                }
            }
            Err(_) => {
                // Kernel doesn't exist - it's brand new
                ComponentOrigin::BrandNew
            }
        }
    } else {
        ComponentOrigin::External
    }
}

/// Check if edge connects existing kernels
fn check_edge_origin(source: &str, target: &str, reader: &OntologyReader) -> ComponentOrigin {
    let source_path = PathBuf::from(source);
    let target_path = PathBuf::from(target);
    let source_exists = reader.read(&source_path).is_ok();
    let target_exists = reader.read(&target_path).is_ok();

    if source_exists && target_exists {
        // Both kernels exist - edge connects forked components
        ComponentOrigin::Forked {
            kernel_path: format!("{} -> {}", source, target),
            version: "connected".to_string(),
        }
    } else {
        // At least one kernel is brand new
        ComponentOrigin::BrandNew
    }
}

/// Analyze component origins in workflow
fn analyze_components(workflow: &CkdlWorkflow) -> ComponentAnalysis {
    let mut forked = Vec::new();
    let mut brand_new = Vec::new();
    let mut external = Vec::new();

    // Analyze EXTERN kernels
    for extern_kernel in &workflow.extern_kernels {
        match &extern_kernel.origin {
            ComponentOrigin::Forked { .. } => forked.push(extern_kernel.urn.clone()),
            ComponentOrigin::BrandNew => brand_new.push(extern_kernel.urn.clone()),
            ComponentOrigin::External => external.push(extern_kernel.urn.clone()),
        }
    }

    // Analyze workflow kernels
    for kernel in &workflow.workflow_kernels {
        match &kernel.origin {
            ComponentOrigin::Forked { .. } => forked.push(kernel.urn.clone()),
            ComponentOrigin::BrandNew => brand_new.push(kernel.urn.clone()),
            ComponentOrigin::External => external.push(kernel.urn.clone()),
        }
    }

    ComponentAnalysis {
        total_extern: workflow.extern_kernels.len(),
        total_workflow_kernels: workflow.workflow_kernels.len(),
        total_edges: workflow.edges.len(),
        forked_kernels: forked,
        brand_new_kernels: brand_new,
        external_dependencies: external,
    }
}

/// Convert CKDL workflow to Workflow struct
pub fn ckdl_to_workflow(ckdl: CkdlWorkflow) -> Workflow {
    let trigger = if ckdl.trigger.contains("daemon-startup") {
        WorkflowTrigger::OnDaemonStartup
    } else if ckdl.trigger.contains("action-request") {
        WorkflowTrigger::OnActionRequest
    } else if ckdl.trigger.contains("schedule") {
        // Extract schedule interval
        let schedule = ckdl.trigger.split('(').nth(1)
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or("daily");
        WorkflowTrigger::OnSchedule(schedule.to_string())
    } else {
        WorkflowTrigger::OnEvent(ckdl.trigger.clone())
    };

    let phases = ckdl.workflow_kernels.iter().map(|k| {
        WorkflowPhase {
            phase_name: k.urn.split(':').next().unwrap_or(&k.urn).to_string(),
            kernel_urn: k.urn.clone(),
            status: PhaseStatus::Pending,
            started_at: None,
            completed_at: None,
        }
    }).collect();

    let edges = ckdl.edges.iter().map(|e| {
        WorkflowEdge {
            edge_urn: e.edge_urn.clone(),
            source: e.source.clone(),
            target: e.target.clone(),
            predicate: e.predicate.clone(),
            trigger: e.trigger.clone(),
            action: None,
        }
    }).collect();

    Workflow {
        workflow_urn: ckdl.workflow_urn,
        label: ckdl.label,
        description: ckdl.description,
        version: "1.0".to_string(),
        trigger,
        phases,
        edges,
        status: WorkflowStatus::Pending,
    }
}
