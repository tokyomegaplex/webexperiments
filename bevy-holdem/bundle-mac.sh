#!/usr/bin/env bash
# Build a self-contained macOS .app bundle for Pokermon Hold 'em.
#
#   ./bundle-mac.sh
#
# Produces "Pokermon Hold 'em.app" in this folder. Zip it and send it to a
# friend; they right-click -> Open the first time (it's unsigned, so Gatekeeper
# warns once). The .app carries its own assets, so it runs from anywhere.
set -euo pipefail
cd "$(dirname "$0")"

APP="Pokermon Hold 'em.app"
BIN="cartoon-holdem-bevy"

echo "==> Building release binary..."
cargo build --release

echo "==> Assembling $APP ..."
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"

# The binary and its assets live together in Contents/MacOS (main.rs chdirs here).
cp "target/release/$BIN" "$APP/Contents/MacOS/$BIN"
cp -R assets "$APP/Contents/MacOS/assets"

# App icon: convert assets/pokermon icon.png -> AppIcon.icns (needs macOS tools).
ICON_SRC="assets/pokermon icon.png"
ICON_NAME=""
if [[ -f "$ICON_SRC" ]]; then
    echo "==> Building app icon from $ICON_SRC ..."
    mkdir -p "$APP/Contents/Resources"
    ICONSET="$(mktemp -d)/AppIcon.iconset"
    mkdir -p "$ICONSET"
    for sz in 16 32 64 128 256 512; do
        sips -z $sz $sz       "$ICON_SRC" --out "$ICONSET/icon_${sz}x${sz}.png"     >/dev/null
        sips -z $((sz*2)) $((sz*2)) "$ICON_SRC" --out "$ICONSET/icon_${sz}x${sz}@2x.png" >/dev/null
    done
    iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"
    rm -rf "$(dirname "$ICONSET")"
    ICON_NAME="AppIcon"
else
    echo "==> (No $ICON_SRC found — skipping icon. Add it and re-run for a custom icon.)"
fi

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>            <string>Pokermon Hold 'em</string>
    <key>CFBundleDisplayName</key>     <string>Pokermon Hold 'em</string>
    <key>CFBundleIdentifier</key>      <string>com.tokyomegaplex.pokermonholdem</string>
    <key>CFBundleVersion</key>         <string>1.0</string>
    <key>CFBundleShortVersionString</key> <string>1.0</string>
    <key>CFBundlePackageType</key>     <string>APPL</string>
    <key>CFBundleExecutable</key>      <string>$BIN</string>
    <key>CFBundleIconFile</key>        <string>$ICON_NAME</string>
    <key>NSHighResolutionCapable</key> <true/>
    <key>LSMinimumSystemVersion</key>  <string>11.0</string>
</dict>
</plist>
PLIST

echo "==> Done: $APP"
echo "   Test it:    open \"$APP\""
echo "   To share:   zip it (right-click -> Compress) and send the .zip."
echo "   Note: an Apple-Silicon build won't run on Intel Macs (or vice versa)."
echo "         For a universal build, see the comment in Cargo notes."
