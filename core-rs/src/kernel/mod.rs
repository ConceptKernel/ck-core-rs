//! Kernel module for governor and process management

mod governor;
mod pid;
mod kernel;
mod manager;
mod builder;
pub mod api;

pub use governor::ConceptKernelGovernor;
pub use pid::PidFile;
pub use kernel::{Kernel, JobFile, Job, InboxIterator};
pub use manager::{KernelManager, KernelStatus, QueueStats, RunningPids, StartResult};
pub use builder::KernelBuilder;
pub use api::{KernelContext, AdoptedContext, EdgeResponse};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_kernel_manager_is_exported() {
        // Verify KernelManager type is accessible via public API
        let temp = TempDir::new().unwrap();
        let _manager: KernelManager = KernelManager::new(temp.path().to_path_buf()).unwrap();
        // If this compiles, export works correctly
    }

    #[test]
    fn test_kernel_is_exported() {
        // Verify Kernel type is accessible via public API
        let temp = TempDir::new().unwrap();
        let _kernel: Kernel = Kernel::new(
            temp.path().to_path_buf(),
            Some("Test".to_string()),
            false
        );
        // If this compiles, export works correctly
    }

    #[test]
    fn test_kernel_builder_type_is_exported() {
        // Verify KernelBuilder type is accessible via public API
        // We don't construct it since we don't know the constructor signature
        // Just verify the type is exported
        fn accepts_kernel_builder(_: KernelBuilder) {}
        // If this compiles, export works correctly
    }

    #[test]
    fn test_kernel_status_types_are_exported() {
        // Verify status-related types are accessible
        // We don't construct them since fields may be private
        // Just verify the types are exported

        fn accepts_kernel_status(_: KernelStatus) {}
        fn accepts_queue_stats(_: QueueStats) {}
        fn accepts_running_pids(_: RunningPids) {}
        fn accepts_start_result(_: StartResult) {}

        // If this compiles, all status types are exported correctly
    }

    #[test]
    fn test_api_types_are_exported() {
        // Verify KernelContext and related API types are accessible
        fn accepts_kernel_context(_: KernelContext) {}
        fn accepts_adopted_context(_: AdoptedContext) {}
        fn accepts_edge_response(_: EdgeResponse) {}

        // If this compiles, all API types are exported correctly
    }
}
