//! yt-dlp download execution and progress parsing.
//!
//! Resolves the yt-dlp binary (sidecar first, then PATH fallback), constructs
//! the command with the appropriate flags, and streams progress events back to
//! the caller via an [`mpsc::Sender`].

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::models::DownloadRequest;

/// Maximum time allowed for a single download subprocess.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60); // 30 minutes

/// YouTube extractor arguments for broader client coverage.
const EXTRACTOR_ARGS: &str = "youtube:player_client=android_vr,web_safari,android,web";

// Pre-compiled regex patterns — built once, reused for every download.
static PROGRESS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[download\]\s+(\d{1,3}(?:\.\d+)?)%").unwrap());
static SPEED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bat\s+([^\s]+)").unwrap());
static ETA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bETA\s+([^\s]+)").unwrap());

/// Events emitted during a download for progress tracking.
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Progress {
        percent: u8,
        speed: Option<String>,
        eta: Option<String>,
    },
    Done {
        path: String,
    },
    Error {
        message: String,
    },
}

/// Execute a download job, streaming progress events through `tx`.
///
/// Attempts the sidecar binary first. If the sidecar fails and a different
/// PATH binary is available, retries once with the PATH binary.
pub async fn run_download(
    job_id: &str,
    request: &DownloadRequest,
    tx: mpsc::Sender<DownloadEvent>,
    config: AppConfig,
) -> Result<()> {
    let yt_dlp = resolve_binary("yt-dlp")?;
    let output_template = build_output_template(request, &config).await;
    let yt_dlp_fallback = which::which("yt-dlp").ok();

    let mut cmd = build_download_command(&yt_dlp, request, &output_template);
    let mut attempted_fallback = false;

    loop {
        let result =
            tokio::time::timeout(DOWNLOAD_TIMEOUT, execute_download(job_id, &mut cmd, &tx)).await;

        match result {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(err)) => {
                // Command failed — try fallback binary once
                if !attempted_fallback {
                    if let Some(path_binary) = &yt_dlp_fallback {
                        if path_binary != &yt_dlp {
                            warn!(job_id, "primary yt-dlp failed, retrying with PATH binary");
                            attempted_fallback = true;
                            cmd = build_download_command(path_binary, request, &output_template);
                            continue;
                        }
                    }
                }
                return Err(err);
            }
            Err(_timeout) => {
                let message = format!(
                    "download timed out after {} minutes",
                    DOWNLOAD_TIMEOUT.as_secs() / 60
                );
                let _ = tx
                    .send(DownloadEvent::Error {
                        message: message.clone(),
                    })
                    .await;
                return Err(anyhow!(message));
            }
        }
    }
}

/// Run a single yt-dlp subprocess, parsing stdout/stderr.
async fn execute_download(
    job_id: &str,
    cmd: &mut Command,
    tx: &mpsc::Sender<DownloadEvent>,
) -> Result<()> {
    let mut child = cmd.spawn().context("failed to spawn yt-dlp")?;
    let stdout = child.stdout.take().context("missing stdout from yt-dlp")?;
    let stderr = child.stderr.take().context("missing stderr from yt-dlp")?;

    let tx_out = tx.clone();
    let job_id_out = job_id.to_string();
    let job_id_err = job_id.to_string();

    let out_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        let mut final_path: Option<String> = None;

        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(caps) = PROGRESS_RE.captures(&line) {
                let percent = caps
                    .get(1)
                    .and_then(|m| m.as_str().split('.').next())
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(0)
                    .min(100);

                let speed = SPEED_RE
                    .captures(&line)
                    .and_then(|m| m.get(1))
                    .map(|m| m.as_str().to_string());
                let eta = ETA_RE
                    .captures(&line)
                    .and_then(|m| m.get(1))
                    .map(|m| m.as_str().to_string());

                let _ = tx_out
                    .send(DownloadEvent::Progress {
                        percent,
                        speed,
                        eta,
                    })
                    .await;
                continue;
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("[download]") && trimmed.ends_with("has already been downloaded")
            {
                let entry = trimmed
                    .trim_start_matches("[download]")
                    .trim()
                    .trim_end_matches("has already been downloaded")
                    .trim();
                if !entry.is_empty() {
                    final_path = Some(entry.to_string());
                }
                continue;
            }

            if !trimmed.starts_with('[') {
                final_path = Some(trimmed.to_string());
            }
        }

        (job_id_out, final_path)
    });

    let err_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut last_error: Option<String> = None;
        while let Ok(Some(line)) = lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with("ERROR:") {
                last_error = Some(trimmed.to_string());
            }
        }
        (job_id_err, last_error)
    });

    let status = child.wait().await.context("yt-dlp process failed")?;

    let (_, final_path) = out_task.await.context("stdout task join error")?;
    let (_, last_error) = err_task.await.context("stderr task join error")?;

    if !status.success() {
        let message = last_error.unwrap_or_else(|| format!("download failed for job {job_id}"));
        let _ = tx.send(DownloadEvent::Error { message }).await;
        return Err(anyhow!("yt-dlp exited with non-zero status"));
    }

    let _ = tx
        .send(DownloadEvent::Progress {
            percent: 100,
            speed: None,
            eta: None,
        })
        .await;

    let done_path =
        final_path.unwrap_or_else(|| crate::config::default_download_dir().display().to_string());
    let _ = tx.send(DownloadEvent::Done { path: done_path }).await;
    Ok(())
}

/// Build the yt-dlp command with all necessary flags.
fn build_download_command(
    binary: &Path,
    request: &DownloadRequest,
    output_template: &str,
) -> Command {
    let mut cmd = Command::new(binary);
    cmd.arg("--newline")
        .arg("--no-warnings")
        .arg("--no-overwrites")
        .arg("--no-post-overwrites")
        .arg("--force-ipv4")
        .arg("--retries")
        .arg("10")
        .arg("--fragment-retries")
        .arg("10")
        .arg("--extractor-retries")
        .arg("3")
        .arg("--file-access-retries")
        .arg("3")
        .arg("--extractor-args")
        .arg(EXTRACTOR_ARGS)
        .arg("-o")
        .arg(output_template)
        .arg("--print")
        .arg("after_move:filepath");

    apply_format_flags(request, &mut cmd);

    if let Some(clip) = &request.clip {
        cmd.arg("--download-sections")
            .arg(format!("*{}-{}", clip.start, clip.end))
            .arg("--force-keyframes-at-cuts");
    }

    cmd.arg(&request.url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

/// Apply format selection flags based on the download request.
fn apply_format_flags(request: &DownloadRequest, cmd: &mut Command) {
    if let Some(format_id) = &request.format_id {
        if !format_id.trim().is_empty() {
            cmd.arg("-f").arg(format_id);
            if request.format.eq_ignore_ascii_case("mp3")
                || request.quality.eq_ignore_ascii_case("audio")
            {
                cmd.arg("--extract-audio").arg("--audio-format").arg("mp3");
            } else {
                cmd.arg("--merge-output-format").arg("mp4");
            }
            return;
        }
    }

    if request.format.eq_ignore_ascii_case("mp3") || request.quality.eq_ignore_ascii_case("audio") {
        cmd.arg("-f")
            .arg("bestaudio/best")
            .arg("--extract-audio")
            .arg("--audio-format")
            .arg("mp3");
        return;
    }

    let height = request
        .quality
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();

    if height.is_empty() {
        cmd.arg("-f")
            .arg("bestvideo*+bestaudio/best")
            .arg("--merge-output-format")
            .arg("mp4");
        return;
    }

    cmd.arg("-f")
        .arg(format!(
            "bestvideo*[height={height}]+bestaudio/best[height={height}]/bestvideo*[height<={height}]+bestaudio/best[height<={height}]/best"
        ))
        .arg("--merge-output-format")
        .arg("mp4");
}

/// Resolve a binary by name, checking sidecar locations first, then PATH.
pub fn resolve_binary(name: &str) -> Result<PathBuf> {
    let candidates = sidecar_candidates(name);
    for candidate in candidates {
        if candidate.exists() && !is_placeholder_sidecar(&candidate) {
            info!(binary = %candidate.display(), "resolved sidecar binary");
            return Ok(candidate);
        }
    }

    which::which(name).with_context(|| format!("{name} not found in sidecar resources or PATH"))
}

/// Check if a sidecar file is just a placeholder.
fn is_placeholder_sidecar(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    content
        .trim()
        .eq_ignore_ascii_case("placeholder: replace with real binary")
}

/// Generate candidate paths for a platform-specific sidecar binary.
fn sidecar_candidates(name: &str) -> Vec<PathBuf> {
    let mut names = Vec::new();
    if cfg!(target_os = "macos") {
        names.push(format!("{name}-mac"));
    } else if cfg!(target_os = "windows") {
        names.push(format!("{name}-win.exe"));
    } else {
        names.push(format!("{name}-linux"));
    }

    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let resource_dir = parent.join("resources");
            for n in &names {
                candidates.push(resource_dir.join(n));
            }
        }
    }

    for n in &names {
        candidates.push(Path::new("resources").join(n));
        candidates.push(Path::new("app/src-tauri/resources").join(n));
    }

    candidates
}

/// Build the output file path template for yt-dlp.
async fn build_output_template(request: &DownloadRequest, config: &AppConfig) -> String {
    let dir = config.effective_download_dir().await;
    let safe_title =
        sanitize_title_for_filename(request.title.as_deref().unwrap_or("ClipDrop Video"));
    let quality = sanitize_name_fragment(&request.quality);
    let format_tag = request
        .format_id
        .as_deref()
        .map(sanitize_name_fragment)
        .filter(|value| !value.is_empty());

    let variant = if request.format.eq_ignore_ascii_case("mp3")
        || request.quality.eq_ignore_ascii_case("audio")
    {
        "audio-mp3".to_string()
    } else if let Some(format_id) = format_tag {
        format!("{quality}-{format_id}")
    } else {
        quality
    };

    let ext = if request.format.eq_ignore_ascii_case("mp3")
        || request.quality.eq_ignore_ascii_case("audio")
    {
        "mp3"
    } else {
        "mp4"
    };
    let base = format!("{safe_title} [{variant}]");
    next_available_path(&dir, &base, ext).display().to_string()
}

/// Sanitize a string for safe use in filenames — keep alphanumerics, spaces,
/// hyphens and underscores.
fn sanitize_title_for_filename(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push(' ');
        }
    }
    let compact = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        "ClipDrop Video".to_string()
    } else if compact.len() > 140 {
        compact[..140].trim().to_string()
    } else {
        compact
    }
}

/// Sanitize a fragment for use in filename metadata brackets.
fn sanitize_name_fragment(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '+' {
            out.push(ch);
        } else {
            out.push('-');
        }
    }

    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "unknown".to_string()
    } else if trimmed.len() > 64 {
        trimmed[..64].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Find the next non-colliding file path with `(2)`, `(3)`, … suffixes.
fn next_available_path(dir: &Path, base: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{base}.{ext}"));
    if !first.exists() {
        return first;
    }
    for n in 2..10_000 {
        let candidate = dir.join(format!("{base} ({n}).{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{base} (overflow).{ext}"))
}
