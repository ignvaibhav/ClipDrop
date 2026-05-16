# Island & Ferry

Island is a local-first YouTube downloader desktop companion, and Ferry is its browser extension.

- `extension/`: Ferry browser extension (Chrome MV3) that injects a download button on YouTube watch pages.
- `app/src-tauri/`: Island desktop companion that runs a local API (`127.0.0.1:49152`) and executes `yt-dlp`.

No cloud backend, no account system, no telemetry.

## Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                        BROWSER                              │
│   YouTube Page                                              │
│   ┌──────────────────────────────────────────────────────┐  │
│   │  [Like] [Dislike] [Share] [Ferry ↓]  ← injected      │  │
│   └──────────────────────────────────────────────────────┘  │
│                          │                                  │
│              Inline panel (format/quality/clip)              │
│                          │                                  │
│              HTTP POST /download                            │
│                          │                                  │
└──────────────────────────┼──────────────────────────────────┘
                           │
                    localhost:49152
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                  ISLAND DESKTOP APP (Tray)                   │
│   Axum Server  →  Download Queue  →  yt-dlp subprocess      │
│                        WebSocket progress stream             │
└─────────────────────────────────────────────────────────────┘
```

## Features

- Injected YouTube action button and in-page download panel
- Dynamic quality options from live `yt-dlp` format probe
- Optional clip-range download
- Queue-based local downloads with progress updates
- Desktop notifications on completion/failure
- Tray app with Settings window for custom download directory

## Repository Structure

```text
.
├── app/
│   ├── src/                  # Tauri frontend pages (tray/settings webview)
│   └── src-tauri/
│       └── src/
│           ├── main.rs       # App entry, tray menu, autostart, settings
│           ├── server.rs     # Axum HTTP + WebSocket server
│           ├── downloader.rs # yt-dlp subprocess manager
│           ├── formats.rs    # Format fetching + parsing
│           ├── queue.rs      # Download job queue
│           ├── models.rs     # Shared data types and enums
│           ├── config.rs     # Thread-safe runtime configuration
│           └── error.rs      # Centralized error types
├── extension/
│   ├── manifest.json         # MV3 manifest
│   ├── content.js            # Injected YouTube UI (primary UX)
│   ├── content.css           # Styles for injected UI
│   ├── background.js         # Service worker — WebSocket + notifications
│   ├── popup.html/js/css     # Toolbar popup (fallback UX)
│   ├── constants.js          # Shared constants
│   ├── api.js                # Shared API client
│   └── runtime.js            # Shared Chrome runtime helpers
├── scripts/                  # Bootstrap, checks, build helpers
├── docs/                     # Architecture, contribution docs
├── .github/workflows/        # CI pipeline
├── README.md
└── LICENSE
```

## Quick Start

### 1) Bootstrap environment (one command)

- Linux:
```bash
./scripts/bootstrap-linux.sh
```

- macOS:
```bash
./scripts/bootstrap-macos.sh
```

- Windows (PowerShell):
```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\bootstrap-windows.ps1
```

### 2) Start desktop app

```bash
cd app/src-tauri
cargo run
```

### 3) Load extension

1. Open `chrome://extensions`
2. Enable Developer Mode
3. Click **Load unpacked**
4. Select `extension/`
5. Open a YouTube watch page and click the injected `Ferry` button

## Development

### Validation / Dev Commands

- Full lint + syntax checks:
```bash
./scripts/dev-check.sh
```

- Local API smoke check (requires running backend):
```bash
node scripts/smoke-api.mjs
```

- Full build (desktop bundle + extension zip):
```bash
./scripts/build-all.sh
```

### Code Quality

- **Rust**: `cargo clippy` for linting, `cargo fmt` for formatting
- **Extension**: `node --check` for syntax validation
- **CI**: Automated checks on every push/PR via GitHub Actions

## Local API

| Endpoint | Method | Description |
|---|---|---|
| `/health` | GET | Health check |
| `/formats` | POST | Fetch available formats for a URL |
| `/download` | POST | Queue a download job |
| `/status/{job_id}` | GET | Poll job status |
| `/reveal` | POST | Open folder in file explorer |
| `/ws` | GET/ANY | WebSocket progress stream |

## Troubleshooting

| Problem | Check |
|---|---|
| Button not appearing | Reload extension at `chrome://extensions`, then refresh YouTube tab |
| "Desktop app not reachable" | Ensure `cargo run` is running in `app/src-tauri/` |
| Download stuck at 0% | Check `yt-dlp` and `ffmpeg` are available: `which yt-dlp ffmpeg` |
| Low quality downloads | Update yt-dlp: download latest from [github.com/yt-dlp/yt-dlp](https://github.com/yt-dlp/yt-dlp) |
| Extension errors after update | Reload extension AND refresh all open YouTube tabs |
| Port 49152 in use | Kill existing process: `lsof -ti:49152 \| xargs kill` |

## Notes

- Firefox support is not finalized in this repo state.
- For production release workflows, keep platform sidecar binaries out of git and package per release artifact.

## License

MIT — see [LICENSE](./LICENSE).
