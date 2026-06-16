#!/usr/bin/env bash
# Assemble, sign, notarize, staple and DMG-package XFChess.app on macOS.
# Run on a macOS runner after `cargo build --release` has produced the binaries.
#
# Required env (see docs/DISTRIBUTION.md):
#   APPLE_SIGNING_IDENTITY   "Developer ID Application: Name (TEAMID)"
#   APPLE_ID, APPLE_PASSWORD (app-specific), APPLE_TEAM_ID
# Optional:
#   APP_VERSION (default 0.1.0)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
VERSION="${APP_VERSION:-0.1.0}"
APP="XFChess.app"
STAGE="$ROOT/release/mac"
APPDIR="$STAGE/$APP"
ENTITLEMENTS="$ROOT/tauri/macos/entitlements.plist"
TARGET="$ROOT/target/release"

echo "==> Assembling $APP"
rm -rf "$STAGE"
mkdir -p "$APPDIR/Contents/MacOS" "$APPDIR/Contents/Resources"

# Main executable is the game; the wallet bridge + stockfish ship alongside it.
cp "$TARGET/xfchess"        "$APPDIR/Contents/MacOS/xfchess"
cp "$TARGET/xfchess-tauri"  "$APPDIR/Contents/MacOS/xfchess-tauri"
[ -f "$ROOT/stockfish" ] && cp "$ROOT/stockfish" "$APPDIR/Contents/MacOS/stockfish" || true
cp -R "$ROOT/assets" "$APPDIR/Contents/Resources/assets"
cp "$ROOT/tauri/icons/icon.icns" "$APPDIR/Contents/Resources/icon.icns" 2>/dev/null || \
  echo "WARN: tauri/icons/icon.icns missing — generate one with 'iconutil' for a proper Dock icon"

# Launcher: starts the wallet bridge, then the game. Mirrors the Windows launch.bat.
cat > "$APPDIR/Contents/MacOS/launch" <<'EOF'
#!/bin/bash
DIR="$(cd "$(dirname "$0")" && pwd)"
export BACKEND_URL="${BACKEND_URL:-https://api.xfchess.com}"
export SIGNING_SERVICE_URL="${SIGNING_SERVICE_URL:-https://api.xfchess.com}"
"$DIR/xfchess-tauri" &
exec "$DIR/xfchess"
EOF
chmod +x "$APPDIR/Contents/MacOS/launch"

cat > "$APPDIR/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>XFChess</string>
  <key>CFBundleDisplayName</key><string>XFChess</string>
  <key>CFBundleIdentifier</key><string>com.xfchess.app</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleExecutable</key><string>launch</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>10.15</string>
  <key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF

if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "==> No APPLE_SIGNING_IDENTITY set — skipping sign/notarize (unsigned .app, dev only)"
else
  echo "==> Codesigning (hardened runtime) inner binaries first, then the bundle"
  for bin in stockfish xfchess-tauri xfchess; do
    [ -f "$APPDIR/Contents/MacOS/$bin" ] && \
      codesign --force --options runtime --timestamp \
        --entitlements "$ENTITLEMENTS" \
        --sign "$APPLE_SIGNING_IDENTITY" "$APPDIR/Contents/MacOS/$bin"
  done
  codesign --force --options runtime --timestamp \
    --entitlements "$ENTITLEMENTS" \
    --sign "$APPLE_SIGNING_IDENTITY" "$APPDIR"
  codesign --verify --deep --strict --verbose=2 "$APPDIR"
fi

echo "==> Building DMG"
DMG="$ROOT/release/XFChess-${VERSION}.dmg"
rm -f "$DMG"
hdiutil create -volname "XFChess" -srcfolder "$APPDIR" -ov -format UDZO "$DMG"

if [ -n "${APPLE_ID:-}" ] && [ -n "${APPLE_PASSWORD:-}" ] && [ -n "${APPLE_TEAM_ID:-}" ]; then
  echo "==> Notarizing $DMG"
  xcrun notarytool submit "$DMG" \
    --apple-id "$APPLE_ID" --password "$APPLE_PASSWORD" --team-id "$APPLE_TEAM_ID" \
    --wait
  echo "==> Stapling"
  xcrun stapler staple "$DMG"
  xcrun stapler validate "$DMG"
else
  echo "==> Apple notarization creds not set — DMG is signed but NOT notarized"
fi

echo "==> Done: $DMG"
