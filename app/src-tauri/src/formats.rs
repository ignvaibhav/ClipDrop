//! Format discovery via yt-dlp.
//!
//! Runs `yt-dlp --dump-json` against a video URL and parses the returned
//! format metadata into a ranked list of [`FormatOption`] entries that the
//! extension can display in its quality selector.

use std::time::Duration;

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::BTreeMap;
use tokio::process::Command;
use tracing::info;

use crate::downloader::resolve_binary;
use crate::models::FormatOption;

/// Maximum time allowed for the format probe subprocess.
const FORMAT_PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// YouTube extractor arguments — shared with downloader.
const EXTRACTOR_ARGS: &str = "youtube:player_client=android_vr,web_safari,android,web";

/// Internal representation of a video stream candidate.
#[derive(Debug, Clone)]
struct VideoCandidate {
    format_id: String,
    height: u16,
    fps: u16,
    tbr: f64,
    ext: String,
    has_audio: bool,
}

/// Internal representation of an audio stream candidate.
#[derive(Debug, Clone)]
struct AudioCandidate {
    format_id: String,
    abr: f64,
    ext: String,
}

/// Fetch available download formats for a video URL.
///
/// Returns a list of [`FormatOption`] entries sorted with the best quality
/// first, followed by per-resolution options and an audio-only option.
pub async fn fetch_formats(url: &str) -> Result<Vec<FormatOption>> {
    let yt_dlp = resolve_binary("yt-dlp")?;

    let output = tokio::time::timeout(
        FORMAT_PROBE_TIMEOUT,
        Command::new(yt_dlp)
            .arg("--dump-json")
            .arg("--no-warnings")
            .arg("--extractor-args")
            .arg(EXTRACTOR_ARGS)
            .arg(url)
            .output(),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "format probe timed out after {}s",
            FORMAT_PROBE_TIMEOUT.as_secs()
        )
    })?
    .context("failed to run yt-dlp --dump-json")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(anyhow::anyhow!("yt-dlp format probe failed: {err}"));
    }

    let data: Value =
        serde_json::from_slice(&output.stdout).context("invalid yt-dlp json output")?;
    let (videos_by_height, audio_candidates) = parse_format_data(&data);

    let result = build_format_options(videos_by_height, &audio_candidates);
    info!(url, count = result.len(), "format options built");
    Ok(result)
}

/// Parse raw yt-dlp JSON into video and audio candidate lists.
fn parse_format_data(data: &Value) -> (BTreeMap<u16, Vec<VideoCandidate>>, Vec<AudioCandidate>) {
    let mut videos_by_height: BTreeMap<u16, Vec<VideoCandidate>> = BTreeMap::new();
    let mut audio_candidates: Vec<AudioCandidate> = Vec::new();

    let Some(formats) = data.get("formats").and_then(Value::as_array) else {
        return (videos_by_height, audio_candidates);
    };

    for item in formats {
        let Some(format_id) = item
            .get("format_id")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };

        let vcodec = item.get("vcodec").and_then(Value::as_str).unwrap_or("");
        let acodec = item.get("acodec").and_then(Value::as_str).unwrap_or("");
        let has_url = item.get("url").and_then(Value::as_str).is_some();
        let ext = item.get("ext").and_then(Value::as_str).unwrap_or("");
        let tbr = item.get("tbr").and_then(Value::as_f64).unwrap_or(0.0);
        let abr = item
            .get("abr")
            .and_then(Value::as_f64)
            .or_else(|| item.get("tbr").and_then(Value::as_f64))
            .unwrap_or(0.0);
        let fps = item.get("fps").and_then(Value::as_f64).unwrap_or(30.0) as u16;

        if !has_url {
            continue;
        }

        // Video stream
        if vcodec != "none" {
            if let Some(height) = item.get("height").and_then(Value::as_i64) {
                if height > 0 {
                    videos_by_height
                        .entry(height as u16)
                        .or_default()
                        .push(VideoCandidate {
                            format_id: format_id.clone(),
                            height: height as u16,
                            fps,
                            tbr,
                            ext: ext.to_string(),
                            has_audio: acodec != "none",
                        });
                }
            }
        }

        // Audio-only stream
        if vcodec == "none" && acodec != "none" {
            audio_candidates.push(AudioCandidate {
                format_id: format_id.clone(),
                abr,
                ext: ext.to_string(),
            });
        }
    }

    (videos_by_height, audio_candidates)
}

/// Build the final list of user-facing format options.
fn build_format_options(
    videos_by_height: BTreeMap<u16, Vec<VideoCandidate>>,
    audio_candidates: &[AudioCandidate],
) -> Vec<FormatOption> {
    let best_audio_m4a = audio_candidates
        .iter()
        .filter(|c| c.ext == "m4a")
        .max_by(|a, b| {
            a.abr
                .partial_cmp(&b.abr)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    let best_audio_any = audio_candidates.iter().max_by(|a, b| {
        a.abr
            .partial_cmp(&b.abr)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let best_audio = best_audio_m4a.or(best_audio_any);

    let mut video_options: Vec<FormatOption> = Vec::new();
    for (_height_key, candidates) in videos_by_height.iter().rev() {
        let Some(best_video) = candidates
            .iter()
            .max_by(|l, r| rank_video_candidate(l).cmp(&rank_video_candidate(r)))
        else {
            continue;
        };

        let selector = if best_video.has_audio {
            best_video.format_id.clone()
        } else if let Some(audio) = best_audio {
            format!(
                "{}+{}/{}",
                best_video.format_id, audio.format_id, best_video.format_id
            )
        } else {
            format!("{}/best", best_video.format_id)
        };

        let fps_label = if best_video.fps >= 50 {
            format!("{}p{} ", best_video.height, best_video.fps)
        } else {
            format!("{}p ", best_video.height)
        };
        let merge_note = if best_video.has_audio {
            "progressive stream"
        } else {
            "video+audio merge"
        };

        video_options.push(FormatOption {
            label: format!("MP4 • {}", fps_label.trim()),
            format: "mp4".to_string(),
            quality: format!("{}p", best_video.height),
            format_id: Some(selector),
            note: Some(format!(
                "{} • source {} • {:.0} kbps",
                merge_note, best_video.ext, best_video.tbr
            )),
        });
    }

    let mut result = Vec::new();

    // "Best available" option derived from highest resolution
    if let Some(best) = video_options.first().cloned() {
        result.push(FormatOption {
            label: format!("Best available • {}", best.quality),
            format: best.format.clone(),
            quality: "best".to_string(),
            format_id: best.format_id.clone(),
            note: Some("Highest downloadable quality for this video".to_string()),
        });
    }

    result.extend(video_options);

    // Audio-only option
    result.push(FormatOption {
        label: "MP3 • Audio only".to_string(),
        format: "mp3".to_string(),
        quality: "audio".to_string(),
        format_id: best_audio.map(|c| c.format_id.clone()),
        note: Some("Extract audio track".to_string()),
    });

    // Ultimate fallback if nothing was discovered
    if result.len() <= 1 {
        result.insert(
            0,
            FormatOption {
                label: "Best available".to_string(),
                format: "mp4".to_string(),
                quality: "best".to_string(),
                format_id: None,
                note: Some("Fallback selection".to_string()),
            },
        );
    }

    result
}

/// Ranking key for selecting the best video candidate at a given resolution.
/// Higher tuple = better candidate.
fn rank_video_candidate(candidate: &VideoCandidate) -> (u8, u8, u16, u64) {
    let progressive_score = u8::from(candidate.has_audio);
    let mp4_score = u8::from(candidate.ext == "mp4");
    let fps_score = candidate.fps;
    let tbr_score = candidate.tbr as u64;
    (progressive_score, mp4_score, fps_score, tbr_score)
}
