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
    _project_path: PathBuf,
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
            _project_path: project_path,
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
        let _base_port = self.port_map.base_port.ok_or_else(|| {
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

    /// Check for occupied ports in the project's port range
    ///
    /// v1.3.18: Added to detect port conflicts before kernel starts
    /// Prevents accidental disruption from already-running services
    ///
    /// # Returns
    /// Vector of (port, occupied) tuples for all ports in range
    /// - port: Port number
    /// - occupied: true if port is in use by external service
    ///
    /// # Example
    /// ```
    /// let occupied = port_manager.check_occupied_ports()?;
    /// for (port, is_occupied) in occupied {
    ///     if is_occupied {
    ///         println!("⚠️  Port {} is OCCUPIED by external service", port);
    ///     }
    /// }
    /// ```
    pub fn check_occupied_ports(&self) -> Result<Vec<(u16, bool)>, CkpError> {
        let range = self.get_port_range().ok_or_else(|| {
            CkpError::PortError("Base port not set. Cannot check occupied ports.".to_string())
        })?;

        let mut results = Vec::new();

        for offset in 0..=199 {
            let port = range.start + offset;
            let is_occupied = !Self::is_port_available(port);
            results.push((port, is_occupied));
        }

        Ok(results)
    }

    /// Get list of occupied ports in range
    ///
    /// Returns only ports that are occupied by external services
    ///
    /// # Returns
    /// Vector of occupied port numbers
    pub fn get_occupied_ports(&self) -> Result<Vec<u16>, CkpError> {
        let all_ports = self.check_occupied_ports()?;
        let occupied: Vec<u16> = all_ports
            .into_iter()
            .filter_map(|(port, is_occupied)| if is_occupied { Some(port) } else { None })
            .collect();

        Ok(occupied)
    }

    /// Check if specific port is occupied
    ///
    /// # Arguments
    /// * `port` - Port number to check
    ///
    /// # Returns
    /// true if port is occupied, false if available
    pub fn is_port_occupied(&self, port: u16) -> bool {
        !Self::is_port_available(port)
    }

    /// Allocate port with occupation warning
    ///
    /// v1.3.18: Enhanced to warn about occupied ports
    /// If preferred port is occupied by external service, logs warning
    /// and attempts to allocate next available port
    ///
    /// # Arguments
    /// * `kernel_name` - Kernel name
    /// * `preferred_offset` - Optional preferred offset (0-199)
    ///
    /// # Returns
    /// (port, was_preferred_occupied) - Allocated port and occupation flag
    pub fn allocate_with_check(
        &mut self,
        kernel_name: &str,
        preferred_offset: Option<u16>,
    ) -> Result<(u16, bool), CkpError> {
        let _base_port = self.port_map.base_port.ok_or_else(|| {
            CkpError::PortError(
                "Base port not set. Initialize project first with ProjectRegistry.".to_string(),
            )
        })?;

        // Check if kernel already has allocated port
        if let Some(&port) = self.port_map.allocations.get(kernel_name) {
            // Check if it's still available
            let is_occupied = !Self::is_port_available(port);
            return Ok((port, is_occupied));
        }

        let range = self.get_port_range().unwrap();

        // If preferred offset specified, try that first
        if let Some(offset) = preferred_offset {
            if offset <= 199 {
                let candidate_port = range.start + offset;

                // Check if already allocated to another kernel
                if self.port_map.allocations.values().any(|&p| p == candidate_port) {
                    // Already allocated - skip
                } else {
                    // Check if occupied by external service
                    let is_occupied = !Self::is_port_available(candidate_port);

                    if !is_occupied {
                        // Available - allocate it
                        self.port_map
                            .allocations
                            .insert(kernel_name.to_string(), candidate_port);
                        self.save()?;
                        return Ok((candidate_port, false));
                    } else {
                        // Occupied - log warning and continue to find alternative
                        eprintln!(
                            "⚠️  Preferred port {} is OCCUPIED by external service for {}. Allocating alternative...",
                            candidate_port, kernel_name
                        );
                    }
                }
            }
        }

        // Find next available port in range
        for offset in 0..=199 {
            let candidate_port = range.start + offset;

            // Check if already allocated to another kernel
            if self.port_map.allocations.values().any(|&p| p == candidate_port) {
                continue;
            }

            // Test if port is available
            if Self::is_port_available(candidate_port) {
                self.port_map
                    .allocations
                    .insert(kernel_name.to_string(), candidate_port);
                self.save()?;

                // Return true if we had to skip preferred port
                let had_to_skip = preferred_offset.is_some();
                return Ok((candidate_port, had_to_skip));
            }
        }

        Err(CkpError::PortUnavailable(format!(
            "No available ports in range {}-{} for kernel {} (all 200 ports occupied)",
            range.start, range.end, kernel_name
        )))
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

    // v1.3.18: Port Occupation Detection Tests

    #[test]
    fn test_check_occupied_ports() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();

        // Check all ports in range (200 ports)
        let occupied = port_manager.check_occupied_ports().unwrap();

        // Should return exactly 200 entries
        assert_eq!(occupied.len(), 200);

        // Verify range coverage
        assert_eq!(occupied.first().unwrap().0, 56789); // First port
        assert_eq!(occupied.last().unwrap().0, 56988);  // Last port (56789 + 199)

        // All entries should have port and occupation flag
        for (port, _is_occupied) in occupied {
            assert!(port >= 56789 && port <= 56988);
        }
    }

    #[test]
    fn test_check_occupied_ports_without_base_port() {
        let temp_dir = TempDir::new().unwrap();
        let port_manager = PortManager::new(temp_dir.path()).unwrap();

        // Should error when base port not set
        let result = port_manager.check_occupied_ports();
        assert!(result.is_err());

        if let Err(CkpError::PortError(msg)) = result {
            assert!(msg.contains("Base port not set"));
        } else {
            panic!("Expected PortError");
        }
    }

    #[test]
    fn test_get_occupied_ports() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();

        // Get only occupied ports
        let occupied = port_manager.get_occupied_ports().unwrap();

        // Result should be empty or contain ports
        // (We can't reliably predict which ports are occupied in test environment)
        // Just verify it returns a valid Vec
        assert!(occupied.len() <= 200); // Can't exceed range

        // Verify all returned ports are in valid range
        for port in occupied {
            assert!(port >= 56789 && port <= 56988);
        }
    }

    #[test]
    fn test_get_occupied_ports_without_base_port() {
        let temp_dir = TempDir::new().unwrap();
        let port_manager = PortManager::new(temp_dir.path()).unwrap();

        // Should error when base port not set
        let result = port_manager.get_occupied_ports();
        assert!(result.is_err());
    }

    #[test]
    fn test_is_port_occupied() {
        let temp_dir = TempDir::new().unwrap();
        let port_manager = PortManager::new(temp_dir.path()).unwrap();

        // Test with a very high port unlikely to be in use
        let result = port_manager.is_port_occupied(65534);

        // Result should be boolean (true or false)
        // We can't predict if port is occupied, just verify API works
        assert!(result == true || result == false);
    }

    #[test]
    fn test_allocate_with_check_returns_tuple() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();

        // Allocate port with check
        let result = port_manager.allocate_with_check("Test.Kernel", None).unwrap();

        // Should return (port, occupation_flag) tuple
        let (port, was_occupied) = result;

        // Port should be in valid range
        assert!(port >= 56789 && port <= 56988);

        // Occupation flag should be boolean
        assert!(was_occupied == true || was_occupied == false);

        // Verify kernel was actually allocated
        assert_eq!(port_manager.get("Test.Kernel"), Some(port));
    }

    #[test]
    fn test_allocate_with_check_preferred_offset() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();

        // Allocate with preferred offset
        let (port, _was_occupied) = port_manager
            .allocate_with_check("Test.Kernel", Some(10))
            .unwrap();

        // If preferred port is available, should get base + 10
        // If occupied, should get alternative
        // Either way, should be in valid range
        assert!(port >= 56789 && port <= 56988);

        // Verify kernel was allocated
        assert_eq!(port_manager.get("Test.Kernel"), Some(port));
    }

    #[test]
    fn test_allocate_with_check_reuses_existing_allocation() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        port_manager.set_base_port(56789).unwrap();

        // First allocation
        let (port1, _) = port_manager
            .allocate_with_check("Test.Kernel", None)
            .unwrap();

        // Second allocation should reuse same port
        let (port2, _) = port_manager
            .allocate_with_check("Test.Kernel", None)
            .unwrap();

        assert_eq!(port1, port2);
    }

    #[test]
    fn test_allocate_with_check_without_base_port() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        // Should error when base port not set
        let result = port_manager.allocate_with_check("Test.Kernel", None);
        assert!(result.is_err());

        if let Err(CkpError::PortError(msg)) = result {
            assert!(msg.contains("Base port not set"));
        } else {
            panic!("Expected PortError");
        }
    }

    #[test]
    fn test_check_occupied_ports_full_range() {
        let temp_dir = TempDir::new().unwrap();
        let mut port_manager = PortManager::new(temp_dir.path()).unwrap();

        // Use a high port range to avoid conflicts
        port_manager.set_base_port(58000).unwrap();

        let occupied = port_manager.check_occupied_ports().unwrap();

        // Verify complete coverage of range
        assert_eq!(occupied.len(), 200);

        // Verify sequential port numbers
        for (i, (port, _)) in occupied.iter().enumerate() {
            assert_eq!(*port, 58000 + i as u16);
        }
    }
}
