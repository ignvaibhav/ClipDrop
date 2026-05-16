# Skills Implementation Map

This file maps the competency areas in `.agents/skills/skills.md` to implemented modules in this MVP.

## 1) Systems architecture & desktop integration

- Tauri app bootstrap, tray menu, and autostart wiring:
  - `app/src-tauri/src/main.rs`
- Tokio async runtime used for queue worker + Axum server tasks:
  - `app/src-tauri/src/main.rs`

## 2) Local networking & IPC

- Axum HTTP and WebSocket local API on `127.0.0.1:49152`:
  - `app/src-tauri/src/server.rs`
- Endpoints implemented: `/health`, `/formats`, `/download`, `/status/:job_id`, `/ws`.

## 3) Browser extension engineering (MV3)

- Manifest V3 + service worker + content script + popup:
  - `extension/manifest.json`
  - `extension/background.js`
  - `extension/content.js`
  - `extension/popup.html`
  - `extension/popup.js`

## 4) Video processing & format management

- `yt-dlp` format probing and popup presets:
  - `app/src-tauri/src/formats.rs`
- Download flag construction for mp4/mp3 and optional clip range:
  - `app/src-tauri/src/downloader.rs`

## 5) Stream parsing and progress events

- Regex-based progress extraction from `yt-dlp` output:
  - `app/src-tauri/src/downloader.rs`
- WebSocket broadcast schema for `progress`, `done`, `error`:
  - `app/src-tauri/src/models.rs`
  - `app/src-tauri/src/server.rs`

## 6) Job queue & state management

- Sequential queue with job status lifecycle:
  - `app/src-tauri/src/queue.rs`
- Status model exposed through `/status/:job_id`.

## 7) Error handling and recovery

- API validation and structured error responses:
  - `app/src-tauri/src/server.rs`
- Downloader process error propagation:
  - `app/src-tauri/src/downloader.rs`
- Extension fallback messaging when desktop app is offline:
  - `extension/popup.js`

## 8) Security & privacy posture

- Local-only architecture (localhost IPC).
- No remote telemetry, analytics, or account code paths.
- Explicit host permissions in extension manifest for local API and YouTube only.
