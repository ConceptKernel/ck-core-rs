//! Edge Event Publisher - Publishes edge lifecycle events to NATS
//!
//! Enables real-time observability of edge operations including:
//! - Edge creation/deletion lifecycle
//! - Edge updates and modifications
//! - Discovery mechanism for web UI
//!
//! ## Configuration
//!
//! Set NATS_URL environment variable to enable event publishing:
//! ```bash
//! export NATS_URL=nats://localhost:4222
//! ```
//!
//! If NATS_URL is not set, events are logged but not published.

use crate::errors::Result;
use async_nats::Client as NatsClient;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Edge lifecycle event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeEventType {
    /// Edge was created
    EdgeCreated,
    /// Edge was deleted
    EdgeDeleted,
    /// Edge was updated (metadata change)
    EdgeUpdated,
    /// Edge announcement (discovery response)
    EdgeAnnounce,
    /// Message routed through edge (runtime event)
    EdgeRouted,
}

impl EdgeEventType {
    /// Get subject suffix for this event type
    fn subject_suffix(&self) -> &str {
        match self {
            Self::EdgeCreated => "lifecycle.created",
            Self::EdgeDeleted => "lifecycle.deleted",
            Self::EdgeUpdated => "lifecycle.updated",
            Self::EdgeAnnounce => "lifecycle.announce",
            Self::EdgeRouted => "routed",
        }
    }
}

/// Edge lifecycle event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeEvent {
    /// Event type (maps to NATS subject suffix)
    pub event_type: EdgeEventType,

    /// Edge URN
    pub edge_urn: String,

    /// Source kernel name
    pub source: String,

    /// Target kernel name
    pub target: String,

    /// Predicate (e.g., PRODUCES, REQUIRES, DEPENDS_ON)
    pub predicate: String,

    /// Version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// ISO 8601 timestamp
    pub timestamp: String,

    /// Event-specific payload
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// Edge event publisher
pub struct EdgeEventPublisher {
    /// Optional NATS client
    pub(crate) nats_client: Option<NatsClient>,

    /// Callback to get all edges (for discovery)
    edge_lister: Option<Arc<dyn Fn() -> Vec<EdgeInfo> + Send + Sync>>,
}

/// Edge information for discovery
#[derive(Debug, Clone)]
pub struct EdgeInfo {
    pub urn: String,
    pub source: String,
    pub target: String,
    pub predicate: String,
    pub version: Option<String>,
}

impl EdgeEventPublisher {
    /// Create new edge event publisher
    ///
    /// # Arguments
    /// * `nats_url` - Optional NATS URL from environment
    ///
    /// # Returns
    /// Publisher instance (with or without NATS connection)
    pub async fn new(nats_url: Option<&str>) -> Result<Self> {
        let nats_client = if let Some(url) = nats_url {
            match async_nats::connect(url).await {
                Ok(client) => {
                    eprintln!("[EdgeEventPublisher] Connected to NATS at {}", url);
                    Some(client)
                }
                Err(e) => {
                    eprintln!("[EdgeEventPublisher] Failed to connect to NATS: {}", e);
                    eprintln!("[EdgeEventPublisher] Continuing without event publishing");
                    None
                }
            }
        } else {
            None
        };

        let publisher = Self {
            nats_client,
            edge_lister: None,
        };

        // Start discovery listener if NATS connected
        publisher.start_discovery_listener().await;

        Ok(publisher)
    }

    /// Set edge lister callback for discovery
    pub fn set_edge_lister<F>(&mut self, lister: F)
    where
        F: Fn() -> Vec<EdgeInfo> + Send + Sync + 'static,
    {
        self.edge_lister = Some(Arc::new(lister));
    }

    /// Start listening for discovery requests and auto-respond
    async fn start_discovery_listener(&self) {
        if let Some(nats) = &self.nats_client {
            let nats_clone = nats.clone();
            let edge_lister = self.edge_lister.clone();

            tokio::spawn(async move {
                let sub_result = nats_clone.subscribe("edge.discovery.request").await;

                if let Ok(mut sub) = sub_result {
                    eprintln!("[EdgeEventPublisher] Listening for edge discovery requests");

                    while let Some(_msg) = sub.next().await {
                        eprintln!("[EdgeEventPublisher] Received discovery request");

                        // Announce all known edges if lister is available
                        if let Some(lister) = &edge_lister {
                            let edges = lister();
                            eprintln!("[EdgeEventPublisher] Announcing {} edge(s)", edges.len());

                            for edge in edges {
                                let announce_subject = format!("edge.{}.lifecycle.announce", edge.urn);
                                let announce_payload = serde_json::json!({
                                    "event_type": "edge_announce",
                                    "edge_urn": edge.urn,
                                    "source": edge.source,
                                    "target": edge.target,
                                    "predicate": edge.predicate,
                                    "version": edge.version,
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                });

                                if let Ok(payload_bytes) = serde_json::to_vec(&announce_payload) {
                                    let _ = nats_clone.publish(announce_subject, payload_bytes.into()).await;
                                }
                            }
                        }
                    }
                } else {
                    eprintln!("[EdgeEventPublisher] Failed to subscribe to edge discovery requests");
                }
            });
        }
    }

    /// Publish an edge event
    ///
    /// Non-blocking: Errors are logged but not propagated
    pub async fn publish(&self, event: EdgeEvent) {
        // Always log event to stderr for debugging
        eprintln!(
            "[EdgeEventPublisher] {:?}: {} -> {} ({})",
            event.event_type,
            event.source,
            event.target,
            event.predicate
        );

        // Publish to NATS if connected
        if let Some(nats) = &self.nats_client {
            let subject = format!("edge.{}.{}", event.edge_urn, event.event_type.subject_suffix());

            match serde_json::to_vec(&event) {
                Ok(payload) => {
                    if let Err(e) = nats.publish(subject.clone(), payload.into()).await {
                        eprintln!("[EdgeEventPublisher] Failed to publish to {}: {}", subject, e);
                    }
                }
                Err(e) => {
                    eprintln!("[EdgeEventPublisher] Failed to serialize event: {}", e);
                }
            }
        }
    }

    /// Publish edge.created event
    pub async fn publish_edge_created(&self, edge_urn: &str, source: &str, target: &str, predicate: &str) {
        self.publish(EdgeEvent {
            event_type: EdgeEventType::EdgeCreated,
            edge_urn: edge_urn.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            predicate: predicate.to_string(),
            version: Some("v1.3.20".to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            payload: Some(serde_json::json!({
                "action": "created",
                "auto_created": true
            })),
        }).await;
    }

    /// Publish edge.deleted event
    pub async fn publish_edge_deleted(&self, edge_urn: &str, source: &str, target: &str, predicate: &str) {
        self.publish(EdgeEvent {
            event_type: EdgeEventType::EdgeDeleted,
            edge_urn: edge_urn.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            predicate: predicate.to_string(),
            version: Some("v1.3.20".to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            payload: Some(serde_json::json!({
                "action": "deleted"
            })),
        }).await;
    }

    /// Publish edge.updated event
    pub async fn publish_edge_updated(&self, edge_urn: &str, source: &str, target: &str, predicate: &str) {
        self.publish(EdgeEvent {
            event_type: EdgeEventType::EdgeUpdated,
            edge_urn: edge_urn.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            predicate: predicate.to_string(),
            version: Some("v1.3.20".to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            payload: Some(serde_json::json!({
                "action": "updated"
            })),
        }).await;
    }

    /// Publish edge routing event (message flowed through edge)
    pub async fn publish_edge_routed(
        &self,
        edge_urn: &str,
        source: &str,
        target: &str,
        predicate: &str,
        job_id: &str,
    ) {
        self.publish(EdgeEvent {
            event_type: EdgeEventType::EdgeRouted,
            edge_urn: edge_urn.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            predicate: predicate.to_string(),
            version: Some("v1.3.20".to_string()),
            timestamp: chrono::Utc::now().to_rfc3339(),
            payload: Some(serde_json::json!({
                "action": "routed",
                "job_id": job_id
            })),
        }).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_edge_publisher_without_nats() {
        let publisher = EdgeEventPublisher::new(None).await.unwrap();

        // Should not panic when NATS not configured
        publisher
            .publish_edge_created(
                "ckp://Edge#Connection-A-to-B-PRODUCES:v1.3.20",
                "A",
                "B",
                "PRODUCES",
            )
            .await;
    }

    #[test]
    fn test_event_type_subject_suffix() {
        assert_eq!(
            EdgeEventType::EdgeCreated.subject_suffix(),
            "lifecycle.created"
        );
        assert_eq!(
            EdgeEventType::EdgeDeleted.subject_suffix(),
            "lifecycle.deleted"
        );
    }
}
