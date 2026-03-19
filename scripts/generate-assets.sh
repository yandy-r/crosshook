#!/usr/bin/env bash
# Regenerates all branding PNGs and ICO from SVG sources.
# Requires: rsvg-convert (librsvg2-bin) and convert or magick (imagemagick).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ASSETS="$ROOT_DIR/assets"
STEAM="$ASSETS/steam"

# Resolve ImageMagick command (IM7 uses "magick", IM6 uses "convert")
if command -v magick &>/dev/null; then
  IM=magick
elif command -v convert &>/dev/null; then
  IM=convert
else
  echo "ImageMagick not found (neither magick nor convert)" >&2
  exit 1
fi

# --- Monogram icon PNGs ---
for size in 16 32 48 256 512; do
  rsvg-convert -w "$size" -h "$size" "$ASSETS/logo-monogram.svg" \
    -o "$ASSETS/icon-${size}.png"
done

# --- Multi-resolution ICO ---
$IM "$ASSETS/icon-16.png" "$ASSETS/icon-32.png" \
    "$ASSETS/icon-48.png" "$ASSETS/icon-256.png" \
    "$ASSETS/crosshook.ico"

# --- Full logo PNG (for README) ---
rsvg-convert -w 960 "$ASSETS/logo-full.svg" -o "$ROOT_DIR/crosshook.png"

# --- Steam artwork PNGs ---
rsvg-convert -w 600  -h 900  "$STEAM/cover.svg"      -o "$STEAM/steam-cover.png"
rsvg-convert -w 3840 -h 1240 "$STEAM/background.svg" -o "$STEAM/steam-background.png"
rsvg-convert -w 1280 -h 720  "$STEAM/logo.svg"        -o "$STEAM/steam-logo.png"
rsvg-convert -w 920  -h 430  "$STEAM/wide-cover.svg"  -o "$STEAM/steam-wide-cover.png"

echo "All branding assets generated from SVG sources."
