# bootleg16

Counter-Strike 1.6 rebuilt in Rust. It reads the maps and models from your own CS 1.6 install; no game files are shipped.

## Hosting

One command starts everything: the lobby (parties, matchmaking) and a game server.

```bash
./host.sh
```

That's it. The first run:

1. copies the 3 game files the server needs from your CS 1.6 install (Steam's default folder; set `BOOTLEG16_DIR` if it's elsewhere),
2. makes a secret token so only your lobby can book your server,
3. finds your public IP and writes the server config,
4. builds and starts both in Docker (they restart by themselves after a reboot).

It ends by printing the line your friends use to start the game.

**You need:** Linux with [Docker](https://docs.docker.com/engine/install/), and UDP ports **27010** (lobby) and **27015** (game) open:

- On a VPS: open them in the provider's firewall (and `sudo ufw allow 27010,27015/udp` if you use ufw).
- At home: forward both UDP ports on your router to this PC.

**Other commands:**

```bash
./host.sh stop
```

```bash
./host.sh logs
```

```bash
./host.sh 203.0.113.7
```

The last one is for when the IP it finds is wrong: give the address players should use.

**Settings:** the match rules (rounds, money, timers) are in `hosting/server1.cfg`; restart with `./host.sh` after editing. Re-running `./host.sh` rewrites it from `config/server.cfg`, so make lasting changes there.

**More servers:** each match takes a whole server. For more matches at once, copy the `server1` block in `compose.yaml` as `server2` on port `27016` (and its own `hosting/server2.cfg` with `public_address` ending in `:27016`).

## Playing

Start the game with your host's lobby address, open **Play**, and share your party code with friends:

```bash
BOOTLEG16_LOBBY=203.0.113.7:27010 cargo run --release -p client --no-default-features
```

- **Casual:** party of 1 to 5, press Find match. 10 players make a match (5 v 5).
- **Custom game:** up to 10 friends, the leader puts people on team A or B and presses Start. A knife round decides who picks sides.
