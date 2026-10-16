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

`update` pulls the latest code and restarts without cutting matches short: the lobby restarts right away (players sign back in by themselves), idle game servers restart now, and busy ones finish their match first (no new match is booked on them meanwhile). `stop` stops everything at once, ending running matches. `logs` shows servers joining the pool, matches being booked, and a status line every minute (players online, queue, free and busy servers).

**Options:** `--servers N` sets how many game servers run on this machine. `--ip X` sets the address players connect to, if the one found is wrong. `--region R` says where the machine is (`eu`, `na`, `sa`, `asia`...).

**Regions:** the game measures each player's ping to every region. The queue puts a party in a region where all of them ping under 60 ms, widening by 2 ms per second of waiting (up to 200 ms), so nobody waits forever in a quiet region. Custom games go to the party's best region.

**Floods (DDoS):** two layers, and one you choose:

- In the programs: per-address packet budgets, sign-in cookies before any expensive work, answers never bigger than requests (nobody can use us to flood others), and a cap on packets handled per tick.
- On the machine: run `./host.sh firewall` once (it asks for sudo). Kernel rules drop junk sizes and fragments, limit every source address, keep our traffic out of connection tracking, and let only your standby lobby reach the lobbies' TCP port. It loads again at every boot.
- Upstream: a flood that fills the machine's internet line can't be stopped on the machine. Rent servers from a provider that filters floods before they reach you (OVHcloud's game servers, Path.net and Gcore offer UDP/game filtering; most big clouds offer something). That part is a provider choice, not code.

**Match recordings:** every booked match is recorded (the whole truth: everyone, everywhere, every event) into `hosting/demos/`, about 12 MB per hour, kept 14 days (`demo_days` in `config/server.cfg`). A report names the recording and the moment (`demo`, `tick` in `reports.jsonl`). To watch one, replay it and join with the game as a spectator (`--from` jumps to a tick, `--speed 2` plays faster):

```bash
cargo run --release -p server -- --play hosting/demos/<file>.bdem --from <tick>
```

```bash
cargo run -p client -- --connect 127.0.0.1:27015
```

**Leavers and reports:** one player leaving changes nothing (their seat waits, they can come back). After 5 minutes away they count as abandoned, and their team can call a surrender vote in game (F3, then F1 yes / F2 no; a majority of the team still there). If a whole side is gone the match pauses, and forfeits after 5 minutes. Player reports land in `hosting/reports/reports.jsonl` on the lobby machine, one line per report with both players' Steam ids, the reason, map and round.

**Match rules** (rounds, money, timers) live in `config/server.cfg`. Edit it, then run `./host.sh` again.

**Check a deployment** from any machine with the repo:

```bash
cargo run -p net --example smoke -- <lobby-ip>:27010
```

Two test players party up, start a custom game and join the booked server. Add `--local` when testing on the hosting machine itself behind a home router (it can't reach its own public IP).

## Test bots

Bots are for testing only: they never play in real matches, because an official lobby only lets in players signed in with Steam. They're real network players (they sign in, queue, get a ticket, connect) with a simple brain: buy a rifle, find their way across the map (a walkable-points graph built from the map itself), and shoot enemies they can see, with a reaction time and aim error. They only know what the server shows them.

- **Whole-match test:** `cargo test -p net --test bots` runs 10 bots through the lobby queue and a short match to the end, in a few seconds.
- **On a deployment:** start the lobby as a test lobby, then fill it with bots (they keep queueing for new matches):

```bash
./host.sh --test
```

```bash
cargo run -p net --example bots -- <lobby-ip>:27010 10
```

Add `--local` when running the bots on the hosting machine itself, and `--difficulty easy|normal|hard|expert`. `./host.sh --official` turns the lobby back into an official one.

**Controlling them:** type orders into the bots' terminal, or from another one on the same machine:

```bash
cargo run -p net --example bots -- --order "t plant b; ct hold; 3 bhop"
```

An order is `<who> <order>`: who is `all`, `t`, `ct` or a bot number. Orders: `normal`, `afk`, `hold`, `goto a|b|tspawn|ctspawn|x y z`, `rush ...`, `plant a|b|any`, `peaceful`, `spray`, `aimbot`, `grief`, `bhop`, `knife`, `leave`, `rejoin`, `difficulty ...`, `buy <weapon>`. They play like CS 1.6's own bots (whose code CS 1.6 ships without its data files): buy, take the bomb to a site and plant it, pick it up when it's dropped, guard sites, defuse (or run when there's no time), check gunfire they hear, and notice enemies after a reaction time, with aim that improves the longer they track.

## Shipping the game

Build the release with the lobby's address inside, so players just open the game:

```bash
BOOTLEG16_LOBBY=203.0.113.7:27010 cargo build --release -p client --no-default-features --features steam
```

Ship `libsteam_api.so` (it lands next to the game in `target/release/`) with the game.

Use a domain name instead of an IP if you have one (`play.example.com:27010`), so the lobby can move to a new machine without a new build. Players still need their own CS 1.6 install for now (maps and models come from it).

## Steam

Players sign in with Steam: their Steam name and avatar, their friends list on the Play screen, invites, and "Join game" from Steam's friends list. Until we have our own App ID the game uses App ID 480 (Valve's public test app, Spacewar), so Steam shows friends as playing "Spacewar".

- The game needs Steam running; without it, it plays anonymously and party codes still work.
- The lobby checks every Steam login with Steam (the Docker image includes Steam's client library for that). A lobby built without `--features steam` trusts what players say, for local testing only.
- **Testing invites** needs two Steam accounts that are friends, each on its own machine (or a VM), both running the game against the same lobby.
- **Our own App ID** (Steam Direct, $100): change `APP_ID` in `client/src/steam.rs` and pass `--steam-app <id>` to the lobby.

## Matchmaking

Casual has no ranks, but every player has a hidden skill rating (a Bayesian team rating, OpenSkill-style: wins and losses only, never shown) so matches come out fair:

1. The party waiting longest goes first. Parties near its skill, with a good ping to the region, are candidates; the skill window widens the longer they wait.
2. Of those, the 10 players and the 5 v 5 split (parties never split) with the predicted win chance closest to 50% win, with small extras for a narrow skill spread and premades facing premades. "Fair enough" widens with waiting too (4% off 50/50 at first, up to 20%).
3. Everyone gets 20 seconds to accept. Not accepting: a short queue cooldown (1 minute, doubling each time that day). The others go back in the queue, keeping their place.
4. At the end the game server reports the result; ratings update. Leaving and not coming back counts as a loss plus a cooldown (30 minutes, doubling). Being voted out doesn't.

Maps: de_dust2, de_inferno, de_nuke, de_train and de_mirage (`MAP_POOL` in `net/src/config.rs`; a server plays the ones it has, `./host.sh` copies them from your install). de_mirage is a community map (BubkeZ's original, which CS:GO's Mirage grew from), not in every CS 1.6 install: each game tells the lobby which pool maps it has, and a match only gets a map all its players have. Casual matches get a random one; custom games pick one, or veto: the team captains (each team's first member) ban in turns until one is left. Deathmatch stays on de_dust2 for now.

In a match: a team can vote a teammate out (a majority of the others; they can't come back), and once a teammate is gone for good (away 5 minutes, or voted out) the team can vote to surrender. A player whose game crashed or who left is offered their seat again (Reconnect) until the match ends: same team, money and score.

## Practice

- **Solo:** the map on your own machine (the game runs a private server inside itself, nothing online), with standing dummies if you like.
- **Deathmatch:** official drop-in servers for warming up: up to 20 players, join any time, straight onto a team, back at once at a random spawn away from enemies, no rounds, timer or bomb, buy anything anywhere for free. When someone reaches 100 kills they're announced and everyone's score starts over. `./host.sh` runs one deathmatch server next to the match servers (`--dm N` for more); fill it with bots for testing with `--deathmatch` on the bots command.

## Playing

Open **Play** and share your party code with friends.

- **Casual:** a party of 1 to 5 presses Find match. 10 players make a match (5 v 5), and a party always stays on one team.
- **Custom game:** up to 10 friends. The leader puts people on team A or B and presses Start. A knife round decides who picks sides.
