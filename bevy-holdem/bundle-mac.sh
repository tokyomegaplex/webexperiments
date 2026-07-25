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

# --- shrink the bundled assets (the source files in ./assets are left alone) ---
BUNDLE_ASSETS="$APP/Contents/MacOS/assets"

# Character art is shipped at ~2k px but only drawn as small billboards. Cap the
# bundled copies at 1024px wide (preserving aspect) — typically ~10x smaller.
echo "==> Downscaling bundled character art to <=1024px wide ..."
while IFS= read -r -d '' p; do
    w="$(sips -g pixelWidth "$p" 2>/dev/null | awk '/pixelWidth/{print $2}')"
    if [[ -n "$w" && "$w" -gt 1024 ]]; then
        sips --resampleWidth 1024 "$p" >/dev/null 2>&1
    fi
done < <(find "$BUNDLE_ASSETS/characters" -iname '*.png' -print0)

# Music is uncompressed WAV (~10x bigger than OGG). If ffmpeg is installed,
# convert the bundled copies to OGG (Bevy plays both); otherwise leave WAV.
if command -v ffmpeg >/dev/null 2>&1; then
    echo "==> Converting bundled music to OGG (ffmpeg found) ..."
    while IFS= read -r -d '' w; do
        if ffmpeg -y -i "$w" -c:a libvorbis -q:a 4 "${w%.wav}.ogg" >/dev/null 2>&1; then
            rm -f "$w"
        fi
    done < <(find "$BUNDLE_ASSETS/music" -iname '*.wav' -print0)
else
    echo "==> (ffmpeg not found — leaving music as WAV. 'brew install ffmpeg' to shrink it.)"
fi

du -sh "$BUNDLE_ASSETS" 2>/dev/null | awk '{print "    bundled assets now: "$1}'

# App icon: convert a PNG -> AppIcon.icns (needs macOS `sips` + `iconutil`).
# Find the source flexibly (any *.png in assets whose name has "icon").
ICON_SRC=""
if [[ -f "assets/pokermon icon.png" ]]; then
    ICON_SRC="assets/pokermon icon.png"
else
    ICON_SRC="$(find assets -maxdepth 1 -iname '*icon*.png' | head -n1 || true)"
fi
ICON_NAME=""
if [[ -n "$ICON_SRC" && -f "$ICON_SRC" ]]; then
    echo "==> Building app icon from: $ICON_SRC"
    mkdir -p "$APP/Contents/Resources"
    ICONSET="$(mktemp -d)/AppIcon.iconset"
    mkdir -p "$ICONSET"
    # iconutil requires EXACTLY these names — any extra/missing file makes it
    # reject the whole iconset, which is why no icon was produced before.
    gen() { sips -z "$1" "$1" "$ICON_SRC" --out "$ICONSET/$2" >/dev/null 2>&1; }
    gen 16   icon_16x16.png
    gen 32   icon_16x16@2x.png
    gen 32   icon_32x32.png
    gen 64   icon_32x32@2x.png
    gen 128  icon_128x128.png
    gen 256  icon_128x128@2x.png
    gen 256  icon_256x256.png
    gen 512  icon_256x256@2x.png
    gen 512  icon_512x512.png
    gen 1024 icon_512x512@2x.png
    if iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"; then
        ICON_NAME="AppIcon"
        echo "    -> Contents/Resources/AppIcon.icns"
    else
        echo "    !! iconutil failed — check that $ICON_SRC is a valid PNG."
    fi
    rm -rf "$(dirname "$ICONSET")"
else
    echo "==> (No icon PNG found in assets/ — skipping. Add 'pokermon icon.png'.)"
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

# Nudge Finder to refresh the icon (it caches aggressively, so a freshly-built
# .app often shows a blank/old icon until the cache is poked).
touch "$APP"
killall Finder >/dev/null 2>&1 || true

echo "==> Done: $APP"
if [[ -n "$ICON_NAME" ]]; then
    echo "   Icon:   embedded ($ICON_SRC)."
    echo "           If Finder still shows the old icon, move the .app to another"
    echo "           folder and back, or log out/in (macOS icon cache)."
fi
echo "   Test it:    open \"$APP\""
echo "   To share:   zip it (right-click -> Compress) and send the .zip."
echo "   Note: an Apple-Silicon build won't run on Intel Macs (or vice versa)."
