# Island & Ferry Architecture (v1)

## Components

- `extension/` (Ferry) injects UI into YouTube pages and submits download requests.
- `app/src-tauri/` (Island) runs a local Axum API inside a Tauri tray app.
- `yt-dlp` performs format discovery and media downloads.

## Data Flow

```
┌────────────┐    HTTP     ┌───────────────┐    spawn    ┌─────────┐
│  Extension │ ──────────> │  Axum Server  │ ──────────> │  yt-dlp │
│ content.js │ <────────── │  server.rs    │ <────────── │ process │
└────────────┘  WebSocket  └───────────────┘   stdout    └─────────┘
                               │      ▲
                               │      │
                               ▼      │
                          ┌───────────────┐
                          │ Download Queue │
                          │   queue.rs    │
                          └───────────────┘
```

## Error Handling Strategy

The backend uses a centralized `AppError` enum (`error.rs`) that implements
`IntoResponse` for Axum. All API handlers return `Result<T, AppError>` for
consistent JSON error responses. Variants map to HTTP status codes:

| Variant | HTTP Status | When Used |
|---|---|---|
| `BadRequest` | 400 | Invalid/missing input |
| `NotFound` | 404 | Job or path not found |
| `BadGateway` | 502 | yt-dlp or reveal command failure |
| `ServiceUnavailable` | 503 | Queue full/unavailable |
| `Internal` | 500 | Unexpected errors |

## Configuration Management

Runtime configuration uses a thread-safe `AppConfig` (`config.rs`) with
`Arc<RwLock>`. This replaces the deprecated `std::env::set_var` approach.
Settings are persisted to disk as JSON in the Tauri config directory and
loaded into `AppConfig` at startup.

## Logging

Structured logging via `tracing` + `tracing-subscriber`. Log level is
controlled by the `RUST_LOG` environment variable (default: `info`).
Key events logged:

- Server startup with port
- Format probe results
- Download job queueing and completion
- Errors and worker panics
- Stale job eviction

## Job Lifecycle

```
Queued → InProgress → Done
                    → Error
```

Status tracked via `JobStatus` enum in `models.rs`. Jobs are stored in
memory with a cap of 1000 records; completed jobs older than 1 hour are
evicted when the cap is reached.

## Extension Architecture

- **content.js** — Primary UX. Injects button + floating panel on YouTube.
  Cannot use ES modules (MV3 content script limitation).
- **background.js** — Service worker (ES module). Manages WebSocket to
  desktop API, creates notifications, tracks activity feed.
- **popup.js** — Fallback UX via toolbar icon (ES module). Imports shared
  modules (`api.js`, `runtime.js`, `constants.js`).
- **content.css** — Styles for injected elements, loaded via manifest.

## Key Design Decisions

1. **Local-only**: No remote backend, no telemetry, no accounts.
2. **Sequential queue**: Downloads processed one at a time to avoid
   bandwidth contention and yt-dlp locking issues.
3. **Bundled sidecars only**: Island resolves `yt-dlp`, `ffmpeg`, and
   `ffprobe` from bundled resources only, with placeholder detection.
4. **Inline panel UX**: Primary interaction is the injected panel, not
   the toolbar popup, so users never leave the YouTube page.
