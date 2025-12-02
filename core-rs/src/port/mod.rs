/**
 * port module
 * Dynamic port allocation for multi-project infrastructure (v1.3.14)
 */

pub mod manager;

pub use manager::{PortManager, PortMap, PortRange};

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: PortManager export is accessible
    ///
    /// Verifies that PortManager type is exported for dynamic port allocation
    /// across multiple projects (200 ports per slot).
    #[test]
    fn test_port_manager_export() {
        // Verify PortManager type is accessible via Option
        fn accepts_port_manager(_: Option<PortManager>) {}
        accepts_port_manager(None);

        // If this compiles, export is correct
    }

    /// Test: PortMap and PortRange exports are accessible
    ///
    /// Verifies that port allocation types are exported for
    /// managing port ranges and assignments.
    #[test]
    fn test_port_types_exports() {
        // Verify PortMap type is accessible via Option
        fn accepts_port_map(_: Option<PortMap>) {}
        accepts_port_map(None);

        // Verify PortRange type is accessible
        fn accepts_port_range(_: PortRange) {}
        let range = PortRange {
            start: 56000,
            end: 56199,
        };
        accepts_port_range(range);

        // If this compiles, exports are correct
    }
}
