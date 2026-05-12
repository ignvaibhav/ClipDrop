//! Centralized error types for the ClipDrop HTTP API.
//!
//! [`AppError`] implements [`axum::response::IntoResponse`] so handlers can
//! return `Result<T, AppError>` and get consistent JSON error bodies.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

/// Application-level error that maps cleanly to HTTP status + JSON body.
#[derive(Debug)]
pub enum AppError {
    /// 400 — invalid input from the client.
    BadRequest(String),
    /// 404 — requested resource does not exist.
    NotFound(String),
    /// 502 — upstream dependency (yt-dlp, file system command) failed.
    BadGateway(String),
    /// 503 — service temporarily unavailable (queue full, etc.).
    ServiceUnavailable(String),
    /// 500 — unexpected internal error.
    Internal(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadRequest(msg) => write!(f, "bad request: {msg}"),
            Self::NotFound(msg) => write!(f, "not found: {msg}"),
            Self::BadGateway(msg) => write!(f, "bad gateway: {msg}"),
            Self::ServiceUnavailable(msg) => write!(f, "service unavailable: {msg}"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            Self::BadGateway(msg) => (StatusCode::BAD_GATEWAY, msg.clone()),
            Self::ServiceUnavailable(msg) => (StatusCode::SERVICE_UNAVAILABLE, msg.clone()),
            Self::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.clone()),
        };

        let body = serde_json::json!({ "error": message });
        (status, Json(body)).into_response()
    }
}
