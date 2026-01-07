//! Register Bakery edges in Jena using updated schema
//!
//! This tool reads edge metadata from filesystem and registers them in Jena
//! with the full entity graph that System.Discovery expects.

use ckp_core::drivers::{EdgeMetadata, JenaStorage};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("[REGISTER-EDGES] Registering Bakery edges in Jena...\n");

    // Jena configuration from .ckproject
    let jena_endpoint = "https://jena.conceptkernel.dev";
    let jena_dataset = "dataset";
    let jena_user = "admin";
    let jena_password = "uIL@xlp8tgG-6s{MR*mJ+re>";

    // Create JenaStorage client
    let jena = JenaStorage::new(
        jena_endpoint.to_string(),
        jena_dataset.to_string(),
        Some(jena_user.to_string()),
        Some(jena_password.to_string()),
    );

    // Bakery edges to register
    let edges = vec![
        EdgeMetadata {
            api_version: "conceptkernel/v1".to_string(),
            kind: "EdgeConnection".to_string(),
            urn: "ckp://Edge#Connection-Usecase.Bakery.OrderProcessor-to-Usecase.Bakery.PaymentProcessor-PRODUCES:v1.3.20".to_string(),
            created_at: "2025-12-24T12:03:13.960858+00:00".to_string(),
            predicate: "PRODUCES".to_string(),
            source: "Usecase.Bakery.OrderProcessor".to_string(),
            target: "Usecase.Bakery.PaymentProcessor".to_string(),
            version: "v1.3.20".to_string(),
        },
        EdgeMetadata {
            api_version: "conceptkernel/v1".to_string(),
            kind: "EdgeConnection".to_string(),
            urn: "ckp://Edge#Connection-Usecase.Bakery.OrderProcessor-to-Usecase.Bakery.InventoryManager-PRODUCES:v1.3.20".to_string(),
            created_at: "2025-12-24T12:02:00Z".to_string(),
            predicate: "PRODUCES".to_string(),
            source: "Usecase.Bakery.OrderProcessor".to_string(),
            target: "Usecase.Bakery.InventoryManager".to_string(),
            version: "v1.3.20".to_string(),
        },
        EdgeMetadata {
            api_version: "conceptkernel/v1".to_string(),
            kind: "EdgeConnection".to_string(),
            urn: "ckp://Edge#Connection-Usecase.Bakery.PaymentProcessor-to-Usecase.Bakery.OrderFulfillment-PRODUCES:v1.3.20".to_string(),
            created_at: "2025-12-24T12:02:00Z".to_string(),
            predicate: "PRODUCES".to_string(),
            source: "Usecase.Bakery.PaymentProcessor".to_string(),
            target: "Usecase.Bakery.OrderFulfillment".to_string(),
            version: "v1.3.20".to_string(),
        },
        EdgeMetadata {
            api_version: "conceptkernel/v1".to_string(),
            kind: "EdgeConnection".to_string(),
            urn: "ckp://Edge#Connection-Usecase.Bakery.InventoryManager-to-Usecase.Bakery.OrderFulfillment-PRODUCES:v1.3.20".to_string(),
            created_at: "2025-12-24T12:02:00Z".to_string(),
            predicate: "PRODUCES".to_string(),
            source: "Usecase.Bakery.InventoryManager".to_string(),
            target: "Usecase.Bakery.OrderFulfillment".to_string(),
            version: "v1.3.20".to_string(),
        },
    ];

    // Register each edge
    for (i, edge) in edges.iter().enumerate() {
        println!("{}. Registering: {} → {} ({})",
            i + 1,
            edge.source,
            edge.target,
            edge.predicate
        );

        match jena.save_edge_metadata_structured(edge).await {
            Ok(_) => {
                println!("   ✓ Registered in Jena");
                println!("   URN: {}\n", edge.urn);
            }
            Err(e) => {
                eprintln!("   ✗ Failed: {}\n", e);
            }
        }
    }

    println!("[REGISTER-EDGES] ✓ Complete - {} edges registered", edges.len());
    println!("[REGISTER-EDGES] Test with: cd concepts/ConceptKernel.UI && node test-discovery-edges.mjs");

    Ok(())
}
