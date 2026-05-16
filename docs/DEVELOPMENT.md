# Development Guide

## Prerequisites

- **Rust** (stable, via [rustup](https://rustup.rs/))
- **Node.js** (v18+, for extension syntax validation)
- **yt-dlp** — latest release binary
- **ffmpeg** — for clip-range downloads and audio extraction
- **Linux**: libwebkit2gtk-4.1-dev, libappindicator3-dev, libgtk-3-dev, libsoup-3.0-dev

Use the bootstrap script for your platform:
```bash
./scripts/bootstrap-linux.sh   # Linux
./scripts/bootstrap-macos.sh   # macOS
```

## Running the Backend

```bash
cd app/src-tauri
cargo run
```

The API server starts on `http://127.0.0.1:49152`. You should see structured
log output:

```
INFO island_desktop::server: Island API listening on http://127.0.0.1:49152
```

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `RUST_LOG` | `info` | Log level filter (e.g. `debug`, `trace`) |

## Loading the Extension

1. Open `chrome://extensions`
2. Enable **Developer Mode**
3. Click **Load unpacked** → select the `extension/` directory
4. Navigate to any YouTube watch page
5. The **Ferry** button should appear in the action bar

### After Code Changes

- **Backend**: Stop `cargo run` and restart.
- **Extension**: Click the reload icon on `chrome://extensions`, then refresh YouTube tabs.

## Code Quality Checks

Run the full validation suite:

```bash
./scripts/dev-check.sh
```

This runs:
1. `cargo check` — compilation
2. `cargo clippy -- -D warnings` — linting
3. `cargo fmt --check` — formatting
4. `node --check` — extension JS syntax
5. Manifest JSON validation

## API Smoke Test

With the backend running:

```bash
# Health + format probe only
node scripts/smoke-api.mjs

# Full test including download
node scripts/smoke-api.mjs --queue
```

## Project Conventions

### Rust

- All public functions and types have doc comments (`///`).
- Errors use the `AppError` enum, not ad-hoc tuples.
- Status values use `JobStatus` / `WsEventType` enums, never raw strings.
- Regex patterns are pre-compiled as `LazyLock` statics.
- Runtime config via `AppConfig` (thread-safe), not environment variables.
- Structured logging via `tracing` macros (`info!`, `warn!`, `error!`).
- Bundled sidecars are required at runtime; no PATH fallback is part of the product flow.

### Extension

- Content script (`content.js`) cannot use ES modules — helpers are inlined.
- Background and popup scripts use ES module imports.
- Shared code lives in `constants.js`, `api.js`, `runtime.js`.
- All injected DOM elements use `ferry-` class prefix.
- CSS for injected elements lives in `content.css`, not inline styles.

### General

- No `console.log` in production code (use `console.warn` for diagnostics).
- All async operations wrapped in try/catch.
- MutationObserver callbacks are debounced.

## Debugging Tips

### Backend not starting
```bash
# Check if port is in use
lsof -ti:49152

# Run with debug logging
RUST_LOG=debug cargo run
```

### Extension not injecting
- Check for errors at `chrome://extensions`
- Open DevTools on the YouTube page → Console tab
- Look for `chrome.runtime` errors

### yt-dlp issues
```bash
# Check version
yt-dlp --version

# Test format probe manually
yt-dlp --dump-json "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
```

### WebSocket not connecting
- Ensure backend is running: `curl http://127.0.0.1:49152/health`
- Check background service worker logs via `chrome://extensions` → Inspect views
