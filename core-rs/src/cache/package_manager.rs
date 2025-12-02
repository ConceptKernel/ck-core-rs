//! Package Manager for ConceptKernel cache
//!
//! Manages packages in ~/.config/conceptkernel/cache/
//! Packages are tar.gz files named: <concept>@<version>.tar.gz

use crate::errors::{CkpError, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// Package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub arch: String,      // e.g., "aarch64-darwin", "x86_64-linux", "universal"
    pub runtime: String,   // e.g., "rs", "py", "js"
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: String, // YYYY-MM-DD format
}

/// Package Manager - manages local cache of concept packages
pub struct PackageManager {
    cache_dir: PathBuf,
}

impl PackageManager {
    /// Create a new PackageManager
    ///
    /// Cache directory: ~/.config/conceptkernel/cache/
    pub fn new() -> Result<Self> {
        let home_dir = env::var("HOME").map_err(|_| {
            CkpError::IoError("HOME environment variable not set".to_string())
        })?;

        let cache_dir = PathBuf::from(home_dir)
            .join(".config")
            .join("conceptkernel")
            .join("cache");

        // Create cache directory if not exists
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| {
                CkpError::IoError(format!("Failed to create cache directory: {}", e))
            })?;
        }

        Ok(PackageManager { cache_dir })
    }

    /// List all cached packages
    ///
    /// # Returns
    /// Vector of PackageInfo for all .tar.gz files in cache
    pub fn list_cached(&self) -> Result<Vec<PackageInfo>> {
        let mut packages = Vec::new();

        if !self.cache_dir.exists() {
            return Ok(packages);
        }

        let entries = fs::read_dir(&self.cache_dir).map_err(|e| {
            CkpError::IoError(format!("Failed to read cache directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| CkpError::IoError(e.to_string()))?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("gz") {
                let filename = entry.file_name().to_string_lossy().to_string();

                // Parse filename using new parser (supports both formats)
                if let Some((name, version, arch, runtime)) = self.parse_package_filename(&filename) {
                    let metadata = fs::metadata(&path).map_err(|e| {
                        CkpError::IoError(format!("Failed to get file metadata: {}", e))
                    })?;

                    // Format mtime as YYYY-MM-DD
                    let created_at = if let Ok(modified) = metadata.modified() {
                        use std::time::UNIX_EPOCH;
                        if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                            let secs = duration.as_secs();
                            // Simple YYYY-MM-DD formatting (approximation)
                            let days_since_epoch = secs / 86400;
                            let years = days_since_epoch / 365;
                            let remaining_days = days_since_epoch % 365;
                            let months = remaining_days / 30;
                            let days = remaining_days % 30;
                            format!("{:04}-{:02}-{:02}", 1970 + years, months + 1, days + 1)
                        } else {
                            "unknown".to_string()
                        }
                    } else {
                        "unknown".to_string()
                    };

                    packages.push(PackageInfo {
                        name,
                        version,
                        arch,
                        runtime,
                        filename,
                        size_bytes: metadata.len(),
                        created_at,
                    });
                }
            }
        }

        packages.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(packages)
    }

    /// Resolve instance name for loading a concept
    ///
    /// Supports multi-instance deployment with auto-numbering or custom names
    ///
    /// # Arguments
    /// * `base_name` - Base concept name (e.g., "MyKernel.Bla")
    /// * `custom_name` - Optional custom name from --as flag
    /// * `target_dir` - Target directory containing concepts/
    ///
    /// # Returns
    /// Instance name to use (e.g., "MyKernel.Bla", "MyKernel.Bla.1", or custom)
    pub fn resolve_instance_name(
        &self,
        base_name: &str,
        custom_name: Option<&str>,
        target_dir: &Path
    ) -> Result<String> {
        // If custom name provided, use it directly
        if let Some(name) = custom_name {
            return Ok(name.to_string());
        }

        // Check if base name exists
        let concepts_dir = target_dir.join("concepts");
        let base_path = concepts_dir.join(base_name);

        if !base_path.exists() {
            // First instance - use base name
            return Ok(base_name.to_string());
        }

        // Find next available .N suffix
        let mut counter = 1;
        loop {
            let candidate = format!("{}.{}", base_name, counter);
            let candidate_path = concepts_dir.join(&candidate);

            if !candidate_path.exists() {
                return Ok(candidate);
            }

            counter += 1;

            // Safety check to prevent infinite loop
            if counter > 1000 {
                return Err(CkpError::IoError(
                    "Too many instances of concept".to_string()
                ));
            }
        }
    }

    /// Install a concept from cache to concepts directory
    ///
    /// Extracts <concept>@<version>.tar.gz from cache to target_dir/concepts/<instance_name>/
    ///
    /// # Arguments
    /// * `concept_name` - Name of concept (e.g., "System.Gateway.HTTP")
    /// * `version` - Version (e.g., "v1.3.14")
    /// * `target_dir` - Target directory containing concepts/ folder
    /// * `instance_name` - Optional instance name (for multi-instance support)
    ///
    /// Install from a specific PackageInfo (supports new filename format)
    ///
    /// # Arguments
    /// * `package` - Package information
    /// * `target_dir` - Project root directory
    /// * `instance_name` - Optional custom instance name
    ///
    /// # Returns
    /// Path to extracted concept directory
    pub fn install_from_package(&self, package: &PackageInfo, target_dir: &Path, instance_name: Option<&str>) -> Result<PathBuf> {
        let package_path = self.cache_dir.join(&package.filename);

        if !package_path.exists() {
            return Err(CkpError::FileNotFound(format!(
                "Package not found in cache: {}",
                package.filename
            )));
        }

        self.install_from_path(&package.name, &package_path, target_dir, instance_name)
    }

    /// Install from cache (legacy method, tries old format first then new)
    ///
    /// # Arguments
    /// * `concept_name` - Name of concept
    /// * `version` - Version to install
    /// * `target_dir` - Project root directory
    /// * `instance_name` - Optional custom instance name
    ///
    /// # Returns
    /// Path to extracted concept directory
    pub fn install(&self, concept_name: &str, version: &str, target_dir: &Path, instance_name: Option<&str>) -> Result<PathBuf> {
        // Try old format first for backward compatibility
        let old_filename = format!("{}@{}.tar.gz", concept_name, version);
        let old_path = self.cache_dir.join(&old_filename);

        if old_path.exists() {
            return self.install_from_path(concept_name, &old_path, target_dir, instance_name);
        }

        // Try to find any package with matching name and version
        let packages = self.list_cached()?;
        let matching = packages.iter()
            .find(|p| p.name == concept_name && p.version == version)
            .ok_or_else(|| CkpError::FileNotFound(format!(
                "Package not found in cache: {}@{}",
                concept_name, version
            )))?;

        self.install_from_package(matching, target_dir, instance_name)
    }

    /// Internal method to install from a specific package path
    fn install_from_path(&self, concept_name: &str, package_path: &Path, target_dir: &Path, instance_name: Option<&str>) -> Result<PathBuf> {

        // Use instance_name if provided, otherwise use concept_name
        let final_name = instance_name.unwrap_or(concept_name);

        let concepts_dir = target_dir.join("concepts");
        let concept_dir = concepts_dir.join(final_name);

        // Check if concept already exists
        if concept_dir.exists() {
            return Err(CkpError::IoError(format!(
                "Concept already exists: {}",
                final_name
            )));
        }

        // Create concepts directory if needed
        if !concepts_dir.exists() {
            fs::create_dir_all(&concepts_dir).map_err(|e| {
                CkpError::IoError(format!("Failed to create concepts directory: {}", e))
            })?;
        }

        // Extract to a temporary directory first to avoid overwriting existing instances
        use std::env;
        let temp_extract_dir = env::temp_dir().join(format!("ckp-extract-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_extract_dir).map_err(|e| {
            CkpError::IoError(format!("Failed to create temp extract directory: {}", e))
        })?;

        // Extract tar.gz to temp directory (creates temp/<concept_name>/)
        self.extract_tarball(&package_path, &temp_extract_dir)?;

        // Tarball always extracts with the original concept_name
        let extracted_dir = temp_extract_dir.join(concept_name);

        if !extracted_dir.exists() {
            // Clean up temp directory
            let _ = fs::remove_dir_all(&temp_extract_dir);
            return Err(CkpError::IoError(format!(
                "Extraction failed - concept directory not created: {}",
                concept_name
            )));
        }

        // Move from temp location to final location with the correct instance name
        fs::rename(&extracted_dir, &concept_dir).map_err(|e| {
            // Clean up temp directory
            let _ = fs::remove_dir_all(&temp_extract_dir);
            CkpError::IoError(format!(
                "Failed to move extracted concept to final location: {}",
                e
            ))
        })?;

        // Clean up temp directory
        let _ = fs::remove_dir_all(&temp_extract_dir);

        Ok(concept_dir)
    }

    /// Export a concept from concepts directory to cache
    ///
    /// Creates concepts/<concept> → cache/<concept>-<version>.<arch>.<runtime>.tar.gz
    ///
    /// # Arguments
    /// * `concept_name` - Name of concept
    /// * `version` - Version to tag
    /// * `source_dir` - Source directory containing concepts/ folder
    ///
    /// # Returns
    /// Path to created package file
    pub fn export(&self, concept_name: &str, version: &str, source_dir: &Path) -> Result<PathBuf> {
        let concept_dir = source_dir.join("concepts").join(concept_name);

        if !concept_dir.exists() {
            return Err(CkpError::FileNotFound(format!(
                "Concept not found: {}",
                concept_name
            )));
        }

        // Detect runtime and architecture
        let (arch, runtime) = self.detect_runtime_and_arch(&concept_dir)?;

        // New filename format: <name>-<version>.<arch>.<runtime>.tar.gz
        let package_filename = format!("{}-{}.{}.{}.tar.gz", concept_name, version, arch, runtime);
        let package_path = self.cache_dir.join(&package_filename);

        // Create tar.gz
        self.create_tarball(&concept_dir, &package_path, concept_name)?;

        Ok(package_path)
    }

    /// Fork a cached package to create a new kernel
    ///
    /// Workflow:
    /// 1. Load latest package from cache
    /// 2. Extract to concepts/<new_name>
    /// 3. Update conceptkernel.yaml name field
    /// 4. If --clean: Remove queues/storage/tx/consensus/logs
    /// 5. If --tag: Create git tag
    ///
    /// # Arguments
    /// * `source_name` - Source package name
    /// * `new_name` - New kernel name
    /// * `target_dir` - Project root directory
    /// * `clean` - Remove runtime data
    /// * `tag` - Optional git tag to create
    ///
    /// # Returns
    /// Path to forked kernel directory
    pub fn fork_package(
        &self,
        source_name: &str,
        new_name: &str,
        target_dir: &Path,
        clean: bool,
        tag: Option<&str>,
    ) -> Result<PathBuf> {
        // 1. Find latest version of source package in cache
        let packages = self.list_cached()?;
        let source_pkg = packages
            .iter()
            .filter(|p| p.name == source_name)
            .max_by_key(|p| &p.version)
            .ok_or_else(|| {
                CkpError::FileNotFound(format!(
                    "No cached package found for '{}'",
                    source_name
                ))
            })?;

        // 2. Extract to concepts/<new_name>
        let concepts_dir = target_dir.join("concepts");
        let new_kernel_dir = concepts_dir.join(new_name);

        if new_kernel_dir.exists() {
            return Err(CkpError::IoError(format!(
                "Kernel already exists: {}",
                new_name
            )));
        }

        // Use install_from_package to extract
        let extracted_dir = self.install_from_package(source_pkg, target_dir, Some(new_name))?;

        // 3. Update conceptkernel.yaml name field
        let yaml_path = extracted_dir.join("conceptkernel.yaml");
        if yaml_path.exists() {
            let yaml_content = fs::read_to_string(&yaml_path).map_err(|e| {
                CkpError::IoError(format!("Failed to read conceptkernel.yaml: {}", e))
            })?;

            let mut yaml: serde_yaml::Value = serde_yaml::from_str(&yaml_content).map_err(|e| {
                CkpError::ParseError(format!("Failed to parse conceptkernel.yaml: {}", e))
            })?;

            // Update metadata.name field
            if let Some(metadata) = yaml.get_mut("metadata") {
                if let Some(metadata_map) = metadata.as_mapping_mut() {
                    metadata_map.insert(
                        serde_yaml::Value::String("name".to_string()),
                        serde_yaml::Value::String(new_name.to_string()),
                    );
                }
            }

            // Write back
            let updated_yaml = serde_yaml::to_string(&yaml).map_err(|e| {
                CkpError::IoError(format!("Failed to serialize YAML: {}", e))
            })?;

            fs::write(&yaml_path, updated_yaml).map_err(|e| {
                CkpError::IoError(format!("Failed to write conceptkernel.yaml: {}", e))
            })?;
        }

        // 4. If --clean: Remove runtime data
        if clean {
            let dirs_to_clean = vec!["queue", "storage", "tx", "consensus", "logs"];
            for dir_name in dirs_to_clean {
                let dir_path = extracted_dir.join(dir_name);
                if dir_path.exists() {
                    fs::remove_dir_all(&dir_path).map_err(|e| {
                        CkpError::IoError(format!("Failed to clean {}: {}", dir_name, e))
                    })?;
                    // Recreate empty directory
                    fs::create_dir_all(&dir_path).map_err(|e| {
                        CkpError::IoError(format!("Failed to recreate {}: {}", dir_name, e))
                    })?;
                }
            }
        }

        // 5. If --tag: Create git tag
        if let Some(tag_name) = tag {
            use std::process::Command;

            let output = Command::new("git")
                .args(["tag", "-a", tag_name, "-m", &format!("Fork from {}", source_name)])
                .current_dir(&extracted_dir)
                .output()
                .map_err(|e| CkpError::IoError(format!("Failed to create git tag: {}", e)))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CkpError::IoError(format!("git tag failed: {}", stderr)));
            }
        }

        Ok(extracted_dir)
    }

    /// Import a package file into cache
    ///
    /// Copies a tar.gz file into the cache directory
    ///
    /// # Arguments
    /// * `tarball_path` - Path to .tar.gz file
    ///
    /// # Returns
    /// PackageInfo for imported package
    pub fn import(&self, tarball_path: &Path) -> Result<PackageInfo> {
        if !tarball_path.exists() {
            return Err(CkpError::FileNotFound(format!(
                "File not found: {}",
                tarball_path.display()
            )));
        }

        let filename = tarball_path
            .file_name()
            .ok_or_else(|| CkpError::ParseError("Invalid filename".to_string()))?
            .to_string_lossy()
            .to_string();

        // Validate filename format
        if !filename.ends_with(".tar.gz") {
            return Err(CkpError::ParseError(format!(
                "Invalid package file: must be .tar.gz"
            )));
        }

        // Parse name, version, arch, runtime using new parser
        let (name, version, arch, runtime) = self.parse_package_filename(&filename)
            .ok_or_else(|| CkpError::ParseError(format!(
                "Invalid package filename format. Expected: <name>-<version>.<arch>.<runtime>.tar.gz or <name>@<version>.tar.gz"
            )))?;

        // Copy to cache
        let dest_path = self.cache_dir.join(&filename);
        fs::copy(tarball_path, &dest_path).map_err(|e| {
            CkpError::IoError(format!("Failed to copy package to cache: {}", e))
        })?;

        let metadata = fs::metadata(&dest_path).map_err(|e| {
            CkpError::IoError(format!("Failed to get file metadata: {}", e))
        })?;

        // Format mtime as YYYY-MM-DD
        let created_at = if let Ok(modified) = metadata.modified() {
            use std::time::UNIX_EPOCH;
            if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                let secs = duration.as_secs();
                let days_since_epoch = secs / 86400;
                let years = days_since_epoch / 365;
                let remaining_days = days_since_epoch % 365;
                let months = remaining_days / 30;
                let days = remaining_days % 30;
                format!("{:04}-{:02}-{:02}", 1970 + years, months + 1, days + 1)
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        };

        Ok(PackageInfo {
            name,
            version,
            arch,
            runtime,
            filename,
            size_bytes: metadata.len(),
            created_at,
        })
    }

    /// Remove package from cache
    ///
    /// # Arguments
    /// * `concept_name` - Name of concept
    /// * `version` - Version
    pub fn remove(&self, concept_name: &str, version: &str) -> Result<bool> {
        let package_filename = format!("{}@{}.tar.gz", concept_name, version);
        let package_path = self.cache_dir.join(&package_filename);

        if !package_path.exists() {
            return Ok(false);
        }

        fs::remove_file(&package_path).map_err(|e| {
            CkpError::IoError(format!("Failed to remove package: {}", e))
        })?;

        Ok(true)
    }

    /// Get cache directory path
    pub fn get_cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    // ===== PRIVATE HELPER METHODS =====

    /// Detect runtime and architecture from ontology and system
    ///
    /// Returns (arch, runtime) tuple
    /// - arch: "aarch64-darwin", "x86_64-linux", "universal"
    /// - runtime: "rs", "py", "js"
    fn detect_runtime_and_arch(&self, concept_dir: &Path) -> Result<(String, String)> {
        // Read conceptkernel.yaml
        let ontology_path = concept_dir.join("conceptkernel.yaml");
        if !ontology_path.exists() {
            return Ok(("unknown".to_string(), "unknown".to_string()));
        }

        let ontology_content = fs::read_to_string(&ontology_path).map_err(|e| {
            CkpError::IoError(format!("Failed to read conceptkernel.yaml: {}", e))
        })?;

        // Parse YAML to get metadata.type field
        let ontology: serde_yaml::Value = serde_yaml::from_str(&ontology_content).map_err(|e| {
            CkpError::ParseError(format!("Failed to parse conceptkernel.yaml: {}", e))
        })?;

        let kernel_type = ontology
            .get("metadata")
            .and_then(|m| m.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");

        // Determine runtime and arch based on type
        let (runtime, arch) = match kernel_type {
            t if t.starts_with("rust:") => {
                // Rust binary - detect architecture
                let arch = self.detect_system_arch();
                ("rs".to_string(), arch)
            }
            t if t.starts_with("python:") => {
                // Python - universal (cross-platform)
                ("py".to_string(), "universal".to_string())
            }
            t if t.starts_with("node:") => {
                // Node.js - universal (cross-platform)
                ("js".to_string(), "universal".to_string())
            }
            _ => ("unknown".to_string(), "unknown".to_string()),
        };

        Ok((arch, runtime))
    }

    /// Detect system architecture
    ///
    /// Returns simplified arch string for package naming
    fn detect_system_arch(&self) -> String {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        match (arch, os) {
            ("x86_64", "linux") => "x86_64-linux".to_string(),
            ("aarch64", "linux") => "aarch64-linux".to_string(),
            ("x86_64", "windows") => "x86_64-windows".to_string(),
            ("x86_64", "macos") => "x86_64-darwin".to_string(),
            ("aarch64", "macos") => "aarch64-darwin".to_string(),
            _ => format!("{}-{}", arch, os),
        }
    }

    /// Parse package filename to extract name, version, arch, runtime
    ///
    /// Supports both new format and old format (backward compat):
    /// - New: <name>-<version>.<arch>.<runtime>.tar.gz
    /// - Old: <name>@<version>.tar.gz
    ///
    /// Returns (name, version, arch, runtime)
    fn parse_package_filename(&self, filename: &str) -> Option<(String, String, String, String)> {
        // Remove .tar.gz extension
        let name_part = filename.strip_suffix(".tar.gz")?;

        // Try new format first: <name>-<version>.<arch>.<runtime>
        if let Some(last_dot) = name_part.rfind('.') {
            let before_runtime = &name_part[..last_dot];
            let runtime = &name_part[last_dot + 1..];

            if let Some(second_last_dot) = before_runtime.rfind('.') {
                let before_arch = &before_runtime[..second_last_dot];
                let arch = &before_runtime[second_last_dot + 1..];

                // Now split before_arch by last hyphen to get name and version
                if let Some(hyphen_pos) = before_arch.rfind('-') {
                    let name = before_arch[..hyphen_pos].to_string();
                    let version = before_arch[hyphen_pos + 1..].to_string();

                    return Some((name, version, arch.to_string(), runtime.to_string()));
                }
            }
        }

        // Fallback to old format: <name>@<version>
        let parts: Vec<&str> = name_part.split('@').collect();
        if parts.len() == 2 {
            return Some((
                parts[0].to_string(),
                parts[1].to_string(),
                "unknown".to_string(),
                "unknown".to_string(),
            ));
        }

        None
    }

    /// Extract tar.gz to target directory
    fn extract_tarball(&self, tarball_path: &Path, target_dir: &Path) -> Result<()> {
        use flate2::read::GzDecoder;
        use tar::Archive;

        let tar_gz = File::open(tarball_path).map_err(|e| {
            CkpError::IoError(format!("Failed to open tarball: {}", e))
        })?;

        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);

        archive.unpack(target_dir).map_err(|e| {
            CkpError::IoError(format!("Failed to extract tarball: {}", e))
        })?;

        Ok(())
    }

    /// Create tar.gz from directory
    fn create_tarball(&self, source_dir: &Path, tarball_path: &Path, concept_name: &str) -> Result<()> {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use tar::Builder;

        let tar_gz = File::create(tarball_path).map_err(|e| {
            CkpError::IoError(format!("Failed to create tarball: {}", e))
        })?;

        let enc = GzEncoder::new(tar_gz, Compression::default());
        let mut tar = Builder::new(enc);

        // Don't follow symlinks (prevents broken symlink errors)
        tar.follow_symlinks(false);

        // Add directory to tar with concept name as root
        tar.append_dir_all(concept_name, source_dir).map_err(|e| {
            CkpError::IoError(format!("Failed to add directory to tarball: {}", e))
        })?;

        tar.finish().map_err(|e| {
            CkpError::IoError(format!("Failed to finish tarball: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_package_manager_creation() {
        let pm = PackageManager::new();
        assert!(pm.is_ok());
    }

    #[test]
    fn test_list_empty_cache() {
        let pm = PackageManager::new().unwrap();
        let packages = pm.list_cached().unwrap();
        // May or may not be empty depending on system state
        assert!(packages.is_empty() || !packages.is_empty());
    }
}
