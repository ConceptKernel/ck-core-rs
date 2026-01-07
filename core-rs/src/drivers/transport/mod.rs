//! Transport driver implementations
//!
//! ## Available Drivers
//!
//! - **LocalTransport**: Filesystem + notify crate (v1.3.19 compatibility)
//! - **NatsTransport**: NATS JetStream messaging (v1.3.20)
//! - **WebSocketTransport**: Browser/A2UI communication (future)
//! - **GrpcTransport**: Agent-to-agent communication (future)

pub mod local;
pub mod nats;

pub use local::LocalTransport;
pub use nats::NatsTransport;
