# Context

Where the engine side of bootleg16 stands and how to work on it. Read this first when picking
the work up. The HUD and screens have their own page: `docs/ContextHUD.md`. Open items are in
`docs/TODO.md`, lessons in `docs/GOTCHAS.md`.

## The project

A CS 1.6 remake in Rust and Bevy 0.20, multiplayer first, shipping on Steam (Windows and
Linux). For now it plays on the CS 1.6 files from your own install. It runs at 128 tick.

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
- Run only the tests for what changed.
- Wayland only: run the client with `env -u DISPLAY`.
- No god files. One module and plugin per concern; split a file past about 400 lines.
- Commits: no Claude co-author or attribution. Never `git commit -a`: two sessions share the
  tree, so add only your own files.
- Game copy has no em dashes.
- Everyone has the same hitbox, height, eye and speed. Characters are cosmetic.
- The AI assistant only knows what the player knows. Every new piece of information the
  player can see gets an assistant trigger and lines.
- `dist/` holds Valve files: private, never publish it.
- Map pool: de_dust2, de_inferno, de_mirage (on hold), de_train. Keep Nuke.
- Decide game-dev questions yourself and proceed. The user is not a game-dev expert.

## How fidelity is checked

- Facts come from ReGameDLL (GPL, used for facts only) and from disassembling CS's
  `client.so`, which has DWARF symbols.
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
- **Getting into a match** (`client/src/menu/connect.rs`, `loading.rs`, `notice.rs`):
  1. Starting server, Contacting server, Loading map and Joining, each slow step on its own
     thread (`loading::Job`).
  2. The map is parsed once and shared as `MapData`. Its `world` goes to the client and its
     `geometry` to `level.rs`.
  3. The loading screen stays up until the first snapshot is in and no shaders are compiling.
  4. Every way back to the menu is a `Notice` (title, body, Retry when retrying can work).
- **Connection lost** (`game.rs`, `connection`):
  - After 3 s of server silence, `Reconnecting` appears and `Client::rejoin` says hello
    again, keeping the seat.
  - After 30 s it gives up with a notice.
  - Test: `net/tests/leavers.rs`, `a_dropped_player_rejoins_their_seat`.
- **Buying** (`net/src/server/buy.rs`):
  - Undo buys sells back this round's purchases while you can still buy, as CS2 does.
- **Visuals**:
  - Decals and impacts (`impacts.rs`), sprites and glows (`glows.rs`), explosions, gibs.
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
- **Sound** (`sound.rs`, `map_sounds.rs`, `ui/sound.rs`, voice): see "Next" below.

## Testing in the game

- Practice on one map: `target/debug/client de_dust2 --practice 0`.
- Join a server: `--connect host:port`.
- Screenshot hooks (environment variables):

| Variable | What it does |
|---|---|
| `BOOTLEG16_SCREENSHOT=path.png` | Save one frame (taken at `_AT` seconds) |
| `_LOOK=yaw,pitch` | Aim the camera |
| `_TEAM` | Join a team |
| `_SPECTATE` | Spectate |
| `_CAM=x,y,z` | Park the camera at a point |
| `BOOTLEG16_FIRE` | Hold fire |
| `BOOTLEG16_MENU_PAGE` | Open a menu page: `characters`, `maps`, settings tabs, `voice` |
| `BOOTLEG16_FAKE_PING` / `_LOSS` | Fake network conditions |

- Net tests (`net/tests/`) run a real server and clients over localhost and skip themselves
  when de_dust2 isn't installed. `common::Game` drives them; `simulate_network(0, 1.0)` cuts
  a client off.

## Two sessions on one repo

- **This session (engine):**
  - Netcode, sim, server and the lobby.
  - Loading, notice and reconnect logic.
  - Sound, rendering, models, players, effects, the view model and map sounds.
- **The UI session ("Game HUD/UI design concepts"):**
  - Menu pages, the HUD, the buy menu, spectating screens, teammate tags.
  - `status_screens.rs`, `design/mockups`, the `ui/` design system.
- Message the other session before touching its files. Commit both halves of a shared
  interface together, so the build never sees one without the other.
- `docs/TODO.md` is shared: pull before editing it.
- The UI session is **paused** by the user. It does the release build (`tools/bundle.sh`)
  only when the user says so.

## State on 2026-10-17

All work is committed and pushed (latest `1ac64be`). Waiting on the user: start the sound
work below, or release first.

## Next (all in `docs/TODO.md`)

1. **Sound system.** Today four separate paths play audio, and the distance falloff is
   written twice. Needed:
   - One mixer that everything plays through.
   - Volume buses with Settings sliders: master, effects, ambience, UI, assistant, voice.
   - GoldSrc channels per player (weapon, body, voice, item), where a new sound cuts the old
     one. Today a reload keeps playing after you switch weapons.
   - A cap on how many sounds play at once.
   - The camera as the listener. Today loudness is worked out from your own body, so
     spectating sounds wrong.
   - Volume changes reaching sounds that are already playing.
   - Room reverb from the maps' `env_sound` areas.
2. **Weapon switch** shows a character model on screen for a moment.
3. **Smoke cloud** is far too see-through.
4. **Footsteps** may use the wrong sound (reported in play, not confirmed): check the
   material lookup.
5. **Not yet seen on screen:** the round timer's ticks, and the full 30 s give-up of
   reconnecting.
6. **Shader warm-up** for effects that appear later (explosions, gibs, glass) could hitch
   the first time they are drawn. Not confirmed yet.
