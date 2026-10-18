#!/usr/bin/env bash
# Install bootleg16 for this user: the game in ~/.local/share/bootleg16, a `bootleg16` command
# in ~/.local/bin and a menu entry. No root. PREFIX=/some/dir to put the game elsewhere.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
prefix="${PREFIX:-$HOME/.local/share/bootleg16}"
bin="${BINDIR:-$HOME/.local/bin}"
apps="${APPDIR:-$HOME/.local/share/applications}"
icons="${ICONDIR:-$HOME/.local/share/icons/hicolor/512x512/apps}"

[ -x "$here/bootleg16/bootleg16" ] || { echo "Run this from the unpacked folder (bootleg16/ next to install.sh)."; exit 1; }
mkdir -p "$prefix" "$bin" "$apps" "$icons"
rm -rf "$prefix"
mkdir -p "$prefix"
cp -r "$here/bootleg16/." "$prefix/"
cp "$here/bootleg16/bootleg16.png" "$icons/bootleg16.png"

cat > "$bin/bootleg16" <<RUN
#!/usr/bin/env bash
# Start bootleg16 with its data next to it.
export BOOTLEG16_DIR="$prefix/data"
export LD_LIBRARY_PATH="$prefix\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}"
exec "$prefix/bootleg16" "\$@"
RUN
chmod +x "$bin/bootleg16"

cat > "$apps/bootleg16.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=bootleg16
Comment=Team shooter
Exec=$bin/bootleg16
Icon=bootleg16
Terminal=false
Categories=Game;ActionGame;
DESK

command -v update-desktop-database >/dev/null && update-desktop-database "$apps" 2>/dev/null || true
echo "Installed to $prefix. Start it from your menu or run: bootleg16"
case ":$PATH:" in *":$bin:"*) ;; *) echo "(add $bin to your PATH to use the command)";; esac
