#!/usr/bin/env bash
# Run the lobby and game servers on this machine, no Docker (for playing and testing at home).
#
#   tools/host-local.sh start [--servers N] [--dm N]   lobby + N match servers + N deathmatch servers
#   tools/host-local.sh stop | status | logs
#
# Uses the release binaries if built (`cargo build --release -p server -p lobby`), else the debug
# ones. Game files come from BOOTLEG16_DIR (default gamedata). Everything it makes is in
# hosting/local/ (git-ignored). The lobby lets in players without Steam: for local use only,
# the game connects to it with `BOOTLEG16_LOBBY=127.0.0.1:27010`.
set -euo pipefail
cd "$(dirname "$0")/.."
run=hosting/local
mkdir -p "$run" hosting/keys hosting/reports hosting/demos
umask 077
cmd="${1:-start}"; shift || true
SERVERS=1 DM=1
while [ $# -gt 0 ]; do
  case "$1" in
    --servers) SERVERS="$2"; shift 2 ;;
    --dm) DM="$2"; shift 2 ;;
    *) echo "Unknown option $1"; exit 1 ;;
  esac
done

bin() { for p in release debug; do [ -x "target/$p/$1" ] && { echo "target/$p/$1"; return; }; done; echo "Build $1 first: cargo build -p $1" >&2; exit 1; }
alive() { [ -f "$run/$1.pid" ] && kill -0 "$(cat "$run/$1.pid")" 2>/dev/null; }

stop() {
  for f in "$run"/*.pid; do
    [ -e "$f" ] || continue
    n=$(basename "$f" .pid)
    alive "$n" && kill "$(cat "$f")" && echo "stopped $n"
    rm -f "$f"
  done
}

case "$cmd" in
  stop) stop; exit ;;
  status)
    for f in "$run"/*.pid; do
      [ -e "$f" ] || { echo "nothing running"; exit; }
      n=$(basename "$f" .pid); alive "$n" && echo "$n running (pid $(cat "$f"))" || echo "$n not running"
    done; exit ;;
  logs) tail -n 40 -F "$run"/*.log; exit ;;
  start) ;;
  *) echo "Usage: $0 start|stop|status|logs"; exit 1 ;;
esac

game="$(realpath "${BOOTLEG16_DIR:-gamedata}")"
[ -f "$game/maps/de_dust2.bsp" ] || { echo "No game data at $game (set BOOTLEG16_DIR)."; exit 1; }
lobby=$(bin lobby); server=$(bin server)
for n in $(ls "$run"/*.pid 2>/dev/null | xargs -rn1 basename | sed 's/\.pid$//'); do alive "$n" && { echo "Already running ($n): $0 stop first."; exit 1; }; done

# The secret that lets the lobby book these servers (made once, shared with host.sh).
[ -s hosting/tokens.txt ] || od -An -tx1 -N16 /dev/urandom | tr -d ' \n' > hosting/tokens.txt
token=$(head -n1 hosting/tokens.txt)

"$lobby" 27010 --tokens hosting/tokens.txt --reports hosting/reports/reports.jsonl --key hosting/keys/lobby.key \
  --state hosting/keys/lobby.state --ratings hosting/keys/lobby.db --allow-anonymous > "$run/lobby.log" 2>&1 &
echo $! > "$run/lobby.pid"

for i in $(seq 1 $((SERVERS + DM))); do
  port=$((27014 + i)); mode=match; slots=10
  [ "$i" -gt "$SERVERS" ] && { mode=deathmatch; slots=20; }
  sed -e "s|^lobby = \"\"|lobby = \"127.0.0.1:27010\"|" -e "s|^master = .*|master = \"127.0.0.1:27010\"|" \
      -e "s|^mode = .*|mode = \"$mode\"|" -e "s|^demos = .*|demos = \"$PWD/hosting/demos\"|" \
      -e "s|^max_players = .*|max_players = $slots|" -e "s|^official_token = .*|official_token = \"$token\"|" \
      -e "s|^public_address = .*|public_address = \"127.0.0.1:$port\"|" -e "s|^name = .*|name = \"local-$i\"|" \
      config/server.cfg > "$run/server$i.cfg"
  BOOTLEG16_DIR="$game" "$server" de_dust2 "$port" --config "$run/server$i.cfg" > "$run/server$i.log" 2>&1 &
  echo $! > "$run/server$i.pid"
done
sleep 2
echo "lobby 127.0.0.1:27010, $SERVERS match + $DM deathmatch server(s) from port 27015 (logs: $0 logs)"
$0 status
