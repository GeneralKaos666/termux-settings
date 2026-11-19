#!/bin/sh
# Exercise scripts/build-deb.sh against the real dpkg-deb, so a release cannot
# publish a package with the wrong name, version, architecture or binary path.
set -eu

repo=$(CDPATH='' cd "$(dirname "$0")/../.." && pwd)
script="$repo/scripts/build-deb.sh"
work=$(mktemp -d "${TMPDIR:-/tmp}/build-deb-test.XXXXXX")
trap 'rm -rf "$work"' EXIT HUP INT TERM

fail() {
  printf 'not ok - %s\n' "$1" >&2
  exit 1
}

pass() {
  printf 'ok - %s\n' "$1"
}

# A stand-in binary; the package only has to carry it, not run it.
fake_bin="$work/fake-binary"
printf '\177ELF fake payload\n' >"$fake_bin"
chmod 755 "$fake_bin"

build() {
  arch=$1
  out="$work/out-$arch"
  "$script" 1.2.3 "$arch" "$fake_bin" "$out"
}

for arch in aarch64 arm; do
  deb=$(build "$arch") || fail "build-deb.sh exits 0 for $arch"
  [ "$deb" = "$work/out-$arch/termux-settings_1.2.3_$arch.deb" ] ||
    fail "prints the package path for $arch"
  [ -f "$deb" ] || fail "creates the package for $arch"

  [ "$(dpkg-deb --field "$deb" Package)" = termux-settings ] ||
    fail "Package is termux-settings for $arch"
  [ "$(dpkg-deb --field "$deb" Version)" = 1.2.3 ] ||
    fail "Version comes from the argument for $arch"
  [ "$(dpkg-deb --field "$deb" Architecture)" = "$arch" ] ||
    fail "Architecture is $arch"
  [ "$(dpkg-deb --field "$deb" Depends)" = termux-tools ] ||
    fail "Depends is termux-tools for $arch"
  [ "$(dpkg-deb --field "$deb" Section)" = utils ] ||
    fail "Section is utils for $arch"
  [ "$(dpkg-deb --field "$deb" Homepage)" = https://github.com/GeneralKaos666/termux-settings ] ||
    fail "Homepage points at the repository for $arch"
  [ -n "$(dpkg-deb --field "$deb" Description)" ] ||
    fail "Description is present for $arch"
  [ -n "$(dpkg-deb --field "$deb" Maintainer)" ] ||
    fail "Maintainer is present for $arch"
  case $(dpkg-deb --field "$deb" Installed-Size) in
  '' | *[!0-9]*) fail "Installed-Size is a number for $arch" ;;
  esac

  dpkg-deb --contents "$deb" >"$work/contents-$arch"
  grep -q ' \./data/data/com.termux/files/usr/bin/termux-settings$' "$work/contents-$arch" ||
    fail "installs \$PREFIX/bin/termux-settings for $arch"
  [ "$(grep -c 'termux-settings$' "$work/contents-$arch")" -eq 1 ] ||
    fail "ships exactly one copy of the binary for $arch"
  grep -q ' \./data/data/com.termux/files/usr/share/doc/termux-settings/copyright$' \
    "$work/contents-$arch" ||
    fail "installs the license as share/doc/termux-settings/copyright for $arch"
  dpkg-deb -x "$deb" "$work/extract-$arch"
  cmp -s "$repo/LICENSE" \
    "$work/extract-$arch/data/data/com.termux/files/usr/share/doc/termux-settings/copyright" ||
    fail "ships the repository LICENSE verbatim for $arch"

  pass "builds a valid $arch package"
done

# The version reaches the package from the tag, so it must be validated.
if "$script" '' aarch64 "$fake_bin" "$work/no-version" >/dev/null 2>&1; then
  fail "rejects an empty version"
fi
if "$script" 1.2.3 armv7 "$fake_bin" "$work/bad-arch" >/dev/null 2>&1; then
  fail "rejects an architecture Termux does not use"
fi
if "$script" 1.2.3 aarch64 "$work/absent" "$work/no-binary" >/dev/null 2>&1; then
  fail "rejects a missing binary"
fi
if "$script" 1.2.3 >/dev/null 2>&1; then
  fail "rejects missing arguments"
fi
pass "rejects bad input"

printf 'all build-deb tests passed\n'
