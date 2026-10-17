#!/usr/bin/env bash
# Run bootleg16's servers in Docker. See README "Hosting".
#
#   ./host.sh                         this machine: the lobby + game servers (one per CPU core)
#   ./host.sh join <lobby-ips> <token> this machine: game servers for an existing lobby
#                                     (<lobby-ips>: the lobby, or "main-ip,standby-ip")
#   ./host.sh standby <main-ip> <token> <key>
#                                     this machine: a standby lobby (+ game servers) that takes
#                                     over if the main one dies (`./host.sh pair-info` on the
#                                     main machine prints this line)
#   options: --servers N   game servers here (each runs one match at a time)
#            --ip X        the public IP players connect to (found automatically otherwise)
#            --region R    where this machine is, e.g. eu, na, sa, asia (players get the closest)
#            --test        a test lobby: lets in players without Steam (test bots, smoke tests)
#            --official    back to an official lobby (Steam players only; the default)
#            --peer IP     (main lobby) the standby lobby machine to stream state to
#            --dm N        deathmatch servers here (drop-in, 20 players; default 1 with the lobby)
#   ./host.sh firewall               kernel-level flood filtering for our ports (asks for sudo)
#   ./host.sh token | status | logs | update | stop
#
# Settings are remembered in hosting/: after the first run, plain `./host.sh` brings the
# same setup back up.
set -euo pipefail
cd "$(dirname "$0")"
# Everything in hosting/ is private: the token (and the configs holding it) are secrets.
umask 077
mkdir -p hosting hosting/reports hosting/keys hosting/demos
chmod -R go-rwx hosting
compose() { docker compose -p bootleg16 -f hosting/compose.yaml "$@"; }
# The containers run as you (not root): they can read the private files and nothing else.
as_me="user: \"$(id -u):$(id -g)\", "

[ -f hosting/settings ] && . hosting/settings
LOBBY="${LOBBY:-here}" SERVERS="${SERVERS:-}" IP="${IP:-}" REGION="${REGION:-eu}" TEST="${TEST:-}" PEER="${PEER:-}" DM="${DM:-}"

case "${1:-}" in
  stop) compose down -t 5; exit ;;   # now: running matches end
  logs) compose logs -f --tail 50; exit ;;
  status) compose ps --format "table {{.Service}}\t{{.Status}}"; exit ;;
  token) cat hosting/tokens.txt; echo; exit ;;
  firewall) FIREWALL=1; shift ;;
  pair-info)
    # Secret: the token and the lobby's private key. Give it only to your standby machine.
    echo "On the standby machine:  ./host.sh standby ${IP:-<this-ip>} $(cat hosting/tokens.txt) $(cat hosting/keys/lobby.key)"
    echo "Then here:               ./host.sh --peer <standby-ip>"
    exit ;;
  standby)
    [ $# -ge 4 ] || { echo "Usage: ./host.sh standby <main-ip> <token> <key>   (see ./host.sh pair-info on the main machine)"; exit 1; }
    LOBBY=here; PEER="$2"; echo -n "$3" > hosting/tokens.txt; echo "$4" > hosting/keys/lobby.key; shift 4 ;;
  update) git pull --ff-only || echo "Not pulling (no remote, or local changes): updating to the code as it is."; UPDATE=1; shift ;;
  join)
    [ $# -ge 3 ] || { echo "Usage: ./host.sh join <lobby-ip> <token>"; exit 1; }
    LOBBY="$2"; echo -n "$3" > hosting/tokens.txt; shift 3 ;;
esac
while [ $# -gt 0 ]; do
  case "$1" in
    --servers) SERVERS="$2"; shift 2 ;;
    --ip) IP="$2"; shift 2 ;;
    --region) REGION="$2"; shift 2 ;;
    --test) TEST=1; shift ;;
    --official) TEST=""; shift ;;
    --peer) PEER="$2"; shift 2 ;;
    --dm) DM="$2"; shift 2 ;;
    *) echo "Unknown option $1 (see the top of host.sh)"; exit 1 ;;
  esac
done

command -v docker >/dev/null || { echo "Install Docker first: https://docs.docker.com/engine/install/"; exit 1; }
SERVERS="${SERVERS:-$(nproc)}"
DM="${DM:-$([ "$LOBBY" = here ] && echo 1 || echo 0)}"
# Ports: match servers from 27015, then the deathmatch ones.
last=$((27014 + SERVERS + DM))

# The few game files the servers read, copied from the game data folder (never shipped).
game="${BOOTLEG16_DIR:-dist/gamedata}"
# The map pool too (de_dust2 is needed; the others are played if they're there).
for m in de_inferno de_nuke de_train; do
  [ -f "hosting/game/maps/$m.bsp" ] || { [ -f "$game/maps/$m.bsp" ] && mkdir -p hosting/game/maps && cp "$game/maps/$m.bsp" hosting/game/maps/; } || true
done
for f in maps/de_dust2.bsp models/player/arctic/arctic.mdl sound/materials.txt; do
  if [ ! -f "hosting/game/$f" ]; then
    [ -f "$game/$f" ] || { echo "Missing $f. Set BOOTLEG16_DIR to the game data folder (the one with maps/ in it), or copy hosting/game/ from a machine that has it."; exit 1; }
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
printf 'LOBBY=%q\nSERVERS=%q\nIP=%q\nREGION=%q\nTEST=%q\nPEER=%q\nDM=%q\n' "$LOBBY" "$SERVERS" "$IP" "$REGION" "$TEST" "$PEER" "$DM" > hosting/settings

# Kernel-level filtering (nftables), before any packet reaches the game: junk sizes and
# fragments are dropped, every source address gets a packet budget, our UDP skips connection
# tracking (a flood filling the kernel's table is a classic way to take a server down), and only
# the standby lobby may reach the lobbies' link. Big floods that fill the line itself can only
# be stopped upstream: pick a provider with DDoS filtering (see README "Hosting").
firewall_rules() {
  local last=$((27014 + SERVERS + DM))
  cat <<NFT
table inet bootleg16
delete table inet bootleg16
table inet bootleg16 {
  chain raw {
    type filter hook prerouting priority raw; policy accept;
    # Our UDP: never through connection tracking.
    udp dport { 27010, 27015-$last } notrack
    # Fragments are never ours (our packets fit in one), and are a favourite flood.
    ip frag-off & 0x3fff != 0 meta l4proto udp drop
    # Sizes our programs never send: empty, or bigger than 1400 bytes of payload.
    udp dport { 27010, 27015-$last } udp length < 9 drop
    udp dport { 27010, 27015-$last } udp length > 1408 drop
    # Per source address: a player sends ~130 packets a second (a household of 6: ~800).
    udp dport 27015-$last meter game4 size 65535 { ip saddr limit rate over 1500/second burst 500 packets } drop
    udp dport 27015-$last meter game6 size 65535 { ip6 saddr and ffff:ffff:ffff:ffff:: limit rate over 1500/second burst 500 packets } drop
    # The lobby: a poll a second per player, a household of them, sign-ins.
    udp dport 27010 meter lobby4 size 65535 { ip saddr limit rate over 60/second burst 60 packets } drop
    udp dport 27010 meter lobby6 size 65535 { ip6 saddr and ffff:ffff:ffff:ffff:: limit rate over 60/second burst 60 packets } drop
  }
  chain out {
    type filter hook output priority raw; policy accept;
    udp sport { 27010, 27015-$last } notrack
  }
  chain input {
    type filter hook input priority filter; policy accept;
    # The lobbies' state link: only from the other lobby.
    tcp dport 27011 ${PEER:+ip saddr != $PEER} drop
  }
}
NFT
}

if [ -n "${FIREWALL:-}" ]; then
  command -v nft >/dev/null || { echo "Needs nftables (the nft command)."; exit 1; }
  firewall_rules > hosting/firewall.nft
  echo "Applying hosting/firewall.nft (needs root):"
  sudo nft -f hosting/firewall.nft
  # Load it again at every boot.
  sudo tee /etc/systemd/system/bootleg16-firewall.service >/dev/null <<UNIT
[Unit]
Description=bootleg16 flood filtering
After=network-pre.target
[Service]
Type=oneshot
ExecStart=$(command -v nft) -f $PWD/hosting/firewall.nft
RemainAfterExit=yes
[Install]
WantedBy=multi-user.target
UNIT
  sudo systemctl daemon-reload && sudo systemctl enable bootleg16-firewall.service >/dev/null
  echo "Done; it loads again at boot. Undo: sudo nft delete table inet bootleg16 && sudo systemctl disable bootleg16-firewall"
  exit
fi

# One config per game server (the shipped rules, pointed at the lobby), and the compose file.
# Every lobby the game servers announce to (and take matches from).
if [ "$LOBBY" = here ]; then
  lobby_addr="127.0.0.1:27010${PEER:+,$PEER:27010}"
else
  lobby_addr=$(echo "$LOBBY" | tr ',' '\n' | sed 's/$/:27010/' | paste -sd,)
fi
rm -f hosting/server*.cfg
{
  echo "# Made by host.sh: edit config/server.cfg instead, then run ./host.sh"
  echo "services:"
  if [ "$LOBBY" = here ]; then
    echo "  lobby: {image: bootleg16, $as_me network_mode: host, restart: unless-stopped, command: lobby 27010 --tokens /bootleg16/tokens.txt --reports /bootleg16/reports/reports.jsonl --key /bootleg16/keys/lobby.key --state /bootleg16/keys/lobby.state --ratings /bootleg16/keys/ratings.db${PEER:+ --peer $PEER:27011 --peer-port 27011}${TEST:+ --allow-anonymous}, volumes: [./tokens.txt:/bootleg16/tokens.txt:ro, ./reports:/bootleg16/reports, ./keys:/bootleg16/keys]}"
  fi
  for i in $(seq 1 $((SERVERS + DM))); do
    port=$((27014 + i))
    # After the match servers: deathmatch ones (drop-in, 20 players).
    mode=match; slots=10
    [ "$i" -gt "$SERVERS" ] && { mode=deathmatch; slots=20; }
    sed -e "s|^lobby = \"\"|lobby = \"$lobby_addr\"|" \
        -e "s|^mode = .*|mode = \"$mode\"|" \
        -e "s|^demos = .*|demos = \"/bootleg16/demos\"|" \
        -e "s|^max_players = .*|max_players = $slots|" \
        -e "s|^official_token = \"\"|official_token = \"$token\"|" \
        -e "s|^public_address = \"\"|public_address = \"$IP:$port\"|" \
        -e "s|^name = .*|name = \"$(hostname)-$i\"|" \
        -e "s|^region = .*|region = \"$REGION\"|" \
        config/server.cfg > "hosting/server$i.cfg"
    echo "  server$i: {image: bootleg16, $as_me network_mode: host, restart: unless-stopped, stop_grace_period: 3h, command: server de_dust2 $port --config /bootleg16/server.cfg, volumes: [./server$i.cfg:/bootleg16/server.cfg:ro, ./game:/game:ro, ./demos:/bootleg16/demos]}"
  done
} > hosting/compose.yaml

docker build -q -t bootleg16 . >/dev/null
# The lobby's key (made once, kept): its public half goes into the game build.
if [ "$LOBBY" = here ]; then
  public=$(docker run --rm --user "$(id -u):$(id -g)" -v "$PWD/hosting/keys:/bootleg16/keys" bootleg16 lobby --print-public /bootleg16/keys/lobby.key 2>/dev/null | tail -n1)
fi
if [ -n "${UPDATE:-}" ] && [ -n "$(compose ps -q 2>/dev/null)" ]; then
  # Rolling update: the lobby now (players sign back in by themselves), each game server once
  # its match is over (it finishes the match on SIGTERM; nothing new is booked on it meanwhile).
  [ "$LOBBY" = here ] && compose up -d --no-deps --force-recreate lobby
  services=$(seq -f "server%g" 1 $((SERVERS + DM)) | tr '\n' ' ')
  nohup bash -c "for s in $services; do docker compose -p bootleg16 -f hosting/compose.yaml up -d --no-deps --force-recreate \$s & done; wait; echo update done" > hosting/update.log 2>&1 &
  echo "Updating: idle game servers restart now, busy ones when their match ends (progress: hosting/update.log)."
  exit
fi
compose up -d --remove-orphans
echo
echo "Running $SERVERS match server(s) and $DM deathmatch server(s) on $IP (UDP 27015-$last)$([ "$LOBBY" = here ] && echo " and the lobby (UDP 27010)")."
echo "Open those UDP ports in your firewall / router.${PEER:+ Lobbies also talk to each other on TCP 27011 (open it to $PEER only).}"
if [ "$LOBBY" = here ]; then
  echo "More machines:  ./host.sh join $IP${PEER:+,$PEER} $token"
  [ -z "$PEER" ] && echo "A standby lobby: ./host.sh pair-info"
  echo "Ship the game:  BOOTLEG16_LOBBY=$IP:27010${PEER:+,$PEER:27010} BOOTLEG16_LOBBY_KEY=$public cargo build --release -p client --no-default-features --features steam"
fi
