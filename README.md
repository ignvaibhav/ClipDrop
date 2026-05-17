# Ferry + Island

<p align="center">
  <img src="./ferry.png" alt="Ferry and Island" width="180" />
</p>

<p align="center">
  <strong>Ferry</strong> is the browser-side YouTube download experience.<br />
  <strong>Island</strong> is the local desktop engine that does the actual download work.
</p>

<p align="center">
  <img alt="Version" src="https://img.shields.io/badge/version-0.1.0-black" />
  <img alt="Status" src="https://img.shields.io/badge/status-alpha-111111" />
  <img alt="macOS" src="https://img.shields.io/badge/macOS-supported-111111?logo=apple&logoColor=white" />
  <img alt="Local First" src="https://img.shields.io/badge/local--first-no%20cloud-111111" />
  <img alt="Chromium Extension" src="https://img.shields.io/badge/extension-Chromium%20MV3-111111?logo=googlechrome&logoColor=white" />
  <img alt="Tauri" src="https://img.shields.io/badge/desktop-Tauri%202-111111?logo=tauri&logoColor=white" />
  <img alt="Rust" src="https://img.shields.io/badge/backend-Rust-111111?logo=rust&logoColor=white" />
  <img alt="License" src="https://img.shields.io/badge/license-MIT-111111" />
</p>

Ferry + Island is a local-first download workflow for YouTube:

- **Ferry** injects a download UI into YouTube pages and gives you a compact extension popup and settings experience.
- **Island** runs locally on your Mac, exposes a localhost API, and uses bundled media tools to download, merge, remux, and track progress.
- **No cloud backend, no account system, no telemetry.**

> Windows and Linux support is planned for a future release.

## What You Get

- Download button injected into the YouTube action area
- Inline panel for **video**, **audio**, and **thumbnail** downloads
- Clip-range selection for video and audio
- Local queue-based downloader powered by `yt-dlp` and `ffmpeg`
- Live progress, completion, and error feedback via WebSocket
- Activity history in the popup
- Dedicated extension settings page with theme controls
- Tray-based desktop companion with local-only API

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                        BROWSER                              │
│                                                             │
│   YouTube Watch Page                                        │
│   ┌─────────────────────────────────────────────────────┐  │
│   │  [Like] [Share] [Ferry]  ← injected UI             │  │
│   └─────────────────────────────────────────────────────┘  │
│                          │                                  │
│                Ferry inline panel / popup                   │
│                          │                                  │
│           localhost HTTP + WebSocket messages               │
└──────────────────────────┼──────────────────────────────────┘
                           │
                    127.0.0.1:49152
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                    ISLAND DESKTOP APP                        │
│                                                             │
│   Axum API  →  Queue  →  yt-dlp / ffmpeg / ffprobe         │
│                                                             │
│                 Downloaded file saved locally               │
└─────────────────────────────────────────────────────────────┘
```

## Repository Layout

```text
.
├── extension/                    # Ferry browser extension
│   ├── manifest.json
│   ├── icons/                    # extension icons
│   ├── _shared/                  # shared ES modules
│   │   ├── api.js                # fetch wrappers for Island API
│   │   ├── constants.js          # API_BASE, WS_URL, storage keys
│   │   └── runtime.js            # runtime availability guards
│   ├── background/
│   │   └── background.js         # service worker: WS, activity, notifications
│   ├── content/
│   │   ├── content.js            # injected YouTube UI
│   │   └── content.css
│   ├── popup/
│   │   ├── popup.html            # toolbar popup
│   │   ├── popup.js
│   │   └── popup.css
│   ├── settings/
│   │   ├── settings.html         # extension settings page
│   │   ├── settings.js
│   │   └── settings.css
│   └── assets/                   # design reference files
├── app/
│   ├── src/                      # Tauri webview pages
│   └── src-tauri/
│       ├── resources/            # sidecar binaries (yt-dlp, ffmpeg, ffprobe)
│       ├── src/
│       │   ├── main.rs           # Tauri entry, tray, settings
│       │   ├── server.rs         # Axum HTTP + WebSocket server
│       │   ├── queue.rs          # sequential download queue
│       │   ├── downloader.rs     # yt-dlp execution + progress parsing
│       │   ├── formats.rs        # format discovery + ranking
│       │   ├── models.rs         # shared types
│       │   ├── config.rs         # thread-safe config
│       │   └── error.rs          # AppError → HTTP responses
│       └── tauri.conf.json
├── docs/
├── scripts/
└── README.md
```

## Download Flow

1. Open a YouTube watch page.
2. Ferry injects its inline UI and starts preparing format data.
3. You choose a mode — **video**, **audio**, or **thumbnail**.
4. Ferry sends the download request to Island over `http://127.0.0.1:49152`.
5. Island queues the job and runs the bundled downloader sidecars.
6. Progress is streamed back to Ferry over WebSocket.
7. The file is saved locally and can be revealed from the popup.

## Setup

### Requirements

- **macOS** (Intel or Apple Silicon)
- A Chromium-based browser (Chrome, Edge, Brave, Arc, etc.)

### Quick start

Run the bootstrap script — it installs Homebrew (if needed), Rust, Node.js, ffmpeg, yt-dlp, Tauri CLI, and verifies the backend build:

```bash
./scripts/bootstrap-macos.sh
```

### Start Island

```bash
cd app/src-tauri
cargo run
```

Expected startup log:

```text
INFO island_desktop::server: Island API listening on http://127.0.0.1:49152
```

### Load Ferry

1. Open `chrome://extensions`
2. Enable **Developer mode**
3. Click **Load unpacked**
4. Select the [`extension/`](./extension) folder
5. Open a YouTube watch page — the Ferry button appears in the action bar

## Using Ferry

### Inline panel

The primary experience is the injected Ferry panel on YouTube watch pages. You can:

- Choose video quality (360p through 4K, depending on the video)
- Choose MP3 bitrate
- Choose thumbnail size
- Set a clip range for video or audio
- Trigger a download without leaving the page

### Popup

The toolbar popup is a companion surface for:

- Recent transfers and their status
- Job progress (live)
- Reveal / cancel actions
- Opening the extension settings page

### Settings page

The Ferry settings page includes:

- Theme mode controls (system / dark / light)
- Island connection status
- Quick actions (open downloads, open Island settings, clear activity)

## Format Surface

Ferry surfaces a curated quality list for each media type:

| Type | Options shown |
|---|---|
| Video | One "Best" + each available height (360p, 720p, 1080p, 1440p, 2160p) |
| Audio | Top 3 highest quality streams |
| Thumbnail | Top 2 largest sizes |

Only formats actually available for a given video are returned.

## Local API

| Endpoint | Method | Purpose |
|---|---|---|
| `/health` | GET | Check whether Island is reachable |
| `/formats` | POST | Fetch available formats for a YouTube URL |
| `/download` | POST | Queue a new download |
| `/status/{job_id}` | GET | Poll job status |
| `/reveal` | POST | Reveal a saved file in Finder |
| `/jobs/{job_id}/cancel` | POST | Cancel a queued or active job |
| `/jobs/{job_id}/skip` | POST | Skip a queued job |
| `/action/open-settings` | POST | Open Island settings window |
| `/action/open-downloads` | POST | Open the downloads folder |
| `/ws` | GET | WebSocket progress stream |

## Development

### Validation

```bash
./scripts/dev-check.sh
```

Covers: `cargo check`, `cargo clippy`, `cargo fmt --check`, and `node --check` on all extension scripts.

### API smoke test

With Island running:

```bash
node scripts/smoke-api.mjs
```

Queue a test download:

```bash
node scripts/smoke-api.mjs --queue
```

### Full build

```bash
./scripts/build-all.sh
```

## Troubleshooting

| Problem | What to check |
|---|---|
| Ferry button missing on YouTube | Reload the extension in `chrome://extensions`, then refresh the YouTube tab |
| Popup says Island is offline | Make sure `cargo run` is active in `app/src-tauri` |
| Downloads do not start | Check Island logs in the terminal |
| Extension changed but YouTube shows old UI | Reload the extension and refresh all YouTube tabs |
| Port `49152` already in use | Stop the existing process on that port |
| Fewer qualities than expected | Ferry surfaces a curated list — only available heights are shown |

## License

MIT — see [LICENSE](./LICENSE)
