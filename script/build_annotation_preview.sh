#!/usr/bin/env bash
# Independent identity and root configuration; never replaces the daily-use bundle.
set -euo pipefail
TASK_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$TASK_ROOT"
if ! command -v cargo >/dev/null 2>&1; then
  if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  else
    export PATH="/private/tmp/ielts-rustup/toolchains/stable-aarch64-apple-darwin/bin:$PATH"
    export CARGO_HOME="${CARGO_HOME:-/private/tmp/ielts-cargo}"
    export RUSTUP_HOME="${RUSTUP_HOME:-/private/tmp/ielts-rustup}"
  fi
fi
if [[ ! -x node_modules/.bin/tauri ]]; then npm ci; fi
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$TASK_ROOT/src-tauri/target}"
npm run tauri -- build --debug --bundles app --config src-tauri/tauri.annotation-preview.conf.json
PREVIEW_NAME="$(node -p "require('./src-tauri/tauri.annotation-preview.conf.json').productName")"
printf '批注预览包：%s/debug/bundle/macos/%s.app\n' "$CARGO_TARGET_DIR" "$PREVIEW_NAME"
