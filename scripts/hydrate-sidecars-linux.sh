#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT_DIR/app/src-tauri"
RES_DIR="$APP_DIR/resources"

log() { echo "[hydrate-sidecars-linux] $*"; }
have_cmd() { command -v "$1" >/dev/null 2>&1; }

ensure_cmd() {
  if ! have_cmd "$1"; then
    echo "error: required command '$1' not found" >&2
    exit 1
  fi
}

copy_binary() {
  local source="$1"
  local destination="$2"
  cp "$source" "$destination"
  chmod +x "$destination"
}

main() {
  ensure_cmd curl
  ensure_cmd ffmpeg
  ensure_cmd ffprobe

  mkdir -p "$RES_DIR"

  local arch
  local asset
  arch="$(uname -m)"
  asset="yt-dlp_linux"
  if [[ "$arch" == "aarch64" || "$arch" == "arm64" ]]; then
    asset="yt-dlp_linux_aarch64"
  fi

  log "Downloading standalone yt-dlp linux binary"
  curl -L "https://github.com/yt-dlp/yt-dlp/releases/latest/download/${asset}" \
    -o "$RES_DIR/yt-dlp-linux"
  chmod +x "$RES_DIR/yt-dlp-linux"

  log "Copying ffmpeg and ffprobe into bundled sidecars"
  copy_binary "$(command -v ffmpeg)" "$RES_DIR/ffmpeg-linux"
  copy_binary "$(command -v ffprobe)" "$RES_DIR/ffprobe-linux"
}

main "$@"
