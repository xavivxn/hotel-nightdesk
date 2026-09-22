#!/usr/bin/env bash
# Cargo runner for `tauri dev` on macOS: run the debug binary from a fake .app
# so Dock and Cmd+Tab use productName instead of the crate name (`nightdesk`).
# Tests and other binaries pass through unchanged.
set -euo pipefail

BIN="${1:-}"
shift || true

if [[ -z "$BIN" || "$BIN" == *"/deps/"* ]]; then
  exec "$BIN" "$@"
fi

if [[ "$(basename "$BIN")" != "nightdesk" ]]; then
  exec "$BIN" "$@"
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONF="$ROOT/tauri.conf.json"
NAME="$(python3 - "$CONF" <<'PY'
import json, sys
print(json.load(open(sys.argv[1], encoding="utf-8"))["productName"])
PY
)"
IDENT="$(python3 - "$CONF" <<'PY'
import json, sys
print(json.load(open(sys.argv[1], encoding="utf-8"))["identifier"])
PY
)"
APP_DIR="$(dirname "$BIN")/${NAME}.app"
MACOS="$APP_DIR/Contents/MacOS"
RES="$APP_DIR/Contents/Resources"
EXEC="$MACOS/$NAME"

mkdir -p "$MACOS" "$RES"
ln -f "$BIN" "$EXEC" 2>/dev/null || cp -f "$BIN" "$EXEC"
chmod +x "$EXEC"

if [[ -f "$ROOT/icons/icon.icns" ]]; then
  cp -f "$ROOT/icons/icon.icns" "$RES/icon.icns"
fi

python3 - "$APP_DIR/Contents/Info.plist" "$NAME" "$IDENT" <<'PY'
import pathlib, sys, xml.sax.saxutils as x
path, name, ident = sys.argv[1], sys.argv[2], sys.argv[3]
esc = x.escape
pathlib.Path(path).write_text(f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>es</string>
  <key>CFBundleDisplayName</key><string>{esc(name)}</string>
  <key>CFBundleExecutable</key><string>{esc(name)}</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundleIdentifier</key><string>{esc(ident)}</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>CFBundleName</key><string>{esc(name)}</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>LSMinimumSystemVersion</key><string>10.13</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
""", encoding="utf-8")
PY

LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
if [[ -x "$LSREGISTER" ]]; then
  "$LSREGISTER" -f "$APP_DIR" >/dev/null 2>&1 || true
fi

exec "$EXEC" "$@"
