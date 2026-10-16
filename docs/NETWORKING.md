# Networking

Code: `net/` (protocol, server, client prediction), `server/` (dedicated server
binary). Plain UDP from the standard library; messages are postcard-encoded.

## How it works

- **The server is in charge.** Clients send only their inputs (keys + view
  angles), 128 per second, each packet repeating the last 8 so lost packets
  lose nothing. The server runs every player's movement with the same `sim`
  code, moves doors, and sends a snapshot of all players and doors 64 times a
  second.
- **Your own movement is instant (prediction).** The client runs each input
  locally right away. When a snapshot arrives it takes the server's state for
  its player and replays the inputs the server hasn't run yet. Same code, same
  inputs, so the correction is normally zero.
- **Other players move smoothly (interpolation).** They are drawn about 31 ms
  (two snapshots) in the past, blending between the two snapshots around that
  time.
- **Speed hacks don't work.** The server runs at most one input per tick on
  average (a client may bank up to 16 while packets are delayed); extra inputs
  are dropped.
- **Playing alone** starts a server inside the game, so it's the same code path.

## Running

```
server [map] [port]                      # dedicated, default de_dust2 27015
client [map] --connect host:port [--name n]
client [map]                             # offline (built-in server)
```

`BOOTLEG16_FAKE_PING=120 BOOTLEG16_FAKE_LOSS=0.05 client ...` simulates a bad
connection. Friends on the internet need UDP port 27015 forwarded to the server.

## Tests (`net/tests/multiplayer.rs`)

Real localhost UDP, server + two clients on de_dust2: on LAN and at 100 ms ping
with 5% loss, the predicted player ends exactly where the server has it, mid-run
corrections stay under 1 unit, and the other client sees the player within 1
unit of the server position. A client sending inputs 3x faster than real time
moves no faster than an honest one.

## Not done yet

- Lag compensation for shooting (comes with weapons).
- Hiding players you can't see from the snapshots (anti-wallhack, with weapons).
- Map check on join (client and server must have the same map file), map change.
- Encryption/authentication, NAT traversal, server browser.
- Snapshots are full state every time; delta compression when bandwidth matters
  (10 players ≈ 40 KB/s per client now).

## Anti-wallhack (server-side visibility)

Each snapshot only contains the enemies the receiving player could see: the server traces
lines from the viewer's eyes (now and 0.15 s ahead along their velocity) to the corners of
the target's box grown by 24 units (now and 0.15 s ahead). One clear line is enough. Only the
static world blocks sight, so glass and doors never hide someone you could see. Someone who
goes out of sight is still sent for 0.25 s, so corners don't flicker. Teammates, and anyone
watching while dead or spectating, get everyone.

Sounds are filtered separately: footsteps and unsilenced shots are sent within 1250 units
(GoldSrc ATTN_NORM falloff), silenced shots within 625, so players behind walls are still heard
exactly where they are, but a cheat gets no player to draw.
