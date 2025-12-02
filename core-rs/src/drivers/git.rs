//! Git versioning driver for ConceptKernel storage
//!
//! **Single Source of Truth for Kernel Versioning**
//!
//! The GitDriver is the authoritative source for all kernel versions.
//! All core kernel APIs (export, cache, status, etc.) should query
//! GitDriver for version information.
//!
//! ## Version Format
//!
//! - **Clean tag**: `v0.2.0` (exactly on tag)
//! - **Between tags**: `v0.2.3-gab12cd` (3 commits ahead → patch 3, hash ab12cd)
//!
//! The commits-ahead count becomes the patch version, and the short hash
//! allows matching to `git log` output for precise state identification.
//!
//! ## Usage
//!
//! ```rust
//! use ckp_core::drivers::GitDriver;
//! use std::path::PathBuf;
//!
//! let driver = GitDriver::new(
//!     PathBuf::from("/project/concepts/System.Consensus"),
//!     "System.Consensus".to_string()
//! );
//!
//! // Get current version (SSOT)
//! let version = driver.get_current_version()?;
//! // Returns: Some("v0.2.0-3-gab12cd") or None if no tags
//!
//! // Check if on clean tag
//! let is_clean = driver.is_clean_tag()?;
//! // Returns: true only if exactly on a tag with no changes
//! ```

use crate::errors::{CkpError, Result};
use crate::drivers::version::{VersionDriver, VersionInfo, VersionBackend};
use std::path::PathBuf;
use std::process::Command;

/// Git driver for kernel versioning
#[derive(Debug, Clone)]
pub struct GitDriver {
    kernel_path: PathBuf,
    kernel_name: String,
}

impl GitDriver {
    /// Create new GitDriver for a kernel
    pub fn new(kernel_path: PathBuf, kernel_name: String) -> Self {
        Self {
            kernel_path,
            kernel_name,
        }
    }

    /// Initialize git repository if not already initialized
    pub fn init(&self) -> Result<()> {
        let git_dir = self.kernel_path.join(".git");

        if git_dir.exists() {
            return Ok(());
        }

        let output = Command::new("git")
            .args(["init"])
            .current_dir(&self.kernel_path)
            .output()
            .map_err(|e| CkpError::IoError(format!("Failed to run git init: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CkpError::IoError(format!("git init failed: {}", stderr)));
        }

        // Set initial config
        self.config_set("user.name", "ConceptKernel")?;
        self.config_set("user.email", "system@conceptkernel.local")?;

        Ok(())
    }

    /// Set git config value
    fn config_set(&self, key: &str, value: &str) -> Result<()> {
        let output = Command::new("git")
            .args(["config", key, value])
            .current_dir(&self.kernel_path)
            .output()
            .map_err(|e| CkpError::IoError(format!("Failed to run git config: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CkpError::IoError(format!("git config failed: {}", stderr)));
        }

        Ok(())
    }

    /// Check if there are uncommitted changes
    pub fn has_changes(&self) -> Result<bool> {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.kernel_path)
            .output()
            .map_err(|e| CkpError::IoError(format!("Failed to run git status: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CkpError::IoError(format!("git status failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(!stdout.trim().is_empty())
    }

    /// Add all changes to staging
    pub fn add_all(&self) -> Result<()> {
        let output = Command::new("git")
            .args(["add", "."])
            .current_dir(&self.kernel_path)
            .output()
            .map_err(|e| CkpError::IoError(format!("Failed to run git add: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CkpError::IoError(format!("git add failed: {}", stderr)));
        }

        Ok(())
    }

    /// Commit changes with message
    pub fn commit(&self, message: &str) -> Result<String> {
        let output = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&self.kernel_path)
            .output()
            .map_err(|e| CkpError::IoError(format!("Failed to run git commit: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CkpError::IoError(format!("git commit failed: {}", stderr)));
        }

        // Get commit hash
        let hash_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.kernel_path)
            .output()
            .map_err(|e| CkpError::IoError(format!("Failed to get commit hash: {}", e)))?;

        let commit_hash = String::from_utf8_lossy(&hash_output.stdout).trim().to_string();
        Ok(commit_hash)
    }

    /// Create git tag
    pub fn tag(&self, version: &str, message: Option<&str>) -> Result<()> {
        let mut args = vec!["tag"];

        if let Some(msg) = message {
            args.push("-a");
            args.push(version);
            args.push("-m");
            args.push(msg);
        } else {
            args.push(version);
        }

        let output = Command::new("git")
            .args(&args)
            .current_dir(&self.kernel_path)
            .output()
            .map_err(|e| CkpError::IoError(format!("Failed to run git tag: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CkpError::IoError(format!("git tag failed: {}", stderr)));
        }

        Ok(())
    }

    /// Get current version from git describe (single source of truth)
    ///
    /// Returns version string based on git state:
    /// - **Clean tag**: `v0.2.0` (semantic version)
    /// - **Between tags**: `v0.2.3-gab12cd` (3 commits ahead → patch version 3)
    /// - **No tags**: `None`
    ///
    /// Format: `v{major}.{minor}.{commits_ahead}-g{short_hash}`
    ///
    /// This is the **authoritative version** for all kernel operations.
    pub fn get_current_version(&self) -> Result<Option<String>> {
        // Get git describe output: v0.2.0-3-gab12cd or v0.2.0
        let output = Command::new("git")
            .args(["describe", "--tags", "--always", "--long"])
            .current_dir(&self.kernel_path)
            .output()
            .map_err(|e| CkpError::IoError(format!("Failed to run git describe: {}", e)))?;

        if !output.status.success() {
            // No git repo or no tags
            return Ok(None);
        }

        let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // If version doesn't start with 'v', it's just a commit hash (no tags)
        if !raw.starts_with('v') {
            return Ok(None);
        }

        // Parse: v0.2.0-3-gab12cd → v0.2.3-gab12cd
        // Or:    v0.2.0-0-gab12cd → v0.2.0 (clean tag)
        let formatted = Self::format_version(&raw)?;

        Ok(Some(formatted))
    }

    /// Format git describe output to semantic version with hash
    ///
    /// Input:  `v0.2.0-3-gab12cd` (3 commits ahead of v0.2.0)
    /// Output: `v0.2.3-gab12cd` (use commits_ahead as patch version)
    ///
    /// Input:  `v0.2.0-0-gab12cd` (on tag)
    /// Output: `v0.2.0` (clean semantic version)
    fn format_version(raw: &str) -> Result<String> {
        // Pattern: v{major}.{minor}.{patch}-{commits}-g{hash}
        let parts: Vec<&str> = raw.split('-').collect();

        if parts.len() < 3 {
            // Malformed, return as-is
            return Ok(raw.to_string());
        }

        let base_version = parts[0]; // v0.2.0
        let commits_ahead = parts[1]; // "3"
        let hash = parts[2];          // "gab12cd"

        // Parse base version: v0.2.0
        let version_parts: Vec<&str> = base_version.trim_start_matches('v').split('.').collect();
        if version_parts.len() != 3 {
            // Malformed, return as-is
            return Ok(raw.to_string());
        }

        let major = version_parts[0];
        let minor = version_parts[1];

        // Parse commits ahead
        let commits: u32 = commits_ahead.parse()
            .map_err(|_| CkpError::IoError(format!("Invalid commits_ahead: {}", commits_ahead)))?;

        if commits == 0 {
            // Clean tag: v0.2.0
            Ok(format!("v{}.{}.{}", major, minor, version_parts[2]))
        } else {
            // Between tags: v0.2.3-gab12cd
            Ok(format!("v{}.{}.{}-{}", major, minor, commits, hash))
        }
    }

    /// Get clean version from latest tag only (no distance/hash)
    pub fn get_latest_tag(&self) -> Result<Option<String>> {
        let output = Command::new("git")
            .args(["describe", "--tags", "--abbrev=0"])
            .current_dir(&self.kernel_path)
            .output()
            .map_err(|e| CkpError::IoError(format!("Failed to get tags: {}", e)))?;

        if !output.status.success() {
            // No tags yet
            return Ok(None);
        }

        let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(Some(tag))
    }

    /// Check if repository is on a clean tag (no commits ahead)
    pub fn is_clean_tag(&self) -> Result<bool> {
        let current = self.get_current_version()?;

        match current {
            Some(version) => {
                // Clean if version has no hash suffix (no '-g' pattern)
                Ok(!version.contains("-g"))
            }
            _ => Ok(false)
        }
    }

    /// Parse semantic version (v0.1.0 -> (0, 1, 0))
    fn parse_version(version: &str) -> Result<(u32, u32, u32)> {
        let version = version.trim_start_matches('v');
        let parts: Vec<&str> = version.split('.').collect();

        if parts.len() != 3 {
            return Err(CkpError::IoError(format!("Invalid version format: {}", version)));
        }

        let major = parts[0].parse::<u32>()
            .map_err(|_| CkpError::IoError(format!("Invalid major version: {}", parts[0])))?;
        let minor = parts[1].parse::<u32>()
            .map_err(|_| CkpError::IoError(format!("Invalid minor version: {}", parts[1])))?;
        let patch = parts[2].parse::<u32>()
            .map_err(|_| CkpError::IoError(format!("Invalid patch version: {}", parts[2])))?;

        Ok((major, minor, patch))
    }

    /// Increment version (patch by default)
    pub fn increment_version(&self, bump: VersionBump) -> Result<String> {
        let current = self.get_latest_tag()?.unwrap_or_else(|| "v0.0.0".to_string());
        let (mut major, mut minor, mut patch) = Self::parse_version(&current)?;

        match bump {
            VersionBump::Major => {
                major += 1;
                minor = 0;
                patch = 0;
            }
            VersionBump::Minor => {
                minor += 1;
                patch = 0;
            }
            VersionBump::Patch => {
                patch += 1;
            }
        }

        Ok(format!("v{}.{}.{}", major, minor, patch))
    }

    /// Commit and tag in one operation
    pub fn commit_and_tag(&self, message: &str, bump: VersionBump) -> Result<String> {
        // Ensure git is initialized
        self.init()?;

        // Check for changes
        if !self.has_changes()? {
            return Err(CkpError::IoError("No changes to commit".to_string()));
        }

        // Add all changes
        self.add_all()?;

        // Commit
        let commit_hash = self.commit(message)?;

        // Increment version and tag
        let new_version = self.increment_version(bump)?;
        self.tag(&new_version, Some(message))?;

        eprintln!(
            "[GitDriver] [{}] Committed and tagged: {} ({})",
            self.kernel_name, new_version, &commit_hash[..8]
        );

        Ok(new_version)
    }

    /// Get list of all tags
    pub fn list_tags(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args(["tag", "-l"])
            .current_dir(&self.kernel_path)
            .output()
            .map_err(|e| CkpError::IoError(format!("Failed to list tags: {}", e)))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let tags = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(tags)
    }
}

/// Version bump type
#[derive(Debug, Clone, Copy)]
pub enum VersionBump {
    Major,
    Minor,
    Patch,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_version() {
        assert_eq!(GitDriver::parse_version("v1.2.3").unwrap(), (1, 2, 3));
        assert_eq!(GitDriver::parse_version("0.1.0").unwrap(), (0, 1, 0));
        assert!(GitDriver::parse_version("invalid").is_err());
    }

    #[test]
    fn test_format_version() {
        // Clean tag (0 commits ahead)
        assert_eq!(
            GitDriver::format_version("v0.2.0-0-gcec82fc").unwrap(),
            "v0.2.0"
        );

        // 1 commit ahead
        assert_eq!(
            GitDriver::format_version("v0.2.0-1-g2a07618").unwrap(),
            "v0.2.1-g2a07618"
        );

        // 3 commits ahead
        assert_eq!(
            GitDriver::format_version("v0.2.0-3-gab12cd").unwrap(),
            "v0.2.3-gab12cd"
        );

        // 15 commits ahead
        assert_eq!(
            GitDriver::format_version("v1.3.0-15-gdeadbeef").unwrap(),
            "v1.3.15-gdeadbeef"
        );
    }

    #[test]
    fn test_git_init() {
        let temp_dir = TempDir::new().unwrap();
        let driver = GitDriver::new(
            temp_dir.path().to_path_buf(),
            "Test.Kernel".to_string(),
        );

        driver.init().unwrap();
        assert!(temp_dir.path().join(".git").exists());
    }
}

/// Implement VersionDriver trait for GitDriver
impl VersionDriver for GitDriver {
    fn get_version(&self) -> Result<Option<VersionInfo>> {
        match self.get_current_version()? {
            Some(version) => {
                let is_clean = self.is_clean_tag()?;
                let metadata = if !is_clean {
                    // Extract hash from version like v0.2.3-gab12cd
                    version.split("-g").nth(1).map(|s| s.to_string())
                } else {
                    None
                };

                Ok(Some(VersionInfo {
                    version,
                    is_clean,
                    metadata,
                    backend: VersionBackend::Git,
                }))
            }
            None => Ok(None),
        }
    }

    fn init(&self) -> Result<()> {
        self.init()
    }

    fn is_initialized(&self) -> bool {
        self.kernel_path.join(".git").exists()
    }

    fn create_version(&self, message: &str) -> Result<String> {
        self.commit_and_tag(message, VersionBump::Patch)
    }

    fn list_versions(&self) -> Result<Vec<String>> {
        self.list_tags()
    }

    fn backend_type(&self) -> VersionBackend {
        VersionBackend::Git
    }
}
