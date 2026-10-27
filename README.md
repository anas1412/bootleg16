# bootleg16

A tactical 5 v 5 shooter in Rust: CS 1.6 movement, guns and economy; CS2's match format and timers. The game is moving from its Bevy client to Godot 4, with our own maps, look and characters (see [Godot client](#godot-client-in-progress)).

## The game

- **Casual:** party of 1 to 5, Find match. 10 players make a 5 v 5; a party always stays on one team.
- **Custom game:** up to 10 friends. The leader puts people on team A or B and presses Start. A knife round decides who picks sides. Pick a map or veto (captains ban in turns).
- **Deathmatch:** drop-in warm-up servers, up to 20 players. Instant respawn away from enemies, no rounds, timer or bomb, buy anything anywhere for free. First to 100 kills is announced and scores reset.
- **Host:** a match or deathmatch on your own PC. Steam friends join from your name in their friends list (Join game), or type your address under Play. For friends outside your home, forward UDP port 27015 on your router.
- **Practice > Solo:** offline on your own machine, with standing dummies if you like.
- **Maps:** de_dust2, de_inferno, de_nuke, de_train (Bevy client) and data_center, our own (Godot client). Casual picks one at random.
- **Parties:** open **Play** and share your party code, or invite Steam friends.
- **Bodies:** the dead stay all match as a bump you step over. Press E on one to carry it as a shield: it takes 75% of a bullet or knife from the front and half of a grenade's blast, your head stays open, and you move slower. E again drops it. Lying bodies take a little (30%) of a low shot through them, so a pile is cover when you crouch.
- **Voice chat**, and an AI assistant instead of the radio.
- **Fair matches:** no ranks, but a hidden skill rating balances every match.
- **In a match:**
  - Vote a teammate out (majority of the others; they can't come back).
  - Surrender vote once a teammate is gone for good: F3, then F1 yes / F2 no.
  - Crashed or left? Reconnect gets you your seat back (same team, money and score) until the match ends.
- **Leaving:** a 20 s accept window, cooldowns for not accepting (1 min, doubling that day) and for abandoning (30 min, doubling; counts as a loss).

## Installing

Download from [Releases](https://github.com/anas1412/bootleg16/releases).

**Steam must be running and signed in.** Otherwise the game shows "Steam needed" and doesn't open.

- **Windows:** run `bootleg16-<version>-windows-setup.exe`. Per user, Start menu entry, uninstaller. Settings in `%APPDATA%\bootleg16`.
- **Linux:** unpack `bootleg16-<version>-linux.tar.gz` and run `./install.sh`.
  - Per user, no root: the game goes in `~/.local/share/bootleg16`, with a `bootleg16` command and a menu entry.
  - `./uninstall.sh` removes it; `--purge` also removes your settings.
  - Needs Wayland and Vulkan.

## For developers

### Layout

| Folder | What |
|---|---|
| `godot/` | The Godot 4 client: scenes, maps, menus, UI kit, headless checks |
| `gdbridge/` | GDExtension that gives Godot our simulation, netcode and lobby |
| `client_core/` | Client logic shared by both clients |
| `client/` | The Bevy client (frozen, replaced by `godot/`): menus, HUD, rendering, sound, voice, Steam |
| `server/` | Game server program (also replays recordings) |
| `lobby/` | Lobby program: parties, matchmaking, the server pool |
| `net/` | Networking shared by all three: server and lobby logic, encryption, recordings, test bots |
| `sim/` | Game rules both sides run: movement, weapons, grenades, collision |
| `goldsrc/` | Readers for CS 1.6 files: maps, models, textures |
| `config/` | `server.cfg`: match rules |
| `docs/` | Design notes, rules, `TODO.md`, `GOTCHAS.md` |
| `design/` | Identity marks, design system tokens, character scripts |
| `tools/` | Helper scripts |
| `packaging/` | Linux and Windows installers |

**Game data:**
- Maps, models, sounds, textures: `gamedata/` in the repo, or `data/` next to a built game, or set `BOOTLEG16_DIR`.
- Contents: `maps/`, `models/`, `sound/`, `sprites/`, `gfx/`, the `.wad` files.
- Part of it is still Valve's and Nexon's (see `docs/TODO.md`).

### Building

Needs Rust stable, Linux with Wayland and Vulkan.

```bash
cargo run -p client
```

Dev builds link Bevy dynamically, play without Steam, and rebuild in about a minute.

Release build:

```bash
cargo build --release -p client --no-default-features --features steam
```

Tests (run the ones for what you changed; they skip without the game data):

```bash
cargo test -p net -p sim
```

### Godot client (in progress)

The game is moving to Godot 4. The Rust core stays: `sim`, `net`, `server` and `lobby`. Done so far: our own map format (`.bmap`, exported from Godot scenes), the identity and design system, the menus, settings and lobby, and the data_center map (measured against Dust2's timings, played by bots). Build the bridge, then open the project:

```bash
systemd-run --user --scope -q -p MemoryMax=3G -- cargo build -p gdbridge -j 2
```

```bash
godot --path godot
```

Headless checks: `godot --headless --path godot --script res://tests/session_check.gd` (also `menu_check.gd`; `lobby_check.gd` needs a lobby running).

### Releasing

Push a `v*` tag. `.github/workflows/installers.yml` builds both installers and publishes a GitHub prerelease. The workflow can also be run from the Actions tab. Set the repo variable `BOOTLEG16_LOBBY` to bake the lobby address in.

- **Linux by hand:** release build, then `packaging/linux/build.sh` → `dist/bootleg16-<version>-linux.tar.gz`.
- **Windows:** Inno Setup script in `packaging/windows/bootleg16.iss`.
- **Lobby address:** `BOOTLEG16_LOBBY=play.example.com:27010 cargo build --release ...`. Use a domain so the lobby can move.
- **Shipping:**
  - Ship `libsteam_api.so` / `steam_api64.dll` next to the game.
  - Ship the data folder as `data/`.

### Steam

- **App ID:** 480 (Valve's test app, Spacewar) until we have our own, so friends see "Spacewar".
- **What it gives:** names, avatars, friends list, invites, "Join game".
- **Release builds:** refuse to open without Steam.
- **Lobby:** checks every login with Steam. A lobby built without `--features steam` trusts players (local testing only).
- **Testing invites:** two Steam accounts that are friends, on separate machines.
- **Own App ID** (Steam Direct, $100): change `APP_ID` in `client/src/steam.rs`, pass `--steam-app <id>` to the lobby.

### How it fits together

- **Game:** what players download. No server inside (except Solo); it talks to the lobby.
- **Lobby (one):** parties, invite codes, the queue, and the server pool. A ready match books a free game server and sends the 10 players there with a ticket each.
- **Game servers (many):** one match each, back to the pool when it ends. Freed after 90 s if nobody joins or everyone leaves.

### Hosting

Everything runs in Docker. On the main machine:

```bash
./host.sh
```

- Starts the lobby, one game server per CPU core, and one deathmatch server (`--dm N` for more).
- **First run:** copies the needed game files, makes the secret token, finds the public IP.
- **State:** saved in `hosting/`. Docker brings it all back after a reboot.

**Extra machines** (repo + Docker), with the line the first machine printed (`./host.sh token` shows it again):

```bash
./host.sh join <lobby-ip> <token>
```

**Ports (UDP):** 27010 lobby, 27015 and up game servers. Open them in the provider's firewall, or forward them on your router at home.

**Commands:** `./host.sh status | logs | update | stop`.
- **`update`:** pulls and restarts without cutting matches. Lobby now, idle servers now, busy ones after their match.
- **`stop`:** ends everything at once.
- **`logs`:** pool joins, bookings, a status line every minute.

**Options:**
- `--servers N`: game servers on this machine.
- `--ip X`: override the public address.
- `--region R`: `eu`, `na`, `sa`, `asia`...

**Regions:** players are placed where the whole party pings under 60 ms. The limit widens 2 ms per second of waiting, up to 200 ms. Custom games go to the party's best region.

**Match rules** (rounds, money, timers): `config/server.cfg`, then run `./host.sh` again.

**Local servers without Docker:**
- `tools/host-local.sh start|stop|status|logs` runs a lobby, a match server and a deathmatch server.
- Play against it with `BOOTLEG16_LOBBY=127.0.0.1:27010`.

**Check a deployment** (add `--local` on the host itself behind a home router):

```bash
cargo run -p net --example smoke -- <lobby-ip>:27010
```

### Flood protection

- **In the programs:**
  - Per-address packet budgets.
  - Sign-in cookies before any expensive work.
  - Answers never bigger than requests.
  - A cap on packets handled per tick.
- **On the machine:** `./host.sh firewall` (once, sudo). Kernel rules:
  - Drop junk sizes and fragments.
  - Rate-limit each source.
  - Skip connection tracking.
  - Only the standby lobby reaches the lobby TCP port.
- **Upstream:** a provider that filters floods (OVHcloud game servers, Path.net, Gcore).

### Matchmaking

Hidden rating: Bayesian team rating, OpenSkill-style, wins and losses only.

1. The longest-waiting party goes first. Candidates: parties near its skill with good ping. The skill window widens with waiting.
2. The 10 players and the 5 v 5 split (parties never split) closest to a 50% predicted win chance are picked.
   - Small bonuses: a narrow skill spread, premades facing premades.
   - Tolerance: 4% off 50/50 at first, widening to 20%.
3. 20 s to accept. Whoever doesn't gets a cooldown; the rest keep their place in the queue.
4. The server reports the result and ratings update.
   - Abandoning counts as a loss plus a cooldown.
   - Being voted out doesn't count.

**Map pool:**
- Defined in `MAP_POOL` in `net/src/config.rs`.
- A match only gets a map that all its players have.
- Deathmatch is on de_dust2.

**Leavers:**
- A player away 5 min counts as abandoned, and their team can surrender.
- A whole side gone pauses the match; it forfeits after 5 min.

### Recordings and reports

**Recordings:**
- Every booked match is recorded in full to `hosting/demos/`, about 12 MB/hour.
- Kept 14 days (`demo_days` in `config/server.cfg`).

**Reports:**
- Saved to `hosting/reports/reports.jsonl` on the lobby machine.
- Each one holds both Steam ids, the reason, map, round, recording and tick.

**Watch a recording** (`--from <tick>`, `--speed 2`), then join it as a spectator:

```bash
cargo run --release -p server -- --play hosting/demos/<file>.bdem --from <tick>
```

```bash
cargo run -p client -- --connect 127.0.0.1:27015
```

**Review reports** (list them, then watch report n from 10 s before; copy the recording from that server's `hosting/demos/` first or pass `--demos`):

```bash
cargo run --release -p server -- --reports hosting/reports/reports.jsonl
```

```bash
cargo run --release -p server -- --review hosting/reports/reports.jsonl 3 --demos hosting/demos
```

### Test bots

Testing only: an official lobby lets in Steam players only.
- They're real network players: they sign in, queue and connect.
- They see only what the server shows them.
- They play like CS 1.6's bots: buy, plant, pick up the bomb, guard, defuse or run, react to gunfire, aim better the longer they track.

- **Whole match:** `cargo test -p net --test bots` (10 bots, a few seconds).
- **On a deployment:**

```bash
./host.sh --test
```

```bash
cargo run -p net --example bots -- <lobby-ip>:27010 10
```

Options: `--local`, `--difficulty easy|normal|hard|expert`, `--deathmatch`. `./host.sh --official` turns the lobby back to official.

**Orders** (type in the bots' terminal, or from another one):

```bash
cargo run -p net --example bots -- --order "t plant b; ct hold; 3 bhop"
```

Format: `<who> <order>`.
- **who:** `all`, `t`, `ct` or a bot number.
- **order:** `normal`, `afk`, `hold`, `goto a|b|tspawn|ctspawn|x y z`, `rush ...`, `plant a|b|any`, `peaceful`, `spray`, `aimbot`, `grief`, `bhop`, `knife`, `leave`, `rejoin`, `difficulty ...`, `buy <weapon>`.
