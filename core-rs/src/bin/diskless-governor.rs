//! Diskless Governor Binary - Stateless job processing daemon
//!
//! Runs a ConceptKernel governor without any filesystem dependencies:
//! - Loads configuration from Jena RDF store via SPARQL
//! - Receives jobs from NATS JetStream
//! - Publishes results to NATS
//! - Emits lifecycle events
//!
//! ## Usage
//!
//! ```bash
//! # Set environment variables
//! export NATS_URL="nats://localhost:4222"
//! export FUSEKI_URL="http://localhost:3030"
//! export FUSEKI_DATASET="ck"
//!
//! # Run diskless governor for a kernel
//! cargo run --bin diskless-governor -- Usecase.SimplePassthrough.Source
//! ```
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐
//! │  Jena Fuseki    │ ──SPARQL──> [Load kernel config]
//! │  (RDF Store)    │
//! └─────────────────┘
//!         ↓
//! ┌─────────────────┐
//! │ DisklessGovernor│ <───────── [This binary]
//! │  (In-Memory)    │
//! └─────────────────┘
//!         ↓
//! ┌─────────────────┐
//! │  NATS JetStream │ ──inbox──> [Receive jobs]
//! │  (Messaging)    │ <─results─ [Send results]
//! └─────────────────┘
//! ```
//!
//! ## Benefits
//!
//! - **Stateless**: No persistent volumes needed in Kubernetes
//! - **Small**: Minimal container footprint (no filesystem operations)
//! - **Scalable**: Horizontal scaling via NATS consumer groups
//! - **RDF-First**: Configuration as linked data (semantic web)

use ckp_core::daemon::DisklessGovernor;
use ckp_core::drivers::{JenaStorage, NatsTransport};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <kernel-name>", args[0]);
        eprintln!("");
        eprintln!("Environment variables:");
        eprintln!("  NATS_URL         - NATS server URL (default: nats://localhost:4222)");
        eprintln!("  FUSEKI_URL       - Jena Fuseki URL (default: http://localhost:3030)");
        eprintln!("  FUSEKI_DATASET   - Fuseki dataset name (default: ck)");
        eprintln!("  VERBOSE          - Enable verbose logging (default: true)");
        eprintln!("");
        eprintln!("Example:");
        eprintln!("  {} Usecase.SimplePassthrough.Source", args[0]);
        std::process::exit(1);
    }

    let kernel_name = args[1].clone();

    // Load configuration from environment
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let fuseki_url = std::env::var("FUSEKI_URL").unwrap_or_else(|_| "http://localhost:3030".to_string());
    let fuseki_dataset = std::env::var("FUSEKI_DATASET").unwrap_or_else(|_| "ck".to_string());
    let fuseki_username = std::env::var("FUSEKI_USERNAME").ok();
    let fuseki_password = std::env::var("FUSEKI_PASSWORD").ok();
    let verbose = std::env::var("VERBOSE")
        .unwrap_or_else(|_| "true".to_string())
        .parse::<bool>()
        .unwrap_or(true);

    eprintln!("═══════════════════════════════════════════════════════════");
    eprintln!("  Diskless Governor - ConceptKernel v1.3.20");
    eprintln!("═══════════════════════════════════════════════════════════");
    eprintln!("");
    eprintln!("Configuration:");
    eprintln!("  Kernel:          {}", kernel_name);
    eprintln!("  NATS URL:        {}", nats_url);
    eprintln!("  Fuseki URL:      {}", fuseki_url);
    eprintln!("  Fuseki Dataset:  {}", fuseki_dataset);
    eprintln!("  Verbose:         {}", verbose);
    eprintln!("");
    eprintln!("───────────────────────────────────────────────────────────");

    // Create transport driver (NATS JetStream)
    let transport = Arc::new(
        NatsTransport::new(&nats_url)
            .await
            .map_err(|e| format!("Failed to connect to NATS: {}", e))?,
    );

    eprintln!("✓ Connected to NATS: {}", nats_url);

    // Create storage driver (Jena RDF store) with optional authentication
    let storage = Arc::new(
        if let (Some(username), Some(password)) = (fuseki_username, fuseki_password) {
            JenaStorage::new_with_auth(fuseki_url.clone(), fuseki_dataset.clone(), username, password)
        } else {
            JenaStorage::new(fuseki_url.clone(), fuseki_dataset.clone())
        }
    );

    eprintln!("✓ Connected to Jena: {}/{}", fuseki_url, fuseki_dataset);
    eprintln!("");

    // Create diskless governor
    let mut governor = DisklessGovernor::new(kernel_name.clone(), transport, storage, verbose)
        .await
        .map_err(|e| format!("Failed to create governor: {}", e))?;

    eprintln!("✓ DisklessGovernor initialized for: {}", kernel_name);
    eprintln!("");
    eprintln!("───────────────────────────────────────────────────────────");
    eprintln!("  Starting job processing...");
    eprintln!("  Press Ctrl+C to shutdown");
    eprintln!("───────────────────────────────────────────────────────────");
    eprintln!("");

    // Setup shutdown signal
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel::<()>(1);

    // Handle Ctrl+C
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        eprintln!("");
        eprintln!("───────────────────────────────────────────────────────────");
        eprintln!("  Ctrl+C received - Initiating graceful shutdown...");
        eprintln!("───────────────────────────────────────────────────────────");
        let _ = shutdown_tx.send(());
    });

    // Start governor
    match governor.start(shutdown_rx).await {
        Ok(_) => {
            eprintln!("");
            eprintln!("═══════════════════════════════════════════════════════════");
            eprintln!("  DisklessGovernor shutdown complete");
            eprintln!("═══════════════════════════════════════════════════════════");
            Ok(())
        }
        Err(e) => {
            eprintln!("");
            eprintln!("═══════════════════════════════════════════════════════════");
            eprintln!("  ERROR: Governor failed: {}", e);
            eprintln!("═══════════════════════════════════════════════════════════");
            Err(format!("Governor failed: {}", e).into())
        }
    }
}
