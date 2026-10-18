#!/usr/bin/env bash
# Wrap the compiled binary into a macOS .app bundle.
#
#   ./scripts/bundle-macos.sh [target-triple] [profile]
#
# A bundle is not strictly required to run rectangle-rs, but it gives the app a
# stable code identity, which is what the Accessibility permission (TCC) is
# remembered against. Without it macOS may ask for permission again after every
# rebuild.
set -euo pipefail

BIN_NAME="rectangle-rs"
APP_NAME="rectangle-rs.app"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${1:-}"
PROFILE="${2:-release}"

cd "$ROOT"

if [[ -z "$TARGET" ]]; then
  TARGET="$(rustc -vV | sed -n 's/^host: //p')"
fi

if [[ -f "target/$TARGET/$PROFILE/$BIN_NAME" ]]; then
  BIN="target/$TARGET/$PROFILE/$BIN_NAME"
elif [[ -f "target/$PROFILE/$BIN_NAME" ]]; then
  BIN="target/$PROFILE/$BIN_NAME"
else
  echo "找不到编译产物：请先运行 cargo build --$PROFILE --target $TARGET" >&2
  exit 1
fi

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
VERSION="${VERSION:-0.0.0}"
APP="target/$APP_NAME"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/$BIN_NAME"
chmod +x "$APP/Contents/MacOS/$BIN_NAME"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>$BIN_NAME</string>
    <key>CFBundleDisplayName</key>
    <string>rectangle-rs</string>
    <key>CFBundleIdentifier</key>
    <string>dev.rectangle-rs.app</string>
    <key>CFBundleExecutable</key>
    <string>$BIN_NAME</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <!--
      Set LSUIElement to <true/> to hide the Dock icon and run as a pure menu bar
      app. Left disabled by default so the settings window behaves like a normal
      window on first launch.
    -->
    <key>LSUIElement</key>
    <false/>
</dict>
</plist>
PLIST

# Ad-hoc signature: enough for TCC to keep the Accessibility grant stable
# between runs of the same build. Distribution builds should use a Developer ID.
if command -v codesign >/dev/null 2>&1; then
  codesign --force --sign - "$APP" >/dev/null 2>&1 || echo "codesign 失败（可忽略，仅影响权限记忆）" >&2
fi

echo "已生成 $APP"
