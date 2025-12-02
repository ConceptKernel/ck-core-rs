//! Version driver abstraction for ConceptKernel
//!
//! Provides unified versioning interface that can be implemented by:
//! - GitDriver (local git repositories)
//! - S3Driver (S3 object versioning)
//! - PostgresDriver (database-backed versioning)
//! - FilesystemDriver (manual version files)
//!
//! ## Design Principle
//!
//! Every concept kernel should have versioning capability regardless of
//! storage backend. The VersionDriver trait provides a unified API so
//! kernel creation tools can automatically set up versioning.

use crate::errors::{CkpError, Result};
use std::path::PathBuf;

/// Version information returned by version drivers
#[derive(Debug, Clone, PartialEq)]
pub struct VersionInfo {
    /// Current version string (e.g., "v0.2.0" or "v0.2.3-gab12cd")
    pub version: String,

    /// Whether this is a clean version (no uncommitted changes)
    pub is_clean: bool,

    /// Optional: Additional metadata (commit hash, S3 version ID, etc.)
    pub metadata: Option<String>,

    /// Backend type that provided this version
    pub backend: VersionBackend,
}

/// Version backend types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VersionBackend {
    Git,
    S3,
    Postgres,
    Filesystem,
    None,
}

impl std::fmt::Display for VersionBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionBackend::Git => write!(f, "git"),
            VersionBackend::S3 => write!(f, "s3"),
            VersionBackend::Postgres => write!(f, "postgres"),
            VersionBackend::Filesystem => write!(f, "filesystem"),
            VersionBackend::None => write!(f, "none"),
        }
    }
}

/// Unified version driver interface
pub trait VersionDriver {
    /// Get current version
    ///
    /// Returns None if no versioning is set up yet
    fn get_version(&self) -> Result<Option<VersionInfo>>;

    /// Initialize versioning for this kernel
    ///
    /// For git: runs `git init`, sets up config
    /// For S3: enables versioning on bucket
    /// For filesystem: creates version file
    fn init(&self) -> Result<()>;

    /// Check if versioning is initialized
    fn is_initialized(&self) -> bool;

    /// Create a new version
    ///
    /// # Arguments
    /// * `message` - Version message (commit message, tag annotation, etc.)
    ///
    /// # Returns
    /// The new version string created
    fn create_version(&self, message: &str) -> Result<String>;

    /// List all versions
    fn list_versions(&self) -> Result<Vec<String>>;

    /// Get backend type
    fn backend_type(&self) -> VersionBackend;
}

/// Factory for creating version drivers based on kernel location
pub struct VersionDriverFactory;

impl VersionDriverFactory {
    /// Detect and create appropriate version driver for a kernel
    ///
    /// Detection order:
    /// 1. Check for .git directory → GitDriver
    /// 2. Check for S3 marker file → S3Driver
    /// 3. Check for .version file → FilesystemDriver
    /// 4. Return None (no versioning)
    pub fn detect(kernel_path: &PathBuf, kernel_name: &str) -> Option<Box<dyn VersionDriver>> {
        use crate::drivers::GitDriver;

        // Check for git
        if kernel_path.join(".git").exists() {
            return Some(Box::new(GitDriver::new(
                kernel_path.clone(),
                kernel_name.to_string(),
            )));
        }

        // Check for S3 marker
        if kernel_path.join(".s3-versioned").exists() {
            // TODO: Implement S3Driver when S3 backend is ready
            eprintln!("[VersionDriver] S3 versioning detected but not yet implemented");
            return None;
        }

        // Check for filesystem versioning
        if kernel_path.join(".version").exists() {
            // TODO: Implement FilesystemDriver
            eprintln!("[VersionDriver] Filesystem versioning detected but not yet implemented");
            return None;
        }

        None
    }

    /// Create version driver with explicit backend
    ///
    /// Used during kernel creation to set up versioning
    pub fn create(
        backend: VersionBackend,
        kernel_path: &PathBuf,
        kernel_name: &str,
    ) -> Result<Box<dyn VersionDriver>> {
        use crate::drivers::GitDriver;

        match backend {
            VersionBackend::Git => {
                Ok(Box::new(GitDriver::new(
                    kernel_path.clone(),
                    kernel_name.to_string(),
                )))
            }
            VersionBackend::S3 => {
                Err(CkpError::IoError("S3 version driver not yet implemented".to_string()))
            }
            VersionBackend::Postgres => {
                Err(CkpError::IoError("Postgres version driver not yet implemented".to_string()))
            }
            VersionBackend::Filesystem => {
                Err(CkpError::IoError("Filesystem version driver not yet implemented".to_string()))
            }
            VersionBackend::None => {
                Err(CkpError::IoError("Cannot create version driver for None backend".to_string()))
            }
        }
    }
}

/// Extension trait for easy version driver access
pub trait VersionedKernel {
    /// Get version driver for this kernel
    fn version_driver(&self) -> Option<Box<dyn VersionDriver>>;

    /// Get current version using detected driver
    fn current_version(&self) -> Result<Option<VersionInfo>> {
        if let Some(driver) = self.version_driver() {
            driver.get_version()
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    /// Test: VersionInfo can be constructed with all fields
    #[test]
    fn test_version_info_construction() {
        let info = VersionInfo {
            version: "v1.0.0".to_string(),
            is_clean: true,
            metadata: Some("commit-abc123".to_string()),
            backend: VersionBackend::Git,
        };

        assert_eq!(info.version, "v1.0.0");
        assert!(info.is_clean);
        assert_eq!(info.metadata, Some("commit-abc123".to_string()));
        assert_eq!(info.backend, VersionBackend::Git);
    }

    /// Test: VersionInfo supports Clone and PartialEq
    #[test]
    fn test_version_info_traits() {
        let info1 = VersionInfo {
            version: "v1.0.0".to_string(),
            is_clean: true,
            metadata: None,
            backend: VersionBackend::Filesystem,
        };

        let info2 = info1.clone();
        assert_eq!(info1, info2);
    }

    /// Test: VersionBackend Display implementation
    #[test]
    fn test_version_backend_display() {
        assert_eq!(format!("{}", VersionBackend::Git), "git");
        assert_eq!(format!("{}", VersionBackend::S3), "s3");
        assert_eq!(format!("{}", VersionBackend::Postgres), "postgres");
        assert_eq!(format!("{}", VersionBackend::Filesystem), "filesystem");
        assert_eq!(format!("{}", VersionBackend::None), "none");
    }

    /// Test: VersionBackend supports Copy and PartialEq
    #[test]
    fn test_version_backend_traits() {
        let backend1 = VersionBackend::Git;
        let backend2 = backend1; // Copy trait
        assert_eq!(backend1, backend2); // PartialEq trait
    }

    /// Test: VersionDriverFactory::detect() returns None for empty directory
    #[test]
    fn test_factory_detect_no_versioning() {
        let temp = TempDir::new().unwrap();
        let kernel_path = temp.path().join("Test.Kernel");
        fs::create_dir_all(&kernel_path).unwrap();

        let driver = VersionDriverFactory::detect(&kernel_path, "Test.Kernel");
        assert!(driver.is_none());
    }

    /// Test: VersionDriverFactory::detect() finds Git repository
    #[test]
    fn test_factory_detect_git() {
        let temp = TempDir::new().unwrap();
        let kernel_path = temp.path().join("Test.Git");
        fs::create_dir_all(&kernel_path).unwrap();

        // Create .git directory
        fs::create_dir_all(kernel_path.join(".git")).unwrap();

        let driver = VersionDriverFactory::detect(&kernel_path, "Test.Git");
        assert!(driver.is_some());

        let driver = driver.unwrap();
        assert_eq!(driver.backend_type(), VersionBackend::Git);
    }

    /// Test: VersionDriverFactory::detect() detects S3 marker (but not yet implemented)
    #[test]
    fn test_factory_detect_s3_marker() {
        let temp = TempDir::new().unwrap();
        let kernel_path = temp.path().join("Test.S3");
        fs::create_dir_all(&kernel_path).unwrap();

        // Create S3 marker file
        fs::write(kernel_path.join(".s3-versioned"), "").unwrap();

        let driver = VersionDriverFactory::detect(&kernel_path, "Test.S3");
        // Returns None because S3Driver not yet implemented
        assert!(driver.is_none());
    }

    /// Test: VersionDriverFactory::detect() detects filesystem marker (but not yet implemented)
    #[test]
    fn test_factory_detect_filesystem_marker() {
        let temp = TempDir::new().unwrap();
        let kernel_path = temp.path().join("Test.Filesystem");
        fs::create_dir_all(&kernel_path).unwrap();

        // Create filesystem version file
        fs::write(kernel_path.join(".version"), "v1.0.0").unwrap();

        let driver = VersionDriverFactory::detect(&kernel_path, "Test.Filesystem");
        // Returns None because FilesystemDriver not yet implemented
        assert!(driver.is_none());
    }

    /// Test: VersionDriverFactory::create() creates Git driver
    #[test]
    fn test_factory_create_git_driver() {
        let temp = TempDir::new().unwrap();
        let kernel_path = temp.path().join("Test.CreateGit");
        fs::create_dir_all(&kernel_path).unwrap();

        let result = VersionDriverFactory::create(
            VersionBackend::Git,
            &kernel_path,
            "Test.CreateGit"
        );

        assert!(result.is_ok());
        let driver = result.unwrap();
        assert_eq!(driver.backend_type(), VersionBackend::Git);
    }

    /// Test: VersionDriverFactory::create() fails for S3 (not implemented)
    #[test]
    fn test_factory_create_s3_not_implemented() {
        let temp = TempDir::new().unwrap();
        let kernel_path = temp.path().join("Test.S3");
        fs::create_dir_all(&kernel_path).unwrap();

        let result = VersionDriverFactory::create(
            VersionBackend::S3,
            &kernel_path,
            "Test.S3"
        );

        assert!(result.is_err());
        if let Err(CkpError::IoError(msg)) = result {
            assert!(msg.contains("S3"));
            assert!(msg.contains("not yet implemented"));
        } else {
            panic!("Expected IoError for unimplemented backend");
        }
    }

    /// Test: VersionDriverFactory::create() fails for Postgres (not implemented)
    #[test]
    fn test_factory_create_postgres_not_implemented() {
        let temp = TempDir::new().unwrap();
        let kernel_path = temp.path().join("Test.Postgres");
        fs::create_dir_all(&kernel_path).unwrap();

        let result = VersionDriverFactory::create(
            VersionBackend::Postgres,
            &kernel_path,
            "Test.Postgres"
        );

        assert!(result.is_err());
        if let Err(CkpError::IoError(msg)) = result {
            assert!(msg.contains("Postgres"));
            assert!(msg.contains("not yet implemented"));
        } else {
            panic!("Expected IoError for unimplemented backend");
        }
    }

    /// Test: VersionDriverFactory::create() fails for Filesystem (not implemented)
    #[test]
    fn test_factory_create_filesystem_not_implemented() {
        let temp = TempDir::new().unwrap();
        let kernel_path = temp.path().join("Test.Filesystem");
        fs::create_dir_all(&kernel_path).unwrap();

        let result = VersionDriverFactory::create(
            VersionBackend::Filesystem,
            &kernel_path,
            "Test.Filesystem"
        );

        assert!(result.is_err());
        if let Err(CkpError::IoError(msg)) = result {
            assert!(msg.contains("Filesystem"));
            assert!(msg.contains("not yet implemented"));
        } else {
            panic!("Expected IoError for unimplemented backend");
        }
    }

    /// Test: VersionDriverFactory::create() fails for None backend
    #[test]
    fn test_factory_create_none_backend() {
        let temp = TempDir::new().unwrap();
        let kernel_path = temp.path().join("Test.None");
        fs::create_dir_all(&kernel_path).unwrap();

        let result = VersionDriverFactory::create(
            VersionBackend::None,
            &kernel_path,
            "Test.None"
        );

        assert!(result.is_err());
        if let Err(CkpError::IoError(msg)) = result {
            assert!(msg.contains("Cannot create version driver"));
            assert!(msg.contains("None backend"));
        } else {
            panic!("Expected IoError for None backend");
        }
    }

    /// Test: VersionBackend enum variants are distinct
    #[test]
    fn test_version_backend_variants() {
        let backends = vec![
            VersionBackend::Git,
            VersionBackend::S3,
            VersionBackend::Postgres,
            VersionBackend::Filesystem,
            VersionBackend::None,
        ];

        // Each backend should be distinct
        for i in 0..backends.len() {
            for j in (i+1)..backends.len() {
                assert_ne!(backends[i], backends[j]);
            }
        }
    }
}
