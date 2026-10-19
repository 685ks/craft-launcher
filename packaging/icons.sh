#!/usr/bin/env bash
# Regenerate every app and tray icon from the Arthavan Projects placeholder mark in
# assets/brand/ (original artwork, MIT OR Apache-2.0 like the code).
#
# - App icon (Dock, taskbar, Start menu, Linux menus, installers): the "AP" monogram in the
#   steel-blue accent on a dark rounded square (assets/brand/arthavan-projects-mark.svg).
# - Tray: the monogram alone in black as the macOS menu-bar template
#   (assets/brand/arthavan-projects-mark-template.svg); the full mark on Windows, Linux and BSD.
#
# Needs: resvg (brew install resvg / cargo install resvg). The .ico and .icns are packed by
# `cargo xtask ico` / `cargo xtask icns`, so this runs on any OS. The outputs are committed, so
# packaging (and the Rust build, which include_bytes!s some of them) never needs these tools.
#
#   packaging/icons.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="$ROOT/assets/app-icon"
BRAND="$ROOT/assets/brand"
SVG="$BRAND/arthavan-projects-mark.svg"
ID="app.freedev.ainz.ks.ArthavanProjects"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

command -v resvg >/dev/null || { echo "error: resvg not found (brew install resvg)" >&2; exit 1; }

# macOS icons get more transparent margin (Apple's 824/1024 icon grid) by widening the view box
# around the mark; everything else uses the file as is.
BOX='viewBox="0 0 128 128"'
grep -q "$BOX" "$SVG" || { echo "error: expected $BOX in $SVG" >&2; exit 1; }
MAC="$TMP/mac.svg"
sed "s/$BOX/viewBox=\"-12 -12 152 152\"/" "$SVG" >"$MAC"
FLAT="$SVG"

render() { resvg -w "$2" -h "$2" "$1" "$3" </dev/null; }

render "$MAC" 1024 "$DIR/arthavan-projects-1024.png"

# Linux hicolor theme.
for s in 16 24 32 48 64 128 256 512; do
  mkdir -p "$DIR/hicolor/${s}x${s}/apps"
  render "$FLAT" "$s" "$DIR/hicolor/${s}x${s}/apps/$ID.png"
done
mkdir -p "$DIR/hicolor/scalable/apps"
cp "$SVG" "$DIR/hicolor/scalable/apps/$ID.svg"
cp "$SVG" "$DIR/arthavan-projects.svg"

# Windows .ico.
ICO_PNGS=()
for s in 16 20 24 32 40 48 64 128 256; do
  render "$FLAT" "$s" "$TMP/ico-$s.png"
  ICO_PNGS+=("$TMP/ico-$s.png")
done
(cd "$ROOT" && cargo run -q -p xtask -- ico "$DIR/arthavan-projects.ico" "${ICO_PNGS[@]}")

# macOS .icns.
ICNS_PNGS=()
for s in 16 32 64 128 256 512 1024; do
  render "$MAC" "$s" "$TMP/icns-$s.png"
  ICNS_PNGS+=("$TMP/icns-$s.png")
done
(cd "$ROOT" && cargo run -q -p xtask -- icns "$DIR/arthavan-projects.icns" "${ICNS_PNGS[@]}")

# Tray icons: the black monogram is the macOS menu-bar template (44 px = 22 pt @2x; macOS tints
# it); the full mark is for the Windows notification area and Linux/BSD status notifiers.
mkdir -p "$DIR/tray"
render "$BRAND/arthavan-projects-mark-template.svg" 44 "$DIR/tray/template-44.png"
render "$SVG" 64 "$DIR/tray/mark-64.png"
echo "icons written to $DIR"
