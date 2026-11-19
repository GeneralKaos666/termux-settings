#!/bin/sh
# Wrap an already-built Android binary in a Termux-installable .deb.
#
# Usage: build-deb.sh <version> <arch> <binary> <output-dir>
#
# <arch> is the Termux name, `aarch64` or `arm`, not the Rust target triple.
# The binary is installed at the Termux prefix, so `pkg install ./file.deb`
# puts it in $PREFIX/bin and `pkg remove termux-settings` takes it away again.
set -eu
# The staged tree is packaged with whatever modes it has, and dpkg-deb refuses a
# control directory that is not group- or world-readable.
umask 022

usage() {
  printf 'usage: build-deb.sh <version> <arch> <binary> <output-dir>\n' >&2
  printf '  arch: aarch64 or arm\n' >&2
  exit 2
}

[ "$#" -eq 4 ] || usage
version=$1
arch=$2
binary=$3
outdir=$4

case $version in
'' | *[!0-9A-Za-z.+~-]*) printf 'invalid version: %s\n' "$version" >&2; exit 2 ;;
esac

case $arch in
aarch64 | arm) ;;
*) printf 'invalid arch: %s (expected aarch64 or arm)\n' "$arch" >&2; exit 2 ;;
esac

[ -f "$binary" ] || {
  printf 'no such binary: %s\n' "$binary" >&2
  exit 2
}

command -v dpkg-deb >/dev/null 2>&1 || {
  printf 'dpkg-deb is required to build packages\n' >&2
  exit 2
}

work=$(mktemp -d "${TMPDIR:-/tmp}/termux-settings-deb.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM

script_dir=$(CDPATH='' cd "$(dirname "$0")" && pwd)
license="$script_dir/../LICENSE"
[ -f "$license" ] || {
  printf 'no such license file: %s\n' "$license" >&2
  exit 2
}

root="$work/termux-settings"
docdir="$root/data/data/com.termux/files/usr/share/doc/termux-settings"
mkdir -p "$root/DEBIAN" "$root/data/data/com.termux/files/usr/bin" "$docdir"
install -m 755 "$binary" "$root/data/data/com.termux/files/usr/bin/termux-settings"
# Debian keeps the license at share/doc/<package>/copyright; Termux follows the
# same layout, so `pkg` installs leave the MIT text on the device.
install -m 644 "$license" "$docdir/copyright"

size_kb=$((($(wc -c <"$binary") + 1023) / 1024))
cat >"$root/DEBIAN/control" <<EOF
Package: termux-settings
Version: $version
Architecture: $arch
Maintainer: Ronald Sloan (for termux) <xkiller1177@gmail.com>
Installed-Size: $size_kb
Depends: termux-tools
Section: utils
Priority: optional
Homepage: https://github.com/GeneralKaos666/termux-settings
Description: keyboard-driven TUI for Termux colours, app theme and fonts
 Change Termux terminal colours, the app theme and fonts from a
 keyboard-driven terminal UI.
EOF

mkdir -p "$outdir"
deb="$outdir/termux-settings_${version}_${arch}.deb"
dpkg-deb --build --root-owner-group "$root" "$deb" >/dev/null
printf '%s\n' "$deb"
