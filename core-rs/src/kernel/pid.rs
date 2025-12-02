//! PID file management to prevent duplicate governors

use crate::errors::{CkpError, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// PID file manager
#[derive(Debug)]
pub struct PidFile {
    path: PathBuf,
    pid: u32,
}

impl PidFile {
    /// Create a new PID file
    ///
    /// Returns an error if a PID file already exists and the process is still running
    ///
    /// # Example
    ///
    /// ```no_run
    /// use ckp_core::kernel::PidFile;
    /// use std::path::Path;
    ///
    /// let pid_file = PidFile::create(Path::new("/test/kernel/tool/.governor.pid")).unwrap();
    /// // Governor runs...
    /// // PID file automatically deleted on drop
    /// ```
    pub fn create(path: &Path) -> Result<Self> {
        let pid = std::process::id();

        // Check if PID file already exists
        if path.exists() {
            let existing_pid = fs::read_to_string(path)?;
            let existing_pid = existing_pid.trim();

            // Check if process is still running
            if Self::is_process_running(existing_pid) {
                return Err(CkpError::Governor(format!(
                    "Governor already running with PID {} (PID file: {})",
                    existing_pid,
                    path.display()
                )));
            } else {
                // Stale PID file, remove it
                fs::remove_file(path).ok();
            }
        }

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write PID file
        fs::write(path, pid.to_string())?;

        Ok(Self {
            path: path.to_path_buf(),
            pid,
        })
    }

    /// Check if a process with the given PID is running
    ///
    /// # Platform-specific behavior
    ///
    /// - Unix: Uses `kill -0` to check if process exists
    /// - Windows: Uses sysinfo crate
    fn is_process_running(pid_str: &str) -> bool {
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => return false,
        };

        #[cfg(unix)]
        {
            // Use kill -0 to check if process exists
            use std::process::Command;
            Command::new("kill")
                .arg("-0")
                .arg(pid.to_string())
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        }

        #[cfg(windows)]
        {
            use sysinfo::{PidExt, ProcessExt, System, SystemExt};
            let mut sys = System::new();
            sys.refresh_processes();
            sys.process(sysinfo::Pid::from_u32(pid)).is_some()
        }
    }

    /// Get the PID
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Get the PID file path
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PidFile {
    fn drop(&mut self) {
        // Remove PID file when governor exits
        fs::remove_file(&self.path).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_pid_file() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join(".governor.pid");

        let pid_file = PidFile::create(&pid_path).unwrap();

        assert!(pid_path.exists());
        assert_eq!(pid_file.pid(), std::process::id());

        let content = fs::read_to_string(&pid_path).unwrap();
        assert_eq!(content, std::process::id().to_string());
    }

    #[test]
    fn test_prevent_duplicate() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join(".governor.pid");

        let _pid_file1 = PidFile::create(&pid_path).unwrap();

        // Try to create another PID file - should fail
        let result = PidFile::create(&pid_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("already running"));
    }

    #[test]
    fn test_cleanup_stale_pid() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join(".governor.pid");

        // Write a stale PID (very unlikely to be running)
        fs::write(&pid_path, "999999").unwrap();

        // Should successfully create new PID file, removing stale one
        let pid_file = PidFile::create(&pid_path).unwrap();
        assert_eq!(pid_file.pid(), std::process::id());
    }

    #[test]
    fn test_auto_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let pid_path = temp_dir.path().join(".governor.pid");

        {
            let _pid_file = PidFile::create(&pid_path).unwrap();
            assert!(pid_path.exists());
        } // PID file dropped here

        // PID file should be removed
        assert!(!pid_path.exists());
    }
}
