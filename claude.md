# Claude Project Memory Context (Start → Current)

## 1) Project Identity and Core Goal

- Project: **Island & Ferry**
- Repo: `/media/ignvaibhav/Shared Volume/CODING/Dummy_projects/clipdrop (codex)`
- Product shape:
  - `extension/` = Ferry browser extension injecting download action into YouTube watch pages
  - `app/src-tauri/` = Island local Tauri desktop companion exposing API on `127.0.0.1:49152`
- Core product intent:
  - User clicks injected Ferry button on YouTube
  - Extension fetches available formats from Island local backend
  - User chooses quality and optional clip
  - Backend downloads locally via `yt-dlp` (and ffmpeg when needed)
  - All local-first, no remote backend/accounts

---

## 2) High-Level Architecture (Current)

- Backend endpoints:
  - `GET /health`
  - `POST /formats`
  - `POST /download`
  - `GET /status/:job_id`
  - `POST /reveal` (opens folder path in file explorer)
  - `GET/ANY /ws` for live job events
- Queue model:
  - Sequential worker (`queue.rs`)
  - Job status tracked in memory
  - Broadcast WS events for progress/done/error
- Extension model:
  - Injected button and floating panel in `content.js`
  - Background service worker manages WS, notifications, activity feed
  - Popup remains for activity/settings-style visibility, but main UX is injected panel

---

## 3) Major Requirements Gathered from User Across Session

- UI should be under injected button; user should not rely on toolbar icon.
- Show only real qualities returned for current video (no hardcoded fake list).
- Quality selection must actually influence output quality.
- Handle duplicate filenames gracefully.
- Provide clip section selection UI.
- Harden extension against reload/context-invalidated errors.
- Provide download completion notifications even if tab switches/closes.
- Add tray settings window in desktop app for changing download path.
- Improve cross-platform setup with one-liner bootstrap scripts.
- Build robust context doc for future model handoff.

---

## 4) Backend Work Completed

### 4.1 Format logic

- `formats.rs` uses `yt-dlp --dump-json` and builds options from actual formats.
- Dynamic format options include:
  - best available
  - per-height video options
  - audio-only MP3 option
- Uses YouTube extractor args:
  - `youtube:player_client=android_vr,web_safari,android,web`

### 4.2 Download logic

- `downloader.rs`:
  - sidecar-first binary resolution + PATH fallback
  - robust progress parsing from stdout
  - retry/fallback execution path for alt yt-dlp binary
  - no-overwrite behavior
  - output filenames include quality/format variant metadata
  - duplicate filename increment behavior (`(2)`, `(3)`, …) on collisions

### 4.3 New reveal API

- Added `POST /reveal` in `server.rs`
- Accepts path and opens containing folder using:
  - `explorer` (Windows)
  - `open` (macOS)
  - `xdg-open` (Linux)
- Used by Ferry extension notification click behavior

### 4.4 Download path settings support

- Added persisted desktop settings in Island app (`main.rs`)
- Commands added:
  - `get_download_settings`
  - `set_download_directory`
  - `reset_download_directory`
  - `browse_download_directory`
- Default remains system Downloads when no custom path set

---

## 5) Extension Work Completed

### 5.1 Injection and panel UX

- Main flow in `extension/content.js`:
  - Inject Ferry button on YouTube watch pages
  - Open floating panel anchored near the button
  - Panel includes quality selector, clip controls, download button, queue/progress text
  - Close button (`×`) included

### 5.2 Runtime hardening

- Added guards against extension context invalidation:
  - runtime checks
  - safe message wrappers
  - listener attach protections
- Removed fragile storage read paths that caused undefined/local errors

### 5.3 Queue/progress behavior

- Queue text only shown during active download flow
- On panel close, queue/progress display resets/hides
- Reopening panel keeps queue hidden until next download
- WS updates still drive live progress when available

### 5.4 Notification behavior

- Background worker now creates desktop notifications on done/error
- Notifications include click-to-reveal folder behavior via `/reveal`
- Added notification creation error logging (`runtime.lastError`) for diagnosis

### 5.5 Download update reliability improvements

- Added `/status/:job_id` polling fallback in Ferry content panel after queueing
- This mitigates missed WS events and prevents “no update” state
- Polling stops on done/error/close

### 5.6 Clip selector UX changes

- Replaced timestamp inputs with a single dual-handle segment track UI
- Clip payload now sent only if user actually interacts with clip controls
  - avoids forcing clipping path on default download

### 5.7 Popup/legacy UI cleanup

- Removed explicit `status` element usage from popup HTML/CSS/JS
- Progress/activity surfaces remain for state reporting

---

## 6) Desktop Tray + Settings Window

- Tray menu now includes:
  - `Settings`
  - `Open Downloads Folder`
  - `Quit`
- Settings opens a small dedicated webview window:
  - can set/reset download path
  - has native `Browse…` folder picker
- Window was resized to be less cramped (1.5x increase)

---

## 7) Cross-Platform/Bootstrap Work

Added one-line bootstrap scripts:

- `scripts/bootstrap-linux.sh`
- `scripts/bootstrap-macos.sh`
- `scripts/bootstrap-windows.ps1`

README updated with one-line commands.

What scripts do:

- install platform prerequisites
- install Rust + Tauri CLI
- install yt-dlp + ffmpeg
- place/update platform sidecar(s)
- run `cargo check`
- print next steps (`cargo run`, extension load path)

---

## 8) Key Bugs Seen During Session and Their Resolutions

1. Extension runtime errors (`undefined local`, `context invalidated`)
- Fixed via runtime guards and safer messaging/storage access.

2. WS refused/noisy reconnect errors
- Added health-gated connect and reconnect strategy.

3. Quality selection seemingly ignored (always low/360-like behavior)
- Multiple fixes:
  - dynamic format IDs from yt-dlp data
  - exact selector path usage
  - output/duplication behavior fixes
  - clip default behavior corrected to avoid forcing clipping mode

4. Duplicate overwrite/reuse confusion
- Added variant-aware naming + incrementing names for existing files.

5. Button not appearing on first page open
- Added improved injection hooks/observer strategy and fallback loops.

6. Notification not consistently appearing
- Background notification path now independent from strict activeJobs gating for done/error.

---

## 9) Current Important Files

- Backend:
  - `app/src-tauri/src/main.rs`
  - `app/src-tauri/src/server.rs`
  - `app/src-tauri/src/downloader.rs`
  - `app/src-tauri/src/formats.rs`
  - `app/src-tauri/src/queue.rs`
  - `app/src-tauri/src/models.rs`
- Frontend extension:
  - `extension/content.js`
  - `extension/background.js`
  - `extension/popup.js`
  - `extension/popup.html`
  - `extension/popup.css`
  - `extension/manifest.json`
- Setup:
  - `scripts/bootstrap-linux.sh`
  - `scripts/bootstrap-macos.sh`
  - `scripts/bootstrap-windows.ps1`
  - `README.md`

---

## 10) Current Known Risks / Follow-Up Recommendations

1. YouTube extraction quality variance can still depend on upstream platform constraints (clients/tokens/cookies).
2. Extension behavior depends on YouTube DOM structure; monitor selector drift.
3. Windows/macOS bootstrap scripts are designed but should be fully runtime-verified on native hosts.
4. If users report “download not starting,” check:
   - backend running (`/health`)
   - `yt-dlp` and `ffmpeg` availability
   - extension service worker logs
   - queue/job status endpoint

---

## 11) Current State Summary

The project has moved from scaffold to a mostly working local-first MVP:

- Injected YouTube UI and local download workflow implemented
- Dynamic format probing and queue system in place
- Notifications + reveal folder workflow present
- Tray settings window with configurable download path added
- Cross-platform bootstrap scripts prepared
- Multiple reliability fixes applied for injection timing, runtime invalidation, and download status propagation
