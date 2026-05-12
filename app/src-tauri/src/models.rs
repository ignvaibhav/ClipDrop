//! Domain models shared across the ClipDrop backend.
//!
//! All request/response types used by the HTTP API and WebSocket event system
//! are defined here. Job lifecycle status is tracked via the [`JobStatus`] enum.

use std::time::Instant;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Lifecycle status of a download job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    InProgress,
    Done,
    Error,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Done => write!(f, "done"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// Type of event broadcast over the WebSocket channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WsEventType {
    Progress,
    Done,
    Error,
}

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

/// Time range for clip-mode downloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipRange {
    pub start: String,
    pub end: String,
}

/// Payload submitted to `POST /download`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    pub title: Option<String>,
    pub format: String,
    pub quality: String,
    pub format_id: Option<String>,
    pub clip: Option<ClipRange>,
}

/// Payload submitted to `POST /formats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatsRequest {
    pub url: String,
}

/// Payload submitted to `POST /reveal`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevealRequest {
    pub path: String,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// A single quality/format option returned by `POST /formats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatOption {
    pub label: String,
    pub format: String,
    pub quality: String,
    pub format_id: Option<String>,
    pub note: Option<String>,
}

/// Response from `POST /formats`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatsResponse {
    pub formats: Vec<FormatOption>,
}

/// Response from `POST /download`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResponse {
    pub job_id: String,
}

/// Response from `GET /health`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub port: u16,
}

/// Response from `GET /status/:job_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub job_id: String,
    pub status: JobStatus,
    pub progress: u8,
    pub message: Option<String>,
    pub output_path: Option<String>,
}

// ---------------------------------------------------------------------------
// WebSocket event
// ---------------------------------------------------------------------------

/// Event broadcast to all connected WebSocket clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEvent {
    pub job_id: String,
    pub event: WsEventType,
    pub percent: Option<u8>,
    pub speed: Option<String>,
    pub eta: Option<String>,
    pub path: Option<String>,
    pub message: Option<String>,
}

impl WsEvent {
    /// Create a progress event.
    pub fn progress(job_id: &str, percent: u8, speed: Option<String>, eta: Option<String>) -> Self {
        Self {
            job_id: job_id.to_string(),
            event: WsEventType::Progress,
            percent: Some(percent),
            speed,
            eta,
            path: None,
            message: None,
        }
    }

    /// Create a completion event.
    pub fn done(job_id: &str, path: String) -> Self {
        Self {
            job_id: job_id.to_string(),
            event: WsEventType::Done,
            percent: Some(100),
            speed: None,
            eta: None,
            path: Some(path),
            message: None,
        }
    }

    /// Create an error event.
    pub fn error(job_id: &str, message: String) -> Self {
        Self {
            job_id: job_id.to_string(),
            event: WsEventType::Error,
            percent: None,
            speed: None,
            eta: None,
            path: None,
            message: Some(message),
        }
    }
}

// ---------------------------------------------------------------------------
// Internal job tracking
// ---------------------------------------------------------------------------

/// Internal record tracking the state of a queued/active download job.
#[derive(Debug, Clone)]
pub struct JobRecord {
    pub status: JobStatus,
    pub progress: u8,
    pub message: Option<String>,
    pub output_path: Option<String>,
    pub created_at: Instant,
}

impl JobRecord {
    /// Create a new record in the [`JobStatus::Queued`] state.
    pub fn queued() -> Self {
        Self {
            status: JobStatus::Queued,
            progress: 0,
            message: None,
            output_path: None,
            created_at: Instant::now(),
        }
    }
}
