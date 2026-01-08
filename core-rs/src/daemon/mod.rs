// Daemon Module - Reusable daemon implementations
//
// This module contains daemon logic that can be used both as:
// 1. Standalone binaries (ckr-edge-router, ckr-governor)
// 2. Subcommands of main binary (ckr daemon edge-router, ckr daemon governor)
//
// By extracting to library, we enable shared dependencies in single binary
// for reduced container size (21MB → 7-10MB target).

pub mod edge_router;
pub mod edge_router_async;
pub mod diskless_governor;

pub use edge_router::EdgeRouterDaemon;
pub use edge_router_async::EdgeRouterDaemonAsync;
pub use diskless_governor::DisklessGovernor;
