#!/usr/bin/env bash
# Linux installer: dist/bootleg16-<version>-linux.tar.gz holding the game, its data and
# install.sh / uninstall.sh (per user, no root). The game data is private: this is a private
# build until the data is our own (docs/TODO.md).
#
#   cargo build --release -p client --no-default-features --features steam
#   packaging/linux/build.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
game="${BOOTLEG16_DIR:-gamedata}"
version="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' client/Cargo.toml | head -n1)}"
[ -x target/release/client ] || { echo "Build the release first (see the top of this file)."; exit 1; }
[ -d "$game/maps" ] || { echo "No game data at $game (set BOOTLEG16_DIR)."; exit 1; }
name="bootleg16-$version-linux"
out="dist/$name"
rm -rf "$out"
mkdir -p "$out/bootleg16/data"
cp target/release/client "$out/bootleg16/bootleg16"
command -v strip >/dev/null && strip "$out/bootleg16/bootleg16"
[ -f target/release/libsteam_api.so ] && cp target/release/libsteam_api.so "$out/bootleg16/"
# Opus (voice) isn't on every Linux: bring ours (the game looks next to itself first).
opus=$(ldd target/release/client | awk '/libopus/ {print $3}')
[ -n "$opus" ] && cp "$opus" "$out/bootleg16/"
cp -r "$game"/. "$out/bootleg16/data/"
cp packaging/bootleg16.png "$out/bootleg16/"
cp packaging/linux/install.sh packaging/linux/uninstall.sh packaging/linux/INSTALL.txt "$out/"
chmod +x "$out/install.sh" "$out/uninstall.sh" "$out/bootleg16/bootleg16"
tar czf "dist/$name.tar.gz" -C dist "$name"
echo "dist/$name.tar.gz ($(du -h "dist/$name.tar.gz" | cut -f1)): unpack and run ./install.sh"
