/// Example: Query kernel ontologies using OntologyLibrary
///
/// This example demonstrates how to query kernel ontology.ttl files
/// to extract roles, functions, and metadata using SPARQL.
///
/// Usage:
///   cargo run --example query_ontology System.Gateway
///   cargo run --example query_ontology System.Consensus
///   cargo run --example query_ontology --all

use ckp_core::{OntologyLibrary, RoleMetadata, FunctionMetadata, KernelMetadata};
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: cargo run --example query_ontology <KernelName>");
        eprintln!("       cargo run --example query_ontology --all");
        std::process::exit(1);
    }

    // Initialize OntologyLibrary with project root
    let project_root = PathBuf::from(env::current_dir()?);
    let mut lib = OntologyLibrary::new(project_root)?;

    if args[1] == "--all" {
        // Query all kernels
        println!("Querying all kernel ontologies...\n");

        let kernels = vec![
            "System.Gateway",
            "System.Consensus",
            "System.Wss",
            "System.Oidc.Provider",
            "System.Oidc.Token",
            "System.Oidc.User",
            "System.Oidc.Role",
        ];

        for kernel_name in kernels {
            println!("═══════════════════════════════════════════");
            println!("KERNEL: {}", kernel_name);
            println!("═══════════════════════════════════════════");
            query_kernel(&mut lib, kernel_name)?;
            println!();
        }
    } else {
        let kernel_name = &args[1];
        query_kernel(&mut lib, kernel_name)?;
    }

    Ok(())
}

fn query_kernel(lib: &mut OntologyLibrary, kernel_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Load kernel ontology
    lib.load_kernel_ontology(kernel_name)?;

    // Get kernel URN
    let urn = lib.get_kernel_urn(kernel_name)?;
    println!("URN: {}", urn);

    // Get kernel metadata
    match lib.get_kernel_metadata(kernel_name) {
        Ok(metadata) => {
            println!("\n📦 METADATA:");
            println!("   Name: {}", metadata.name);
            println!("   Type: {}", metadata.kernel_type);
            println!("   Version: {}", metadata.version);
            println!("   Description: {}", metadata.description);
        }
        Err(e) => {
            eprintln!("⚠️  Failed to get metadata: {}", e);
        }
    }

    // Get roles
    match lib.get_kernel_roles(kernel_name) {
        Ok(roles) => {
            println!("\n🎭 ROLES ({} total):", roles.len());
            for (i, role) in roles.iter().enumerate() {
                println!("   {}. {} ({})", i + 1, role.name, role.context);
                println!("      URI: {}", role.uri);
                println!("      Description: {}", role.description);
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to get roles: {}", e);
        }
    }

    // Get functions
    match lib.get_kernel_functions(kernel_name) {
        Ok(functions) => {
            println!("\n⚙️  FUNCTIONS ({} total):", functions.len());
            for (i, func) in functions.iter().enumerate() {
                println!("   {}. {}", i + 1, func.name);
                println!("      URI: {}", func.uri);
                println!("      Description: {}", func.description);
                println!("      Capabilities: [{}]", func.capabilities.join(", "));
            }
        }
        Err(e) => {
            eprintln!("⚠️  Failed to get functions: {}", e);
        }
    }

    Ok(())
}
