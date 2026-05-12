# ClipDrop: Technical Skills & Core Competencies

This document outlines the specific technical skills and architectural domains required to execute the ClipDrop project. It serves as a directive for AI agents, a roadmap for development, and a structural reference for building a high-level engineering portfolio.

## 1. Systems Architecture & Desktop Integration (Rust + Tauri)
*   **Asynchronous Runtime Management:** Leveraging `tokio` for non-blocking execution of the local server and background processes.
*   **Subprocess Orchestration:** Utilizing `tokio::process::Command` to securely spawn, manage, and monitor `yt-dlp` and `ffmpeg` binaries with proper signal handling and graceful termination.
*   **Daemonization:** Building headless, system-tray-only applications with OS-level autostart capabilities (`tauri-plugin-autostart`).
*   **Binary Bundling (Sidecars):** Configuring Tauri to package and execute cross-platform external binaries dynamically without relying on user PATH variables, using `tauri::api::path::resource_dir()` for runtime resolution.
*   **Thread-Safe State Management:** Handling a concurrent download queue using `Arc<Mutex<T>>` to ensure safe data access across the Axum server and the background worker threads.
*   **System Tray Integration:** Implementing native system tray menus with platform-specific behaviors (macOS Menu Bar, Windows System Tray, Linux AppIndicator/StatusNotifier).

## 2. Local Networking & IPC (Inter-Process Communication)
*   **HTTP Server (Axum):** Designing a fast, lightweight local API (`localhost:49152`) to receive metadata payloads from the browser with RESTful endpoint design (`/health`, `/formats`, `/download`, `/status/:job_id`).
*   **Real-Time Data Streaming:** Implementing **WebSockets** (`/ws`) to broadcast live `stdout` metrics (percentage, speed, ETA) from the Rust backend to the browser extension with structured event schemas.
*   **Security & Port Management:** Managing CORS policies to strictly allow communication only from the registered browser extension, and handling port fallback scanning when the default port is occupied.
*   **Health Check Protocol:** Implementing lightweight ping/pong mechanisms for the extension to verify the desktop app is running before attempting downloads.

## 3. Browser Extension Engineering (JavaScript & DOM)
*   **Manifest V3 (MV3) Architecture:** Structuring service workers (`background.js`), content scripts, and popups according to modern browser security standards with proper permission declarations (`activeTab`, `storage`, `notifications`).
*   **Dynamic DOM Manipulation:** Utilizing `MutationObserver` to reliably inject native-feeling, minimalist UI elements into complex Single Page Applications (SPAs) like YouTube, ensuring the components survive dynamic page navigations and match platform styling.
*   **Message Passing:** Handling asynchronous communication between the content script, the service worker, and the local Tauri server using `chrome.runtime.sendMessage` and fetch API.
*   **Cross-Browser Compatibility:** Writing extension code that works across Chromium-based browsers (Chrome, Brave, Edge) with considerations for future Firefox (Gecko) support.
*   **Extension Popup UI:** Building responsive, accessible popup interfaces for format/quality selection with dynamic data population from the backend.
*   **Browser Notifications API:** Implementing toast notifications with actionable buttons ("Show in Folder") for download completion feedback.

## 4. Video Processing & Format Management
*   **Format Detection & Parsing:** Executing `yt-dlp --list-formats --dump-json` to retrieve available video/audio formats and parsing the JSON output to populate UI dropdowns dynamically.
*   **Quality Selection Logic:** Implementing format string construction for yt-dlp based on user preferences (e.g., `bestvideo[height<=1080]+bestaudio/best` for merged video+audio).
*   **Audio Extraction:** Handling audio-only downloads with `--extract-audio --audio-format mp3` flags.
*   **Video Clipping (ffmpeg):** Integrating ffmpeg for precise timestamp-based video trimming using `--download-sections "*HH:MM:SS-HH:MM:SS"` with `--force-keyframes-at-cuts` for clean cuts.
*   **Output Template Management:** Configuring yt-dlp output paths with template variables (`-o "~/Downloads/%(title)s.%(ext)s"`).

## 5. Data Parsing & Stream Processing
*   **Regex-Based Stream Parsing:** Writing robust Regular Expressions to intercept raw CLI `stdout` from `yt-dlp`, parsing unstructured text into structured JSON payloads for the WebSocket stream (extracting percentage, speed, ETA).
*   **JSON Serialization/Deserialization:** Utilizing `serde` and `serde_json` in Rust to safely parse incoming extension requests and format responses with proper error handling.
*   **Metadata Extraction:** Parsing video metadata (title, URL, duration) from both the browser DOM and yt-dlp JSON output for consistent data flow.

## 6. Job Queue & State Management
*   **Sequential Download Queue:** Implementing a FIFO job queue for v0.1 (single download at a time) with unique job IDs for tracking.
*   **Job Status Tracking:** Maintaining job state (`queued`, `in_progress`, `done`, `error`) with thread-safe access patterns.
*   **Progress Event Broadcasting:** Streaming real-time progress updates to connected WebSocket clients with proper event typing (`progress`, `done`, `error`).
*   **Graceful Cancellation:** Handling user-initiated download cancellation with proper subprocess cleanup.

## 7. Error Handling & Recovery
*   **Network Failure Handling:** Detecting and reporting network errors from yt-dlp (video unavailable, geo-restricted, private videos).
*   **Format Unavailability:** Gracefully handling cases where requested quality/format is not available with fallback suggestions.
*   **Desktop App Offline Detection:** Implementing soft error prompts in the extension when the local server is unreachable, with clear instructions and install links.
*   **Subprocess Error Parsing:** Extracting meaningful error messages from yt-dlp stderr and translating them into user-friendly notifications.
*   **Port Conflict Resolution:** Implementing automatic port scanning and fallback when the default port is occupied.

## 8. Cross-Platform OS Integration
*   **macOS Specifics:** Code signing, notarization for Gatekeeper, `.dmg` installer creation, Login Items registration, Menu Bar integration.
*   **Windows Specifics:** `.msi`/`.exe` installer creation, Registry Run key for autostart, System Tray integration, Windows Defender SmartScreen handling.
*   **Linux Specifics:** `.AppImage`/`.deb` packaging, systemd user service or XDG autostart configuration, AppIndicator/StatusNotifier tray support.
*   **Path Resolution:** Handling platform-specific default download directories (`~/Downloads` on macOS/Linux, `%USERPROFILE%\Downloads` on Windows).

## 9. Security & Privacy
*   **Local-Only Architecture:** Ensuring all processing happens on the user's machine with zero external server communication.
*   **No Telemetry:** Implementing a completely offline system with no analytics, crash reporting, or usage tracking.
*   **Extension Origin Validation:** Restricting local API access to only the registered browser extension via CORS and origin checking.
*   **Secure Subprocess Execution:** Preventing command injection by using structured command builders rather than shell string interpolation.
*   **No Credential Storage:** Avoiding any user authentication, API keys, or account systems.

## 10. UI/UX & Design Implementation
*   **Minimalist Interface Design:** Translating core functionality into a modern, high-contrast (black and white) aesthetic that feels native to the host environment, minimizing user friction.
*   **Event-Driven Feedback:** Implementing non-intrusive progress bars and system toast notifications to provide immediate, clear state visibility to the user.
*   **Platform-Native Styling:** Matching YouTube's native action bar styling for the injected download button to avoid feeling like a foreign element.
*   **Zero-Configuration UX:** Designing the system to work out-of-the-box with no settings, API keys, or configuration required.
*   **Accessibility Compliance:** Ensuring keyboard navigation, screen reader support, and proper ARIA labels in extension UI components.

## 11. Testing & Quality Assurance
*   **Integration Testing:** Testing the full browser-to-desktop communication flow with mock yt-dlp responses.
*   **Cross-Browser Testing:** Validating extension behavior across Chrome, Brave, and Edge with different DOM structures.
*   **Cross-Platform Testing:** Ensuring binary execution and tray behavior works correctly on macOS, Windows, and Linux.
*   **Error Scenario Testing:** Simulating network failures, unavailable videos, format mismatches, and app offline states.
*   **Performance Testing:** Monitoring memory usage during long downloads and ensuring WebSocket connections don't leak.

## 12. Build, Release & DevOps
*   **Cross-Platform Compilation:** Managing CI/CD pipelines (GitHub Actions) to compile Rust and build platform-specific installers (`.msi` for Windows, `.dmg` for macOS, `.AppImage`/`.deb` for Linux).
*   **Release Engineering:** Managing versioning, dependency tracking, and executable signing to prevent OS-level warnings (e.g., macOS Gatekeeper, Windows Defender).
*   **Binary Sidecar Management:** Automating the download and bundling of platform-specific yt-dlp and ffmpeg binaries during the build process.
*   **Extension Store Submission:** Preparing extension packages for Chrome Web Store and Firefox Add-ons with appropriate metadata and privacy policy language.
*   **Automated Testing in CI:** Running unit tests, integration tests, and linting checks on every commit.

## 13. Open Source & Community Management
*   **MIT License Compliance:** Understanding permissive licensing implications and user responsibility for usage.
*   **Documentation Writing:** Creating clear README, CONTRIBUTING, and ARCHITECTURE docs for community contributors.
*   **Issue Triage:** Managing GitHub issues with proper labeling, reproduction steps, and prioritization.
*   **DMCA & Legal Posture:** Framing the project as a local tool (equivalent to youtube-dl, yt-dlp) with no hosted content or proxying.
*   **Store Listing Strategy:** Using careful language ("media companion launcher") to avoid triggering automated rejections.

## 14. Future-Proofing & Extensibility
*   **Plugin Architecture Considerations:** Designing the downloader module to be platform-agnostic for future support of Instagram, Twitter/X, Reddit.
*   **Auto-Update Mechanism:** Planning for yt-dlp version checking and automatic binary updates in future versions.
*   **Settings Persistence:** Designing a configuration system for future features (custom download paths, concurrent downloads, format presets).
*   **Playlist/Batch Download Architecture:** Structuring the job queue to support multiple simultaneous downloads in future versions.