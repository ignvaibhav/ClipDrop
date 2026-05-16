Place platform-specific sidecar binaries here:

- `yt-dlp-mac`
- `yt-dlp-win.exe`
- `yt-dlp-linux`
- `ffmpeg-mac`
- `ffprobe-mac`
- `ffmpeg-win.exe`
- `ffprobe-win.exe`
- `ffmpeg-linux`
- `ffprobe-linux`

At runtime, the backend uses these bundled resources only.

Notes:

- macOS binaries are committed in-repo today.
- Windows and Linux binaries are hydrated during CI and bootstrap scripts so release builds do not depend on placeholder files.
- Placeholder or zero-byte sidecars are intentionally rejected by the backend at runtime.
