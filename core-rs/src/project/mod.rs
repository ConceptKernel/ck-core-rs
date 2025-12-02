/**
 * project module
 * Multi-project infrastructure components (v1.3.14)
 */

pub mod config;
pub mod registry;

pub use config::{DefaultUser, Features, Metadata, OntologyConfig, PortConfig, ProjectConfig, ProtocolMapping, Spec};
pub use registry::{ProjectEntry, ProjectInfo, ProjectRegistry};

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: ProjectConfig types are exported
    ///
    /// Verifies that .ckproject configuration types are exported for
    /// multi-project management and configuration loading.
    #[test]
    fn test_project_config_exports() {
        // Verify ProjectConfig type is accessible via Option
        fn accepts_project_config(_: Option<ProjectConfig>) {}
        accepts_project_config(None);

        // Verify related config types are accessible
        fn accepts_metadata(_: Option<Metadata>) {}
        fn accepts_spec(_: Option<Spec>) {}
        fn accepts_features(_: Option<Features>) {}
        fn accepts_port_config(_: Option<PortConfig>) {}

        accepts_metadata(None);
        accepts_spec(None);
        accepts_features(None);
        accepts_port_config(None);

        // If this compiles, exports are correct
    }

    /// Test: ProjectRegistry types are exported
    ///
    /// Verifies that multi-project registry types are exported for
    /// managing multiple ConceptKernel projects with isolation.
    #[test]
    fn test_project_registry_exports() {
        // Verify ProjectRegistry type is accessible via Option
        fn accepts_registry(_: Option<ProjectRegistry>) {}
        accepts_registry(None);

        // Verify ProjectInfo and ProjectEntry types are accessible
        fn accepts_project_info(_: Option<ProjectInfo>) {}
        fn accepts_project_entry(_: Option<ProjectEntry>) {}

        accepts_project_info(None);
        accepts_project_entry(None);

        // If this compiles, exports are correct
    }
}
