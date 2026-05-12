#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="${ROOT_DIR}/app/src-tauri"
DIST_DIR="${ROOT_DIR}/dist"
EXT_DIR="${ROOT_DIR}/extension"

export DEBIAN_FRONTEND=noninteractive

log() {
  echo "[bootstrap-build] $*"
}

require_cmd() {
  local cmd="$1"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "error: required command '$cmd' not found" >&2
    exit 1
  fi
}

ensure_root_or_sudo() {
  if [[ "${EUID}" -eq 0 ]]; then
    SUDO=""
    return
  fi

  if command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
    SUDO="sudo -n"
    return
  fi

  echo "error: run as root or with passwordless sudo for fully non-interactive execution" >&2
  exit 1
}

pick_webkit_dev_pkg() {
  if apt-cache show libwebkit2gtk-4.1-dev >/dev/null 2>&1; then
    echo "libwebkit2gtk-4.1-dev"
    return
  fi

  if apt-cache show libwebkit2gtk-4.0-dev >/dev/null 2>&1; then
    echo "libwebkit2gtk-4.0-dev"
    return
  fi

  echo "error: neither libwebkit2gtk-4.1-dev nor libwebkit2gtk-4.0-dev is available" >&2
  exit 1
}

install_apt_deps() {
  local webkit_pkg
  webkit_pkg="$(pick_webkit_dev_pkg)"

  log "Installing apt dependencies (${webkit_pkg} + GTK/Tauri deps)"
  ${SUDO} apt-get update -y
  ${SUDO} apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    file \
    pkg-config \
    zip \
    ca-certificates \
    libgtk-3-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    "${webkit_pkg}" \
    patchelf
}

install_rust_if_missing() {
  if command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1; then
    log "Rust already installed"
    return
  fi

  log "Installing Rust toolchain"
  local rustup_script
  rustup_script="$(mktemp "${ROOT_DIR}/.rustup-init.XXXXXX.sh")"

  if ! curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "${rustup_script}"; then
    rm -f "${rustup_script}"
    echo "error: failed downloading rustup installer (check free disk space and write permissions)" >&2
    exit 1
  fi

  sh "${rustup_script}" -y --profile default
  rm -f "${rustup_script}"
}

source_cargo_env() {
  if [[ -f "$HOME/.cargo/env" ]]; then
    # shellcheck disable=SC1090
    source "$HOME/.cargo/env"
    return
  fi

  echo "error: cargo environment not found after rustup install" >&2
  exit 1
}

install_tauri_cli_if_missing() {
  if cargo tauri --version >/dev/null 2>&1; then
    log "Tauri CLI already available"
    return
  fi

  log "Installing Tauri CLI"
  cargo install tauri-cli --version '^2'
}

install_latest_ytdlp_sidecar() {
  log "Installing latest yt-dlp sidecar binary"
  mkdir -p "${APP_DIR}/resources"
  curl -L "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux" \
    -o "${APP_DIR}/resources/yt-dlp-linux"
  chmod +x "${APP_DIR}/resources/yt-dlp-linux"
}

ensure_rgba_icon() {
  log "Ensuring Tauri icons are valid square RGBA PNGs"
  python3 - <<'PY'
import struct, zlib
from pathlib import Path

def chunk(t, data):
    return struct.pack("!I", len(data)) + t + data + struct.pack("!I", zlib.crc32(t + data) & 0xFFFFFFFF)

w, h = 64, 64
def make_png(w, h):
    row = bytes([0] + [17, 17, 17, 255] * w)
    raw = row * h
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack("!IIBBBBB", w, h, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(raw, 9))
    png += chunk(b"IEND", b"")
    return png

Path("app/src-tauri/icons").mkdir(parents=True, exist_ok=True)
Path("extension/icons").mkdir(parents=True, exist_ok=True)
Path("app/src-tauri/icons/icon.png").write_bytes(make_png(64, 64))
Path("app/src-tauri/icons/32x32.png").write_bytes(make_png(32, 32))
Path("app/src-tauri/icons/128x128.png").write_bytes(make_png(128, 128))
Path("app/src-tauri/icons/256x256.png").write_bytes(make_png(256, 256))
Path("app/src-tauri/icons/512x512.png").write_bytes(make_png(512, 512))

Path("extension/icons/icon16.png").write_bytes(make_png(16, 16))
Path("extension/icons/icon48.png").write_bytes(make_png(48, 48))
Path("extension/icons/icon128.png").write_bytes(make_png(128, 128))
PY
}

ensure_sidecar_placeholders() {
  log "Ensuring sidecar resource files exist for Tauri bundling"
  mkdir -p "${APP_DIR}/resources"

  local files=(
    "yt-dlp-mac"
    "yt-dlp-win.exe"
    "yt-dlp-linux"
    "ffmpeg-mac"
    "ffmpeg-win.exe"
    "ffmpeg-linux"
  )

  local file
  for file in "${files[@]}"; do
    local target="${APP_DIR}/resources/${file}"
    if [[ ! -f "${target}" ]]; then
      printf "placeholder: replace with real binary\n" > "${target}"
    fi
  done

  chmod +x \
    "${APP_DIR}/resources/yt-dlp-mac" \
    "${APP_DIR}/resources/yt-dlp-linux" \
    "${APP_DIR}/resources/ffmpeg-mac" \
    "${APP_DIR}/resources/ffmpeg-linux" || true
}

build_backend_and_extension() {
  mkdir -p "$DIST_DIR"

  log "Running cargo check"
  (
    cd "$APP_DIR"
    cargo check
  )

  log "Building Tauri desktop bundles"
  (
    cd "$APP_DIR"
    cargo tauri build
  )

  log "Packaging extension zip"
  local ext_zip="$DIST_DIR/clipdrop-extension.zip"
  rm -f "$ext_zip"
  (
    cd "$EXT_DIR"
    zip -r "$ext_zip" . -x "*.DS_Store" "*/.DS_Store"
  )

  log "Build complete"
  log "Desktop bundles: $APP_DIR/target/release/bundle"
  log "Extension zip: $ext_zip"
}

main() {
  require_cmd apt-get
  require_cmd apt-cache
  require_cmd curl
  require_cmd zip
  require_cmd python3

  ensure_root_or_sudo
  install_apt_deps
  install_rust_if_missing
  source_cargo_env

  require_cmd cargo
  require_cmd rustc

  install_tauri_cli_if_missing
  install_latest_ytdlp_sidecar
  ensure_rgba_icon
  ensure_sidecar_placeholders
  build_backend_and_extension
}

main "$@"
