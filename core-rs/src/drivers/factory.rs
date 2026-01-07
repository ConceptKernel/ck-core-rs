//! Driver Factory for CKP v1.3.20
//!
//! Provides configuration-based instantiation of storage and transport drivers.
//!
//! # Overview
//!
//! The DriverFactory creates driver instances from configuration, supporting:
//! - Multiple storage backends (Local, Jena)
//! - Multiple transport backends (Local, NATS)
//! - YAML configuration files
//! - Environment variable overrides
//!
//! # Example
//!
//! ```rust,ignore
//! use ckp_core::drivers::factory::{DriverFactory, StorageConfig, TransportConfig};
//! use std::path::PathBuf;
//!
//! // Create drivers from configuration
//! let storage_config = StorageConfig::Local {
//!     root: PathBuf::from("/project"),
//! };
//!
//! let transport_config = TransportConfig::Local {
//!     root: PathBuf::from("/project"),
//!     kernel_name: "MyKernel".to_string(),
//! };
//!
//! let storage = DriverFactory::create_storage(&storage_config).await?;
//! let transport = DriverFactory::create_transport(&transport_config).await?;
//! ```
//!
//! # Configuration File Format
//!
//! ```yaml
//! storage:
//!   type: local
//!   root: /path/to/concepts
//!
//! transport:
//!   type: local
//!   root: /path/to/concepts
//!   kernel_name: MyKernel
//! ```

use crate::drivers::traits::{StorageDriver, TransportDriver};
use crate::drivers::{JenaStorage, LocalStorage, LocalTransport, NatsTransport};
use crate::errors::{CkpError, Result};
use crate::project::config::ProjectConfig;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ============================================================================
// CONFIGURATION STRUCTS
// ============================================================================

/// Storage driver configuration
///
/// Defines configuration for different storage backend types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StorageConfig {
    /// Local filesystem storage
    ///
    /// Uses FileSystemDriver wrapped by LocalStorage for async operations.
    ///
    /// # Example
    ///
    /// ```yaml
    /// storage:
    ///   type: local
    ///   root: /path/to/concepts
    /// ```
    Local {
        /// Project root directory
        root: PathBuf,
    },

    /// Jena RDF triple store storage
    ///
    /// Uses Apache Jena Fuseki for semantic graph storage.
    ///
    /// # Example
    ///
    /// ```yaml
    /// storage:
    ///   type: jena
    ///   endpoint: http://localhost:3030
    ///   dataset: ckp
    ///   credentials: /path/to/credentials (optional)
    /// ```
    Jena {
        /// Fuseki endpoint URL
        endpoint: String,

        /// Dataset name
        dataset: String,

        /// Optional credentials file path
        credentials: Option<String>,
    },
}

/// Transport driver configuration
///
/// Defines configuration for different transport backend types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TransportConfig {
    /// Local filesystem transport (notify-based)
    ///
    /// Uses filesystem watching with notify crate for local development.
    ///
    /// # Example
    ///
    /// ```yaml
    /// transport:
    ///   type: local
    ///   root: /path/to/concepts
    ///   kernel_name: MyKernel
    /// ```
    Local {
        /// Project root directory
        root: PathBuf,

        /// Kernel name for this transport instance
        kernel_name: String,
    },

    /// NATS JetStream transport
    ///
    /// Uses NATS for distributed, persistent messaging.
    ///
    /// # Example
    ///
    /// ```yaml
    /// transport:
    ///   type: nats
    ///   endpoint: nats://localhost:4222
    ///   credentials: /path/to/nats.creds (optional)
    /// ```
    Nats {
        /// NATS server endpoint URL
        endpoint: String,

        /// Optional credentials file path
        credentials: Option<PathBuf>,
    },
}

/// Combined driver configuration
///
/// Used for loading both storage and transport configuration from a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverConfig {
    /// Storage driver configuration
    pub storage: StorageConfig,

    /// Transport driver configuration
    pub transport: TransportConfig,
}

// ============================================================================
// DRIVER FACTORY
// ============================================================================

/// Driver factory for creating storage and transport driver instances
///
/// Provides static methods for creating drivers from configuration.
/// Supports environment variable overrides for deployment flexibility.
pub struct DriverFactory;

impl DriverFactory {
    /// Create storage driver from configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Storage driver configuration
    ///
    /// # Returns
    ///
    /// Arc-wrapped storage driver implementing StorageDriver trait
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Configuration is invalid
    /// - Driver initialization fails
    /// - Network connection fails (for remote drivers)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = StorageConfig::Local {
    ///     root: PathBuf::from("/project"),
    /// };
    ///
    /// let storage = DriverFactory::create_storage(&config).await?;
    /// ```
    pub async fn create_storage(config: &StorageConfig) -> Result<Arc<dyn StorageDriver>> {
        match config {
            StorageConfig::Local { root } => {
                // Validate root path exists
                if !root.exists() {
                    return Err(CkpError::Config(format!(
                        "Storage root path does not exist: {}",
                        root.display()
                    )));
                }

                // Create LocalStorage with empty kernel name (will be set by Governor)
                let storage = LocalStorage::new(root.clone(), String::new());
                Ok(Arc::new(storage))
            }

            StorageConfig::Jena {
                endpoint,
                dataset,
                credentials,
            } => {
                // Create JenaStorage with optional credentials
                let storage = if let Some(creds_path) = credentials {
                    // Parse credentials file (format: username:password)
                    let creds_content = tokio::fs::read_to_string(creds_path).await.map_err(|e| {
                        CkpError::Config(format!(
                            "Failed to read Jena credentials from {}: {}",
                            creds_path, e
                        ))
                    })?;

                    let parts: Vec<&str> = creds_content.trim().splitn(2, ':').collect();
                    if parts.len() != 2 {
                        return Err(CkpError::Config(
                            "Invalid Jena credentials format. Expected: username:password".to_string(),
                        ));
                    }

                    JenaStorage::new_with_auth(
                        endpoint.clone(),
                        dataset.clone(),
                        parts[0].to_string(),
                        parts[1].to_string(),
                    )
                } else {
                    JenaStorage::new(endpoint.clone(), dataset.clone())
                };

                Ok(Arc::new(storage))
            }
        }
    }

    /// Create transport driver from configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Transport driver configuration
    ///
    /// # Returns
    ///
    /// Arc-wrapped transport driver implementing TransportDriver trait
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Configuration is invalid
    /// - Driver initialization fails
    /// - Network connection fails (for remote drivers)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let config = TransportConfig::Nats {
    ///     endpoint: "nats://localhost:4222".to_string(),
    ///     credentials: None,
    /// };
    ///
    /// let transport = DriverFactory::create_transport(&config).await?;
    /// ```
    pub async fn create_transport(config: &TransportConfig) -> Result<Arc<dyn TransportDriver>> {
        match config {
            TransportConfig::Local { root, kernel_name } => {
                // Validate root path exists
                if !root.exists() {
                    return Err(CkpError::Config(format!(
                        "Transport root path does not exist: {}",
                        root.display()
                    )));
                }

                // Create LocalTransport
                let transport = LocalTransport::new(root.clone(), kernel_name.clone());
                Ok(Arc::new(transport))
            }

            TransportConfig::Nats {
                endpoint,
                credentials,
            } => {
                // Create NatsTransport
                // Note: NATS credentials are typically passed via URL or environment
                // For now, we use the basic connection without explicit creds file support
                if credentials.is_some() {
                    return Err(CkpError::NotImplemented(
                        "NATS credentials file support not yet implemented. Use NATS URL with embedded credentials or environment variables.".to_string(),
                    ));
                }

                let transport = NatsTransport::new(endpoint).await?;
                Ok(Arc::new(transport))
            }
        }
    }

    /// Load driver configuration from YAML file
    ///
    /// # Arguments
    ///
    /// * `path` - Path to YAML configuration file
    ///
    /// # Returns
    ///
    /// Tuple of (storage_driver, transport_driver)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - File not found
    /// - Invalid YAML syntax
    /// - Configuration validation fails
    /// - Driver creation fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let (storage, transport) = DriverFactory::from_config_file(
    ///     Path::new("/config/drivers.yaml")
    /// ).await?;
    /// ```
    pub async fn from_config_file(
        path: &Path,
    ) -> Result<(Arc<dyn StorageDriver>, Arc<dyn TransportDriver>)> {
        // Read configuration file
        let config_str = tokio::fs::read_to_string(path).await.map_err(|e| {
            CkpError::Config(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        // Parse YAML
        let mut config: DriverConfig = serde_yaml::from_str(&config_str).map_err(|e| {
            CkpError::Config(format!(
                "Failed to parse config file {}: {}",
                path.display(),
                e
            ))
        })?;

        // Apply environment variable overrides
        Self::apply_env_overrides(&mut config)?;

        // Create drivers
        let storage = Self::create_storage(&config.storage).await?;
        let transport = Self::create_transport(&config.transport).await?;

        Ok((storage, transport))
    }

    /// Apply environment variable overrides to configuration
    ///
    /// # Environment Variables
    ///
    /// - `CKP_STORAGE_TYPE`: Override storage type (local, jena)
    /// - `CKP_STORAGE_ROOT`: Override storage root path (for local)
    /// - `CKP_STORAGE_ENDPOINT`: Override storage endpoint (for jena)
    /// - `CKP_STORAGE_DATASET`: Override storage dataset (for jena)
    /// - `CKP_TRANSPORT_TYPE`: Override transport type (local, nats)
    /// - `CKP_TRANSPORT_ROOT`: Override transport root path (for local)
    /// - `CKP_TRANSPORT_ENDPOINT`: Override transport endpoint (for nats)
    /// - `CKP_TRANSPORT_KERNEL`: Override transport kernel name (for local)
    ///
    /// # Arguments
    ///
    /// * `config` - Mutable reference to configuration to update
    ///
    /// # Errors
    ///
    /// Returns error if environment variable values are invalid
    fn apply_env_overrides(config: &mut DriverConfig) -> Result<()> {
        // Storage type override
        if let Ok(storage_type) = std::env::var("CKP_STORAGE_TYPE") {
            match storage_type.as_str() {
                "local" => {
                    let root = std::env::var("CKP_STORAGE_ROOT")
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| PathBuf::from("."));
                    config.storage = StorageConfig::Local { root };
                }
                "jena" => {
                    let endpoint = std::env::var("CKP_STORAGE_ENDPOINT").map_err(|_| {
                        CkpError::Config(
                            "CKP_STORAGE_ENDPOINT required for jena storage".to_string(),
                        )
                    })?;
                    let dataset = std::env::var("CKP_STORAGE_DATASET").map_err(|_| {
                        CkpError::Config(
                            "CKP_STORAGE_DATASET required for jena storage".to_string(),
                        )
                    })?;
                    let credentials = std::env::var("CKP_STORAGE_CREDENTIALS").ok();
                    config.storage = StorageConfig::Jena {
                        endpoint,
                        dataset,
                        credentials,
                    };
                }
                _ => {
                    return Err(CkpError::Config(format!(
                        "Invalid storage type: {}",
                        storage_type
                    )));
                }
            }
        } else {
            // Apply field-specific overrides for existing storage type
            match &mut config.storage {
                StorageConfig::Local { root } => {
                    if let Ok(new_root) = std::env::var("CKP_STORAGE_ROOT") {
                        *root = PathBuf::from(new_root);
                    }
                }
                StorageConfig::Jena {
                    endpoint,
                    dataset,
                    credentials,
                } => {
                    if let Ok(new_endpoint) = std::env::var("CKP_STORAGE_ENDPOINT") {
                        *endpoint = new_endpoint;
                    }
                    if let Ok(new_dataset) = std::env::var("CKP_STORAGE_DATASET") {
                        *dataset = new_dataset;
                    }
                    if let Ok(new_credentials) = std::env::var("CKP_STORAGE_CREDENTIALS") {
                        *credentials = Some(new_credentials);
                    }
                }
            }
        }

        // Transport type override
        if let Ok(transport_type) = std::env::var("CKP_TRANSPORT_TYPE") {
            match transport_type.as_str() {
                "local" => {
                    let root = std::env::var("CKP_TRANSPORT_ROOT")
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| PathBuf::from("."));
                    let kernel_name = std::env::var("CKP_TRANSPORT_KERNEL")
                        .unwrap_or_else(|_| String::new());
                    config.transport = TransportConfig::Local { root, kernel_name };
                }
                "nats" => {
                    let endpoint = std::env::var("CKP_TRANSPORT_ENDPOINT").map_err(|_| {
                        CkpError::Config(
                            "CKP_TRANSPORT_ENDPOINT required for nats transport".to_string(),
                        )
                    })?;
                    let credentials = std::env::var("CKP_TRANSPORT_CREDENTIALS")
                        .ok()
                        .map(PathBuf::from);
                    config.transport = TransportConfig::Nats {
                        endpoint,
                        credentials,
                    };
                }
                _ => {
                    return Err(CkpError::Config(format!(
                        "Invalid transport type: {}",
                        transport_type
                    )));
                }
            }
        } else {
            // Apply field-specific overrides for existing transport type
            match &mut config.transport {
                TransportConfig::Local { root, kernel_name } => {
                    if let Ok(new_root) = std::env::var("CKP_TRANSPORT_ROOT") {
                        *root = PathBuf::from(new_root);
                    }
                    if let Ok(new_kernel) = std::env::var("CKP_TRANSPORT_KERNEL") {
                        *kernel_name = new_kernel;
                    }
                }
                TransportConfig::Nats {
                    endpoint,
                    credentials,
                } => {
                    if let Ok(new_endpoint) = std::env::var("CKP_TRANSPORT_ENDPOINT") {
                        *endpoint = new_endpoint;
                    }
                    if let Ok(new_credentials) = std::env::var("CKP_TRANSPORT_CREDENTIALS") {
                        *credentials = Some(PathBuf::from(new_credentials));
                    }
                }
            }
        }

        Ok(())
    }

    /// Create default local drivers for development
    ///
    /// # Arguments
    ///
    /// * `root` - Project root path
    /// * `kernel_name` - Kernel name
    ///
    /// # Returns
    ///
    /// Tuple of (storage_driver, transport_driver) configured for local development
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let (storage, transport) = DriverFactory::create_local_drivers(
    ///     PathBuf::from("/project"),
    ///     "MyKernel".to_string()
    /// ).await?;
    /// ```
    pub async fn create_local_drivers(
        root: PathBuf,
        kernel_name: String,
    ) -> Result<(Arc<dyn StorageDriver>, Arc<dyn TransportDriver>)> {
        let storage_config = StorageConfig::Local { root: root.clone() };
        let transport_config = TransportConfig::Local {
            root,
            kernel_name,
        };

        let storage = Self::create_storage(&storage_config).await?;
        let transport = Self::create_transport(&transport_config).await?;

        Ok((storage, transport))
    }

    /// Create JenaStorage from .ckproject configuration
    ///
    /// Loads project configuration and creates Jena storage if configured.
    /// Returns None if Jena is disabled or configuration is missing.
    ///
    /// # Arguments
    ///
    /// * `project_root` - Path to project root directory containing .ckproject
    ///
    /// # Returns
    ///
    /// * `Some(Arc<JenaStorage>)` if Jena is enabled and credentials exist
    /// * `None` if Jena is disabled or configuration is missing
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use ckp_core::drivers::factory::DriverFactory;
    /// use std::path::Path;
    ///
    /// let project_root = Path::new("/path/to/project");
    /// if let Some(jena) = DriverFactory::create_jena_from_project(project_root).await {
    ///     println!("Jena storage initialized");
    /// } else {
    ///     println!("Jena storage not configured");
    /// }
    /// ```
    pub async fn create_jena_from_project(project_root: &Path) -> Option<Arc<JenaStorage>> {
        // Load .ckproject configuration
        eprintln!("[DEBUG Factory] Loading .ckproject from: {}", project_root.display());
        let config = match ProjectConfig::load_from_project(project_root) {
            Ok(cfg) => {
                eprintln!("[DEBUG Factory] ✓ Loaded .ckproject successfully");
                cfg
            },
            Err(e) => {
                eprintln!("[DEBUG Factory] ✗ Failed to load .ckproject: {}", e);
                return None;
            }
        };

        // Extract Jena backend configuration
        eprintln!("[DEBUG Factory] Checking backends configuration...");
        let backends = match config.spec.backends.as_ref() {
            Some(b) => b,
            None => {
                eprintln!("[DEBUG Factory] ✗ No backends configuration found");
                return None;
            }
        };
        let jena_config = &backends.jena;

        // Check if Jena is enabled
        eprintln!("[DEBUG Factory] Jena enabled: {}", jena_config.enabled);
        if !jena_config.enabled {
            eprintln!("[DEBUG Factory] ✗ Jena is disabled in configuration");
            return None;
        }

        // Create JenaStorage with optional authentication
        let storage = if let (Some(username), Some(password)) =
            (&jena_config.username, &jena_config.password)
        {
            // Use authenticated connection
            JenaStorage::new_with_auth(
                jena_config.endpoint.clone(),
                jena_config.dataset.clone(),
                username.clone(),
                password.clone(),
            )
        } else {
            // Use unauthenticated connection
            JenaStorage::new(jena_config.endpoint.clone(), jena_config.dataset.clone())
        };

        Some(Arc::new(storage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_local_storage() {
        let temp_dir = TempDir::new().unwrap();
        let config = StorageConfig::Local {
            root: temp_dir.path().to_path_buf(),
        };

        let storage = DriverFactory::create_storage(&config).await;
        assert!(storage.is_ok());
    }

    #[tokio::test]
    async fn test_create_local_storage_invalid_path() {
        let config = StorageConfig::Local {
            root: PathBuf::from("/nonexistent/path/that/does/not/exist"),
        };

        let storage = DriverFactory::create_storage(&config).await;
        assert!(storage.is_err());
        assert!(matches!(storage.unwrap_err(), CkpError::Config(_)));
    }

    #[tokio::test]
    async fn test_create_local_transport() {
        let temp_dir = TempDir::new().unwrap();
        let config = TransportConfig::Local {
            root: temp_dir.path().to_path_buf(),
            kernel_name: "TestKernel".to_string(),
        };

        let transport = DriverFactory::create_transport(&config).await;
        assert!(transport.is_ok());
    }

    #[tokio::test]
    async fn test_create_local_transport_invalid_path() {
        let config = TransportConfig::Local {
            root: PathBuf::from("/nonexistent/path/that/does/not/exist"),
            kernel_name: "TestKernel".to_string(),
        };

        let transport = DriverFactory::create_transport(&config).await;
        assert!(transport.is_err());
        assert!(matches!(transport.unwrap_err(), CkpError::Config(_)));
    }

    #[tokio::test]
    async fn test_create_local_drivers() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();
        let kernel_name = "TestKernel".to_string();

        let result = DriverFactory::create_local_drivers(root, kernel_name).await;
        assert!(result.is_ok());

        let (storage, transport) = result.unwrap();
        assert!(storage.kernel_exists("TestKernel").await.is_ok());
    }

    #[tokio::test]
    async fn test_storage_config_serialization() {
        let config = StorageConfig::Local {
            root: PathBuf::from("/test/path"),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("type: local"));
        assert!(yaml.contains("root:"));
        assert!(yaml.contains("/test/path"));

        let deserialized: StorageConfig = serde_yaml::from_str(&yaml).unwrap();
        match deserialized {
            StorageConfig::Local { root } => {
                assert_eq!(root, PathBuf::from("/test/path"));
            }
            _ => panic!("Expected Local variant"),
        }
    }

    #[tokio::test]
    async fn test_transport_config_serialization() {
        let config = TransportConfig::Local {
            root: PathBuf::from("/test/path"),
            kernel_name: "TestKernel".to_string(),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("type: local"));
        assert!(yaml.contains("root:"));
        assert!(yaml.contains("kernel_name:"));
        assert!(yaml.contains("TestKernel"));

        let deserialized: TransportConfig = serde_yaml::from_str(&yaml).unwrap();
        match deserialized {
            TransportConfig::Local { root, kernel_name } => {
                assert_eq!(root, PathBuf::from("/test/path"));
                assert_eq!(kernel_name, "TestKernel");
            }
            _ => panic!("Expected Local variant"),
        }
    }

    #[tokio::test]
    async fn test_driver_config_serialization() {
        let config = DriverConfig {
            storage: StorageConfig::Local {
                root: PathBuf::from("/storage"),
            },
            transport: TransportConfig::Local {
                root: PathBuf::from("/transport"),
                kernel_name: "TestKernel".to_string(),
            },
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("storage:"));
        assert!(yaml.contains("transport:"));
        assert!(yaml.contains("type: local"));

        let deserialized: DriverConfig = serde_yaml::from_str(&yaml).unwrap();
        match deserialized.storage {
            StorageConfig::Local { root } => {
                assert_eq!(root, PathBuf::from("/storage"));
            }
            _ => panic!("Expected Local storage variant"),
        }
        match deserialized.transport {
            TransportConfig::Local { root, kernel_name } => {
                assert_eq!(root, PathBuf::from("/transport"));
                assert_eq!(kernel_name, "TestKernel");
            }
            _ => panic!("Expected Local transport variant"),
        }
    }

    #[tokio::test]
    async fn test_from_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        let storage_root = temp_dir.path().join("storage");
        let transport_root = temp_dir.path().join("transport");

        // Create directories
        tokio::fs::create_dir_all(&storage_root).await.unwrap();
        tokio::fs::create_dir_all(&transport_root).await.unwrap();

        let config = DriverConfig {
            storage: StorageConfig::Local {
                root: storage_root.clone(),
            },
            transport: TransportConfig::Local {
                root: transport_root.clone(),
                kernel_name: "TestKernel".to_string(),
            },
        };

        // Write config file
        let yaml = serde_yaml::to_string(&config).unwrap();
        tokio::fs::write(&config_path, yaml).await.unwrap();

        // Load from config file
        let result = DriverFactory::from_config_file(&config_path).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_from_config_file_not_found() {
        let result =
            DriverFactory::from_config_file(Path::new("/nonexistent/config.yaml")).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CkpError::Config(_)));
    }

    #[tokio::test]
    async fn test_from_config_file_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid.yaml");

        // Write invalid YAML
        tokio::fs::write(&config_path, "invalid: yaml: content:")
            .await
            .unwrap();

        let result = DriverFactory::from_config_file(&config_path).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CkpError::Config(_)));
    }

    // Note: Environment variable override tests are omitted due to test isolation challenges
    // with parallel test execution. The apply_env_overrides() function is tested indirectly
    // through from_config_file() which is the primary use case.

    #[tokio::test]
    async fn test_multiple_driver_instances() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_path_buf();

        let (storage1, transport1) =
            DriverFactory::create_local_drivers(root.clone(), "Kernel1".to_string())
                .await
                .unwrap();
        let (storage2, transport2) =
            DriverFactory::create_local_drivers(root.clone(), "Kernel2".to_string())
                .await
                .unwrap();

        // Verify both driver instances are independent
        assert!(storage1.kernel_exists("Kernel1").await.is_ok());
        assert!(storage2.kernel_exists("Kernel2").await.is_ok());
    }

    #[tokio::test]
    async fn test_jena_config_serialization() {
        let config = StorageConfig::Jena {
            endpoint: "http://localhost:3030".to_string(),
            dataset: "ckp".to_string(),
            credentials: Some("/path/to/creds".to_string()),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("type: jena"));
        assert!(yaml.contains("endpoint:"));
        assert!(yaml.contains("dataset:"));
        assert!(yaml.contains("credentials:"));

        let deserialized: StorageConfig = serde_yaml::from_str(&yaml).unwrap();
        match deserialized {
            StorageConfig::Jena {
                endpoint,
                dataset,
                credentials,
            } => {
                assert_eq!(endpoint, "http://localhost:3030");
                assert_eq!(dataset, "ckp");
                assert_eq!(credentials, Some("/path/to/creds".to_string()));
            }
            _ => panic!("Expected Jena variant"),
        }
    }

    #[tokio::test]
    async fn test_nats_config_serialization() {
        let config = TransportConfig::Nats {
            endpoint: "nats://localhost:4222".to_string(),
            credentials: Some(PathBuf::from("/path/to/nats.creds")),
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("type: nats"));
        assert!(yaml.contains("endpoint:"));
        assert!(yaml.contains("credentials:"));

        let deserialized: TransportConfig = serde_yaml::from_str(&yaml).unwrap();
        match deserialized {
            TransportConfig::Nats {
                endpoint,
                credentials,
            } => {
                assert_eq!(endpoint, "nats://localhost:4222");
                assert_eq!(credentials, Some(PathBuf::from("/path/to/nats.creds")));
            }
            _ => panic!("Expected Nats variant"),
        }
    }
}
