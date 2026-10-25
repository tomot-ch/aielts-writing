#!/usr/bin/env bash
# Install the normal bundle to a stable, discoverable location. Never touches writing data.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
APP_NAME="$(cd "$ROOT_DIR" && node -p "require('./src-tauri/tauri.conf.json').productName")"
APP_ID="$(cd "$ROOT_DIR" && node -p "require('./src-tauri/tauri.conf.json').identifier")"
SOURCE_APP="${1:-$ROOT_DIR/src-tauri/target/debug/bundle/macos/$APP_NAME.app}"
[[ -d "$SOURCE_APP/Contents" ]] || { printf '应用包不存在：%s\n' "$SOURCE_APP" >&2; exit 1; }
SOURCE_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$SOURCE_APP/Contents/Info.plist")"
[[ "$SOURCE_ID" == "$APP_ID" ]] || { printf '只安装正式应用，不安装验收包。\n' >&2; exit 1; }
if pgrep -x ielts-writing >/dev/null; then
  printf '请先正常退出 %s，让草稿保存完成。\n' "$APP_NAME" >&2
  exit 1
fi

# Tauri debug builds may have only a linker signature. Keep valid existing signatures.
if ! /usr/bin/codesign --verify --deep --strict "$SOURCE_APP" >/dev/null 2>&1; then
  /usr/bin/codesign --force --sign - --identifier "$APP_ID" --timestamp=none "$SOURCE_APP"
fi
/usr/bin/codesign --verify --deep --strict "$SOURCE_APP"

INSTALL_DIR=/Applications
if [[ ! -w "$INSTALL_DIR" ]]; then INSTALL_DIR="$HOME/Applications"; mkdir -p "$INSTALL_DIR"; fi
INSTALLED_APP="$INSTALL_DIR/$APP_NAME.app"
if [[ -e "$INSTALLED_APP" ]]; then
  INSTALLED_ID="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$INSTALLED_APP/Contents/Info.plist")"
  [[ "$INSTALLED_ID" == "$APP_ID" ]] || { printf '同名应用的标识不同，保留已有应用：%s\n' "$INSTALLED_APP" >&2; exit 1; }
fi

STAGING_DIR="$(mktemp -d "$INSTALL_DIR/.aielts-install.XXXXXX")"
cleanup() {
  if [[ ! -e "$INSTALLED_APP" && -e "$STAGING_DIR/previous.app" ]]; then
    mv "$STAGING_DIR/previous.app" "$INSTALLED_APP"
  fi
  # Only remove this invocation's temporary installation directory.
  if [[ "$STAGING_DIR" == "$INSTALL_DIR"/.aielts-install.* ]]; then rm -rf -- "$STAGING_DIR"; fi
}
trap cleanup EXIT
/usr/bin/ditto "$SOURCE_APP" "$STAGING_DIR/new.app"
/usr/bin/codesign --verify --deep --strict "$STAGING_DIR/new.app"
if [[ -e "$INSTALLED_APP" ]]; then mv "$INSTALLED_APP" "$STAGING_DIR/previous.app"; fi
mv "$STAGING_DIR/new.app" "$INSTALLED_APP"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$INSTALLED_APP" >/dev/null
if [[ "$SOURCE_APP" != "$INSTALLED_APP" ]]; then
  /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -u "$SOURCE_APP" >/dev/null
fi
printf '%s\n' "$INSTALLED_APP"
