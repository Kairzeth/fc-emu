#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROFILE="${1:-debug}"
BIN_NAME="${2:-fc-emu}"
APP_NAME="${3:-$BIN_NAME}"
APP_ID_NAME="${APP_NAME//_/-}"
TARGET_DIR="$ROOT_DIR/target/$PROFILE"
APP_DIR="$ROOT_DIR/dist/$APP_NAME.app"
CONTENTS_DIR="$APP_DIR/Contents"
MACOS_DIR="$CONTENTS_DIR/MacOS"
RESOURCES_DIR="$CONTENTS_DIR/Resources"
BINARY="$TARGET_DIR/$BIN_NAME"
ROM_NAME="Super Mario Bros. (Japan, USA).nes"

if [[ "$PROFILE" == "release" ]]; then
  cargo build --release --bin "$BIN_NAME" --manifest-path "$ROOT_DIR/Cargo.toml"
else
  cargo build --bin "$BIN_NAME" --manifest-path "$ROOT_DIR/Cargo.toml"
fi

if [[ ! -x "$BINARY" ]]; then
  echo "missing executable: $BINARY" >&2
  exit 1
fi

install -d "$MACOS_DIR" "$RESOURCES_DIR/rom"
install -m 755 "$BINARY" "$MACOS_DIR/$BIN_NAME"

if [[ -f "$ROOT_DIR/rom/$ROM_NAME" ]]; then
  install -m 644 "$ROOT_DIR/rom/$ROM_NAME" "$RESOURCES_DIR/rom/$ROM_NAME"
fi

cat > "$CONTENTS_DIR/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>__BIN_NAME__</string>
  <key>CFBundleIdentifier</key>
  <string>local.__APP_ID_NAME__</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>__APP_NAME__</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>0.1.0</string>
  <key>CFBundleVersion</key>
  <string>1</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
PLIST
perl -0pi -e "s/__BIN_NAME__/$BIN_NAME/g; s/__APP_NAME__/$APP_NAME/g; s/__APP_ID_NAME__/$APP_ID_NAME/g" "$CONTENTS_DIR/Info.plist"

printf 'APPL????' > "$CONTENTS_DIR/PkgInfo"
xattr -cr "$APP_DIR" 2>/dev/null || true

echo "$APP_DIR"
