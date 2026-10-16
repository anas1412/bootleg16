#!/usr/bin/env bash
# Run bootleg16's servers in Docker. See README "Hosting".
#
#   ./host.sh                         this machine: the lobby + game servers (one per CPU core)
#   ./host.sh join <lobby-ip> <token> this machine: game servers for an existing lobby
#   options: --servers N   game servers here (each runs one match at a time)
#            --ip X        the public IP players connect to (found automatically otherwise)
#            --region R    where this machine is, e.g. eu, na, sa, asia (players get the closest)
#   ./host.sh token | status | logs | update | stop
#
# Settings are remembered in hosting/: after the first run, plain `./host.sh` brings the
# same setup back up.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p hosting
compose() { docker compose -p bootleg16 -f hosting/compose.yaml "$@"; }

[ -f hosting/settings ] && . hosting/settings
LOBBY="${LOBBY:-here}" SERVERS="${SERVERS:-}" IP="${IP:-}" REGION="${REGION:-eu}"

case "${1:-}" in
  stop) compose down; exit ;;
  logs) compose logs -f --tail 50; exit ;;
  status) compose ps --format "table {{.Service}}\t{{.Status}}"; exit ;;
  token) cat hosting/tokens.txt; echo; exit ;;
  update) git pull --ff-only; shift ;;
  join)
    [ $# -ge 3 ] || { echo "Usage: ./host.sh join <lobby-ip> <token>"; exit 1; }
    LOBBY="$2"; echo -n "$3" > hosting/tokens.txt; shift 3 ;;
esac
while [ $# -gt 0 ]; do
  case "$1" in
    --servers) SERVERS="$2"; shift 2 ;;
    --ip) IP="$2"; shift 2 ;;
    --region) REGION="$2"; shift 2 ;;
    *) echo "Unknown option $1 (see the top of host.sh)"; exit 1 ;;
  esac
done

command -v docker >/dev/null || { echo "Install Docker first: https://docs.docker.com/engine/install/"; exit 1; }
SERVERS="${SERVERS:-$(nproc)}"

# The few game files the servers read, copied from your own CS 1.6 install (never shipped).
game="${BOOTLEG16_DIR:-$HOME/.steam/steam/steamapps/common/Half-Life}"
for f in cstrike/maps/de_dust2.bsp cstrike/models/player/arctic/arctic.mdl cstrike/sound/materials.txt; do
  if [ ! -f "hosting/game/$f" ]; then
    [ -f "$game/$f" ] || { echo "Missing $f. Set BOOTLEG16_DIR to your Half-Life folder (the one with cstrike/ in it), or copy hosting/game/ from a machine that has it."; exit 1; }
    mkdir -p "hosting/game/$(dirname "$f")"
    cp "$game/$f" "hosting/game/$f"
  fi
done

# The secret that lets the lobby book game servers (made once; `./host.sh token` shows it).
[ -s hosting/tokens.txt ] || od -An -tx1 -N16 /dev/urandom | tr -d ' \n' > hosting/tokens.txt
token=$(head -n1 hosting/tokens.txt)

if [ -z "$IP" ]; then
  IP=$(curl -4fsS --max-time 5 https://api.ipify.org || true)
  [ -n "$IP" ] || { echo "Couldn't find this machine's public IP: run again with --ip <address>."; exit 1; }
fi
printf 'LOBBY=%q\nSERVERS=%q\nIP=%q\nREGION=%q\n' "$LOBBY" "$SERVERS" "$IP" "$REGION" > hosting/settings

# One config per game server (the shipped rules, pointed at the lobby), and the compose file.
lobby_addr=$([ "$LOBBY" = here ] && echo 127.0.0.1:27010 || echo "$LOBBY:27010")
rm -f hosting/server*.cfg
{
  echo "# Made by host.sh: edit config/server.cfg instead, then run ./host.sh"
  echo "services:"
  if [ "$LOBBY" = here ]; then
    echo "  lobby: {image: bootleg16, network_mode: host, restart: unless-stopped, command: master 27010 --tokens /bootleg16/tokens.txt, volumes: [./tokens.txt:/bootleg16/tokens.txt:ro]}"
  fi
  for i in $(seq 1 "$SERVERS"); do
    port=$((27014 + i))
    sed -e "s|^master = \"\"|master = \"$lobby_addr\"|" \
        -e "s|^official_token = \"\"|official_token = \"$token\"|" \
        -e "s|^public_address = \"\"|public_address = \"$IP:$port\"|" \
        -e "s|^name = .*|name = \"$(hostname)-$i\"|" \
        -e "s|^region = .*|region = \"$REGION\"|" \
        config/server.cfg > "hosting/server$i.cfg"
    echo "  server$i: {image: bootleg16, network_mode: host, restart: unless-stopped, command: server de_dust2 $port --config /bootleg16/server.cfg, volumes: [./server$i.cfg:/bootleg16/server.cfg:ro, ./game:/game:ro]}"
  done
} > hosting/compose.yaml

docker build -q -t bootleg16 . >/dev/null
compose up -d --remove-orphans
last=$((27014 + SERVERS))
echo
echo "Running $SERVERS game server(s) on $IP (UDP $([ "$SERVERS" = 1 ] && echo 27015 || echo "27015-$last"))$([ "$LOBBY" = here ] && echo " and the lobby (UDP 27010)")."
echo "Open those UDP ports in your firewall / router."
if [ "$LOBBY" = here ]; then
  echo "More machines:  ./host.sh join $IP $token"
  echo "Ship the game:  BOOTLEG16_LOBBY=$IP:27010 cargo build --release -p client --no-default-features"
fi
