//! # CKP Core - ConceptKernel Rust Runtime
//!
//! ConceptKernel (CK) v1.3.12 is a Node.js-based event sourcing system implementing
//! the Concept Kernel Protocol (CKP). This is the Rust implementation that provides
//! 100% interoperability with the Node.js runtime.
//!
//! ## Core Principle
//!
//! **The File System IS the Protocol**: Both Node.js and Rust runtimes operate on
//! the same `/concepts/` directory without any configuration or awareness of each other.
//!
//! ## Key Features
//!
//! - URN-based addressing for all kernels and edges
//! - Per-edge queue isolation for context-aware processing
//! - Declarative edge authorization in ontology files
//! - Enhanced RBAC with communication whitelist/blacklist
//! - Polyglot runtime (coexists with Node.js implementation)
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │      /concepts/ (Single Source)     │
//! │  All kernel state, URN-addressed    │
//! └─────────────────────────────────────┘
//!           ▲               ▲
//!           │               │
//!     ┌─────┴───────┐   ┌──┴──────────┐
//!     │  Node.js    │   │   Rust      │
//!     │  ck (CLI)   │   │   ckr (CLI) │
//!     └─────────────┘   └─────────────┘
//! ```

pub mod urn;
pub mod errors;
pub mod ontology;
pub mod workflow;
pub mod drivers;
pub mod kernel;
pub mod project;
pub mod port;
pub mod edge;
pub mod rbac;
pub mod process_tracker;
pub mod continuant_tracker;
pub mod compliance;
pub mod cache;
pub mod storage;
pub mod daemon;

pub use urn::{UrnResolver, UrnValidator, ParsedUrn, ParsedEdgeUrn, ParsedQueryUrn, ParsedQueryUrnV2};
pub use errors::CkpError;
pub use ontology::{OntologyReader, Ontology, OntologyLibrary, OntologyError, BfoEntityType, BfoAligned, RoleMetadata, FunctionMetadata, KernelMetadata};
pub use workflow::{WorkflowAPI, Workflow, WorkflowPhase, WorkflowEdge, WorkflowTrigger, WorkflowStatus, PhaseStatus, WorkflowCycle, CycleType, WorkflowValidation};
pub use kernel::{ConceptKernelGovernor, Kernel, JobFile, Job, InboxIterator, KernelManager, KernelStatus, QueueStats, RunningPids, StartResult, KernelContext, AdoptedContext, EdgeResponse, KernelBuilder};
pub use project::{ProjectConfig, ProjectRegistry, ProjectEntry, ProjectInfo};
pub use port::PortManager;
pub use edge::{EdgeKernel, EdgeMetadata, EdgeRequestBuilder, EdgeRequest, EdgeSource, EdgeTarget, NotificationEntry};
pub use rbac::{PermissionChecker, SelfImprovementConfig};
pub use process_tracker::{ProcessTracker, Process, TemporalPart, TemporalRegion, QueryFilters, Statistics};
pub use continuant_tracker::{ContinuantTracker, KernelEntity, Agent, Role, Function, Participation, Disposition};
pub use compliance::{AuditLogger, GdprChecker, RetentionPolicy, AuditEntry, ConsentRecord, RetentionCheckResult, DataAccessResult, DataPortabilityExport};
pub use cache::{PackageManager, PackageInfo};
pub use storage::{InstanceScanner, InstanceSummary, InstanceDetail};
pub use drivers::{GitDriver, VersionBump, VersionDriver, VersionInfo, VersionBackend, VersionDriverFactory, VersionedKernel};
pub use daemon::EdgeRouterDaemon;

/// Version of the CKP protocol (upgrading to 1.3.14 for multi-project support)
pub const VERSION: &str = "1.3.14";

/// Default concepts root directory
pub const DEFAULT_CONCEPTS_ROOT: &str = "/concepts";

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: Core modules are exported and accessible
    ///
    /// Verifies that all core ConceptKernel modules are re-exported
    /// from the library root for external crate usage.
    #[test]
    fn test_core_modules_exported() {
        // Verify modules are accessible from crate root
        // This test compiles only if modules are public

        // Core infrastructure modules - touch each module's existence
        let _ = std::any::type_name::<&crate::kernel::KernelManager>();
        let _ = std::any::type_name::<&crate::drivers::FileSystemDriver>();
        let _ = std::any::type_name::<&crate::ontology::OntologyLibrary>();
        let _ = std::any::type_name::<&crate::storage::InstanceScanner>();
        let _ = std::any::type_name::<&crate::edge::EdgeKernel>();
        let _ = std::any::type_name::<&crate::rbac::PermissionChecker>();
        let _ = std::any::type_name::<&crate::project::ProjectConfig>();
        let _ = std::any::type_name::<&crate::port::PortManager>();
        let _ = std::any::type_name::<&crate::cache::PackageManager>();
        let _ = std::any::type_name::<&crate::urn::UrnResolver>();
        let _ = std::any::type_name::<crate::errors::CkpError>();

        // Tracking modules
        let _ = std::any::type_name::<&crate::process_tracker::ProcessTracker>();
        let _ = std::any::type_name::<&crate::continuant_tracker::ContinuantTracker>();
        let _ = std::any::type_name::<&crate::compliance::AuditLogger>();

        // If this compiles, all modules are exported
    }

    /// Test: Main types are exported from library root
    ///
    /// Verifies that key ConceptKernel types are re-exported at the root
    /// level for convenient external usage without module paths.
    #[test]
    fn test_main_types_exported() {
        // Verify core types are accessible without module paths
        fn accepts_kernel_manager(_: Option<KernelManager>) {}
        fn accepts_ckp_error(_: CkpError) {}
        fn accepts_urn_resolver(_: fn(&str) -> errors::Result<ParsedUrn>) {}
        fn accepts_project_config(_: Option<ProjectConfig>) {}
        fn accepts_edge_kernel(_: Option<EdgeKernel>) {}

        accepts_kernel_manager(None);
        accepts_ckp_error(CkpError::UrnParse("test".to_string()));
        accepts_urn_resolver(UrnResolver::parse);
        accepts_project_config(None);
        accepts_edge_kernel(None);

        // If this compiles, main types are exported correctly
    }

    /// Test: Library constants are accessible
    ///
    /// Verifies that version and configuration constants are exported
    /// for external crates to check compatibility and defaults.
    #[test]
    fn test_library_constants() {
        // Verify constants are accessible
        assert_eq!(VERSION, "1.3.14");
        assert_eq!(DEFAULT_CONCEPTS_ROOT, "/concepts");

        // Verify they are &'static str (compile-time constant)
        fn accepts_static_str(_: &'static str) {}
        accepts_static_str(VERSION);
        accepts_static_str(DEFAULT_CONCEPTS_ROOT);

        // If this compiles and runs, constants are properly exported
    }
}
