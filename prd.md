# Island & Ferry — Product Requirements Document (PRD)

**Version:** 0.1 (MVP)
**Status:** Draft
**Last Updated:** May 2026

---

## 1. Overview

### 1.1 Product Summary
Island & Ferry is an open-source, two-part system that lets users download videos from YouTube (and later other platforms) directly from their browser — with no terminal, no technical knowledge, and no friction. It consists of the **Ferry browser extension** and the **Island desktop companion app**, connected via a local server.

### 1.2 Problem Statement
Tools like `yt-dlp` are incredibly powerful but locked behind the terminal — inaccessible to the majority of users who just want to save a video. Existing GUI wrappers are either bloated, paid, ad-ridden, or require manual setup. There is no clean, open-source, browser-native solution that feels like it belongs.

### 1.3 Solution
A lightweight browser extension (Ferry) that injects a native-feeling download button on video pages. The extension captures user intent (format, quality, optional clip range) and hands off to a locally-running Tauri desktop app (Island) that silently executes `yt-dlp` in the background. The user never leaves the browser.

### 1.4 Vision
> Make downloading a video as easy as liking one.

---

## 2. Goals & Non-Goals

### Goals
- Zero terminal interaction required
- Works across all major platforms: macOS, Windows, Linux
- Works across all major browsers: Chrome, Firefox, Brave, Edge
- Open source with MIT license
- No servers, no accounts, no telemetry
- Ships with all dependencies bundled (yt-dlp, ffmpeg)
- Live progress feedback inside the browser

### Non-Goals (v0.1)
- No cloud sync or remote downloading
- No browser-only solution (desktop app is required)
- No support for non-YouTube platforms in v0.1
- No video editing beyond basic clipping
- No playlist/batch downloads in v0.1

---

## 3. Target Users

| User Type | Description |
|---|---|
| **Primary** | Non-technical users who want to save videos without using a terminal |
| **Secondary** | Developers/power users who know yt-dlp but prefer a GUI for daily use |
| **Tertiary** | Content creators who archive reference videos for offline use |

**User mindset:** They've Googled "how to download YouTube videos," bounced off sketchy websites, and want something that just works — installed once, used forever.

---

## 4. System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        BROWSER                              │
│                                                             │
│   YouTube Page                                              │
│   ┌─────────────────────────────────────────────────────┐  │
│   │  [Like] [Dislike] [Share] [Ferry ↓]  ← injected     │  │
│   └─────────────────────────────────────────────────────┘  │
│                          │                                  │
│              Extension Popup (format/quality/clip)          │
│                          │                                  │
│              HTTP POST /download                            │
│                          │                                  │
└──────────────────────────┼──────────────────────────────────┘
                           │
                    localhost:49152
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                  ISLAND DESKTOP APP (Tray)                   │
│                                                             │
│   Axum Server  →  Download Queue  →  yt-dlp subprocess      │
│                                          │                  │
│                                     ffmpeg (if clipping)    │
│                                          │                  │
│                                   File saved to disk        │
│                                          │                  │
│                        WebSocket progress stream            │
│                                          │                  │
└──────────────────────────────────────────┼──────────────────┘
                                           │
                           ┌───────────────▼──────────────┐
                           │  Extension — Progress Bar +   │
                           │  "Done ✓" Toast Notification  │
                           └──────────────────────────────┘
```

---

## 5. Components

### 5.1 Browser Extension (Ferry)

**Role:** UI injector + metadata extractor + status display

**Responsibilities:**
- Detect supported video pages (YouTube in v0.1)
- Inject a download button into the native video action bar (styled to match the platform UI)
- On click: render a popup with format, quality, and optional clip range selector
- Fetch available formats from the desktop app (which queries yt-dlp) and populate the UI dynamically
- POST metadata payload to local app
- Open a WebSocket connection to receive live progress updates
- Display progress bar and "Done ✓" toast on completion
- Show a soft error if desktop app is not running (with install link)

**Metadata payload:**
```json
{
  "url": "https://youtube.com/watch?v=xxxx",
  "title": "Video Title",
  "format": "mp4",
  "quality": "1080p",
  "clip": {
    "start": "00:01:10",
    "end": "00:02:30"
  }
}
```
*`clip` is optional. Omit if full video download.*

**Browser support:** Chrome, Firefox, Brave, Edge (Manifest V3)

**Files:**
```
extension/
├── manifest.json       # MV3 — permissions: activeTab, storage, notifications
├── content.js          # Injected into video pages — button + metadata extraction
├── popup.html          # Format/quality/clip selector UI
├── popup.js            # Handles user selection + sends to app
├── background.js       # Service worker — WebSocket handler + notifications
└── icons/              # Extension icons (16, 48, 128px)
```

---

### 5.2 Island Desktop App

**Role:** Local HTTP/WebSocket server + yt-dlp orchestrator + system tray daemon

**Responsibilities:**
- Run silently in the system tray at OS startup
- Host a local server at `localhost:49152`
- Receive download requests from extension
- Query yt-dlp for available formats for a given URL
- Spawn yt-dlp as a subprocess with correct flags
- Stream stdout/stderr from yt-dlp back to extension via WebSocket
- Manage a download queue (sequential in v0.1)
- Provide a minimal tray menu: Open Downloads Folder, Quit

**Tech:**
- **Tauri v2** (Rust backend + system webview for optional settings UI)
- **Axum** for HTTP + WebSocket server (runs inside Tauri's async runtime)
- **`tauri-plugin-autostart`** for OS startup registration
- **`tokio::process::Command`** for spawning yt-dlp subprocess non-blocking

**Bundled resources:**
```
resources/
├── yt-dlp-mac
├── yt-dlp-win.exe
├── yt-dlp-linux
├── ffmpeg-mac
├── ffmpeg-win.exe
└── ffmpeg-linux
```
Resolved at runtime via `tauri::api::path::resource_dir()`. User installs zero dependencies.

---

### 5.3 Local Server API

Base URL: `http://localhost:49152`

| Endpoint | Method | Request Body | Response | Description |
|---|---|---|---|---|
| `/health` | GET | — | `{ "status": "ok", "version": "0.1.0" }` | Extension health check on page load |
| `/formats` | POST | `{ "url": "..." }` | `{ "formats": [...] }` | Fetch available formats for a URL |
| `/download` | POST | Metadata payload | `{ "job_id": "abc123" }` | Queue a download job |
| `/status/:job_id` | GET | — | `{ "status": "...", "progress": 72 }` | Poll job status |
| `/ws` | WebSocket | — | Progress events | Real-time progress stream |

**WebSocket event schema:**
```json
{ "job_id": "abc123", "event": "progress", "percent": 72, "speed": "3.2MiB/s", "eta": "00:00:12" }
{ "job_id": "abc123", "event": "done", "path": "/Users/user/Downloads/video.mp4" }
{ "job_id": "abc123", "event": "error", "message": "..." }
```

---

### 5.4 yt-dlp Integration

**Format selection flags:**
```bash
# Video + audio (merged)
yt-dlp -f "bestvideo[height<=1080]+bestaudio/best" --merge-output-format mp4

# Audio only
yt-dlp -f "bestaudio" --extract-audio --audio-format mp3

# With clipping (requires ffmpeg)
yt-dlp --download-sections "*00:01:10-00:02:30" --force-keyframes-at-cuts

# Output template
-o "~/Downloads/%(title)s.%(ext)s"
```

**Format fetch (for popup UI):**
```bash
yt-dlp --list-formats --dump-json [URL]
```
Parsed and returned to extension to dynamically populate quality dropdown.

---

## 6. User Flows

### 6.1 First-Time Setup
1. User downloads Island desktop app (installer for their OS)
2. App installs, registers for auto-start, launches in system tray
3. User installs Ferry browser extension from Chrome/Firefox store
4. Done — no configuration required

### 6.2 Download a Video
1. User visits a YouTube video page
2. Extension detects the page, injects "↓ Ferry" button in action bar
3. User clicks the button
4. Extension pings `/health` — if app not running, shows "Open Island app" prompt
5. Extension calls `/formats` — popup shows available quality options
6. User selects format + quality → clicks "Download"
7. Extension POSTs to `/download`, opens WebSocket
8. Progress bar appears in popup with live speed + ETA
9. Download completes → toast: "Done ✓ — Saved to Downloads"

### 6.3 Clip a Portion
1. Same as above, but user expands "Clip" section in popup
2. Inputs start/end timestamps (e.g. `01:10` → `02:30`)
3. `clip` object included in payload
4. App passes `--download-sections` to yt-dlp + invokes ffmpeg for cuts
5. Clipped file saved to disk

---

## 7. UX Requirements

| Requirement | Detail |
|---|---|
| Button blends in | Styled to match YouTube's native action bar — not a foreign element |
| No app window | Desktop app has no main window — tray only |
| Zero config | Works out of the box — no API keys, no settings required |
| App not running | Extension shows a clear, non-scary prompt with instructions |
| Progress visibility | Live % + speed + ETA visible in popup during download |
| Error handling | Friendly error messages (e.g. "Video unavailable", "Format not supported") |
| Completion feedback | Toast notification with filename + "Show in Folder" button |

---

## 8. Platform Support

| OS | Tray | Auto-start | Installer Format |
|---|---|---|---|
| macOS | Menu Bar (top right) | Login Items | `.dmg` |
| Windows | System Tray (bottom right) | Registry Run key | `.msi` / `.exe` |
| Linux | AppIndicator / StatusNotifier | systemd user service / XDG autostart | `.AppImage` / `.deb` |

**Browser extension compatibility:**
| Browser | Engine | Support |
|---|---|---|
| Chrome | Chromium/MV3 | v0.1 |
| Brave | Chromium/MV3 | v0.1 |
| Edge | Chromium/MV3 | v0.1 |
| Firefox | Gecko/MV3 | v0.2 |
| Safari | WebKit | Stretch goal |

---

## 9. Open Source & Legal Strategy

- **License:** MIT — permissive, widely trusted, user bears usage responsibility
- **No servers:** All processing happens on user's local machine
- **No telemetry:** Zero data collection, zero analytics
- **No accounts:** No login, no registration
- **DMCA posture:** Island is a local tool — equivalent to youtube-dl, yt-dlp, JDownloader. No hosted content, no proxying.
- **Store listing language:** Describe as *"media companion launcher for Island desktop"* — avoid "download", "rip", "save" in store descriptions
- **yt-dlp license:** Unlicense (public domain) — compatible with MIT

---

## 10. Project Structure (Monorepo)

```
island-ferry/
├── app/                          # Island desktop app
│   ├── src-tauri/
│   │   ├── src/
│   │   │   ├── main.rs           # App entry, tray menu, autostart
│   │   │   ├── server.rs         # Axum HTTP + WebSocket server
│   │   │   ├── downloader.rs     # yt-dlp subprocess manager
│   │   │   ├── formats.rs        # Format fetching + parsing
│   │   │   └── queue.rs          # Download job queue
│   │   ├── resources/            # Bundled yt-dlp + ffmpeg binaries
│   │   ├── icons/                # App icons
│   │   └── tauri.conf.json
│   └── src/                      # Optional settings webview (future)
├── extension/
│   ├── manifest.json
│   ├── content.js
│   ├── popup.html
│   ├── popup.js
│   ├── background.js
│   └── icons/
├── docs/
│   ├── PRD.md                    # This document
│   ├── ARCHITECTURE.md
│   └── CONTRIBUTING.md
├── .github/
│   └── workflows/                # CI: build + release per platform
├── LICENSE                       # MIT
└── README.md
```

---

## 11. MVP Scope (v0.1)

### In Scope
- YouTube support only
- Formats: mp4 (video), mp3 (audio-only)
- Quality options: 720p, 1080p, audio-only
- Full video download (no clipping)
- Single download at a time (no queue UI)
- macOS + Windows
- Chrome + Brave
- Live progress in extension popup
- "Done" toast with filename

### Out of Scope (deferred)
- Video clipping / trim
- Linux support
- Firefox support
- Other platforms (Instagram, Twitter/X, Reddit)
- Playlist / batch downloads
- Download history UI
- Settings UI (download path, concurrent downloads)
- Auto yt-dlp update checker

---

## 12. Roadmap

| Version | Target | Key Features |
|---|---|---|
| **v0.1** | MVP | YouTube, mp4/mp3, 720p/1080p, macOS + Windows, Chrome + Brave |
| **v0.2** | Clipping + Linux + Firefox | Trim support, ffmpeg integration, Linux builds, Firefox extension |
| **v0.3** | Platform expansion | Instagram, Twitter/X, Reddit support |
| **v0.4** | Power features | Playlist/batch downloads, concurrent downloads, download queue UI |
| **v0.5** | Polish | Download history, custom output path settings, format presets |
| **v1.0** | Stable | Auto yt-dlp updates, subtitle download, full cross-platform parity |

---

## 13. Technical Risks & Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| YouTube breaks yt-dlp extractor | High (periodic) | yt-dlp auto-updates frequently; ship update checker in v1.0 |
| Chrome Web Store rejects extension | Medium | Frame as "companion launcher", avoid downloader language |
| Port 49152 conflict on user machine | Low | Fallback port scan; store selected port in app config |
| ffmpeg binary size bloat | Medium | Ship platform-specific minimal ffmpeg builds |
| Tauri WebSocket CORS with extension | Low | Whitelist extension origin in Axum CORS config |
| macOS Gatekeeper blocking app | Medium | Code-sign + notarize macOS builds in CI pipeline |

---

## 14. Success Metrics (Post-launch)

| Metric | Target (3 months post-launch) |
|---|---|
| GitHub Stars | 500+ |
| Extension installs | 1,000+ |
| Open issues resolved | >80% within 2 weeks |
| Crash-free sessions | >98% |
| Download success rate | >95% on supported platforms |

---

*This document is the single source of truth for Island & Ferry v0.1 scope and architecture. Update it as decisions evolve.*
