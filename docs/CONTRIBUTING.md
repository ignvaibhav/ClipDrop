# Contributing

## Development setup

- Install Rust (stable) and Tauri prerequisites for your OS.
- Load the extension folder in a Chromium browser.
- Start desktop app from `app/src-tauri` with `cargo run`.

## Code principles

- Keep all data flow local-only; avoid remote services.
- Preserve API contracts in `server.rs`.
- Prefer explicit error messages for extension UX.
- Keep queue semantics sequential for MVP.

## Pull requests

- Keep changes focused.
- Include manual verification notes for:
  - `/health`, `/formats`, `/download`
  - WebSocket progress event shape
  - Extension popup behavior on YouTube pages
