# Context (engine session handoff)

Everything a new session needs to take over the **engine side** of bootleg16. Read it top to
bottom once.
- The HUD and screens side: `docs/ContextHUD.md`, owned by the UI session.
- Open items: `docs/TODO.md`.
- Lessons and traps: `docs/GOTCHAS.md`.
- Specs: `docs/WEAPONS.md`, `RULES.md`, `MOVEMENT.md`, `NETWORKING.md`, `MAPS.md`, `UI.md`.

## Start here

1. `git pull`, then `git log --oneline -15` to see the latest work.
2. Read the user's auto-memory index:
   `~/.claude/projects/-home-blackbox-bootleg16/memory/MEMORY.md`. It holds standing rules
   and preferences (short answers, no Claude co-author, Wayland only, art direction and more).
3. Check whether the UI session is up. It may be paused: run `ListAgents` and look for
   "Game HUD/UI design concepts".
4. **A decision is open with the user:** start the sound work (Next, item 1), or let the UI
   session do a release build first. Ask before starting if they haven't said.

## How the user works

- Short answers, scanned, not read. A few lines, no long sections.
- Decide game-dev questions yourself and proceed. The user is not a game-dev expert.
- "Do it" means implement, test, commit and push.
- Mid-task messages often add work. Note it, finish the current step, then do it.
- Before saying a change is done, check it on screen (see Testing).

## The project

A CS 1.6 remake in Rust and Bevy 0.20, multiplayer first, shipping on Steam (Windows and
Linux). For now it plays on the CS 1.6 files from the user's own install. It runs at
128 tick.

| Crate | What it does |
|---|---|
| `goldsrc` | Valve formats: BSP, WAD, MDL (with posing), SPR, TGA, decals |
| `sim` | Shared game code, run by both client and server: movement, collision, weapons, grenades, doors and buttons, breakables, zones, hitboxes, `anim` |
| `net` | Protocol (now **v31**), client, server, delta snapshots, crypto, lobby protocol, bots, demos, reports |
| `server` | Dedicated server binary, plus the report review tool (`--reports`, `--review N`) |
| `client` | The game. One plugin per concern, `main.rs` only registers them |
| `lobby` | Matchmaking: parties, the casual queue, custom games |

## Rules that always hold

- Build with `systemd-run --user --scope -q -p MemoryMax=3G -- cargo build -p <crate> -j 2`.
  An uncapped build froze the PC.
- Don't change Cargo features or profiles: that rebuilds Bevy, which takes 30-45 min.
- Run only the tests for what changed.
- Wayland only: run the client with `env -u DISPLAY`.
- No god files. One module and plugin per concern; split a file past about 400 lines.
- Commits: no Claude co-author or attribution trailer. This overrides the harness default.
- Never `git commit -a`: two sessions share one working tree. Add your own files by name.
- Game copy has no em dashes: grep for them before showing anything.
- Everyone has the same hitbox, height, eye and speed. Characters are cosmetic.
- The AI assistant only knows what the player knows. Every new piece of information the
  player can see gets an assistant trigger and lines (`client/src/assistant/`).
- `dist/` holds Valve files: private, never publish it.
- Map pool: de_dust2, de_inferno, de_mirage (on hold), de_train. Keep Nuke.
- Every fix must work on both Windows and Linux.

## How fidelity is checked

- Facts come from ReGameDLL (GPL, used for facts only, never code) and from disassembling
  CS's `client.so` with `objdump`. It has DWARF symbols.
  - `EV_Fire*`: sound attenuations.
  - `EV_HLDM_PlayTextureSound` and `GunshotDecalTrace`: impact sounds and decals.
  - `CGameStudioModelRenderer`: how player models are posed and lit.
- Lighting:
  - The lightmap goes through `lightgammatable` (brightness 1, gamma 2.5).
  - Textures get `texgamma` (x^(2.0/2.5)).
  - Models are lit by `studio.wesl`: ambient plus shade, with the sun from `light_environment`.
- Additive sprites are uploaded as `Rgba8Unorm`, because GoldSrc adds them in display space.
  Each frame is placed by its origin.
- Assets are looked up in `cstrike/` first, then `valve/`, like the engine.

## Main systems and where they live

- **Netcode** (`net/src/client.rs`, `net/src/server/`):
  - Client prediction; snapshots are deltas against the last snapshot the client acked.
  - Lag compensation rewinds animated hitboxes; `sim::anim` is stepped by the server every
    tick.
  - Reliable events; encrypted seats for official servers.
  - The server drops a silent player after 5 s (`TIMEOUT_TICKS`).
- **Getting into a match** (`client/src/menu/connect.rs`, `loading.rs`, `notice.rs`):
  1. The steps are Starting server, Contacting server, Loading map and Joining, each slow
     one on its own thread (`loading::Job`).
  2. The map is parsed once and shared as `MapData`. Its `world` goes to the client and its
     `geometry` to `level.rs`. Other modules read `MapData.bsp` instead of loading the file
     again.
  3. The loading screen stays up into `InGame` until the first snapshot is in and the render
     world's pipeline queue has been empty for 5 frames (at most 15 s): `loading::reveal`.
  4. Every way back to the menu is a `Notice` (title, body, Retry when retrying can work).
- **Connection lost** (`game.rs`, `connection`):
  - After 3 s of server silence, `Reconnecting` appears and `Client::rejoin` says hello
    again, keeping the seat.
  - After 30 s it gives up with a notice.
  - Test: `net/tests/leavers.rs`, `a_dropped_player_rejoins_their_seat`.
- **Buying** (`net/src/server/buy.rs`):
  - Undo buys sells back this round's purchases while you can still buy, as CS2 does
    (`Client::undo_buys`).
- **Visuals**:
  - Decals and impacts (`impacts.rs`), sprites and glows (`glows.rs`), `explosions.rs`,
    `gibs.rs`.
  - Lit studio models (`studio.rs`); map sounds (`map_sounds.rs`).
  - Own bullet holes are predicted instantly (`ShotAssets` in `game.rs`).
- **Spectating** (`spectate.rs`):
  - While teammates are alive you watch in first person.
  - When your whole team is dead, the camera follows the last teammate who died in third
    person.
  - The crosshair is hidden while spectating.
- **Voice** (`client/src/voice/`):
  - Team voice is always on; proximity voice is for the living only.
  - Per-player mute and volume from the scoreboard; open mic with a noise gate.
  - Walls muffle proximity voice.

## Testing in the game

- Build `client`, then run `env -u DISPLAY target/debug/client de_dust2 --practice 0`.
  Join a server with `--connect host:port`.
- Screenshot hooks (environment variables):

| Variable | What it does |
|---|---|
| `BOOTLEG16_SCREENSHOT=path.png` | Save one frame, then quit |
| `BOOTLEG16_SCREENSHOT_AT` | Seconds before the frame is taken |
| `_LOOK=yaw,pitch` | Aim the camera |
| `_TEAM` | Join a team |
| `_SPECTATE` | Spectate |
| `_CAM=x,y,z` | Park the camera at a point |
| `BOOTLEG16_FIRE` | Hold fire |
| `BOOTLEG16_MENU_PAGE` | Open a menu page: `practice`, `join`, `settings`, `crosshair`, `assistant`, `keys`, `voice`, `characters`, `maps` |
| `BOOTLEG16_FAKE_PING` / `_LOSS` | Fake network conditions |

- Put screenshots in the session scratchpad and look at them before saying a change is done.
- Net tests (`net/tests/`) run a real server and clients over localhost and skip themselves
  when de_dust2 isn't installed. `common::Game` drives them; `simulate_network(0, 1.0)` cuts
  a client off.
- Killing processes: `pkill -f` once killed its own shell. Use `ps` and `kill <pid>`.

## Two sessions on one repo

- **Engine session (you):**
  - Netcode, sim, server and the lobby.
  - Loading, notice and reconnect logic.
  - Sound, rendering, models, players, effects, the view model and map sounds.
- **UI session ("Game HUD/UI design concepts"):**
  - Menu pages, the HUD, the buy menu, spectating screens, teammate tags.
  - `status_screens.rs`, `design/mockups`, the `ui/` design system.
- Message it with `SendMessage` (find it with `ListAgents`) before touching its files.
- For a shared interface, you write the logic and its resources, it draws the view. Commit
  both halves together, so the build never sees one without the other.
- `docs/TODO.md` is shared: pull before editing it.
- The UI session is **paused** by the user, and does the release build (`tools/bundle.sh`)
  only when the user says so. Its `docs/ContextHUD.md` was uncommitted at handoff: leave it
  to that session.

## State at handoff (2026-10-17)

All engine work is committed and pushed. Last engine commit: this file. Before it:
`47228f5` (the loading cover) and `1dc724e` (loading, notices and reconnect). The tree has no
engine changes left uncommitted.

## Next (all in `docs/TODO.md`)

### 1. Sound system: one mixer

Today four separate paths play audio:

| Path | File | How it plays |
|---|---|---|
| Game sounds | `client/src/sound.rs` | A `Cue` enum; `play_cues` works out the gain once, at spawn, from `ear = w.client.me().eye()` and GoldSrc falloff |
| Map sounds | `map_sounds.rs` | Its own `gain()`, `one_shot` and `spawn_loop` (the falloff is written a second time) |
| UI tones | `ui/sound.rs` | Synthesized tones |
| Voice | `voice/` | Its own output stream |

All of them use `settings.volume` directly. Build:
- **One play function**, used by every path.
- **Volume buses**, each with a Settings slider: master, effects, ambience, UI, assistant,
  voice.
- **GoldSrc channels** per entity (weapon, body, voice, item, static): a new sound on a
  channel stops the one before it. Today a reload keeps playing after you switch weapons.
- **A voice cap**: limit how many sounds play at once and drop the quietest or oldest.
- **The camera as the listener.** `SpatialListener` is on the camera, but loudness is worked
  out from your own body, so spectating sounds wrong.
- **Live volume**: volume changes must reach sounds that are already playing.
- **Room reverb** from the maps' `env_sound` areas (`roomtype`), the way CS 1.6 changes the
  echo per area. Unverified: check the maps for these entities before building on it.

### 2. Weapon switch shows a character model for a moment

- Hypothesis, NOT verified, in `client/src/viewmodel.rs`, `update`:
  1. On a switch it spawns the new skinned parts with `spawn_skinned`, then calls
     `set_pose` on joints that don't exist until the commands are applied.
  2. So the first frame draws the bind or reference pose.
- If confirmed, fix: spawn the parts hidden, and show them once a pose has been applied.

### 3. Smoke cloud far too see-through

- `client/src/smoke.rs` draws 30 puffs of size 110, with puff alpha `0.9 x` a soft texture
  that falls off to the edge.
- CS uses gas puff sprites and is near-opaque in the middle.
- Make the core opaque. Check it from inside and outside the cloud.

### 4. Footsteps may use the wrong sound

- Reported in play, not confirmed.
- The file-number mapping already matches CS.
- Check the surface lookup in `sound.rs`, `Cue::Footstep`: it traces from the feet to 64
  units below. Unverified whether CS does the same: compare against the disassembly.
- Also check `step_set` and the materials map against CS for each map surface.

### 5. Not yet seen on screen

- The round timer's ticks.
- The full 30 s give-up of reconnecting.

### 6. Possible first-use hitches

- Effects drawn for the first time (explosions, gibs, glass) may compile shaders mid-game.
  Not confirmed.
- If it happens, warm them up behind the loading cover.
