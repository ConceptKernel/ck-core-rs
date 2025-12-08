//! Kernel build system using proper abstractions
//!
//! Builds Rust kernels using OntologyReader for metadata and proper URN resolution

use crate::errors::{CkpError, Result};
use crate::ontology::OntologyReader;
use crate::drivers::{GitDriver, VersionDriver};
use std::path::PathBuf;
use std::process::Command;

/// Kernel builder that uses proper ConceptKernel abstractions
pub struct KernelBuilder {
    root: PathBuf,
    ontology_reader: OntologyReader,
}

impl KernelBuilder {
    /// Create new builder from project root
    pub fn new(root: PathBuf) -> Self {
        let ontology_reader = OntologyReader::new(root.clone());
        Self { root, ontology_reader }
    }

    /// Build a single kernel using its ontology metadata
    ///
    /// This method:
    /// 1. Reads kernel metadata via OntologyReader (not direct file access)
    /// 2. Uses the entrypoint field to locate build directory
    /// 3. Invokes cargo build with proper paths
    /// 4. Returns build artifacts location
    pub fn build_kernel(&self, kernel_name: &str, release: bool) -> Result<PathBuf> {
        println!("[KernelBuilder] Building kernel: {}", kernel_name);

        // Use OntologyReader to get kernel metadata (proper abstraction)
        let ontology = self.ontology_reader.read_by_kernel_name(kernel_name)
            .map_err(|e| CkpError::BuildError(format!(
                "Failed to read ontology for {}: {}",
                kernel_name, e
            )))?;

        // Check if this is a Rust kernel
        if !ontology.metadata.kernel_type.starts_with("rust:") {
            return Err(CkpError::BuildError(format!(
                "Kernel {} is not a Rust kernel (type: {})",
                kernel_name, ontology.metadata.kernel_type
            )));
        }

        // Get entrypoint from ontology
        let entrypoint_raw = ontology.metadata.entrypoint
            .as_ref()
            .ok_or_else(|| CkpError::BuildError(format!(
                "Kernel {} has no entrypoint defined in ontology",
                kernel_name
            )))?;

        // Extract build directory from entrypoint
        // Entrypoint might be:
        // - "tool/rs" (correct - points to Cargo project)
        // - "tool/rs/target/release/binary" (legacy - points to binary)
        // We need the directory containing Cargo.toml
        let build_subdir = if entrypoint_raw.contains("target/") {
            // Legacy format - extract up to /rs/
            entrypoint_raw.split("/target/").next().unwrap_or(entrypoint_raw)
        } else {
            entrypoint_raw.as_str()
        };

        println!("[KernelBuilder] Kernel type: {}", ontology.metadata.kernel_type);
        println!("[KernelBuilder] Version: {}", ontology.metadata.version.as_ref().unwrap_or(&"unknown".to_string()));
        println!("[KernelBuilder] Build directory: {}", build_subdir);

        // Build path: concepts/{kernel_name}/{build_subdir}
        let build_dir = self.root
            .join("concepts")
            .join(kernel_name)
            .join(build_subdir);

        if !build_dir.join("Cargo.toml").exists() {
            return Err(CkpError::BuildError(format!(
                "No Cargo.toml found at: {}",
                build_dir.display()
            )));
        }

        println!("[KernelBuilder] Build directory: {}", build_dir.display());

        // Invoke cargo build
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .current_dir(&build_dir);

        if release {
            cmd.arg("--release");
        }

        println!("[KernelBuilder] Running: cargo build{}", if release { " --release" } else { "" });

        let output = cmd.output()
            .map_err(|e| CkpError::BuildError(format!(
                "Failed to execute cargo: {}",
                e
            )))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CkpError::BuildError(format!(
                "Build failed for {}: {}",
                kernel_name, stderr
            )));
        }

        // Return path to built binary
        let target_dir = build_dir.join("target");
        let profile_dir = if release {
            target_dir.join("release")
        } else {
            target_dir.join("debug")
        };

        println!("[KernelBuilder] ✓ Build successful: {}", kernel_name);
        println!("[KernelBuilder] Binaries at: {}", profile_dir.display());

        Ok(profile_dir)
    }

    /// Build all Rust kernels in the project
    ///
    /// Discovers kernels using OntologyReader, then builds each Rust kernel
    pub fn build_all(&self, release: bool) -> Result<Vec<String>> {
        println!("[KernelBuilder] Discovering kernels...");

        let concepts_dir = self.root.join("concepts");
        if !concepts_dir.exists() {
            return Err(CkpError::BuildError(
                "concepts/ directory not found".to_string()
            ));
        }

        let mut built_kernels = Vec::new();
        let mut failed_kernels = Vec::new();

        // Scan concepts directory for kernels
        let entries = std::fs::read_dir(&concepts_dir)
            .map_err(|e| CkpError::BuildError(format!(
                "Failed to read concepts directory: {}",
                e
            )))?;

        for entry in entries {
            let entry = entry.map_err(|e| CkpError::BuildError(e.to_string()))?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let kernel_name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Skip hidden directories
            if kernel_name.starts_with('.') {
                continue;
            }

            // Try to read ontology - if it fails, skip this kernel
            let ontology = match self.ontology_reader.read_by_kernel_name(&kernel_name) {
                Ok(o) => o,
                Err(_) => {
                    println!("[KernelBuilder] ⊘ Skipping {} (no ontology)", kernel_name);
                    continue;
                }
            };

            // Only build Rust kernels
            if !ontology.metadata.kernel_type.starts_with("rust:") {
                continue;
            }

            // Build the kernel
            match self.build_kernel(&kernel_name, release) {
                Ok(_) => {
                    built_kernels.push(kernel_name);
                }
                Err(e) => {
                    eprintln!("[KernelBuilder] ✗ Failed to build {}: {}", kernel_name, e);
                    failed_kernels.push(kernel_name);
                }
            }
        }

        println!("\n[KernelBuilder] ═══════════════════════════════");
        println!("[KernelBuilder] Build Summary");
        println!("[KernelBuilder] ═══════════════════════════════");
        println!("[KernelBuilder] Total:   {} kernels", built_kernels.len() + failed_kernels.len());
        println!("[KernelBuilder] Success: {} kernels", built_kernels.len());
        println!("[KernelBuilder] Failed:  {} kernels", failed_kernels.len());

        if !failed_kernels.is_empty() {
            println!("\n[KernelBuilder] Failed kernels:");
            for kernel in &failed_kernels {
                println!("[KernelBuilder]   - {}", kernel);
            }
        }

        Ok(built_kernels)
    }

    /// Check if kernel needs rebuilding based on git version and modification times
    ///
    /// Uses GitDriver as Single Source of Truth for versioning.
    /// Rebuild is needed if:
    /// 1. Binary doesn't exist
    /// 2. Git repository has uncommitted changes (not clean)
    /// 3. Source files are newer than binary (fallback check)
    pub fn needs_rebuild(&self, kernel_name: &str, release: bool) -> Result<bool> {
        // Use OntologyReader to get entrypoint
        let ontology = self.ontology_reader.read_by_kernel_name(kernel_name)?;

        let entrypoint_raw = ontology.metadata.entrypoint
            .as_ref()
            .ok_or_else(|| CkpError::BuildError(format!(
                "Kernel {} has no entrypoint",
                kernel_name
            )))?;

        // Extract build directory from entrypoint (same logic as build_kernel)
        let build_subdir = if entrypoint_raw.contains("target/") {
            entrypoint_raw.split("/target/").next().unwrap_or(entrypoint_raw)
        } else {
            entrypoint_raw.as_str()
        };

        let kernel_dir = self.root.join("concepts").join(kernel_name).join(build_subdir);
        let binary_name = build_subdir.split('/').last().unwrap_or(kernel_name);

        let target_dir = if release { "release" } else { "debug" };
        let binary_path = kernel_dir.join("target").join(target_dir).join(binary_name);

        // If binary doesn't exist, needs rebuild
        if !binary_path.exists() {
            return Ok(true);
        }

        // Use GitDriver as SSOT for version checking
        let kernel_path = self.root.join("concepts").join(kernel_name);
        let git_driver = GitDriver::new(kernel_path.clone(), kernel_name.to_string());

        // Check if there are uncommitted changes (git is not clean)
        match git_driver.get_version() {
            Ok(Some(version_info)) => {
                if !version_info.is_clean {
                    println!("[KernelBuilder] Git has uncommitted changes, rebuild needed: {}", kernel_name);
                    return Ok(true);
                }
            }
            Ok(None) => {
                // No git tags/version, fall through to file mtime check
            }
            Err(_) => {
                // Git not available or error, fall through to file mtime check
            }
        }

        // Fallback: Check if any source file is newer than binary
        let binary_mtime = std::fs::metadata(&binary_path)
            .map_err(|e| CkpError::IoError(e.to_string()))?
            .modified()
            .map_err(|e| CkpError::IoError(e.to_string()))?;

        let src_dir = kernel_dir.join("src");
        if !src_dir.exists() {
            return Ok(false);
        }

        for entry in walkdir::WalkDir::new(&src_dir) {
            let entry = entry.map_err(|e| CkpError::IoError(e.to_string()))?;

            if entry.path().extension().and_then(|s| s.to_str()) == Some("rs") {
                let source_mtime = entry.metadata()
                    .map_err(|e| CkpError::IoError(e.to_string()))?
                    .modified()
                    .map_err(|e| CkpError::IoError(e.to_string()))?;
                if source_mtime > binary_mtime {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_builder_requires_ontology() {
        let temp_dir = TempDir::new().unwrap();
        let builder = KernelBuilder::new(temp_dir.path().to_path_buf());

        // Should fail when kernel doesn't have ontology
        let result = builder.build_kernel("NonExistentKernel", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_rejects_non_rust_kernels() {
        let temp_dir = TempDir::new().unwrap();
        let concepts_dir = temp_dir.path().join("concepts");
        let kernel_dir = concepts_dir.join("NodeKernel");
        fs::create_dir_all(&kernel_dir).unwrap();

        // Create ontology for node kernel
        let ontology = r#"apiVersion: conceptkernel/v1
kind: Ontology
metadata:
  name: NodeKernel
  type: node:cold
  version: v0.1
"#;
        fs::write(kernel_dir.join("conceptkernel.yaml"), ontology).unwrap();

        let builder = KernelBuilder::new(temp_dir.path().to_path_buf());
        let result = builder.build_kernel("NodeKernel", false);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a Rust kernel"));
    }
}
