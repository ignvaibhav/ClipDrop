# Platform Audit

This audit covers the current Ferry + Island desktop release path for:

- macOS
- Windows
- Linux

## Release Matrix

| Platform | GitHub runner | Bundled artifact(s) | Sidecar source in CI | Status |
|---|---|---|---|---|
| macOS | `macos-latest` | `Island-macOS-app.zip` | committed repo sidecars | shipping |
| Windows | `windows-latest` | `Island-windows-setup.exe` | CI-hydrated `yt-dlp`, `ffmpeg`, `ffprobe` | shipping |
| Linux | `ubuntu-22.04` | `Island-linux.AppImage`, `Island-linux.deb` | CI-hydrated `yt-dlp`, `ffmpeg`, `ffprobe` | shipping |

## Findings Fixed

### 1. Windows and Linux sidecars were placeholders

The repository resources folder contained placeholder or zero-byte files for:

- `yt-dlp-win.exe`
- `ffmpeg-win.exe`
- `ffprobe-win.exe`
- `yt-dlp-linux`
- `ffmpeg-linux`
- `ffprobe-linux`

Impact:

- packaged Windows/Linux apps could build from source, but downloads would fail at runtime
- release automation did not previously replace those placeholders

Fix:

- release workflows now hydrate real Windows/Linux sidecars before Tauri bundling
- bootstrap scripts now do the same for local Windows/Linux development

### 2. Windows file reveal used a brittle Explorer invocation

The reveal endpoint passed:

- `/select,`
- then the path as a separate argument

Impact:

- Windows Explorer selection behavior can be inconsistent

Fix:

- Ferry now passes a single `/select,<path>` argument when revealing files on Windows

## Remaining Risks

### Linux portability

Linux bundles now ship with hydrated sidecars, but distro-level media/runtime differences still make Linux the least uniform release target. The `.AppImage` and `.deb` outputs are the intended primary download paths.

### macOS signing

macOS builds are still ad-hoc signed for direct-download convenience, not Developer ID signed or notarized.

### Smoke testing

The new matrix builds all three platforms, but the strongest next step is artifact smoke-testing on:

- a clean Windows machine
- an Ubuntu machine

That verifies:

- bundle launch
- sidecar discovery
- at least one video, audio, and thumbnail download path
