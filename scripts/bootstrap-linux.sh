#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="$ROOT_DIR/app/src-tauri"

log() { echo "[bootstrap-linux] $*"; }

have_cmd() { command -v "$1" >/dev/null 2>&1; }

ensure_sudo() {
  if [[ "${EUID}" -eq 0 ]]; then
    SUDO=""
  elif have_cmd sudo; then
    SUDO="sudo"
  else
    echo "error: run as root or install sudo" >&2
    exit 1
  fi
}

install_deps_apt() {
  local webkit_pkg="libwebkit2gtk-4.1-dev"
  if ! apt-cache show "$webkit_pkg" >/dev/null 2>&1; then
    webkit_pkg="libwebkit2gtk-4.0-dev"
  fi
  $SUDO apt-get update -y
  $SUDO apt-get install -y --no-install-recommends \
    build-essential curl file pkg-config zip ca-certificates \
    libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
    "$webkit_pkg" ffmpeg yt-dlp nodejs npm
}

install_deps_dnf() {
  $SUDO dnf install -y \
    curl file gcc gcc-c++ make pkgconf-pkg-config zip ca-certificates \
    gtk3-devel libappindicator-gtk3-devel librsvg2-devel webkit2gtk4.1-devel \
    ffmpeg yt-dlp nodejs npm
}

install_deps_pacman() {
  $SUDO pacman -Sy --noconfirm \
    curl file base-devel pkgconf zip ca-certificates \
    gtk3 libappindicator-gtk3 librsvg webkit2gtk-4.1 \
    ffmpeg yt-dlp nodejs npm
}

install_deps_zypper() {
  $SUDO zypper --non-interactive refresh
  $SUDO zypper --non-interactive install \
    curl file gcc gcc-c++ make pkg-config zip ca-certificates \
    gtk3-devel libappindicator3-devel librsvg-devel webkit2gtk3-devel \
    ffmpeg yt-dlp nodejs18 npm18
}

install_platform_deps() {
  if have_cmd apt-get; then
    install_deps_apt
  elif have_cmd dnf; then
    install_deps_dnf
  elif have_cmd pacman; then
    install_deps_pacman
  elif have_cmd zypper; then
    install_deps_zypper
  else
    echo "error: unsupported Linux package manager. Install Tauri + yt-dlp + ffmpeg deps manually." >&2
    exit 1
  fi
}

install_rust() {
  if have_cmd cargo && have_cmd rustc; then
    return
  fi
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
}

source_cargo() {
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
}

install_tauri_cli() {
  if cargo tauri --version >/dev/null 2>&1; then
    return
  fi
  cargo install tauri-cli --version '^2'
}

main() {
  ensure_sudo
  log "Installing Linux dependencies"
  install_platform_deps
  log "Installing Rust toolchain"
  install_rust
  source_cargo
  log "Installing Tauri CLI"
  install_tauri_cli
  log "Hydrating bundled yt-dlp / ffmpeg / ffprobe sidecars"
  "$ROOT_DIR/scripts/hydrate-sidecars-linux.sh"
  log "Verifying backend build"
  (cd "$APP_DIR" && cargo check)
  log "Done. Load extension from: $ROOT_DIR/extension"
  log "Run desktop app: cd $APP_DIR && cargo run"
}

main "$@"
