# bootleg16

Counter-Strike 1.6 rebuilt in Rust. It reads the maps and models from your own CS 1.6 install; no game files are shipped.

## How it fits together

- **The game** is what players download. It holds no server; it talks to the lobby.
- **The lobby** (one for the whole game) holds parties, invite codes and the casual queue, and keeps the pool of game servers. When a match is ready it books a free game server and sends the 10 players there with a ticket each.
- **Game servers** (as many as we need, over as many machines as we want) each run one match at a time and go back to the pool when it ends. A server nobody joins, or everyone leaves, frees itself after 90 seconds.

## Hosting

All of it runs in Docker. On the main machine:

```bash
./host.sh
```

That starts the lobby and one game server per CPU core, so one match per core. The first run copies the 3 game files the servers need from your CS 1.6 install, makes the secret token, and finds the public IP. It remembers everything in `hosting/`, so later runs bring the same setup back up, and Docker restarts it all after a reboot.

**More capacity:** on each extra machine (it needs the repo and Docker), run the line the first machine printed:

```bash
./host.sh join <lobby-ip> <token>
```

Its game servers join the same pool. Run `./host.sh token` on the main machine if you lost the token.

**Ports (UDP):** 27010 for the lobby, 27015 and up for game servers (27015-27022 for 8). Open them in the provider's firewall, or forward them on your router at home.

**Running it:**

```bash
./host.sh status
```

```bash
./host.sh logs
```

```bash
./host.sh update
```

```bash
./host.sh stop
```

`update` pulls the latest code and restarts everything (matches in progress end, so do it when it's quiet). `logs` shows servers joining the pool, matches being booked, and a status line every minute (players online, queue, free and busy servers).

**Options:** `--servers N` sets how many game servers run on this machine. `--ip X` sets the address players connect to, if the one found is wrong.

**Match rules** (rounds, money, timers) live in `config/server.cfg`. Edit it, then run `./host.sh` again.

**Check a deployment** from any machine with the repo:

```bash
cargo run -p net --example smoke -- <lobby-ip>:27010
```

Two test players party up, start a custom game and join the booked server. Add `--local` when testing on the hosting machine itself behind a home router (it can't reach its own public IP).

## Shipping the game

Build the release with the lobby's address inside, so players just open the game:

```bash
BOOTLEG16_LOBBY=203.0.113.7:27010 cargo build --release -p client --no-default-features
```

Use a domain name instead of an IP if you have one (`play.example.com:27010`), so the lobby can move to a new machine without a new build. Players still need their own CS 1.6 install for now (maps and models come from it).

## Playing

Open **Play** and share your party code with friends.

- **Casual:** a party of 1 to 5 presses Find match. 10 players make a match (5 v 5), and a party always stays on one team.
- **Custom game:** up to 10 friends. The leader puts people on team A or B and presses Start. A knife round decides who picks sides.
