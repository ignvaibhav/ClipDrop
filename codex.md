# Codex Project Context

## 1. Repository Identity

- Project name: `ClipDrop`
- Working directory: `/media/ignvaibhav/Shared Volume/CODING/Dummy_projects/clipdrop (codex)`
- Current shape: monorepo with:
  - `app/src-tauri/` for desktop companion
  - `extension/` for browser extension
  - `docs/` for supporting docs
  - `scripts/` for setup/build/smoke utilities

## 2. Product Intent

This repo is implementing the MVP described in `prd.md`.

Core idea:

- A browser extension injects a `ClipDrop` button on YouTube watch pages.
- A local Tauri desktop app exposes an HTTP/WebSocket API on `127.0.0.1:49152`.
- The extension asks the local app for available formats and submits download requests.
- The desktop app runs `yt-dlp` and optionally `ffmpeg` locally.
- All processing is local-only. No accounts, telemetry, or remote backend.

User specifically asked to build the product following `prd.md` and to incorporate the engineering competencies outlined in `.agents/skills/skills.md`.

## 3. Scope Implemented So Far

### Desktop app

- Tauri v2 desktop app scaffolded.
- Axum server implemented with:
  - `GET /health`
  - `POST /formats`
  - `POST /download`
  - `GET /status/:job_id`
  - `GET/ANY /ws`
- Download queue implemented as sequential worker.
- yt-dlp subprocess management implemented.
- Progress parsing from yt-dlp stdout implemented.
- Download WebSocket events implemented.

### Extension

- MV3 extension scaffolded.
- YouTube content script injects `ClipDrop` button.
- Initial popup-based UX existed, but then user requested inline UX.
- Current direction: inline panel under the injected button instead of relying on toolbar popup.
- Background service worker handles WebSocket fan-out and notifications.

### Project automation

- `scripts/bootstrap-build.sh` created to install deps/build.
- `scripts/build-all.sh` created for build pipeline.
- `scripts/dev-check.sh` created for validation.
- `scripts/smoke-api.mjs` created for API smoke testing.

## 4. Important Files

### Backend / desktop

- `app/src-tauri/src/main.rs`
- `app/src-tauri/src/server.rs`
- `app/src-tauri/src/queue.rs`
- `app/src-tauri/src/downloader.rs`
- `app/src-tauri/src/formats.rs`
- `app/src-tauri/src/models.rs`
- `app/src-tauri/tauri.conf.json`

### Extension

- `extension/manifest.json`
- `extension/background.js`
- `extension/content.js`
- `extension/popup.html`
- `extension/popup.js`
- `extension/popup.css`

### Build / docs

- `README.md`
- `docs/ARCHITECTURE.md`
- `docs/CONTRIBUTING.md`
- `docs/SKILLS_IMPLEMENTATION.md`
- `scripts/bootstrap-build.sh`
- `scripts/build-all.sh`
- `scripts/dev-check.sh`
- `scripts/smoke-api.mjs`

## 5. Environment and Tooling State

- Rust was not initially installed; `rustup` was installed during this session.
- Cargo and Rust are available now in the environment used during the work.
- Tauri Linux system dependencies were required and build was blocked until Linux GUI/system libs were present.
- `ffmpeg` is available on the machine.
- `yt-dlp` from Debian `apt` was too old and broken for current YouTube extraction.
- Latest official `yt-dlp` Linux binary was downloaded into:
  - `app/src-tauri/resources/yt-dlp-linux`
- Verified version after update:
  - `2026.03.17`

## 6. Important Runtime / Build Constraints Discovered

### Tauri resource behavior

- `tauri.conf.json` bundle resources must physically exist or build fails.
- Placeholder sidecars were created for:
  - `yt-dlp-mac`
  - `yt-dlp-win.exe`
  - `yt-dlp-linux`
  - `ffmpeg-mac`
  - `ffmpeg-win.exe`
  - `ffmpeg-linux`
- Later, backend logic was patched to ignore placeholder files and fall back to PATH binaries if placeholders are detected.

### Tauri icon behavior

- Tauri requires RGBA PNG icons for macro generation.
- Linux AppImage bundling also requires square icons explicitly listed in `bundle.icon`.
- Valid RGBA icons were generated for:
  - `app/src-tauri/icons/icon.png`
  - `app/src-tauri/icons/32x32.png`
  - `app/src-tauri/icons/128x128.png`
  - `app/src-tauri/icons/256x256.png`
  - `app/src-tauri/icons/512x512.png`

### YouTube / yt-dlp behavior

- Old Debian `yt-dlp` produced:
  - signature extraction issues
  - HTTP 400 / precondition failures
  - “Requested format is not available”
  - only image/low-quality behaviors
- New official binary successfully extracted metadata and formats.
- High-quality availability on YouTube is constrained by:
  - client variant
  - PO token restrictions
  - whether progressive streams exist
  - whether DASH separate video+audio can be combined

## 7. Significant Bugs Encountered and Fixes Applied

### 7.1 Health endpoint OK but extension still failed

Observed:

- `/health` returned OK.
- `/formats` failed because yt-dlp execution failed.

Resolution:

- Replaced placeholder/resource handling and later installed latest `yt-dlp` sidecar.

### 7.2 Extension button not opening usable interface

Observed:

- Clicking injected button did not reliably open the extension popup.
- User had to manually click the extension icon.

Resolution:

- UX moved away from toolbar popup dependency.
- Current extension flow uses inline UI under the injected button in `extension/content.js`.

### 7.3 Wrong / low quality selected even when video supports 4K

Observed:

- Only low resolutions were shown.
- Selecting higher quality still downloaded low-quality output.

Root causes identified:

- Previous format list logic used heuristic/hardcoded quality buckets.
- It relied on progressive-only assumptions in some iterations.
- Download command used non-exact selectors and fallback behavior that could silently degrade quality.

Fixes applied:

- Backend `formats.rs` now builds quality options from actual yt-dlp `formats` data.
- Quality options are generated dynamically per video instead of from hardcoded predefined list.
- Returned entries include `format_id` selectors.
- Download requests carry `format_id`.
- Downloader uses exact selector strings when available.
- Format probing and download both use broader client extraction args:
  - `youtube:player_client=android_vr,web_safari,android,web`

### 7.4 Existing file overwrite / duplicate handling

User requested fallbacks if the file already exists.

Fixes applied:

- Added `--no-overwrites`
- Added `--no-post-overwrites`
- Added `--download-archive`
- Output template changed to include video ID:
  - `%(title)s [%(id)s].%(ext)s`
- Backend attempts to parse the already-downloaded line from yt-dlp output.
- Download archive path:
  - `~/Downloads/.clipdrop-download-archive.txt` by default

### 7.5 Extension runtime errors

Observed on Chrome extension page:

- `Cannot read properties of undefined (reading 'local')`
- `Failed writing extension storage Error: Extension context invalidated.`
- `Uncaught (in promise) Error: Extension context invalidated.`
- `WebSocket ... ERR_CONNECTION_REFUSED`

Root causes identified:

- Stale content/popup contexts after extension reload.
- Unsafe direct calls to `chrome.runtime.sendMessage` and `chrome.storage.local`.
- Background WebSocket connecting aggressively even when backend was not needed or not running.

Fixes applied:

- Added runtime guards:
  - `chrome.runtime.id` checks
  - safe message wrappers
  - safe storage access in popup/content
- Background service worker no longer auto-connects on install.
- WebSocket connection is now demand-driven through explicit messages.
- Background reconnection only happens while socket use is intended.
- `extension/content.js`, `extension/background.js`, and `extension/popup.js` were hardened against context invalidation.

## 8. Current Backend Format Strategy

Current implementation in `app/src-tauri/src/formats.rs`:

- Calls `yt-dlp --dump-json`
- Reads `formats` array
- Builds:
  - `VideoCandidate`
  - `AudioCandidate`
- Groups video formats by actual height.
- Ranks candidates by:
  - progressive stream availability
  - mp4 preference
  - fps
  - bitrate
- Constructs options dynamically from actual available formats.
- Adds:
  - “Best available” option using the top discovered selector
  - per-height options
  - MP3 audio-only option from best audio candidate

Important nuance:

- High resolutions on YouTube often require merging separate video+audio streams.
- Current selector creation attempts:
  - direct format if audio already exists
  - otherwise `video+audio/video` fallback style selector

## 9. Current Downloader Strategy

Current implementation in `app/src-tauri/src/downloader.rs`:

- Resolves binary using sidecar-first, then PATH fallback.
- Ignores placeholder sidecar files.
- Uses:
  - `--newline`
  - `--force-ipv4`
  - retry-related flags
  - YouTube extractor args
  - `--download-archive`
  - no-overwrite flags
  - output template with video ID
- Parses progress from stdout regex.
- Tries sidecar `yt-dlp` first.
- If command fails and PATH `yt-dlp` is different, attempts one fallback retry with PATH binary.

## 10. Extension UX Direction

User explicitly requested:

- no top-toolbar manual popup interaction
- interface should appear below the injected button
- option selection should happen there

Current state:

- `extension/content.js` was rewritten to render inline panel under the action bar/button.
- Panel includes:
  - title
  - status
  - quality selector
  - optional clip inputs
  - download button
  - progress area
- It communicates directly with local API and background worker.

Important note:

- The old `popup.js`/`popup.html` still exist and were hardened because they were already part of the extension and could still be opened.
- The intended user path is now the inline content-script panel.

## 11. Build and Validation Status

### Checks that were run successfully during the session

- Multiple `cargo check` runs completed successfully after patches.
- `node --check` completed successfully for:
  - `extension/background.js`
  - `extension/content.js`
  - `extension/popup.js`
  - `scripts/smoke-api.mjs`

### Build blockers encountered historically

- missing Rust toolchain
- missing Linux Tauri dependencies
- invalid/non-RGBA icon
- missing bundle resource paths
- AppImage missing square icon
- old `yt-dlp`
- `Text file busy` when compile/build happened while running app held the sidecar binary open

## 12. Operational Instructions That Were Important

To apply frontend changes reliably:

1. reload extension in `chrome://extensions`
2. refresh any open YouTube tabs

Reason:

- content scripts and popup/background contexts can remain stale after reloads
- old injected content scripts can continue showing old errors until tab refresh

To apply backend changes reliably:

1. stop old `cargo run`
2. restart from `app/src-tauri`

## 13. User Requests That Must Remain True

These are explicit requirements from the user during the session:

- Follow `prd.md` while building the product.
- Keep the skills/competency directions from `.agents/skills/skills.md` in mind throughout the project.
- Fix root causes rather than surface-level behavior when possible.
- Provide various quality options.
- Do not hardcode quality options; only show qualities actually available for the current video.
- If the file already exists, handle that gracefully.
- The extension should not require manual click on the extension icon at the top.
- The selection interface should be under the injected button on the page.
- Preserve as much working context as possible for future models/AI.

## 14. Current Known Risks / Open Questions

### 14.1 High-quality YouTube availability may still depend on PO-token-sensitive clients

Even with latest yt-dlp and broader client selection, some videos and some networks/sessions may still expose only limited direct formats without:

- cookies
- PO token workflow
- additional client/provider integration

This is a platform-level constraint, not just a UI bug.

### 14.2 `extension/content.js` was rewritten multiple times

It was recently replaced with a clean version, but this file is high-churn and should be re-read carefully before further edits.

### 14.3 `popup.js` and inline panel coexist

The primary intended UX is inline panel under button.
The popup path is still present for compatibility/hardening but should not be treated as the main path.

### 14.4 Running backend while building can lock sidecar binary

This caused:

- `Text file busy (os error 26)`

Stop `cargo run` before replacing/rebuilding sidecar resources.

## 15. Recommended Immediate Next Checks for a New Model

If another model picks this up, the first practical checks should be:

1. Read:
   - `app/src-tauri/src/formats.rs`
   - `app/src-tauri/src/downloader.rs`
   - `extension/content.js`
   - `extension/background.js`
   - `extension/popup.js`
2. Confirm whether inline UI works after extension reload and tab refresh.
3. Confirm whether `/formats` returns realistic high-res options for current videos.
4. Confirm whether chosen option maps to actual output resolution.
5. If only 360p still appears universally, investigate YouTube client/PO-token/cookie strategy rather than tweaking labels only.

## 16. Summary of Current State

The codebase is no longer at the initial scaffold stage. It now contains:

- a functioning local desktop API
- a real yt-dlp integration
- updated yt-dlp sidecar
- sequential queue and progress streaming
- inline extension UI under the injected button
- dynamic format generation
- download archive and no-overwrite behavior
- hardening for extension reload/context invalidation

The main unresolved functional uncertainty is not general scaffolding anymore; it is the reliability of truly high-resolution YouTube stream discovery/download across videos under current extractor/client constraints.
