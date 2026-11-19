#!/bin/sh
# Drive the TUI through a real pseudo-terminal and quit immediately.
#
# The unit and integration tests render into a `TestBackend`, which never enters
# raw mode. This checks the crossterm lifecycle end to end: raw mode and the
# alternate screen are entered, a key is read, and the process exits cleanly.
set -eu

bin=${1:-target/debug/termux-settings}
if [ ! -x "$bin" ]; then
    echo "tui smoke test: missing binary: $bin" >&2
    exit 1
fi

work=$(mktemp -d "${TMPDIR:-/tmp}/termux-settings-tui.XXXXXX")
trap 'rm -rf "$work"' EXIT INT TERM
mkdir -p "$work/home" "$work/termux"

export HOME="$work/home"
export TERMUX_HOME="$work/termux"
export TERM=xterm-256color

# `script` allocates the pseudo-terminal; `-e` forwards the child's exit code.
printf 'q' | timeout 30 script -q -e -c "$bin" /dev/null >/dev/null

# Quitting from the menu must not write any configuration.
if [ -e "$TERMUX_HOME/colors.properties" ]; then
    echo "tui smoke test: quitting wrote colors.properties" >&2
    exit 1
fi

echo "tui smoke test: ok"
