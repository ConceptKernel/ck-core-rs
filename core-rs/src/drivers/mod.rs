//! Drivers module for storage and transport operations (v1.3.20)
//!
//! ## Storage Drivers
//! - **FileSystemDriver**: Local filesystem storage (v1.3.19 synchronous, v1.3.20 async)
//! - **LocalStorage**: Async wrapper around FileSystemDriver (v1.3.20)
//! - **JenaStorage**: RDF triple store backend (v1.3.20)
//! - **HttpDriver**: Remote HTTP storage
//! - **GitDriver**: Git versioning for concept kernels
//! - **VersionDriver**: Unified versioning abstraction
//! - Future: AgeStorage, SeaweedFSStorage
//!
//! ## Transport Drivers
//! - **LocalTransport**: Filesystem + notify crate (v1.3.20)
//! - **NatsTransport**: NATS JetStream messaging (v1.3.20)
//! - Future: WebSocketTransport, GrpcTransport

mod traits;
mod filesystem;
mod http;
mod git;
pub mod version;

// v1.3.20: Storage and Transport driver implementations
pub mod storage;
pub mod transport;

// v1.3.20: Driver Factory
pub mod factory;

// Core trait exports
pub use traits::{
    // Storage traits
    StorageDriver, StorageDriverFactory, StorageLocation, JobFile, JobHandle,
    // Storage events (v1.3.20)
    StorageEvent, StorageEventStream,
    // Transport traits (v1.3.20)
    TransportDriver, JobMessage, ToolResponse, JobStream, ResultStream,
    // Edge routing (v1.3.20)
    EdgeMessage, EdgeMetadata, EdgeStream,
    // Execution types (v1.3.20)
    ExecutionMode, ToolDefinition, ResourceRequirements,
};

// Storage driver exports
pub use filesystem::FileSystemDriver;
pub use http::HttpDriver;
pub use git::{GitDriver, VersionBump};
pub use version::{VersionDriver, VersionInfo, VersionBackend, VersionDriverFactory, VersionedKernel};

// v1.3.20: Storage driver exports
pub use storage::{LocalStorage, JenaStorage};

// v1.3.20: Transport driver exports
pub use transport::{LocalTransport, NatsTransport};

// v1.3.20: Driver Factory exports
pub use factory::{DriverFactory, StorageConfig, TransportConfig, DriverConfig};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Test: StorageDriver trait and factory exports are accessible
    ///
    /// Verifies that the core storage driver abstractions are exported
    /// and can be used to implement custom storage backends.
    #[test]
    fn test_storage_driver_trait_exports() {
        // Verify StorageDriver trait is accessible via generic bounds
        #[allow(dead_code)]
        fn requires_storage_driver<T: StorageDriver>(_t: &T) {}

        // Verify StorageDriverFactory trait is accessible via generic bounds
        #[allow(dead_code)]
        fn requires_storage_driver_factory<T: StorageDriverFactory>() {}

        // Verify StorageLocation enum is accessible
        fn accepts_location(_: StorageLocation) {}
        accepts_location(StorageLocation::Local(PathBuf::from("/tmp")));
        accepts_location(StorageLocation::Remote("http://example.com".to_string()));
        accepts_location(StorageLocation::Urn("ckp://Test.Kernel".to_string()));

        // If this compiles, trait and factory exports are correct
    }

    /// Test: JobFile and JobHandle exports are accessible
    ///
    /// Verifies that job management types are exported for queue operations.
    #[test]
    fn test_job_types_exports() {
        use chrono::Utc;

        // Verify JobFile struct is accessible
        fn accepts_job_file(_: JobFile) {}

        let job_file = JobFile {
            target: "Test.Kernel".to_string(),
            payload: serde_json::json!({}),
            timestamp: Utc::now().to_rfc3339(),
            tx_id: "20251129-abc123".to_string(),
            source: "external".to_string(),
        };

        accepts_job_file(job_file.clone());

        // Verify JobHandle struct is accessible (opaque struct with methods)
        fn accepts_job_handle(_: JobHandle) {}

        let job_handle = JobHandle {
            tx_id: "20251129-abc123".to_string(),
            content: job_file,
            storage_id: "storage-id-123".to_string(),
        };

        accepts_job_handle(job_handle);

        // If this compiles, job type exports are correct
    }

    /// Test: FileSystemDriver and HttpDriver exports are accessible
    ///
    /// Verifies that concrete storage driver implementations are exported
    /// and can be constructed for local and remote storage operations.
    #[test]
    fn test_driver_implementations_exports() {
        // Verify FileSystemDriver is accessible
        fn accepts_fs_driver(_: FileSystemDriver) {}
        let fs_driver = FileSystemDriver::new(
            PathBuf::from("/tmp"),
            "Test.Kernel".to_string()
        );
        accepts_fs_driver(fs_driver);

        // Verify HttpDriver is accessible
        fn accepts_http_driver(_: HttpDriver) {}
        let http_driver = HttpDriver::new("http://example.com".to_string());
        accepts_http_driver(http_driver);

        // If this compiles, driver implementation exports are correct
    }

    /// Test: GitDriver and VersionBump exports are accessible
    ///
    /// Verifies that Git versioning driver is exported and can be used
    /// for managing kernel versions with git tags and commits.
    #[test]
    fn test_git_driver_exports() {
        // Verify GitDriver is accessible
        fn accepts_git_driver(_: GitDriver) {}
        let git_driver = GitDriver::new(
            PathBuf::from("/tmp/kernel"),
            "Test.Kernel".to_string()
        );
        accepts_git_driver(git_driver);

        // Verify VersionBump enum is accessible
        fn accepts_version_bump(_: VersionBump) {}
        accepts_version_bump(VersionBump::Major);
        accepts_version_bump(VersionBump::Minor);
        accepts_version_bump(VersionBump::Patch);

        // If this compiles, git driver exports are correct
    }

    /// Test: VersionDriver trait and types are accessible
    ///
    /// Verifies that unified versioning abstraction is exported and can be used
    /// to implement version drivers for different backends (Git, S3, Postgres, etc.).
    #[test]
    fn test_version_driver_trait_exports() {
        // Verify VersionDriver trait is accessible
        #[allow(dead_code)]
        fn requires_version_driver<T: VersionDriver>() {}

        // Verify VersionInfo struct is accessible
        fn accepts_version_info(_: VersionInfo) {}
        let version_info = VersionInfo {
            version: "v1.0.0".to_string(),
            is_clean: true,
            metadata: None,
            backend: VersionBackend::Git,
        };
        accepts_version_info(version_info);

        // Verify VersionBackend enum is accessible
        fn accepts_backend(_: VersionBackend) {}
        accepts_backend(VersionBackend::Git);
        accepts_backend(VersionBackend::S3);
        accepts_backend(VersionBackend::Postgres);
        accepts_backend(VersionBackend::Filesystem);
        accepts_backend(VersionBackend::None);

        // Verify VersionDriverFactory is accessible
        fn accepts_factory_detect(_: fn(&PathBuf, &str) -> Option<Box<dyn VersionDriver>>) {}
        accepts_factory_detect(VersionDriverFactory::detect);

        // If this compiles, version driver exports are correct
    }

    /// Test: VersionedKernel trait is accessible
    ///
    /// Verifies that VersionedKernel extension trait is exported and can be
    /// implemented on kernel types to provide version management.
    #[test]
    fn test_versioned_kernel_trait_export() {
        // Verify VersionedKernel trait is accessible
        #[allow(dead_code)]
        fn requires_versioned_kernel<T: VersionedKernel>() {}

        // If this compiles, trait export is correct
    }
}
