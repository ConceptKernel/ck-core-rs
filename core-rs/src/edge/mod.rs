//! Edge management module
//!
//! Provides functionality for managing edges between kernels,
//! including edge creation, routing, and authorization.
//!
//! Reference: Node.js v1.3.14 - EdgeKernel.js

pub mod kernel;
pub mod metadata;
pub mod request_builder;

pub use kernel::EdgeKernel;
pub use metadata::EdgeMetadata;
pub use request_builder::{EdgeRequestBuilder, EdgeRequest, EdgeSource, EdgeTarget, NotificationEntry};

#[cfg(test)]
mod tests {
    use super::*;

    /// Test: EdgeKernel and EdgeMetadata exports are accessible
    ///
    /// Verifies that edge management core types are exported for
    /// edge routing and kernel-to-kernel communication.
    #[test]
    fn test_edge_kernel_exports() {
        use std::path::PathBuf;

        // Verify EdgeKernel type is accessible
        fn accepts_edge_kernel(_: EdgeKernel) {}
        let kernel = EdgeKernel::new(PathBuf::from("/tmp/test")).unwrap();
        accepts_edge_kernel(kernel);

        // Verify EdgeMetadata type is accessible via Option
        fn accepts_edge_metadata(_: Option<EdgeMetadata>) {}
        accepts_edge_metadata(None);

        // If this compiles, exports are correct
    }

    /// Test: EdgeRequest types are exported
    ///
    /// Verifies that edge request building types are exported for
    /// constructing edge invocations between kernels.
    #[test]
    fn test_edge_request_types_exports() {
        use std::path::PathBuf;

        // Verify EdgeRequestBuilder type is accessible
        fn accepts_builder(_: EdgeRequestBuilder) {}
        let builder = EdgeRequestBuilder::new(PathBuf::from("/tmp/test"));
        accepts_builder(builder);

        // Verify EdgeRequest type accessible via Option
        fn accepts_request(_: Option<EdgeRequest>) {}
        accepts_request(None);

        // Verify EdgeSource and EdgeTarget accessible via Option
        fn accepts_source(_: Option<EdgeSource>) {}
        fn accepts_target(_: Option<EdgeTarget>) {}
        accepts_source(None);
        accepts_target(None);

        // If this compiles, exports are correct
    }
}
