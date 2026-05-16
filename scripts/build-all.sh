#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="${ROOT_DIR}/app/src-tauri"
DIST_DIR="${ROOT_DIR}/dist"
EXT_DIR="${ROOT_DIR}/extension"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found. Install Rust first." >&2
  exit 1
fi

mkdir -p "$DIST_DIR"

echo "[1/3] Compile-check backend"
(
  cd "$APP_DIR"
  cargo check
)

echo "[2/3] Build Tauri desktop bundle"
(
  cd "$APP_DIR"
  cargo tauri build
)

echo "[3/3] Package extension"
EXT_ZIP="$DIST_DIR/ferry-extension.zip"
rm -f "$EXT_ZIP"
(
  cd "$EXT_DIR"
  zip -r "$EXT_ZIP" . -x "*.DS_Store" "*/.DS_Store"
)

echo "Build artifacts"
echo "- Tauri bundles: $APP_DIR/target/release/bundle"
echo "- Extension zip: $EXT_ZIP"
