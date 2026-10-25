#!/usr/bin/env bash
# Build the current checkout into a self-contained Tauri app before opening it.
set -euo pipefail

MODE="${1:-run}"
case "$MODE" in
  run|--verify|--debug|--logs|--telemetry) ;;
  *) printf 'Usage: %s [run|--verify|--debug|--logs|--telemetry]\n' "$0" >&2; exit 2 ;;
esac

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT_DIR"
APP_NAME="$(node -p "require('./src-tauri/tauri.conf.json').productName")"
APP_BUNDLE="$ROOT_DIR/src-tauri/target/debug/bundle/macos/$APP_NAME.app"
APP_BINARY="$APP_BUNDLE/Contents/MacOS/ielts-writing"

if pgrep -x ielts-writing >/dev/null; then
  printf '请先从 %s 应用内退出，让草稿保存完成，再重新运行。\n' "$APP_NAME" >&2
  exit 1
fi

# Prefer the normal Rust installation; also support this Mac's existing toolchain.
if ! command -v cargo >/dev/null 2>&1; then
  if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  else
    case "$(uname -m)" in arm64) RUST_HOST=aarch64-apple-darwin ;; x86_64) RUST_HOST=x86_64-apple-darwin ;; *) RUST_HOST=unsupported ;; esac
    TOOLCHAIN_DIR="/private/tmp/ielts-rustup/toolchains/stable-$RUST_HOST/bin"
    if [[ ! -x "$TOOLCHAIN_DIR/cargo" ]]; then
      printf '未找到 Rust 工具链，请先安装 Rust 后重试。\n' >&2
      exit 1
    fi
    export PATH="$TOOLCHAIN_DIR:$PATH"
    export CARGO_HOME="${CARGO_HOME:-/private/tmp/ielts-cargo}"
    export RUSTUP_HOME="${RUSTUP_HOME:-/private/tmp/ielts-rustup}"
  fi
fi

if [[ ! -x node_modules/.bin/tauri || package-lock.json -nt node_modules/.package-lock.json ]]; then
  npm ci
fi

# Frontend builds alone do not update an already packaged .app.
export CARGO_TARGET_DIR="$ROOT_DIR/src-tauri/target"
npm run tauri -- build --debug --bundles app
[[ -x "$APP_BINARY" ]] || { printf '构建未生成预期的应用包：%s\n' "$APP_BUNDLE" >&2; exit 1; }
printf '已构建提交 %s：%s\n' "$(git rev-parse --short HEAD)" "$APP_BUNDLE"

# Use the same installed app for Finder, Dock, and future development updates.
INSTALLED_APP="$("$ROOT_DIR/script/install_app.sh" "$APP_BUNDLE")"
printf '已安装：%s\n' "$INSTALLED_APP"

if [[ "$MODE" == --debug ]]; then
  exec lldb -- "$INSTALLED_APP/Contents/MacOS/ielts-writing"
fi
/usr/bin/open -n "$INSTALLED_APP"
case "$MODE" in
  --verify)
    for attempt in {1..25}; do
      if pgrep -x ielts-writing >/dev/null; then
        printf '新版应用已启动。\n'
        exit 0
      fi
      sleep 0.2
    done
    printf '构建成功，但未检测到应用进程。\n' >&2
    exit 1
    ;;
  --logs) exec /usr/bin/log stream --info --style compact --predicate 'process == "ielts-writing"' ;;
  --telemetry) exec /usr/bin/log stream --info --style compact --predicate 'subsystem == "com.local.ieltswriting"' ;;
esac
