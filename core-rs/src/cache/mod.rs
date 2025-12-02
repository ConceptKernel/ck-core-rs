//! Cache module for ConceptKernel packages
//!
//! Manages local package cache at ~/.config/conceptkernel/cache/
//! Handles tar.gz packages for concepts

pub mod package_manager;

pub use package_manager::{PackageManager, PackageInfo};

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: PackageManager export is accessible
    ///
    /// Verifies that PackageManager type is exported and can be constructed
    /// for managing local package cache and bootstrap kernel distribution.
    #[test]
    fn test_package_manager_export() {
        // Verify PackageManager type is accessible
        fn accepts_package_manager(_: PackageManager) {}

        let manager = PackageManager::new().unwrap();

        accepts_package_manager(manager);

        // If this compiles, export is correct
    }

    /// Test: PackageInfo export is accessible
    ///
    /// Verifies that PackageInfo struct is exported and can be used
    /// for package metadata and cache entry information.
    #[test]
    fn test_package_info_export() {
        // Verify PackageInfo type is accessible
        fn accepts_package_info(_: PackageInfo) {}

        let info = PackageInfo {
            name: "Test.Kernel".to_string(),
            version: "v1.0.0".to_string(),
            arch: "aarch64-darwin".to_string(),
            runtime: "rs".to_string(),
            filename: "Test.Kernel-v1.0.0-aarch64-darwin-rs.tar.gz".to_string(),
            size_bytes: 1024,
            created_at: "2025-11-29".to_string(),
        };

        accepts_package_info(info);

        // If this compiles, export is correct
    }
}
