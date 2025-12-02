//! Contract Tests - Protocol Invariant Protection
//!
//! This file aggregates all contract test modules.
//! Contract tests verify invariants that MUST NEVER BREAK.
//!
//! See tests/contracts/README.md for philosophy and usage.

// Contract test modules
mod contracts {
    // Port allocation contracts
    mod port_allocation {
        include!("port_allocation_contracts.rs");
    }

    // Process tracker contracts
    mod process_tracker {
        include!("process_tracker_contracts.rs");
    }

    // Kernel manager contracts
    mod kernel_manager {
        include!("kernel_manager_contracts.rs");
    }

    // Protocol purity contracts
    mod protocol_purity {
        include!("protocol_purity_contracts.rs");
    }
}
