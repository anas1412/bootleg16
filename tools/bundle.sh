#!/usr/bin/env bash
# A personal offline copy of the game for your own other machine: the release build plus the
# CS 1.6 files it reads, copied from YOUR install. Valve's files: never publish or share it.
#
#   cargo build --release -p client --no-default-features --features steam
#   tools/bundle.sh              -> dist/bootleg16-linux.tar.gz
#
# On the other machine: unpack anywhere, run ./play.sh (Practice > Solo works offline).
set -euo pipefail
cd "$(dirname "$0")/.."
game="${BOOTLEG16_DIR:-$HOME/.steam/steam/steamapps/common/Half-Life}"
[ -x target/release/client ] || { echo "Build the release first (see the top of this file)."; exit 1; }
[ -d "$game/cstrike" ] || { echo "No CS 1.6 at $game (set BOOTLEG16_DIR)."; exit 1; }
out=dist/bootleg16
rm -rf "$out"
mkdir -p "$out/Half-Life/cstrike" "$out/Half-Life/valve"
cp target/release/client "$out/bootleg16"
# Smaller without debug symbols (crash traces lose line numbers; fine for a test copy).
command -v strip >/dev/null && strip "$out/bootleg16"
[ -f target/release/libsteam_api.so ] && cp target/release/libsteam_api.so "$out/"
# Opus (voice chat) isn't on every Linux: bring ours (the game looks next to itself first).
opus=$(ldd target/release/client | awk '/libopus/ {print $3}')
[ -n "$opus" ] && cp "$opus" "$out/"
# Just what the game reads: maps (all of them), models, sounds, sprites, skies, radar
# images and textures.
for d in maps models sound sprites gfx overviews; do
  [ -d "$game/cstrike/$d" ] && cp -r "$game/cstrike/$d" "$out/Half-Life/cstrike/"
done
cp "$game"/cstrike/*.wad "$out/Half-Life/cstrike/"
cp -r "$game/valve/gfx" "$out/Half-Life/valve/"
cp "$game"/valve/*.wad "$out/Half-Life/valve/"
cat > "$out/play.sh" <<'RUN'
#!/usr/bin/env bash
# Start bootleg16 with the CS 1.6 files next to it.
here="$(cd "$(dirname "$0")" && pwd)"
BOOTLEG16_DIR="$here/Half-Life" exec "$here/bootleg16" "$@"
RUN
chmod +x "$out/play.sh" "$out/bootleg16"
tar czf dist/bootleg16-linux.tar.gz -C dist bootleg16
echo "dist/bootleg16-linux.tar.gz ($(du -h dist/bootleg16-linux.tar.gz | cut -f1)): copy it over, unpack, run ./play.sh"
