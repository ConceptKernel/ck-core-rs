//! BFO-aligned process tracking
//!
//! Implements temporal tracking of Occurrents (processes that unfold over time)
//! with explicit temporal parts, participants, and provenance chains.
//!
//! Reference: Node.js v1.3.14 - ProcessTracker.js

use crate::errors::{CkpError, Result};
use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

static PROCESS_TRACKER_INSTANCE: OnceCell<ProcessTracker> = OnceCell::new();

/// BFO-aligned process tracker
pub struct ProcessTracker {
    /// Root concepts directory
    _concepts_root: PathBuf,

    /// Processes storage directory
    processes_dir: PathBuf,
}

/// Process (Occurrent) - temporal entity that unfolds over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Process {
    /// Process URN (format: ckp://Process#{type}-{txId})
    pub urn: String,

    /// Process type (invoke, edge-comm, consensus, broadcast)
    #[serde(rename = "type")]
    pub process_type: String,

    /// Transaction ID
    #[serde(rename = "txId")]
    pub tx_id: String,

    /// Process participants (kernels, instances, edges)
    pub participants: HashMap<String, Value>,

    /// Temporal parts (phases within the process)
    #[serde(rename = "temporalParts")]
    pub temporal_parts: Vec<TemporalPart>,

    /// Temporal region (when the process occurs)
    #[serde(rename = "temporalRegion")]
    pub temporal_region: TemporalRegion,

    /// Process status
    pub status: String,

    /// Additional metadata
    pub metadata: HashMap<String, Value>,

    /// Creation timestamp
    #[serde(rename = "createdAt")]
    pub created_at: String,

    /// Last update timestamp
    #[serde(rename = "updatedAt")]
    pub updated_at: String,

    /// Process result (set on completion)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<HashMap<String, Value>>,

    /// Error message (set on failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Temporal part (phase within a process)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalPart {
    /// Phase name
    pub phase: String,

    /// Phase timestamp
    pub timestamp: String,

    /// Phase-specific data
    pub data: HashMap<String, Value>,
}

/// Temporal region (when a process occurs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalRegion {
    /// Start timestamp
    pub start: String,

    /// End timestamp (null until completion/failure)
    pub end: Option<String>,

    /// Duration in milliseconds (null until completion/failure)
    pub duration: Option<i64>,
}

/// Query filters for process searches
#[derive(Debug, Clone, Default)]
pub struct QueryFilters {
    /// Filter by process type
    pub process_type: Option<String>,

    /// Filter by kernel URN
    pub kernel: Option<String>,

    /// Filter by status
    pub status: Option<String>,

    /// Filter processes starting after this time
    pub start_after: Option<DateTime<Utc>>,

    /// Filter processes starting before this time
    pub start_before: Option<DateTime<Utc>>,

    /// Maximum results
    pub limit: Option<usize>,

    /// Sort order (asc or desc)
    pub order: Option<String>,

    /// Field to sort by (default: createdAt)
    pub sort_field: Option<String>,
}

/// Process statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Statistics {
    /// Total processes
    pub total: usize,

    /// Count by status
    #[serde(rename = "byStatus")]
    pub by_status: HashMap<String, usize>,

    /// Count by type
    #[serde(rename = "byType")]
    pub by_type: HashMap<String, usize>,

    /// Average duration in milliseconds
    #[serde(rename = "avgDuration")]
    pub avg_duration: f64,

    /// Total duration in milliseconds
    #[serde(rename = "totalDuration")]
    pub total_duration: i64,
}

impl ProcessTracker {
    /// Create a new ProcessTracker
    ///
    /// # Arguments
    ///
    /// * `concepts_root` - Root concepts directory
    pub fn new(concepts_root: PathBuf) -> Result<Self> {
        let processes_dir = concepts_root.join(".processes");

        // Create processes directory if it doesn't exist
        if !processes_dir.exists() {
            fs::create_dir_all(&processes_dir)
                .map_err(|e| CkpError::IoError(format!("Failed to create processes directory: {}", e)))?;
        }

        Ok(Self {
            _concepts_root: concepts_root,
            processes_dir,
        })
    }

    /// Get singleton instance
    ///
    /// # Arguments
    ///
    /// * `concepts_root` - Root concepts directory (only used on first call)
    ///
    /// # Returns
    ///
    /// Reference to singleton ProcessTracker instance
    pub fn get_instance(concepts_root: PathBuf) -> &'static Self {
        PROCESS_TRACKER_INSTANCE.get_or_init(|| {
            Self::new(concepts_root).expect("Failed to initialize ProcessTracker")
        })
    }

    /// Generate process URN
    ///
    /// # Arguments
    ///
    /// * `process_type` - Process type (invoke, edge-comm, etc.)
    /// * `tx_id` - Transaction ID
    ///
    /// # Returns
    ///
    /// Process URN in format: ckp://Process#{type}-{txId}
    ///
    /// # Example
    ///
    /// ```
    /// # use ckp_core::ProcessTracker;
    /// # use std::path::PathBuf;
    /// let tracker = ProcessTracker::new(PathBuf::from("/tmp")).unwrap();
    /// let urn = tracker.generate_process_urn("invoke", "1763656265921-c8788f41");
    /// assert_eq!(urn, "ckp://Process#invoke-1763656265921-c8788f41");
    /// ```
    pub fn generate_process_urn(&self, process_type: &str, tx_id: &str) -> String {
        format!("ckp://Process#{}-{}", process_type, tx_id)
    }

    /// Create new process
    ///
    /// # Arguments
    ///
    /// * `process_type` - Process type
    /// * `tx_id` - Transaction ID
    /// * `participants` - Process participants
    /// * `metadata` - Additional metadata
    ///
    /// # Returns
    ///
    /// Created process object
    pub fn create_process(
        &self,
        process_type: &str,
        tx_id: &str,
        participants: HashMap<String, Value>,
        metadata: HashMap<String, Value>,
    ) -> Result<Process> {
        let urn = self.generate_process_urn(process_type, tx_id);
        let now = Utc::now().to_rfc3339();

        let process = Process {
            urn,
            process_type: process_type.to_string(),
            tx_id: tx_id.to_string(),
            participants,
            temporal_parts: Vec::new(),
            temporal_region: TemporalRegion {
                start: now.clone(),
                end: None,
                duration: None,
            },
            status: "created".to_string(),
            metadata,
            created_at: now.clone(),
            updated_at: now,
            result: None,
            error: None,
        };

        self.save_process(&process)?;
        Ok(process)
    }

    /// Add temporal part to process
    ///
    /// # Arguments
    ///
    /// * `process_urn` - Process URN
    /// * `phase` - Phase name (accepted, processing, completed, failed, etc.)
    /// * `data` - Phase-specific data
    pub fn add_temporal_part(
        &self,
        process_urn: &str,
        phase: &str,
        data: HashMap<String, Value>,
    ) -> Result<()> {
        let mut process = self.load_process(process_urn)
            .ok_or_else(|| CkpError::ProcessError(format!("Process not found: {}", process_urn)))?;

        let now = Utc::now();
        let timestamp = now.to_rfc3339();

        // Add temporal part
        process.temporal_parts.push(TemporalPart {
            phase: phase.to_string(),
            timestamp: timestamp.clone(),
            data,
        });

        // Update status based on phase
        match phase {
            "accepted" => {
                process.status = "accepted".to_string();
                // Set start time if not already set
                if process.temporal_region.start.is_empty() {
                    process.temporal_region.start = timestamp;
                }
            }
            "processing" => {
                process.status = "processing".to_string();
            }
            "completed" => {
                process.status = "completed".to_string();
                // Set end time and calculate duration
                let start_time = DateTime::parse_from_rfc3339(&process.temporal_region.start)
                    .map_err(|e| CkpError::ParseError(format!("Invalid start time: {}", e)))?;
                let duration = now.signed_duration_since(start_time.with_timezone(&Utc)).num_milliseconds();

                process.temporal_region.end = Some(timestamp);
                process.temporal_region.duration = Some(duration);
            }
            "failed" => {
                process.status = "failed".to_string();
                // Set end time and calculate duration
                let start_time = DateTime::parse_from_rfc3339(&process.temporal_region.start)
                    .map_err(|e| CkpError::ParseError(format!("Invalid start time: {}", e)))?;
                let duration = now.signed_duration_since(start_time.with_timezone(&Utc)).num_milliseconds();

                process.temporal_region.end = Some(timestamp);
                process.temporal_region.duration = Some(duration);
            }
            _ => {}
        }

        process.updated_at = now.to_rfc3339();
        self.save_process(&process)?;

        Ok(())
    }

    /// Complete process
    ///
    /// # Arguments
    ///
    /// * `process_urn` - Process URN
    /// * `result` - Process result data
    pub fn complete_process(
        &self,
        process_urn: &str,
        result: HashMap<String, Value>,
    ) -> Result<()> {
        let mut process = self.load_process(process_urn)
            .ok_or_else(|| CkpError::ProcessError(format!("Process not found: {}", process_urn)))?;

        let now = Utc::now();

        // Add completed temporal part
        process.temporal_parts.push(TemporalPart {
            phase: "completed".to_string(),
            timestamp: now.to_rfc3339(),
            data: result.clone(),
        });

        // Update status and temporal region
        process.status = "completed".to_string();
        let start_time = DateTime::parse_from_rfc3339(&process.temporal_region.start)
            .map_err(|e| CkpError::ParseError(format!("Invalid start time: {}", e)))?;
        let duration = now.signed_duration_since(start_time.with_timezone(&Utc)).num_milliseconds();

        process.temporal_region.end = Some(now.to_rfc3339());
        process.temporal_region.duration = Some(duration);
        process.result = Some(result);
        process.updated_at = now.to_rfc3339();

        self.save_process(&process)?;
        Ok(())
    }

    /// Fail process
    ///
    /// # Arguments
    ///
    /// * `process_urn` - Process URN
    /// * `error` - Error message
    pub fn fail_process(&self, process_urn: &str, error: &str) -> Result<()> {
        let mut process = self.load_process(process_urn)
            .ok_or_else(|| CkpError::ProcessError(format!("Process not found: {}", process_urn)))?;

        let now = Utc::now();

        // Add failed temporal part with error
        let mut data = HashMap::new();
        data.insert("error".to_string(), Value::String(error.to_string()));

        process.temporal_parts.push(TemporalPart {
            phase: "failed".to_string(),
            timestamp: now.to_rfc3339(),
            data,
        });

        // Update status and temporal region
        process.status = "failed".to_string();
        let start_time = DateTime::parse_from_rfc3339(&process.temporal_region.start)
            .map_err(|e| CkpError::ParseError(format!("Invalid start time: {}", e)))?;
        let duration = now.signed_duration_since(start_time.with_timezone(&Utc)).num_milliseconds();

        process.temporal_region.end = Some(now.to_rfc3339());
        process.temporal_region.duration = Some(duration);
        process.error = Some(error.to_string());
        process.updated_at = now.to_rfc3339();

        self.save_process(&process)?;
        Ok(())
    }

    /// Save process to disk
    ///
    /// # Arguments
    ///
    /// * `process` - Process to save
    pub fn save_process(&self, process: &Process) -> Result<()> {
        let type_dir = self.processes_dir.join(&process.process_type);

        if !type_dir.exists() {
            fs::create_dir_all(&type_dir)
                .map_err(|e| CkpError::IoError(format!("Failed to create type directory: {}", e)))?;
        }

        let file_path = type_dir.join(format!("{}.json", process.tx_id));
        let json = serde_json::to_string_pretty(process)
            .map_err(|e| CkpError::Json(e))?;

        fs::write(&file_path, json)
            .map_err(|e| CkpError::IoError(format!("Failed to write process file: {}", e)))?;

        Ok(())
    }

    /// Load process from disk
    ///
    /// # Arguments
    ///
    /// * `process_urn` - Process URN
    ///
    /// # Returns
    ///
    /// Process object or None if not found
    pub fn load_process(&self, process_urn: &str) -> Option<Process> {
        // Parse URN: ckp://Process#{type}-{txId}
        // Use [^-]+ for type to match only up to first dash (non-greedy)
        let re = Regex::new(r"^ckp://Process#([^-]+)-(.+)$").ok()?;
        let caps = re.captures(process_urn)?;

        let process_type = caps.get(1)?.as_str();
        let tx_id = caps.get(2)?.as_str();

        let file_path = self.processes_dir
            .join(process_type)
            .join(format!("{}.json", tx_id));

        if !file_path.exists() {
            return None;
        }

        let json = fs::read_to_string(&file_path).ok()?;
        serde_json::from_str(&json).ok()
    }

    /// Query processes with filters
    ///
    /// # Arguments
    ///
    /// * `filters` - Query filters
    ///
    /// # Returns
    ///
    /// Array of matching processes sorted by txId descending (most recent first)
    pub fn query_processes(&self, filters: QueryFilters) -> Result<Vec<Process>> {
        let limit = filters.limit.unwrap_or(100);
        let mut results = Vec::new();

        // Determine which directories to search
        let type_dirs: Vec<PathBuf> = if let Some(ref process_type) = filters.process_type {
            vec![self.processes_dir.join(process_type)]
        } else {
            fs::read_dir(&self.processes_dir)
                .map_err(|e| CkpError::IoError(e.to_string()))?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_dir())
                .map(|e| e.path())
                .collect()
        };

        // Collect all process files
        let mut files: Vec<PathBuf> = Vec::new();
        for type_dir in type_dirs {
            if !type_dir.exists() {
                continue;
            }

            let dir_files: Vec<PathBuf> = fs::read_dir(&type_dir)
                .map_err(|e| CkpError::IoError(e.to_string()))?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
                .map(|e| e.path())
                .collect();

            files.extend(dir_files);
        }

        // Load all processes first for flexible sorting
        let mut all_processes: Vec<Process> = Vec::new();
        for file_path in files {
            let json = match fs::read_to_string(&file_path) {
                Ok(j) => j,
                Err(_) => continue,
            };

            let process: Process = match serde_json::from_str(&json) {
                Ok(p) => p,
                Err(_) => continue,
            };

            // Apply filters
            if let Some(ref kernel) = filters.kernel {
                if process.participants.get("kernel").and_then(|v| v.as_str()) != Some(kernel) {
                    continue;
                }
            }

            if let Some(ref status) = filters.status {
                if &process.status != status {
                    continue;
                }
            }

            if let Some(ref start_after) = filters.start_after {
                if let Ok(start_time) = DateTime::parse_from_rfc3339(&process.temporal_region.start) {
                    if start_time.with_timezone(&Utc) <= *start_after {
                        continue;
                    }
                }
            }

            if let Some(ref start_before) = filters.start_before {
                if let Ok(start_time) = DateTime::parse_from_rfc3339(&process.temporal_region.start) {
                    if start_time.with_timezone(&Utc) >= *start_before {
                        continue;
                    }
                }
            }

            all_processes.push(process);
        }

        // Sort processes based on sort_field and order
        let sort_field = filters.sort_field.unwrap_or_else(|| "createdAt".to_string());
        let is_ascending = filters.order.as_ref().map(|o| o == "asc").unwrap_or(false);

        all_processes.sort_by(|a, b| {
            let ordering = match sort_field.as_str() {
                "createdAt" => a.created_at.cmp(&b.created_at),
                "updatedAt" => a.updated_at.cmp(&b.updated_at),
                "txId" => a.tx_id.cmp(&b.tx_id),
                "status" => a.status.cmp(&b.status),
                "type" => a.process_type.cmp(&b.process_type),
                _ => a.created_at.cmp(&b.created_at), // Default to createdAt
            };

            if is_ascending {
                ordering
            } else {
                ordering.reverse()
            }
        });

        // Apply limit
        results = all_processes.into_iter().take(limit).collect();

        Ok(results)
    }

    /// Get statistics for processes
    ///
    /// # Arguments
    ///
    /// * `filters` - Query filters
    ///
    /// # Returns
    ///
    /// Statistics summary
    pub fn get_statistics(&self, filters: QueryFilters) -> Result<Statistics> {
        let processes = self.query_processes(QueryFilters {
            limit: Some(10000),
            ..filters
        })?;

        let mut by_status: HashMap<String, usize> = HashMap::new();
        let mut by_type: HashMap<String, usize> = HashMap::new();
        let mut total_duration: i64 = 0;
        let mut duration_count: usize = 0;

        for process in &processes {
            *by_status.entry(process.status.clone()).or_insert(0) += 1;
            *by_type.entry(process.process_type.clone()).or_insert(0) += 1;

            if let Some(duration) = process.temporal_region.duration {
                total_duration += duration;
                duration_count += 1;
            }
        }

        let avg_duration = if duration_count > 0 {
            total_duration as f64 / duration_count as f64
        } else {
            0.0
        };

        Ok(Statistics {
            total: processes.len(),
            by_status,
            by_type,
            avg_duration,
            total_duration,
        })
    }

    /// Get provenance chain for an instance
    ///
    /// # Arguments
    ///
    /// * `instance_urn` - Instance URN
    ///
    /// # Returns
    ///
    /// Array of processes that created this instance, sorted by start time ascending
    pub fn get_provenance_chain(&self, instance_urn: &str) -> Result<Vec<Process>> {
        let processes = self.query_processes(QueryFilters {
            limit: Some(10000),
            ..Default::default()
        })?;

        let mut matching: Vec<Process> = processes
            .into_iter()
            .filter(|p| {
                p.participants.get("outputInstance").and_then(|v| v.as_str()) == Some(instance_urn)
            })
            .collect();

        // Sort by start time ascending (oldest first)
        matching.sort_by(|a, b| a.temporal_region.start.cmp(&b.temporal_region.start));

        Ok(matching)
    }

    // ========================================================================
    // PROCESS ANALYTICS (Phase 4 Stage 3)
    // ========================================================================

    /// Get collaboration patterns - which kernels frequently collaborate
    ///
    /// Analyzes participations to find kernel pairs that frequently appear together.
    ///
    /// # Returns
    /// Vector of (kernel1, kernel2, count) tuples sorted by frequency
    pub fn get_collaboration_patterns(&self) -> Result<Vec<(String, String, usize)>> {
        use std::collections::HashMap;

        let processes = self.query_processes(QueryFilters {
            limit: Some(10000),
            ..Default::default()
        })?;

        // Count co-occurrences
        let mut collaborations: HashMap<(String, String), usize> = HashMap::new();

        for process in &processes {
            // Extract kernel participants
            let kernels: Vec<String> = process
                .participants
                .values()
                .filter_map(|v| v.as_str())
                .filter(|s| s.starts_with("ckp://"))
                .map(|s| s.to_string())
                .collect();

            // Count pairwise collaborations
            for i in 0..kernels.len() {
                for j in (i + 1)..kernels.len() {
                    let pair = if kernels[i] < kernels[j] {
                        (kernels[i].clone(), kernels[j].clone())
                    } else {
                        (kernels[j].clone(), kernels[i].clone())
                    };

                    *collaborations.entry(pair).or_insert(0) += 1;
                }
            }
        }

        // Convert to sorted vec
        let mut results: Vec<(String, String, usize)> = collaborations
            .into_iter()
            .map(|((k1, k2), count)| (k1, k2, count))
            .collect();

        results.sort_by(|a, b| b.2.cmp(&a.2)); // Sort by count descending

        Ok(results)
    }

    /// Get process duration statistics by process type
    ///
    /// Analyzes completed processes to compute average, min, max duration.
    ///
    /// # Returns
    /// Map of process_type -> (count, avg_ms, min_ms, max_ms)
    pub fn get_duration_statistics(&self) -> Result<HashMap<String, (usize, f64, u64, u64)>> {
        let processes = self.query_processes(QueryFilters {
            status: Some("completed".to_string()),
            limit: Some(10000),
            ..Default::default()
        })?;

        use std::collections::HashMap;
        let mut stats: HashMap<String, Vec<u64>> = HashMap::new();

        for process in &processes {
            if let Some(end) = &process.temporal_region.end {
                // Parse ISO 8601 timestamps
                if let (Ok(start_time), Ok(end_time)) = (
                    chrono::DateTime::parse_from_rfc3339(&process.temporal_region.start),
                    chrono::DateTime::parse_from_rfc3339(end),
                ) {
                    let duration_ms = (end_time.timestamp_millis() - start_time.timestamp_millis()) as u64;
                    stats
                        .entry(process.process_type.clone())
                        .or_insert_with(Vec::new)
                        .push(duration_ms);
                }
            }
        }

        // Compute statistics
        let mut results = HashMap::new();
        for (process_type, durations) in stats {
            if !durations.is_empty() {
                let count = durations.len();
                let sum: u64 = durations.iter().sum();
                let avg = sum as f64 / count as f64;
                let min = *durations.iter().min().unwrap();
                let max = *durations.iter().max().unwrap();

                results.insert(process_type, (count, avg, min, max));
            }
        }

        Ok(results)
    }

    /// Get most active kernels by process count
    ///
    /// Returns kernels sorted by number of processes they've participated in.
    ///
    /// # Arguments
    /// * `limit` - Maximum number of kernels to return
    ///
    /// # Returns
    /// Vector of (kernel_urn, process_count) tuples
    pub fn get_most_active_kernels(&self, limit: usize) -> Result<Vec<(String, usize)>> {
        use std::collections::HashMap;

        let processes = self.query_processes(QueryFilters {
            limit: Some(10000),
            ..Default::default()
        })?;

        let mut kernel_counts: HashMap<String, usize> = HashMap::new();

        for process in &processes {
            for value in process.participants.values() {
                if let Some(kernel_urn) = value.as_str() {
                    if kernel_urn.starts_with("ckp://") {
                        *kernel_counts.entry(kernel_urn.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        let mut results: Vec<(String, usize)> = kernel_counts.into_iter().collect();
        results.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending
        results.truncate(limit);

        Ok(results)
    }

    /// Get process failure analysis
    ///
    /// Analyzes failed processes to identify common failure patterns.
    ///
    /// # Returns
    /// Map of process_type -> (failure_count, total_count, failure_rate)
    pub fn get_failure_analysis(&self) -> Result<HashMap<String, (usize, usize, f64)>> {
        use std::collections::HashMap;

        let all_processes = self.query_processes(QueryFilters {
            limit: Some(10000),
            ..Default::default()
        })?;

        let mut totals: HashMap<String, (usize, usize)> = HashMap::new(); // (failures, total)

        for process in &all_processes {
            let entry = totals
                .entry(process.process_type.clone())
                .or_insert((0, 0));

            entry.1 += 1; // Total count

            if process.status == "failed" {
                entry.0 += 1; // Failure count
            }
        }

        let mut results = HashMap::new();
        for (process_type, (failures, total)) in totals {
            let failure_rate = if total > 0 {
                failures as f64 / total as f64
            } else {
                0.0
            };

            results.insert(process_type, (failures, total, failure_rate));
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_tracker() -> (TempDir, ProcessTracker) {
        let temp_dir = TempDir::new().unwrap();
        let tracker = ProcessTracker::new(temp_dir.path().to_path_buf()).unwrap();
        (temp_dir, tracker)
    }

    #[test]
    fn test_generate_process_urn() {
        let (_temp, tracker) = setup_tracker();
        let urn = tracker.generate_process_urn("invoke", "1763656265921-c8788f41");
        assert_eq!(urn, "ckp://Process#invoke-1763656265921-c8788f41");
    }

    #[test]
    fn test_create_process() {
        let (_temp, tracker) = setup_tracker();

        let mut participants = HashMap::new();
        participants.insert("kernel".to_string(), Value::String("ckp://Test:v0.1".to_string()));

        let metadata = HashMap::new();

        let process = tracker.create_process("invoke", "test-123", participants, metadata).unwrap();

        assert_eq!(process.process_type, "invoke");
        assert_eq!(process.tx_id, "test-123");
        assert_eq!(process.status, "created");
        assert!(process.temporal_parts.is_empty());
    }

    #[test]
    fn test_add_temporal_part() {
        let (_temp, tracker) = setup_tracker();

        let process = tracker.create_process("invoke", "test-123", HashMap::new(), HashMap::new()).unwrap();

        tracker.add_temporal_part(&process.urn, "accepted", HashMap::new()).unwrap();

        let loaded = tracker.load_process(&process.urn).unwrap();
        assert_eq!(loaded.status, "accepted");
        assert_eq!(loaded.temporal_parts.len(), 1);
        assert_eq!(loaded.temporal_parts[0].phase, "accepted");
    }

    #[test]
    fn test_complete_process() {
        let (_temp, tracker) = setup_tracker();

        let process = tracker.create_process("invoke", "test-123", HashMap::new(), HashMap::new()).unwrap();

        let mut result = HashMap::new();
        result.insert("status".to_string(), Value::String("success".to_string()));

        tracker.complete_process(&process.urn, result).unwrap();

        let loaded = tracker.load_process(&process.urn).unwrap();
        assert_eq!(loaded.status, "completed");
        assert!(loaded.temporal_region.end.is_some());
        assert!(loaded.temporal_region.duration.is_some());
        assert!(loaded.result.is_some());
    }

    #[test]
    fn test_fail_process() {
        let (_temp, tracker) = setup_tracker();

        let process = tracker.create_process("invoke", "test-123", HashMap::new(), HashMap::new()).unwrap();

        tracker.fail_process(&process.urn, "Test error").unwrap();

        let loaded = tracker.load_process(&process.urn).unwrap();
        assert_eq!(loaded.status, "failed");
        assert!(loaded.temporal_region.end.is_some());
        assert!(loaded.temporal_region.duration.is_some());
        assert_eq!(loaded.error, Some("Test error".to_string()));
    }

    #[test]
    fn test_load_process() {
        let (_temp, tracker) = setup_tracker();

        let process = tracker.create_process("invoke", "test-123", HashMap::new(), HashMap::new()).unwrap();

        let loaded = tracker.load_process(&process.urn).unwrap();
        assert_eq!(loaded.urn, process.urn);
        assert_eq!(loaded.tx_id, process.tx_id);
    }

    #[test]
    fn test_load_process_not_found() {
        let (_temp, tracker) = setup_tracker();

        let loaded = tracker.load_process("ckp://Process#invoke-nonexistent");
        assert!(loaded.is_none());
    }

    #[test]
    fn test_query_processes() {
        let (_temp, tracker) = setup_tracker();

        tracker.create_process("invoke", "test-1", HashMap::new(), HashMap::new()).unwrap();
        tracker.create_process("invoke", "test-2", HashMap::new(), HashMap::new()).unwrap();
        tracker.create_process("edgecomm", "test-3", HashMap::new(), HashMap::new()).unwrap();

        let results = tracker.query_processes(QueryFilters::default()).unwrap();
        assert_eq!(results.len(), 3);

        let invoke_results = tracker.query_processes(QueryFilters {
            process_type: Some("invoke".to_string()),
            ..Default::default()
        }).unwrap();
        assert_eq!(invoke_results.len(), 2);
    }

    #[test]
    fn test_query_processes_by_status() {
        let (_temp, tracker) = setup_tracker();

        let process = tracker.create_process("invoke", "test-1", HashMap::new(), HashMap::new()).unwrap();
        tracker.complete_process(&process.urn, HashMap::new()).unwrap();

        tracker.create_process("invoke", "test-2", HashMap::new(), HashMap::new()).unwrap();

        let completed = tracker.query_processes(QueryFilters {
            status: Some("completed".to_string()),
            ..Default::default()
        }).unwrap();
        assert_eq!(completed.len(), 1);

        let created = tracker.query_processes(QueryFilters {
            status: Some("created".to_string()),
            ..Default::default()
        }).unwrap();
        assert_eq!(created.len(), 1);
    }

    #[test]
    fn test_get_statistics() {
        let (_temp, tracker) = setup_tracker();

        let p1 = tracker.create_process("invoke", "test-1", HashMap::new(), HashMap::new()).unwrap();
        tracker.complete_process(&p1.urn, HashMap::new()).unwrap();

        let p2 = tracker.create_process("invoke", "test-2", HashMap::new(), HashMap::new()).unwrap();
        tracker.fail_process(&p2.urn, "error").unwrap();

        tracker.create_process("edgecomm", "test-3", HashMap::new(), HashMap::new()).unwrap();

        let stats = tracker.get_statistics(QueryFilters::default()).unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.by_status.get("completed"), Some(&1));
        assert_eq!(stats.by_status.get("failed"), Some(&1));
        assert_eq!(stats.by_status.get("created"), Some(&1));
        assert_eq!(stats.by_type.get("invoke"), Some(&2));
        assert_eq!(stats.by_type.get("edgecomm"), Some(&1));
    }

    #[test]
    fn test_get_provenance_chain() {
        let (_temp, tracker) = setup_tracker();

        let mut participants1 = HashMap::new();
        participants1.insert("outputInstance".to_string(), Value::String("ckp://Test:v0.1#instance-1".to_string()));

        let mut participants2 = HashMap::new();
        participants2.insert("outputInstance".to_string(), Value::String("ckp://Test:v0.1#instance-2".to_string()));

        tracker.create_process("invoke", "test-1", participants1, HashMap::new()).unwrap();
        tracker.create_process("invoke", "test-2", participants2, HashMap::new()).unwrap();

        let chain = tracker.get_provenance_chain("ckp://Test:v0.1#instance-1").unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].tx_id, "test-1");
    }

    #[test]
    fn test_temporal_region_duration() {
        let (_temp, tracker) = setup_tracker();

        let process = tracker.create_process("invoke", "test-123", HashMap::new(), HashMap::new()).unwrap();

        // Small delay to ensure measurable duration
        std::thread::sleep(std::time::Duration::from_millis(10));

        tracker.complete_process(&process.urn, HashMap::new()).unwrap();

        let loaded = tracker.load_process(&process.urn).unwrap();
        assert!(loaded.temporal_region.duration.is_some());
        assert!(loaded.temporal_region.duration.unwrap() >= 10);
    }

    #[test]
    fn test_query_by_tx_id_large_dataset() {
        let (_temp, tracker) = setup_tracker();

        // Create 100+ transactions across different types
        for i in 0..120 {
            let process_type = if i % 3 == 0 { "invoke" } else if i % 3 == 1 { "edgecomm" } else { "consensus" };
            let tx_id = format!("test-{:04}", i);
            tracker.create_process(process_type, &tx_id, HashMap::new(), HashMap::new()).unwrap();
        }

        // Query all processes (should respect limit)
        let all_results = tracker.query_processes(QueryFilters {
            limit: Some(100),
            ..Default::default()
        }).unwrap();
        assert_eq!(all_results.len(), 100);

        // Query specific type
        let invoke_results = tracker.query_processes(QueryFilters {
            process_type: Some("invoke".to_string()),
            limit: Some(50),
            ..Default::default()
        }).unwrap();
        assert_eq!(invoke_results.len(), 40); // 120 / 3 = 40 invoke processes

        // Verify sorting (most recent first by tx_id)
        assert!(all_results[0].tx_id > all_results[1].tx_id);
    }

    #[test]
    fn test_query_recent_transactions() {
        let (_temp, tracker) = setup_tracker();

        // Create transactions with slight delays to ensure ordering
        for i in 0..10 {
            let tx_id = format!("test-{:02}", i);
            tracker.create_process("invoke", &tx_id, HashMap::new(), HashMap::new()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        // Query recent 5 transactions
        let recent = tracker.query_processes(QueryFilters {
            limit: Some(5),
            ..Default::default()
        }).unwrap();
        assert_eq!(recent.len(), 5);

        // Verify they are the most recent (sorted by tx_id descending)
        assert_eq!(recent[0].tx_id, "test-09");
        assert_eq!(recent[1].tx_id, "test-08");
        assert_eq!(recent[2].tx_id, "test-07");
        assert_eq!(recent[3].tx_id, "test-06");
        assert_eq!(recent[4].tx_id, "test-05");
    }

    #[test]
    fn test_track_transaction_provenance() {
        let (_temp, tracker) = setup_tracker();

        // Create parent transaction
        let mut parent_participants = HashMap::new();
        parent_participants.insert("kernel".to_string(), Value::String("ckp://Parent:v0.1".to_string()));
        parent_participants.insert("outputInstance".to_string(), Value::String("ckp://Parent:v0.1#instance-1".to_string()));

        let parent = tracker.create_process("invoke", "parent-tx-001", parent_participants, HashMap::new()).unwrap();
        tracker.complete_process(&parent.urn, HashMap::new()).unwrap();

        // Create child transaction that references parent's output
        let mut child_participants = HashMap::new();
        child_participants.insert("kernel".to_string(), Value::String("ckp://Child:v0.1".to_string()));
        child_participants.insert("inputInstance".to_string(), Value::String("ckp://Parent:v0.1#instance-1".to_string()));
        child_participants.insert("outputInstance".to_string(), Value::String("ckp://Child:v0.1#instance-2".to_string()));

        let child = tracker.create_process("edgecomm", "child-tx-002", child_participants, HashMap::new()).unwrap();
        tracker.complete_process(&child.urn, HashMap::new()).unwrap();

        // Verify provenance relationship
        let parent_loaded = tracker.load_process(&parent.urn).unwrap();
        let child_loaded = tracker.load_process(&child.urn).unwrap();

        assert_eq!(parent_loaded.status, "completed");
        assert_eq!(child_loaded.status, "completed");

        // Child should reference parent's output as input
        assert_eq!(
            child_loaded.participants.get("inputInstance").and_then(|v| v.as_str()),
            Some("ckp://Parent:v0.1#instance-1")
        );
    }

    #[test]
    fn test_provenance_chain_query() {
        let (_temp, tracker) = setup_tracker();

        // Create a chain: tx1 -> instance-1 -> tx2 -> instance-2 -> tx3 -> instance-3
        let mut participants1 = HashMap::new();
        participants1.insert("kernel".to_string(), Value::String("ckp://Kernel1:v0.1".to_string()));
        participants1.insert("outputInstance".to_string(), Value::String("ckp://Test:v0.1#instance-1".to_string()));
        tracker.create_process("invoke", "tx1-001", participants1, HashMap::new()).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let mut participants2 = HashMap::new();
        participants2.insert("kernel".to_string(), Value::String("ckp://Kernel2:v0.1".to_string()));
        participants2.insert("inputInstance".to_string(), Value::String("ckp://Test:v0.1#instance-1".to_string()));
        participants2.insert("outputInstance".to_string(), Value::String("ckp://Test:v0.1#instance-2".to_string()));
        tracker.create_process("edgecomm", "tx2-002", participants2, HashMap::new()).unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        let mut participants3 = HashMap::new();
        participants3.insert("kernel".to_string(), Value::String("ckp://Kernel3:v0.1".to_string()));
        participants3.insert("inputInstance".to_string(), Value::String("ckp://Test:v0.1#instance-2".to_string()));
        participants3.insert("outputInstance".to_string(), Value::String("ckp://Test:v0.1#instance-3".to_string()));
        tracker.create_process("consensus", "tx3-003", participants3, HashMap::new()).unwrap();

        // Query provenance chain for instance-2
        let chain2 = tracker.get_provenance_chain("ckp://Test:v0.1#instance-2").unwrap();
        assert_eq!(chain2.len(), 1);
        assert_eq!(chain2[0].tx_id, "tx2-002");

        // Query provenance chain for instance-3
        let chain3 = tracker.get_provenance_chain("ckp://Test:v0.1#instance-3").unwrap();
        assert_eq!(chain3.len(), 1);
        assert_eq!(chain3[0].tx_id, "tx3-003");

        // Query provenance chain for instance-1
        let chain1 = tracker.get_provenance_chain("ckp://Test:v0.1#instance-1").unwrap();
        assert_eq!(chain1.len(), 1);
        assert_eq!(chain1[0].tx_id, "tx1-001");

        // Verify chain ordering (oldest first)
        assert!(chain1[0].temporal_region.start < chain2[0].temporal_region.start);
        assert!(chain2[0].temporal_region.start < chain3[0].temporal_region.start);
    }
}
