//! Sequential download job queue.
//!
//! Jobs are processed one at a time in FIFO order. Status updates are broadcast
//! to all connected WebSocket clients and stored in-memory for polling via
//! `GET /status/:job_id`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::downloader::{self, DownloadEvent};
use crate::models::{DownloadRequest, JobRecord, JobStatus, StatusResponse, WsEvent};

/// Maximum number of job records kept in memory.
const MAX_JOB_RECORDS: usize = 1000;

/// Age threshold (seconds) for evicting completed/errored jobs.
const EVICT_AGE_SECS: u64 = 3600; // 1 hour

/// A queued download job waiting to be processed.
#[derive(Debug, Clone)]
pub struct QueuedJob {
    pub job_id: String,
    pub request: DownloadRequest,
}

/// Shared queue state accessible from the API server and worker.
#[derive(Clone)]
pub struct QueueState {
    jobs: Arc<RwLock<HashMap<String, JobRecord>>>,
    sender: mpsc::Sender<QueuedJob>,
    ws_sender: broadcast::Sender<WsEvent>,
}

impl QueueState {
    /// Create a new queue with the given channel buffer size.
    /// Returns the state handle and the receiver for the worker.
    pub fn new(buffer: usize) -> (Self, mpsc::Receiver<QueuedJob>) {
        let (sender, receiver) = mpsc::channel(buffer);
        let (ws_sender, _) = broadcast::channel(256);

        (
            Self {
                jobs: Arc::new(RwLock::new(HashMap::new())),
                sender,
                ws_sender,
            },
            receiver,
        )
    }

    /// Add a new download job to the queue.
    pub async fn enqueue(&self, job_id: String, request: DownloadRequest) -> anyhow::Result<()> {
        // Evict stale records before inserting to prevent unbounded growth.
        self.evict_stale_jobs().await;

        {
            let mut jobs = self.jobs.write().await;
            jobs.insert(job_id.clone(), JobRecord::queued());
        }

        self.sender
            .send(QueuedJob { job_id, request })
            .await
            .map_err(|e| anyhow::anyhow!("queue send failed: {e}"))
    }

    /// Retrieve the current status of a job.
    pub async fn get_status(&self, job_id: &str) -> Option<StatusResponse> {
        let jobs = self.jobs.read().await;
        jobs.get(job_id).map(|record| StatusResponse {
            job_id: job_id.to_string(),
            status: record.status.clone(),
            progress: record.progress,
            message: record.message.clone(),
            output_path: record.output_path.clone(),
        })
    }

    /// Subscribe to WebSocket event broadcasts.
    pub fn subscribe_ws(&self) -> broadcast::Receiver<WsEvent> {
        self.ws_sender.subscribe()
    }

    /// Update the status of an existing job.
    pub async fn set_status(
        &self,
        job_id: &str,
        status: JobStatus,
        progress: u8,
        message: Option<String>,
        output_path: Option<String>,
    ) {
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.get_mut(job_id) {
            job.status = status;
            job.progress = progress;
            job.message = message;
            job.output_path = output_path;
        }
    }

    /// Broadcast a WebSocket event to all connected clients.
    pub fn publish_event(&self, event: WsEvent) {
        let _ = self.ws_sender.send(event);
    }

    /// Remove completed/errored jobs older than [`EVICT_AGE_SECS`] when the
    /// total record count exceeds [`MAX_JOB_RECORDS`].
    async fn evict_stale_jobs(&self) {
        let mut jobs = self.jobs.write().await;
        if jobs.len() < MAX_JOB_RECORDS {
            return;
        }

        let cutoff = Instant::now() - std::time::Duration::from_secs(EVICT_AGE_SECS);
        let before = jobs.len();
        jobs.retain(|_, record| {
            // Keep active/queued jobs and recently completed ones.
            matches!(record.status, JobStatus::Queued | JobStatus::InProgress)
                || record.created_at > cutoff
        });

        let evicted = before - jobs.len();
        if evicted > 0 {
            info!(evicted, remaining = jobs.len(), "evicted stale job records");
        }
    }
}

/// Run the sequential download worker loop.
///
/// Consumes jobs from the channel and processes them one at a time. Download
/// events are forwarded to the queue for WebSocket broadcast and status tracking.
pub async fn run_worker(
    queue: QueueState,
    config: AppConfig,
    mut receiver: mpsc::Receiver<QueuedJob>,
) {
    while let Some(job) = receiver.recv().await {
        info!(job_id = %job.job_id, url = %job.request.url, "processing download job");

        queue
            .set_status(&job.job_id, JobStatus::InProgress, 0, None, None)
            .await;
        queue.publish_event(WsEvent::progress(&job.job_id, 0, None, None));

        let (event_tx, mut event_rx) = mpsc::channel(128);
        let job_id = job.job_id.clone();
        let request = job.request.clone();
        let worker_config = config.clone();

        let runner = tokio::spawn(async move {
            downloader::run_download(&job_id, &request, event_tx, worker_config).await
        });

        while let Some(event) = event_rx.recv().await {
            match event {
                DownloadEvent::Progress {
                    percent,
                    speed,
                    eta,
                } => {
                    queue
                        .set_status(&job.job_id, JobStatus::InProgress, percent, None, None)
                        .await;
                    queue.publish_event(WsEvent::progress(&job.job_id, percent, speed, eta));
                }
                DownloadEvent::Done { path } => {
                    info!(job_id = %job.job_id, path = %path, "download complete");
                    queue
                        .set_status(&job.job_id, JobStatus::Done, 100, None, Some(path.clone()))
                        .await;
                    queue.publish_event(WsEvent::done(&job.job_id, path));
                }
                DownloadEvent::Error { message } => {
                    warn!(job_id = %job.job_id, error = %message, "download error");
                    queue
                        .set_status(
                            &job.job_id,
                            JobStatus::Error,
                            0,
                            Some(message.clone()),
                            None,
                        )
                        .await;
                    queue.publish_event(WsEvent::error(&job.job_id, message));
                }
            }
        }

        if let Err(join_error) = runner.await {
            let message = format!("worker join error: {join_error}");
            error!(job_id = %job.job_id, error = %message, "worker task panicked");
            queue
                .set_status(
                    &job.job_id,
                    JobStatus::Error,
                    0,
                    Some(message.clone()),
                    None,
                )
                .await;
            queue.publish_event(WsEvent::error(&job.job_id, message));
        }
    }
}
