#!/usr/bin/env bash
# Remove bootleg16 (the game and its menu entry). Your settings in ~/.config/bootleg16 stay
# unless you pass --purge.
set -euo pipefail
prefix="${PREFIX:-$HOME/.local/share/bootleg16}"
bin="${BINDIR:-$HOME/.local/bin}"
apps="${APPDIR:-$HOME/.local/share/applications}"
icons="${ICONDIR:-$HOME/.local/share/icons/hicolor/512x512/apps}"
rm -rf "$prefix"
rm -f "$bin/bootleg16" "$apps/bootleg16.desktop" "$icons/bootleg16.png"
[ "${1:-}" = "--purge" ] && rm -rf "$HOME/.config/bootleg16"
echo "bootleg16 removed."
