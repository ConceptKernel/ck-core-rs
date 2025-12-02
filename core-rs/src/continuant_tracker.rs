//! BFO Continuant Tracking (Phase 4 Stage 2)
//!
//! Tracks entities that persist through time (BFO Continuants):
//! - Material Entities (Kernels, Agents)
//! - Realizable Entities (Roles, Functions, Dispositions)
//! - Temporal Participation (when Continuants participate in Processes)
//!
//! This complements ProcessTracker (Occurrents) with persistent entity tracking.

use crate::errors::{CkpError, Result};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// ContinuantTracker - Tracks BFO Continuants (persistent entities)
pub struct ContinuantTracker {
    /// Root concepts directory
    concepts_root: PathBuf,
}

impl ContinuantTracker {
    /// Create new tracker
    pub fn new(concepts_root: PathBuf) -> Self {
        Self { concepts_root }
    }

    /// Generate Continuant URN
    ///
    /// Format: `ckp://Continuant#{type}-{identifier}`
    ///
    /// Examples:
    /// - `ckp://Continuant#Kernel-System.Gateway`
    /// - `ckp://Continuant#Agent-user:alice`
    /// - `ckp://Continuant#Role-admin`
    pub fn generate_continuant_urn(&self, continuant_type: &str, identifier: &str) -> String {
        format!("ckp://Continuant#{}-{}", continuant_type, identifier)
    }

    /// Create a Material Entity (Kernel)
    ///
    /// BFO Material Entity (bfo:0000040) - Physical or digital entity
    ///
    /// Examples:
    /// - Kernel instance
    /// - File system entity
    /// - Storage artifact
    pub fn create_kernel_entity(
        &self,
        kernel_name: &str,
        version: &str,
        kernel_type: &str,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Result<KernelEntity> {
        let urn = self.generate_continuant_urn("Kernel", kernel_name);
        let created_at = chrono::Utc::now().to_rfc3339();

        let entity = KernelEntity {
            urn: urn.clone(),
            kernel_name: kernel_name.to_string(),
            version: version.to_string(),
            kernel_type: kernel_type.to_string(),
            created_at: created_at.clone(),
            bfo_type: crate::ontology::BfoEntityType::MaterialEntity,
            roles: Vec::new(),
            functions: Vec::new(),
            participations: Vec::new(),
            metadata,
        };

        // Store to disk
        self.store_kernel_entity(&entity)?;

        Ok(entity)
    }

    /// Create an Agent (User or System)
    ///
    /// BFO Material Entity → ckp:Agent
    ///
    /// Examples:
    /// - User: alice, bob
    /// - System: ConceptKernel runtime
    pub fn create_agent(
        &self,
        agent_type: &str,  // "User" or "System"
        identifier: &str,
        roles: Vec<Role>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Result<Agent> {
        let urn = self.generate_continuant_urn("Agent", identifier);
        let created_at = chrono::Utc::now().to_rfc3339();

        let agent = Agent {
            urn: urn.clone(),
            agent_type: agent_type.to_string(),
            identifier: identifier.to_string(),
            created_at: created_at.clone(),
            roles,
            participations: Vec::new(),
            metadata,
        };

        // Store to disk
        self.store_agent(&agent)?;

        Ok(agent)
    }

    /// Assign Role to Kernel or Agent
    ///
    /// BFO Realizable Entity → Role
    ///
    /// Roles are things entities can *bear* (e.g., "admin", "voter", "gateway")
    pub fn assign_role(
        &self,
        continuant_urn: &str,
        role: Role,
    ) -> Result<()> {
        // Load continuant
        if continuant_urn.contains("Kernel-") {
            let mut entity = self.load_kernel_entity_by_urn(continuant_urn)?;
            entity.roles.push(role);
            self.store_kernel_entity(&entity)?;
        } else if continuant_urn.contains("Agent-") {
            let mut agent = self.load_agent_by_urn(continuant_urn)?;
            agent.roles.push(role);
            self.store_agent(&agent)?;
        } else {
            return Err(CkpError::ValidationError(format!(
                "Unknown continuant type: {}",
                continuant_urn
            )));
        }

        Ok(())
    }

    /// Assign Function to Kernel
    ///
    /// BFO Realizable Entity → Function
    ///
    /// Functions are what entities are *designed to do* (e.g., "consensus", "gateway", "storage")
    pub fn assign_function(
        &self,
        kernel_urn: &str,
        function: Function,
    ) -> Result<()> {
        let mut entity = self.load_kernel_entity_by_urn(kernel_urn)?;
        entity.functions.push(function);
        self.store_kernel_entity(&entity)?;

        Ok(())
    }

    /// Record participation in a Process
    ///
    /// BFO Relation: Continuant participates_in Occurrent
    ///
    /// Links persistent entities (Kernels, Agents) to temporal processes
    pub fn record_participation(
        &self,
        continuant_urn: &str,
        process_urn: &str,
        role_in_process: &str,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let timestamp = chrono::Utc::now().to_rfc3339();

        let participation = Participation {
            process_urn: process_urn.to_string(),
            role_in_process: role_in_process.to_string(),
            timestamp: timestamp.clone(),
            metadata,
        };

        // Add to continuant
        if continuant_urn.contains("Kernel-") {
            let mut entity = self.load_kernel_entity_by_urn(continuant_urn)?;
            entity.participations.push(participation);
            self.store_kernel_entity(&entity)?;
        } else if continuant_urn.contains("Agent-") {
            let mut agent = self.load_agent_by_urn(continuant_urn)?;
            agent.participations.push(participation);
            self.store_agent(&agent)?;
        }

        Ok(())
    }

    /// Query kernel entities by role
    pub fn query_kernels_by_role(&self, role_name: &str) -> Result<Vec<KernelEntity>> {
        let storage_dir = self.concepts_root.join(".continuants").join("kernels");
        if !storage_dir.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        for entry in fs::read_dir(&storage_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                let entity: KernelEntity = serde_json::from_str(&content)?;

                // Check if entity has this role
                if entity.roles.iter().any(|r| r.name == role_name) {
                    results.push(entity);
                }
            }
        }

        Ok(results)
    }

    /// Query agents by role
    pub fn query_agents_by_role(&self, role_name: &str) -> Result<Vec<Agent>> {
        let storage_dir = self.concepts_root.join(".continuants").join("agents");
        if !storage_dir.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        for entry in fs::read_dir(&storage_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                let agent: Agent = serde_json::from_str(&content)?;

                if agent.roles.iter().any(|r| r.name == role_name) {
                    results.push(agent);
                }
            }
        }

        Ok(results)
    }

    /// Query participations for a process
    pub fn query_participants(&self, process_urn: &str) -> Result<Vec<String>> {
        let mut participants = Vec::new();

        // Check kernels
        let kernel_storage = self.concepts_root.join(".continuants").join("kernels");
        if kernel_storage.exists() {
            for entry in fs::read_dir(&kernel_storage)? {
                let entry = entry?;
                let content = fs::read_to_string(entry.path())?;
                let entity: KernelEntity = serde_json::from_str(&content)?;

                if entity.participations.iter().any(|p| p.process_urn == process_urn) {
                    participants.push(entity.urn.clone());
                }
            }
        }

        // Check agents
        let agent_storage = self.concepts_root.join(".continuants").join("agents");
        if agent_storage.exists() {
            for entry in fs::read_dir(&agent_storage)? {
                let entry = entry?;
                let content = fs::read_to_string(entry.path())?;
                let agent: Agent = serde_json::from_str(&content)?;

                if agent.participations.iter().any(|p| p.process_urn == process_urn) {
                    participants.push(agent.urn.clone());
                }
            }
        }

        Ok(participants)
    }

    /// Get roles assigned to a specific kernel (Phase 4 Stage 3 - RBAC integration)
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name (e.g., "System.Gateway.HTTP")
    ///
    /// # Returns
    /// Vector of Role entities assigned to the kernel
    pub fn get_kernel_roles(&self, kernel_name: &str) -> Result<Vec<Role>> {
        let storage_dir = self.concepts_root.join(".continuants").join("kernels");
        let filename = format!("{}.json", kernel_name.replace("/", "_"));
        let path = storage_dir.join(filename);

        if !path.exists() {
            // Kernel entity not found - return empty roles
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&path)?;
        let entity: KernelEntity = serde_json::from_str(&content)?;

        Ok(entity.roles)
    }

    // ========================================================================
    // PRIVATE STORAGE METHODS
    // ========================================================================

    fn store_kernel_entity(&self, entity: &KernelEntity) -> Result<()> {
        let storage_dir = self.concepts_root.join(".continuants").join("kernels");
        fs::create_dir_all(&storage_dir)?;

        let filename = format!("{}.json", entity.kernel_name.replace("/", "_"));
        let path = storage_dir.join(filename);

        let json = serde_json::to_string_pretty(entity)?;
        fs::write(path, json)?;

        Ok(())
    }

    fn store_agent(&self, agent: &Agent) -> Result<()> {
        let storage_dir = self.concepts_root.join(".continuants").join("agents");
        fs::create_dir_all(&storage_dir)?;

        let filename = format!("{}.json", agent.identifier.replace("/", "_").replace(":", "_"));
        let path = storage_dir.join(filename);

        let json = serde_json::to_string_pretty(agent)?;
        fs::write(path, json)?;

        Ok(())
    }

    fn load_agent_by_urn(&self, urn: &str) -> Result<Agent> {
        let identifier = urn.split("Agent-").nth(1)
            .ok_or_else(|| CkpError::ParseError(format!("Invalid agent URN: {}", urn)))?;

        let storage_dir = self.concepts_root.join(".continuants").join("agents");
        let filename = format!("{}.json", identifier.replace("/", "_").replace(":", "_"));
        let path = storage_dir.join(filename);

        let content = fs::read_to_string(&path)
            .map_err(|_| CkpError::ValidationError(format!("Agent not found: {}", identifier)))?;

        let agent: Agent = serde_json::from_str(&content)?;
        Ok(agent)
    }

    /// List all kernel entities
    ///
    /// Returns all KernelEntity records from `.continuants/kernels/` directory.
    /// If directory doesn't exist (no kernels started yet), returns empty vec.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ckp_core::ContinuantTracker;
    /// use std::path::PathBuf;
    ///
    /// let tracker = ContinuantTracker::new(PathBuf::from("/project/concepts"));
    /// let entities = tracker.list_kernel_entities().unwrap();
    ///
    /// for entity in entities {
    ///     println!("{}: {} ({})",
    ///         entity.kernel_name,
    ///         entity.version,
    ///         entity.bfo_type.label()
    ///     );
    /// }
    /// ```
    pub fn list_kernel_entities(&self) -> Result<Vec<KernelEntity>> {
        let storage_dir = self.concepts_root.join(".continuants").join("kernels");

        if !storage_dir.exists() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();

        for entry in fs::read_dir(&storage_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = fs::read_to_string(&path)?;
                let entity: KernelEntity = serde_json::from_str(&content)?;
                results.push(entity);
            }
        }

        // Sort by kernel_name for consistent ordering
        results.sort_by(|a, b| a.kernel_name.cmp(&b.kernel_name));

        Ok(results)
    }

    /// Get kernel entity by name
    ///
    /// Returns a specific KernelEntity by kernel name using URN resolution.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ckp_core::ContinuantTracker;
    /// use std::path::PathBuf;
    ///
    /// let tracker = ContinuantTracker::new(PathBuf::from("/project/concepts"));
    /// let entity = tracker.get_kernel_entity("System.Gateway").unwrap();
    /// println!("Found: {}", entity.kernel_name);
    /// ```
    pub fn get_kernel_entity(&self, kernel_name: &str) -> Result<KernelEntity> {
        // Generate the Continuant URN
        let urn = self.generate_continuant_urn("Kernel", kernel_name);

        // Load using the internal helper that uses URN resolution
        self.load_kernel_entity_by_urn(&urn)
    }

    /// Internal helper: Load kernel entity by URN
    fn load_kernel_entity_by_urn(&self, urn: &str) -> Result<KernelEntity> {
        // Extract kernel name from URN (format: ckp://Continuant#Kernel-{name})
        let kernel_name = urn.split("Kernel-").nth(1)
            .ok_or_else(|| CkpError::ParseError(format!("Invalid kernel URN: {}", urn)))?;

        let storage_dir = self.concepts_root.join(".continuants").join("kernels");
        let filename = format!("{}.json", kernel_name);
        let path = storage_dir.join(filename);

        if !path.exists() {
            return Err(CkpError::ValidationError(format!(
                "Kernel entity not found: {}",
                kernel_name
            )));
        }

        let content = fs::read_to_string(&path)?;
        let entity: KernelEntity = serde_json::from_str(&content)?;
        Ok(entity)
    }
}

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Kernel Entity (BFO Material Entity → ckp:Kernel)
///
/// A persistent entity that processes jobs and maintains state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelEntity {
    /// Continuant URN (ckp://Continuant#Kernel-{name})
    pub urn: String,

    /// Kernel name (e.g., "System.Gateway")
    pub kernel_name: String,

    /// Version (from ontology or git)
    pub version: String,

    /// Kernel type (rust:hot, rust:cold, node:hot, node:cold)
    pub kernel_type: String,

    /// Creation timestamp
    pub created_at: String,

    /// BFO classification (always MaterialEntity for kernels)
    #[serde(default = "default_kernel_bfo_type")]
    pub bfo_type: crate::ontology::BfoEntityType,

    /// Roles this kernel bears (BFO Roles)
    pub roles: Vec<Role>,

    /// Functions this kernel realizes (BFO Functions)
    pub functions: Vec<Function>,

    /// Processes this kernel has participated in
    pub participations: Vec<Participation>,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Default BFO type for kernels (Material Entity)
fn default_kernel_bfo_type() -> crate::ontology::BfoEntityType {
    crate::ontology::BfoEntityType::MaterialEntity
}

impl crate::ontology::BfoAligned for KernelEntity {
    fn bfo_type(&self) -> crate::ontology::BfoEntityType {
        self.bfo_type
    }
}

/// Agent (BFO Material Entity → ckp:Agent)
///
/// A user or system entity that can initiate actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Continuant URN (ckp://Continuant#Agent-{identifier})
    pub urn: String,

    /// Agent type: "User" or "System"
    pub agent_type: String,

    /// Identifier (e.g., "alice", "system:runtime")
    pub identifier: String,

    /// Creation timestamp
    pub created_at: String,

    /// Roles this agent bears
    pub roles: Vec<Role>,

    /// Processes this agent has participated in
    pub participations: Vec<Participation>,

    /// Additional metadata (email, permissions, etc.)
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Role (BFO Realizable Entity)
///
/// Something an entity can *bear* (e.g., "admin", "voter", "gateway-operator")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    /// Role name (e.g., "admin", "consensus-voter")
    pub name: String,

    /// Role description
    pub description: String,

    /// When this role was assigned
    pub assigned_at: String,

    /// Role-specific metadata (permissions, scope, etc.)
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Function (BFO Realizable Entity)
///
/// What an entity is *designed to do* (e.g., "consensus", "gateway", "storage")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    /// Function name (e.g., "gateway", "consensus")
    pub name: String,

    /// Function description
    pub description: String,

    /// When this function was assigned/recognized
    pub assigned_at: String,

    /// Function-specific metadata (capabilities, contracts, etc.)
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Participation (BFO Relation: participates_in)
///
/// Records when a Continuant participates in an Occurrent (Process)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participation {
    /// Process URN this entity participated in
    pub process_urn: String,

    /// Role in this specific process (e.g., "source", "target", "validator")
    pub role_in_process: String,

    /// When participation occurred
    pub timestamp: String,

    /// Participation-specific metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Disposition (BFO Realizable Entity)
///
/// Tendency of an entity to behave in a certain way
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disposition {
    /// Disposition name (e.g., "always-validates", "caches-results")
    pub name: String,

    /// Description
    pub description: String,

    /// When recognized
    pub recognized_at: String,

    /// Disposition metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_kernel_entity() {
        let temp_dir = TempDir::new().unwrap();
        let tracker = ContinuantTracker::new(temp_dir.path().to_path_buf());

        let mut metadata = HashMap::new();
        metadata.insert("port".to_string(), serde_json::json!(56000));

        let entity = tracker.create_kernel_entity(
            "System.Gateway",
            "v1.3.16",
            "rust:hot",
            metadata,
        ).unwrap();

        assert_eq!(entity.kernel_name, "System.Gateway");
        assert_eq!(entity.version, "v1.3.16");
        assert!(entity.urn.contains("Continuant#Kernel-System.Gateway"));
    }

    #[test]
    fn test_assign_role() {
        let temp_dir = TempDir::new().unwrap();
        let tracker = ContinuantTracker::new(temp_dir.path().to_path_buf());

        let entity = tracker.create_kernel_entity(
            "System.Consensus",
            "v1.0",
            "rust:hot",
            HashMap::new(),
        ).unwrap();

        let role = Role {
            name: "consensus-voter".to_string(),
            description: "Can vote on proposals".to_string(),
            assigned_at: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        };

        tracker.assign_role(&entity.urn, role).unwrap();

        // Reload and verify
        let reloaded = tracker.load_kernel_entity_by_urn(&entity.urn).unwrap();
        assert_eq!(reloaded.roles.len(), 1);
        assert_eq!(reloaded.roles[0].name, "consensus-voter");
    }

    #[test]
    fn test_query_by_role() {
        let temp_dir = TempDir::new().unwrap();
        let tracker = ContinuantTracker::new(temp_dir.path().to_path_buf());

        // Create kernel with role
        let entity = tracker.create_kernel_entity(
            "System.Gateway",
            "v1.0",
            "rust:hot",
            HashMap::new(),
        ).unwrap();

        let role = Role {
            name: "gateway".to_string(),
            description: "HTTP gateway".to_string(),
            assigned_at: chrono::Utc::now().to_rfc3339(),
            metadata: HashMap::new(),
        };

        tracker.assign_role(&entity.urn, role).unwrap();

        // Query
        let results = tracker.query_kernels_by_role("gateway").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kernel_name, "System.Gateway");
    }
}
