#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_DIR="${ROOT_DIR}/app/src-tauri"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found. Install Rust first." >&2
  exit 1
fi

echo "[1/5] Rust toolchain"
cargo --version
rustc --version

echo "[2/5] Backend compile check"
(
  cd "$APP_DIR"
  cargo check
)

echo "[3/5] Backend linting (clippy)"
(
  cd "$APP_DIR"
  cargo clippy -- -D warnings
)

echo "[4/5] Backend formatting check"
(
  cd "$APP_DIR"
  cargo fmt --check
)

echo "[5/5] Extension + smoke script syntax"
node --check "$ROOT_DIR/extension/content.js"
node --check "$ROOT_DIR/extension/background.js"
node --check "$ROOT_DIR/extension/popup.js"
node --check "$ROOT_DIR/extension/constants.js"
node --check "$ROOT_DIR/extension/api.js"
node --check "$ROOT_DIR/extension/runtime.js"
node --check "$ROOT_DIR/scripts/smoke-api.mjs"
node -e "JSON.parse(require('fs').readFileSync('$ROOT_DIR/extension/manifest.json', 'utf8')); console.log('manifest ok')"

echo ""
echo "✓ dev-check completed successfully"
