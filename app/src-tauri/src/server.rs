//! Axum HTTP + WebSocket server for the ClipDrop local API.
//!
//! Binds to `127.0.0.1:{port}` and exposes endpoints consumed by the browser
//! extension. All traffic is local-only — no external network access.

use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::process::Command as StdCommand;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{any, get, post};
use axum::{Json, Router};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::formats;
use crate::models::{
    DownloadRequest, DownloadResponse, FormatsRequest, FormatsResponse, HealthResponse,
    RevealRequest, StatusResponse, WsEvent,
};
use crate::queue::QueueState;

/// Application version — single source of truth.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct ApiState {
    queue: QueueState,
    /// Available for handlers that need runtime config (e.g. custom download path).
    #[allow(dead_code)]
    config: AppConfig,
    port: u16,
}

/// Start the local API server.
pub async fn run(queue: QueueState, config: AppConfig, port: u16) -> anyhow::Result<()> {
    let state = ApiState {
        queue,
        config,
        port,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/formats", post(formats_endpoint))
        .route("/download", post(download))
        .route("/reveal", post(reveal))
        .route("/status/{job_id}", get(status))
        .route("/ws", any(ws_handler))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_methods(Any)
                .allow_origin(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    info!("ClipDrop API listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn health(State(state): State<ApiState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: VERSION.to_string(),
        port: state.port,
    })
}

async fn formats_endpoint(
    State(state): State<ApiState>,
    Json(payload): Json<FormatsRequest>,
) -> Result<Json<FormatsResponse>, AppError> {
    if payload.url.trim().is_empty() {
        return Err(AppError::BadRequest("url is required".to_string()));
    }

    match formats::fetch_formats(&payload.url).await {
        Ok(list) => {
            info!(url = %payload.url, count = list.len(), "format probe complete");
            Ok(Json(FormatsResponse { formats: list }))
        }
        Err(error) => {
            warn!(url = %payload.url, error = %error, "format probe failed");
            state.queue.publish_event(WsEvent::error(
                "system",
                format!("format lookup failed: {error}"),
            ));
            Err(AppError::BadGateway(error.to_string()))
        }
    }
}

async fn download(
    State(state): State<ApiState>,
    Json(payload): Json<DownloadRequest>,
) -> Result<Json<DownloadResponse>, AppError> {
    if payload.url.trim().is_empty() {
        return Err(AppError::BadRequest("url is required".to_string()));
    }
    if payload.format.trim().is_empty() || payload.quality.trim().is_empty() {
        return Err(AppError::BadRequest(
            "format and quality are required".to_string(),
        ));
    }

    let job_id = Uuid::new_v4().to_string();
    info!(job_id = %job_id, url = %payload.url, quality = %payload.quality, "queueing download");

    state
        .queue
        .enqueue(job_id.clone(), payload)
        .await
        .map_err(|e| AppError::ServiceUnavailable(format!("queue unavailable: {e}")))?;

    Ok(Json(DownloadResponse { job_id }))
}

async fn status(
    State(state): State<ApiState>,
    Path(job_id): Path<String>,
) -> Result<Json<StatusResponse>, AppError> {
    state
        .queue
        .get_status(&job_id)
        .await
        .map(Json)
        .ok_or_else(|| AppError::NotFound("job not found".to_string()))
}

async fn reveal(Json(payload): Json<RevealRequest>) -> Result<Json<serde_json::Value>, AppError> {
    let input = payload.path.trim();
    if input.is_empty() {
        return Err(AppError::BadRequest("path is required".to_string()));
    }

    let raw = PathBuf::from(input);
    let target = if raw.is_file() {
        raw.parent()
            .map(FsPath::to_path_buf)
            .unwrap_or_else(|| raw.clone())
    } else {
        raw.clone()
    };

    if !target.exists() {
        return Err(AppError::NotFound("path does not exist".to_string()));
    }

    let result = if cfg!(target_os = "windows") {
        StdCommand::new("explorer").arg(&target).status()
    } else if cfg!(target_os = "macos") {
        StdCommand::new("open").arg(&target).status()
    } else {
        StdCommand::new("xdg-open").arg(&target).status()
    };

    match result {
        Ok(exit) if exit.success() => Ok(Json(serde_json::json!({ "ok": true }))),
        Ok(exit) => Err(AppError::BadGateway(format!(
            "reveal command failed: {exit}"
        ))),
        Err(error) => Err(AppError::BadGateway(format!(
            "failed to start reveal command: {error}"
        ))),
    }
}

// ---------------------------------------------------------------------------
// WebSocket
// ---------------------------------------------------------------------------

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<ApiState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state.queue))
}

async fn handle_socket(mut socket: WebSocket, queue: QueueState) {
    let mut rx = queue.subscribe_ws();

    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            evt = rx.recv() => {
                match evt {
                    Ok(event) => {
                        if let Ok(text) = serde_json::to_string(&event) {
                            if socket.send(Message::Text(text)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }
}
