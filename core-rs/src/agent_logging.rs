//! Agent Logging and NATS Streaming
//!
//! Provides utilities for logging agent conversations to kernel log/ folders
//! and publishing agent messages to NATS for real-time viewing.

use crate::errors::{CkpError, Result};
use async_nats::Client as NatsClient;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;

/// Message type for agent communications
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMessageType {
    Message,
    Question,
    Response,
    Tool,
    Error,
    System,
}

/// Agent message for NATS streaming and logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Message type
    #[serde(rename = "type")]
    pub msg_type: AgentMessageType,

    /// Source kernel name
    pub kernel: String,

    /// Message content
    pub content: String,

    /// ISO 8601 timestamp
    pub timestamp: String,

    /// Optional process URN
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_urn: Option<String>,

    /// Optional agent ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
}

impl AgentMessage {
    /// Create a new agent message
    pub fn new(
        msg_type: AgentMessageType,
        kernel: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            msg_type,
            kernel: kernel.into(),
            content: content.into(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            process_urn: None,
            agent_id: None,
        }
    }

    /// Add process URN
    pub fn with_process_urn(mut self, urn: impl Into<String>) -> Self {
        self.process_urn = Some(urn.into());
        self
    }

    /// Add agent ID
    pub fn with_agent_id(mut self, id: impl Into<String>) -> Self {
        self.agent_id = Some(id.into());
        self
    }
}

/// Agent logger - writes to kernel log/ folder and publishes to NATS
pub struct AgentLogger {
    kernel_name: String,
    log_file_path: PathBuf,
    nats_client: Option<NatsClient>,
}

impl AgentLogger {
    /// Create a new agent logger
    ///
    /// # Arguments
    /// * `kernel_name` - Name of the kernel
    /// * `project_root` - Project root path
    /// * `nats_url` - Optional NATS URL (e.g., "nats://localhost:4222")
    pub async fn new(
        kernel_name: impl Into<String>,
        project_root: impl AsRef<Path>,
        nats_url: Option<&str>,
    ) -> Result<Self> {
        let kernel_name = kernel_name.into();
        let project_root = project_root.as_ref();

        // Create log file path
        let log_dir = project_root.join("concepts").join(&kernel_name).join("logs");
        tokio::fs::create_dir_all(&log_dir).await.map_err(|e| {
            CkpError::IoError(format!("Failed to create logs directory: {}", e))
        })?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let log_file_path = log_dir.join(format!("agent_{}.log", timestamp));

        // Connect to NATS if URL provided
        let nats_client = if let Some(url) = nats_url {
            match async_nats::connect(url).await {
                Ok(client) => Some(client),
                Err(e) => {
                    eprintln!("[AgentLogger] Failed to connect to NATS at {}: {}", url, e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            kernel_name,
            log_file_path,
            nats_client,
        })
    }

    /// Log an agent message
    ///
    /// Writes METADATA ONLY to log file (NO payloads - those go to storage/)
    /// Publishes full message to NATS for real-time viewing
    pub async fn log(&self, message: &AgentMessage) -> Result<()> {
        // Write METADATA ONLY to log file (not payload content)
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file_path)
            .await
            .map_err(|e| CkpError::IoError(format!("Failed to open log file: {}", e)))?;

        // Log metadata only: timestamp, type, agent_id, process_urn
        let log_line = format!(
            "[{}] [{:?}] agent={} process={} content_length={}\n",
            message.timestamp,
            message.msg_type,
            message.agent_id.as_deref().unwrap_or("main"),
            message.process_urn.as_deref().unwrap_or("none"),
            message.content.len()
        );

        file.write_all(log_line.as_bytes())
            .await
            .map_err(|e| CkpError::IoError(format!("Failed to write to log: {}", e)))?;

        file.flush()
            .await
            .map_err(|e| CkpError::IoError(format!("Failed to flush log: {}", e)))?;

        // Publish to NATS if connected
        if let Some(nats) = &self.nats_client {
            let subject = format!("kernel.{}.agent.{:?}", self.kernel_name, message.msg_type);
            let payload = serde_json::to_vec(message)
                .map_err(|e| CkpError::Json(e))?;

            if let Err(e) = nats.publish(subject.to_lowercase(), payload.into()).await {
                eprintln!("[AgentLogger] Failed to publish to NATS: {}", e);
            }

            // Also publish to global stream
            let global_subject = "kernel.global.agent";
            if let Err(e) = nats.publish(global_subject, serde_json::to_vec(message)?.into()).await {
                eprintln!("[AgentLogger] Failed to publish to global stream: {}", e);
            }
        }

        Ok(())
    }

    /// Log a message
    pub async fn message(&self, content: impl Into<String>) -> Result<()> {
        let msg = AgentMessage::new(AgentMessageType::Message, &self.kernel_name, content);
        self.log(&msg).await
    }

    /// Log a question
    pub async fn question(&self, content: impl Into<String>) -> Result<()> {
        let msg = AgentMessage::new(AgentMessageType::Question, &self.kernel_name, content);
        self.log(&msg).await
    }

    /// Log a response
    pub async fn response(&self, content: impl Into<String>) -> Result<()> {
        let msg = AgentMessage::new(AgentMessageType::Response, &self.kernel_name, content);
        self.log(&msg).await
    }

    /// Log a tool call
    pub async fn tool(&self, content: impl Into<String>) -> Result<()> {
        let msg = AgentMessage::new(AgentMessageType::Tool, &self.kernel_name, content);
        self.log(&msg).await
    }

    /// Log an error
    pub async fn error(&self, content: impl Into<String>) -> Result<()> {
        let msg = AgentMessage::new(AgentMessageType::Error, &self.kernel_name, content);
        self.log(&msg).await
    }

    /// Log a system message
    pub async fn system(&self, content: impl Into<String>) -> Result<()> {
        let msg = AgentMessage::new(AgentMessageType::System, &self.kernel_name, content);
        self.log(&msg).await
    }

    /// Get the log file path
    pub fn log_path(&self) -> &Path {
        &self.log_file_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_agent_logger_file() {
        let temp_dir = TempDir::new().unwrap();
        let kernel_name = "TestKernel";

        // Create kernel directory structure
        let concepts_dir = temp_dir.path().join("concepts");
        tokio::fs::create_dir_all(&concepts_dir).await.unwrap();

        let logger = AgentLogger::new(kernel_name, temp_dir.path(), None)
            .await
            .unwrap();

        // Log a message
        logger.message("Test message").await.unwrap();

        // Verify log file exists
        assert!(logger.log_path().exists());

        // Read log file
        let content = tokio::fs::read_to_string(logger.log_path()).await.unwrap();
        assert!(content.contains("Test message"));
        assert!(content.contains("[Message]"));
    }
}
