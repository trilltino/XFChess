#!/usr/bin/env bash
# Assemble, sign, notarize, staple and DMG-package XFChess.app on macOS.
# Run on a macOS runner after `cargo build --release` has produced the binaries.
#
# Required env (see docs/PUBLISHING.md):
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

if [ ! -f "$ROOT/stockfish" ]; then
  echo "ERROR: $ROOT/stockfish not found — a release build must ship with the engine bundled" >&2
  exit 1
fi
cp "$ROOT/stockfish" "$APPDIR/Contents/MacOS/stockfish"

cp -R "$ROOT/assets" "$APPDIR/Contents/Resources/assets"

# wallet-ui is served by xfchess-tauri itself from next to its own binary
# (see wallet_ui_dist_path in tauri/src/main.rs, resolved via current_exe()).
if [ ! -d "$ROOT/tauri/wallet-ui/dist" ]; then
  echo "ERROR: $ROOT/tauri/wallet-ui/dist not found — build it first: cd tauri/wallet-ui && npm run build" >&2
  exit 1
fi
mkdir -p "$APPDIR/Contents/MacOS/wallet-ui"
cp -R "$ROOT/tauri/wallet-ui/dist" "$APPDIR/Contents/MacOS/wallet-ui/dist"

if [ -f "$ROOT/tauri/icons/icon.icns" ]; then
  cp "$ROOT/tauri/icons/icon.icns" "$APPDIR/Contents/Resources/icon.icns"
else
  echo "==> No pre-made icon.icns checked in — generating one from tauri/icons/128x128.png"
  SRC_PNG="$ROOT/tauri/icons/128x128.png"
  [ -f "$SRC_PNG" ] || { echo "ERROR: neither tauri/icons/icon.icns nor tauri/icons/128x128.png found" >&2; exit 1; }
  ICONSET="$STAGE/icon.iconset"
  rm -rf "$ICONSET" && mkdir -p "$ICONSET"
  for size in 16 32 128 256 512; do
    double=$((size * 2))
    sips -z "$size" "$size" "$SRC_PNG" --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
    sips -z "$double" "$double" "$SRC_PNG" --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$ICONSET" -o "$APPDIR/Contents/Resources/icon.icns"
  rm -rf "$ICONSET"
fi

# Launcher: starts the wallet bridge, then the game. Mirrors the Windows launch.bat.
cat > "$APPDIR/Contents/MacOS/launch" <<'EOF'
#!/bin/bash
DIR="$(cd "$(dirname "$0")" && pwd)"
export BACKEND_URL="${BACKEND_URL:-https://xfchess.com}"
export SIGNING_SERVICE_URL="${SIGNING_SERVICE_URL:-https://xfchess.com}"
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
# An Applications symlink alongside the .app is what makes the mounted DMG
# show a drag-to-install target — without it the window contains only the
# app icon and there's nothing to drag it onto.
ln -sf /Applications "$STAGE/Applications"
hdiutil create -volname "XFChess" -srcfolder "$STAGE" -ov -format UDZO "$DMG"

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
