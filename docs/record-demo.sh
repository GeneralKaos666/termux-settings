#!/bin/sh
# Rebuild docs/demo.gif from a scripted TUI session.
#
# VHS needs a Chromium that Termux does not ship, so this uses termtosvg's
# still-frame mode and converts the frames with rsvg-convert and ImageMagick.
# Requires: termtosvg, rsvg-convert, convert (or magick), a built binary.
set -eu

bin=${1:-./target/debug/termux-settings}
out=${2:-docs/demo.gif}
work="$HOME/.tmp/opencode/termux-settings-demo"
frames="docs/.demo-frames.$$"

for tool in termtosvg rsvg-convert convert; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "record-demo: missing $tool" >&2
        exit 1
    fi
done
if [ ! -x "$bin" ]; then
    echo "record-demo: missing binary: $bin (run cargo build first)" >&2
    exit 1
fi

trap 'rm -rf "$work" "$frames"' EXIT INT TERM
rm -rf "$work" "$frames"
mkdir -p "$work" "$frames"

# Keys: open Preset Themes, walk three themes, apply one, go back, open
# Terminal Colors, then quit. Each pause keeps a frame readable.
{
    sleep 1.2
    printf '\r'
    sleep 1.5
    printf '\033[B'
    sleep 0.8
    printf '\033[B'
    sleep 0.8
    printf '\033[B'
    sleep 0.8
    printf '\r'
    sleep 1.2
    printf '\033'
    sleep 0.6
    printf '\033[B'
    sleep 0.4
    printf '\r'
    sleep 1.6
    printf '\033'
    sleep 0.5
    printf 'q'
    sleep 0.5
} | TERMUX_HOME="$work" timeout 60 termtosvg -s "$frames" \
    -c "$bin" -g 100x30 >/dev/null

for frame in "$frames"/*.svg; do
    rsvg-convert -w 1000 "$frame" -o "${frame%.svg}.png"
done
convert -delay 12 -loop 0 "$frames"/*.png "$out"

echo "record-demo: wrote $out"
