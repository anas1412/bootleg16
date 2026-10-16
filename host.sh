#!/usr/bin/env bash
# Host bootleg16: `./host.sh` (or `./host.sh <public-ip>`). Sets everything up the first time
# (game files, secret token, configs) and starts the lobby + game server in Docker.
# `./host.sh stop` stops them, `./host.sh logs` shows what they're doing.
set -euo pipefail
cd "$(dirname "$0")"

case "${1:-}" in
  stop) exec docker compose down ;;
  logs) exec docker compose logs -f ;;
esac

command -v docker >/dev/null || { echo "Install Docker first: https://docs.docker.com/engine/install/"; exit 1; }
mkdir -p hosting

# 1. The few game files the server reads, copied from your own CS 1.6 install (never shipped).
game="${BOOTLEG16_DIR:-$HOME/.steam/steam/steamapps/common/Half-Life}"
files=(cstrike/maps/de_dust2.bsp cstrike/models/player/arctic/arctic.mdl cstrike/sound/materials.txt)
for f in "${files[@]}"; do
  if [ ! -f "hosting/game/$f" ]; then
    [ -f "$game/$f" ] || { echo "Can't find $game/$f. Point BOOTLEG16_DIR at your Half-Life folder (the one with cstrike/ inside)."; exit 1; }
    mkdir -p "hosting/game/$(dirname "$f")"
    cp "$game/$f" "hosting/game/$f"
  fi
done

# 2. The secret that lets the lobby book this server (made once, kept).
[ -f hosting/tokens.txt ] || od -An -tx1 -N16 /dev/urandom | tr -d ' \n' > hosting/tokens.txt
token=$(cat hosting/tokens.txt)

# 3. The address players connect to: given, or this machine's public IP.
ip="${1:-}"
if [ -z "$ip" ]; then
  ip=$(curl -4fsS --max-time 5 https://api.ipify.org || true)
  [ -n "$ip" ] || { echo "Couldn't find your public IP. Run: ./host.sh <your-public-ip>"; exit 1; }
fi

# 4. Server config: the shipped defaults, pointed at our lobby.
sed -e 's|^master = ""|master = "127.0.0.1:27010"|' \
    -e "s|^official_token = \"\"|official_token = \"$token\"|" \
    -e "s|^public_address = \"\"|public_address = \"$ip:27015\"|" \
    config/server.cfg > hosting/server1.cfg

docker compose up -d --build
echo
echo "Hosting on $ip. Open UDP ports 27010 (lobby) and 27015 (game) in your firewall / router."
echo "Players start the game with:  BOOTLEG16_LOBBY=$ip:27010 client"
