//! Sequential download job queue.
//!
//! Jobs are processed one at a time in FIFO order. Status updates are broadcast
//! to all connected WebSocket clients and stored in-memory for polling via
//! `GET /status/:job_id`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
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
    active_controls: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    pending_stop_reasons: Arc<RwLock<HashMap<String, String>>>,
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
                active_controls: Arc::new(RwLock::new(HashMap::new())),
                pending_stop_reasons: Arc::new(RwLock::new(HashMap::new())),
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

    /// Cancel a running job or mark a queued one as cancelled.
    pub async fn cancel_job(&self, job_id: &str) -> bool {
        self.request_stop(job_id, "Cancelled by user".to_string()).await
    }

    /// Skip a queued/running job.
    pub async fn skip_job(&self, job_id: &str) -> bool {
        self.request_stop(job_id, "Skipped by user".to_string()).await
    }

    async fn request_stop(&self, job_id: &str, message: String) -> bool {
        let status = {
            let jobs = self.jobs.read().await;
            jobs.get(job_id).map(|job| job.status.clone())
        };

        match status {
            Some(JobStatus::Queued) => {
                self.pending_stop_reasons
                    .write()
                    .await
                    .insert(job_id.to_string(), message.clone());
                self.set_status(job_id, JobStatus::Error, 0, Some(message.clone()), None)
                    .await;
                self.publish_event(WsEvent::error(job_id, message));
                true
            }
            Some(JobStatus::InProgress) => {
                let flag = {
                    let controls = self.active_controls.read().await;
                    controls.get(job_id).cloned()
                };
                if let Some(flag) = flag {
                    flag.store(true, Ordering::SeqCst);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub async fn register_control(&self, job_id: String, flag: Arc<AtomicBool>) {
        self.active_controls.write().await.insert(job_id, flag);
    }

    pub async fn unregister_control(&self, job_id: &str) {
        self.active_controls.write().await.remove(job_id);
        self.pending_stop_reasons.write().await.remove(job_id);
    }

    pub async fn take_pending_stop_reason(&self, job_id: &str) -> Option<String> {
        self.pending_stop_reasons.write().await.remove(job_id)
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
        let cancel_flag = Arc::new(AtomicBool::new(false));
        queue
            .register_control(job.job_id.clone(), cancel_flag.clone())
            .await;

        if let Some(reason) = queue.take_pending_stop_reason(&job.job_id).await {
            info!(job_id = %job.job_id, reason = %reason, "skipping queued job before start");
            queue.unregister_control(&job.job_id).await;
            continue;
        }

        info!(job_id = %job.job_id, url = %job.request.url, "processing download job");

        queue
            .set_status(&job.job_id, JobStatus::InProgress, 0, None, None)
            .await;
        queue.publish_event(WsEvent::progress(&job.job_id, 0, None, None));

        let (event_tx, mut event_rx) = mpsc::channel(128);
        let job_id = job.job_id.clone();
        let request = job.request.clone();
        let worker_config = config.clone();
        let worker_cancel_flag = cancel_flag.clone();

        let runner = tokio::spawn(async move {
            downloader::run_download(
                &job_id,
                &request,
                event_tx,
                worker_config,
                worker_cancel_flag,
            )
            .await
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

        match runner.await {
            Ok(Ok(())) => {}
            Ok(Err(run_error)) => {
                let message = run_error.to_string();
                let already_terminal = {
                    let jobs = queue.jobs.read().await;
                    jobs.get(&job.job_id).is_some_and(|record| {
                        matches!(record.status, JobStatus::Done | JobStatus::Error)
                    })
                };

                if !already_terminal {
                    warn!(job_id = %job.job_id, error = %message, "download runner failed");
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
            Err(join_error) => {
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

        queue.unregister_control(&job.job_id).await;
    }
}
