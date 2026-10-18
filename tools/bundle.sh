#!/usr/bin/env bash
# A personal offline copy of the game for your own other machine: the release build plus the
# game data (gamedata). It holds Valve's files: never publish or share it.
#
#   cargo build --release -p client --no-default-features --features steam
#   tools/bundle.sh              -> dist/bootleg16-linux.tar.gz
#
# On the other machine: unpack anywhere, run ./play.sh (Practice > Solo works offline).
set -euo pipefail
cd "$(dirname "$0")/.."
game="${BOOTLEG16_DIR:-gamedata}"
[ -x target/release/client ] || { echo "Build the release first (see the top of this file)."; exit 1; }
[ -d "$game/maps" ] || { echo "No game data at $game (set BOOTLEG16_DIR)."; exit 1; }
out=dist/bootleg16
rm -rf "$out"
mkdir -p "$out/data"
cp target/release/client "$out/bootleg16"
# Smaller without debug symbols (crash traces lose line numbers; fine for a test copy).
command -v strip >/dev/null && strip "$out/bootleg16"
[ -f target/release/libsteam_api.so ] && cp target/release/libsteam_api.so "$out/"
# Opus (voice chat) isn't on every Linux: bring ours (the game looks next to itself first).
opus=$(ldd target/release/client | awk '/libopus/ {print $3}')
[ -n "$opus" ] && cp "$opus" "$out/"
# The game data (the game finds `data/` next to itself).
cp -r "$game"/. "$out/data/"
cat > "$out/play.sh" <<'RUN'
#!/usr/bin/env bash
# Start bootleg16 with its data next to it.
here="$(cd "$(dirname "$0")" && pwd)"
BOOTLEG16_DIR="$here/data" exec "$here/bootleg16" "$@"
RUN
chmod +x "$out/play.sh" "$out/bootleg16"
tar czf dist/bootleg16-linux.tar.gz -C dist bootleg16
echo "dist/bootleg16-linux.tar.gz ($(du -h dist/bootleg16-linux.tar.gz | cut -f1)): copy it over, unpack, run ./play.sh"
