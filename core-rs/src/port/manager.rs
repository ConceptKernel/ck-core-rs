/**
 * manager.rs
 * Per-project dynamic port allocation
 *
 * Each project has:
 * - Base port (discovery port): Always base of range
 * - Port range: [base, base+199]
 * - Dynamic allocation: Assigns ports within range to kernels
 *
 * Port allocation strategy:
 * - Discovery port (base): Reserved for System.Registry or primary gateway
 * - Offset 0: base (discovery)
 * - Offset 1-199: Dynamic allocation for hot kernels
 *
 * Example:
 * - Project slot 1: base=56789, range=[56789, 56988]
 * - System.Registry → 56789 (offset 0, discovery)
 * - System.Gateway.HTTP → 56790 (offset 1)
 * - System.WssHub → 56791 (offset 2)
 *
 * Reference: Node.js v1.3.14 - PortManager.js
 */

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use crate::errors::CkpError;

/// Port allocation map structure (.ckports file format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMap {
    pub base_port: Option<u16>,
    pub allocations: HashMap<String, u16>,
}

/// Port range for a project
#[derive(Debug, Clone, PartialEq)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
}

impl PortRange {
    /// Check if port is within this range
    pub fn contains(&self, port: u16) -> bool {
        port >= self.start && port <= self.end
    }
}

/// Port Manager - manages .ckports file and dynamic port allocation
pub struct PortManager {
    project_path: PathBuf,
    port_map_path: PathBuf,
    port_map: PortMap,
}

impl PortManager {
    /// Create a new PortManager for a project
    ///
    /// # Arguments
    /// * `project_path` - Path to project root directory
    ///
    /// # Example
    /// ```
    /// let port_manager = PortManager::new(".")?;
    /// ```
    pub fn new<P: AsRef<Path>>(project_path: P) -> Result<Self, CkpError> {
        let project_path = project_path.as_ref().to_path_buf();
        let port_map_path = project_path.join(".ckports");
        let port_map = Self::load_port_map(&port_map_path)?;

        Ok(PortManager {
            project_path,
            port_map_path,
            port_map,
        })
    }

    /// Load port allocation map from .ckports file
    ///
    /// Returns empty map if file doesn't exist
    fn load_port_map(port_map_path: &Path) -> Result<PortMap, CkpError> {
        if !port_map_path.exists() {
            return Ok(PortMap {
                base_port: None,
                allocations: HashMap::new(),
            });
        }

        let content = fs::read_to_string(port_map_path).map_err(|e| {
            CkpError::IoError(format!("Failed to read .ckports: {}", e))
        })?;

        let port_map: PortMap = serde_json::from_str(&content).map_err(|e| {
            eprintln!("Failed to parse .ckports, resetting: {}", e);
            // Return empty map if parsing fails
            return CkpError::ParseError(format!("Invalid .ckports JSON: {}", e));
        })?;

        Ok(port_map)
    }

    /// Save port allocation map to .ckports file
    fn save(&self) -> Result<(), CkpError> {
        let json = serde_json::to_string_pretty(&self.port_map).map_err(|e| {
            CkpError::SerializationError(format!("Failed to serialize .ckports: {}", e))
        })?;

        fs::write(&self.port_map_path, json).map_err(|e| {
            CkpError::IoError(format!("Failed to write .ckports: {}", e))
        })?;

        Ok(())
    }

    /// Set base port for this project
    ///
    /// # Arguments
    /// * `base_port` - Base port (discovery port)
    pub fn set_base_port(&mut self, base_port: u16) -> Result<(), CkpError> {
        self.port_map.base_port = Some(base_port);
        self.save()
    }

    /// Get base port
    ///
    /// # Returns
    /// Base port or None if not set
    pub fn get_base_port(&self) -> Option<u16> {
        self.port_map.base_port
    }

    /// Calculate port range
    ///
    /// # Returns
    /// Port range or None if base port not set
    pub fn get_port_range(&self) -> Option<PortRange> {
        self.port_map.base_port.map(|base| PortRange {
            start: base,
            end: base + 199,
        })
    }

    /// Test if port is available
    ///
    /// # Arguments
    /// * `port` - Port to test
    ///
    /// # Returns
    /// true if available, false if in use
    pub fn is_port_available(port: u16) -> bool {
        TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    /// Allocate port for a kernel
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name
    /// * `preferred_offset` - Optional preferred offset (0-199)
    ///
    /// # Returns
    /// Allocated port number
    ///
    /// # Errors
    /// Returns error if:
    /// - Base port not set
    /// - No available ports in range
    pub fn allocate(
        &mut self,
        kernel_name: &str,
        preferred_offset: Option<u16>,
    ) -> Result<u16, CkpError> {
        let base_port = self.port_map.base_port.ok_or_else(|| {
            CkpError::PortError(
                "Base port not set. Initialize project first with ProjectRegistry.".to_string(),
            )
        })?;

        // Check if kernel already has allocated port
        if let Some(&port) = self.port_map.allocations.get(kernel_name) {
            return Ok(port);
        }

        let range = self.get_port_range().unwrap();

        // If preferred offset specified, try that first
        if let Some(offset) = preferred_offset {
            if offset <= 199 {
                let candidate_port = range.start + offset;

                // Check if already allocated
                if !self.port_map.allocations.values().any(|&p| p == candidate_port) {
                    if Self::is_port_available(candidate_port) {
                        self.port_map
                            .allocations
                            .insert(kernel_name.to_string(), candidate_port);
                        self.save()?;
                        return Ok(candidate_port);
                    }
                }
            }
        }

        // Find next available port in range
        for offset in 0..=199 {
            let candidate_port = range.start + offset;

            // Check if already allocated to another kernel
            if self.port_map.allocations.values().any(|&p| p == candidate_port) {
                continue; // Already allocated
            }

            // Test if port is available
            if Self::is_port_available(candidate_port) {
                self.port_map
                    .allocations
                    .insert(kernel_name.to_string(), candidate_port);
                self.save()?;
                return Ok(candidate_port);
            }
        }

        Err(CkpError::PortUnavailable(format!(
            "No available ports in range {}-{} for kernel {}",
            range.start, range.end, kernel_name
        )))
    }

    /// Get allocated port for a kernel
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name
    ///
    /// # Returns
    /// Port number or None if not allocated
    pub fn get(&self, kernel_name: &str) -> Option<u16> {
        self.port_map.allocations.get(kernel_name).copied()
    }

    /// Release port allocation for a kernel
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name
    ///
    /// # Returns
    /// true if released, false if not allocated
    pub fn release(&mut self, kernel_name: &str) -> Result<bool, CkpError> {
        if self.port_map.allocations.remove(kernel_name).is_some() {
            self.save()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get all allocated ports
    ///
    /// # Returns
    /// HashMap of kernel names to ports
    pub fn get_all_allocations(&self) -> &HashMap<String, u16> {
        &self.port_map.allocations
    }

    /// Clear all allocations (keeps base port)
    pub fn clear_allocations(&mut self) -> Result<(), CkpError> {
        self.port_map.allocations.clear();
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_new_empty_port_map() {
        let temp_dir = TempDir::new().unwrap();
        let port_manager = PortManager::new(temp_dir.path()).unwrap();

        assert_eq!(port_manager.get_base_port(), None);
        assert_eq!(port_manager.get_all_allocations().len(), 0);
    }

    #[test]
    fn test_set_base_port() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();

        assert_eq!(port_manager.get_base_port(), Some(56789));

        // Verify .ckports file was created
        let ckports_path = temp_dir.path().join(".ckports");
        assert!(ckports_path.exists());
    }

    #[test]
    fn test_get_port_range() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        // No base port set
        assert_eq!(port_manager.get_port_range(), None);

        // Set base port
        port_manager.set_base_port(56789).unwrap();

        let range = port_manager.get_port_range().unwrap();
        assert_eq!(range.start, 56789);
        assert_eq!(range.end, 56988);
    }

    #[test]
    fn test_allocate_port() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();

        // Allocate first port (should get base port)
        let port1 = port_manager.allocate("System.Gateway.HTTP", None).unwrap();
        assert_eq!(port1, 56789);

        // Allocate second port
        let port2 = port_manager.allocate("System.WssHub", None).unwrap();
        assert!(port2 > 56789 && port2 <= 56988);

        // Re-allocating same kernel should return same port
        let port1_again = port_manager.allocate("System.Gateway.HTTP", None).unwrap();
        assert_eq!(port1_again, port1);
    }

    #[test]
    fn test_allocate_with_preferred_offset() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();

        // Allocate with preferred offset 5
        let port = port_manager
            .allocate("Test.Kernel", Some(5))
            .unwrap();
        assert_eq!(port, 56794); // 56789 + 5
    }

    #[test]
    fn test_allocate_without_base_port() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        // Try to allocate without setting base port
        let result = port_manager.allocate("Test.Kernel", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_allocated_port() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();

        // No allocation yet
        assert_eq!(port_manager.get("Test.Kernel"), None);

        // Allocate
        let port = port_manager.allocate("Test.Kernel", None).unwrap();

        // Get should return allocated port
        assert_eq!(port_manager.get("Test.Kernel"), Some(port));
    }

    #[test]
    fn test_release_port() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();

        // Allocate
        port_manager.allocate("Test.Kernel", None).unwrap();
        assert!(port_manager.get("Test.Kernel").is_some());

        // Release
        let released = port_manager.release("Test.Kernel").unwrap();
        assert!(released);
        assert_eq!(port_manager.get("Test.Kernel"), None);

        // Release non-existent
        let not_released = port_manager.release("NonExistent").unwrap();
        assert!(!not_released);
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create and allocate
        {
            let mut port_manager = PortManager::new(temp_dir.path()).unwrap();
            port_manager.set_base_port(56789).unwrap();
            port_manager.allocate("Kernel1", None).unwrap();
            port_manager.allocate("Kernel2", None).unwrap();
        }

        // Load again
        {
            let port_manager = PortManager::new(temp_dir.path()).unwrap();
            assert_eq!(port_manager.get_base_port(), Some(56789));
            assert!(port_manager.get("Kernel1").is_some());
            assert!(port_manager.get("Kernel2").is_some());
        }
    }

    #[test]
    fn test_clear_allocations() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();
        port_manager.allocate("Kernel1", None).unwrap();
        port_manager.allocate("Kernel2", None).unwrap();

        assert_eq!(port_manager.get_all_allocations().len(), 2);

        port_manager.clear_allocations().unwrap();

        assert_eq!(port_manager.get_all_allocations().len(), 0);
        // Base port should still be set
        assert_eq!(port_manager.get_base_port(), Some(56789));
    }

    #[test]
    fn test_port_range_contains() {
        let range = PortRange {
            start: 56789,
            end: 56988,
        };

        assert!(range.contains(56789));
        assert!(range.contains(56888));
        assert!(range.contains(56988));
        assert!(!range.contains(56788));
        assert!(!range.contains(56989));
    }
}
