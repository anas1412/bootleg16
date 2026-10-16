# Gotchas and lessons learned

Things that cost time once. Read before touching the area; add to it when
something surprises you.

## Building on this machine

- **Cap every build**: `systemd-run --user --scope -p MemoryMax=3G -p MemorySwapMax=1G -- cargo build -j 2`.
  `.cargo/config.toml` sets `jobs = 2`. An uncapped Bevy build ran 8 rustc
  processes (~7 GB) and froze the desktop. First Bevy build ~15 min, rebuilds ~1 min.
- Automated screenshot runs open real windows on the user's desktop. They must
  ignore mouse and keyboard (`BOOTLEG16_SCREENSHOT` does that), or the user's
  clicks fire weapons in the test window.
- Time screenshots in seconds (`BOOTLEG16_SCREENSHOT_AT`), not frames: the frame
  rate varies a lot, so "frame 900" was sometimes only 3 s.

## Reverse engineering CS 1.6

- `cstrike/dlls/cs.so` (32-bit) has **full DWARF debug info** (types, enums, struct
  offsets). `cs_amd64.so` has none. Use `gdb -batch -ex 'ptype ...' cs.so` and
  `objdump -d -l -C` (the `-l` line info recovers the **source order** of code
  blocks that the compiler reordered: e.g. the 4 KickBack sets).
- Constants hide as: enums (prices), static tables (`weaponInfo`), immediates
  in `movl $0x...` (floats), `fld 0xADDR` (rodata floats/doubles), and integer
  division "magic numbers" (`0x51eb851f` + `sar 6` = `/200`).
- ReGameDLL_CS is a useful cross-check but **GPL**: read it for facts only, never
  copy code. Its `#ifdef REGAMEDLL_ADD/FIXES` branches change behaviour; the
  original is the `#else` branch. When it disagrees with the binary, the binary wins.
- Client-side cvars (e.g. `cl_movespeedkey 0.52`) live in `cl_dlls/client.so`
  as string pairs; engine cvars (`sv_*`) in `engine_i486.so`/`hw.so`. CS also
  forces some every round (`sv_accelerate 5`, `sv_friction 4`, `sv_stopspeed 75`).

## GoldSrc formats

- BSP texture names in the miptex lump have junk after the NUL: cut at the first NUL.
- Spawn points float above the floor (players drop onto it). Don't assume
  `origin.z - 36` is the floor; trace down first.
- Two kinds of BSP trees: clip hulls 1-3 (player-sized, for movement) and hull 0
  (exact geometry, render nodes + leaf contents: bullets, water, sky). A test
  world must give *both* a root, or line traces silently miss everything.
- GoldSrc traces that **start inside solid report fraction 0**. Bullet
  penetration relies on this (a bullet restarted inside a thick wall is stopped).
- MDL sequence header: `numblends` at +120, `animindex` at +124 (not +116/+120).
- CS player models have **9 aim blend sets** (pitch); level aim is the middle one.
  The rifle aim pose leans the head ~12 units forward: hitbox tests must use the
  posed positions, not "straight ahead at head height".
- `v_` view models are stored **left-handed**; CS 1.6 mirrors them (`cl_righthand 1`).
  Mirroring flips triangle winding, so swap two indices per triangle.
- `p_` weapon models have no animation of their own: pose them from the player's
  skeleton by bone name (`Model::borrow_pose`).
- `p_` models have extra bones the player lacks (`flash`, `Line02`, carrying the
  gun's vertices). Merge like GoldSrc: borrow matching bones, rebuild unmatched
  ones as `borrowed parent x own local`, never leave them at their rest pose.
- Each weapon has its own player body animation set (`ref_aim_ak47`, `_carbine`,
  `_rifle`, `_mp5`, `_onehanded`, ...): pose the body for the weapon in hand.
- Seen from the front, a rifle pointing at the camera looks like a vertical stick.
  Measure model extents before "fixing" orientation.
- `materials.txt` compares the first 12 characters of a texture name; a short
  listed name does *not* match a longer texture name.

## Coordinates and rendering (Bevy 0.20)

- GoldSrc is Z-up: `(x, y, z) -> (x, z, -y)` in Bevy. Yaw about Z equals the same
  angle about Bevy's Y. GoldSrc faces are clockwise: reverse winding.
- GoldSrc pitch is positive looking **down**; Bevy's is positive looking up.
- **Meshes must keep their CPU data** (`RenderAssetUsages::default()`). With
  `RENDER_WORLD` only, Bevy can't compute bounds, falls back to a box at the
  origin, and frustum-culls the whole map whenever the origin is off screen.
- Lightmaps: upload as linear `Rgba8Unorm` (not sRGB), `lightmap_exposure` 2.0,
  camera `Exposure { ev100: -log2(1.2) }` for exposure 1.0, `Tonemapping::None`.
- Bevy 0.20 API changes vs older docs: shaders are WESL; `FontSize::Px(..)`;
  `CursorOptions` is its own component; `MessageWriter<AppExit>`;
  `Assets::get_mut` returns a guard (`if let Some(mut m) = ...`); built-in
  `Lightmap` component with `UV_1`.
- Translucent effects seen from inside (smoke) need `cull_mode: None`.
- Audio: rodio's spatial sink scales loudness by 1/distance^2 (capped at 1). Shrink
  `SpatialScale` (1/4096) so it only pans, and apply GoldSrc falloff
  (`1 - dist * attn / 1000`) yourself. WAV needs Bevy's `wav` feature.
- Reload/draw/pin sounds are **animation events** (id 5004, file name in options)
  inside the `v_` models; event 5001 is the muzzle flash. Play them as frames pass.
- Glock: `shoot3` is the single-shot animation, `shoot`/`shoot2` are burst ones.
- A spectator/chase camera must trace to its spot and stop short of walls.

## Simulation and netcode

- CS clamps your input to the weapon's max speed **before** the crouch/walk
  multipliers (knife crouch-walk is 83 u/s, not 133).
- Jump stamina slows landing speed hard for ~1.3 s; a path that jumps often is
  much slower than 250 u/s. Not a bug.
- Weapon timers run on each player's **command clock** (`input number x tick`), not
  server time, so the client can replay its own shooting exactly.
- Accuracy uses **integer** division (`shots^3 / 200`): it is a step function.
- Recoil caps are reached right after each shot and then decay between shots;
  measure the peak, not a random moment.
- Lag compensation must rewind through **snapshot ticks only** (what clients
  actually blend between), or there's a ~0.6 unit error even on LAN.
- `HashMap` iteration order on the server is random: never rely on client order.
- After bumping `PROTOCOL_VERSION`, rebuild **all** binaries (client, server,
  examples). A stale server rejects new clients, which then just sit "connecting".
- Reload is ignored until the current fire cycle (`next_primary`) has passed.

## Tests

- Write the assertion from CS behaviour, then check the test itself when it
  fails: several "bugs" were wrong expectations (zoom lockout, shotgun cycle
  count, spawn heights, flash going off behind the victim).
- Prefer path length over displacement when checking that a player moved (walls).

## Client structure / UI (2026-10-15)
- Bevy 0.20: `BorderRadius` is a field of `Node` (`border_radius`), not a component.
- An `OnEnter` system that fails and sets `NextState` still lets that frame's `Update` run in the old state:
  in-game sets also need `run_if(resource_exists::<Walker>)` or they panic on the missing resource.
- Player models: GoldSrc draws legs from the gait sequence and the rest from the upper-body one,
  split at the first "Bip01 Spine" bone, in *local* bone space (`Model::player_pose`); mixing model-space
  matrices detaches the torso.
- Player aim blend: 0.5 + pitch/180 with GoldSrc pitch (down positive) — checked: -60 points the gun up.
- Death events arrive before the interpolated snapshot shows the player dead (4 ticks later):
  keep a "dying" set or the live model is redrawn on top of the corpse.

## Performance (2026-10-15)
- Server input queue: the speed-hack budget only allows real time, so a burst of inputs after a client
  hitch (map load) became a permanent backlog = ~120 ms of fake ping. Backlogs that last 1/4 s are dropped.
- KDE Wayland: `PresentMode::AutoNoVsync` still ran at 60; `Mailbox` runs free without tearing.
  FPS readings also drop to 60 whenever KWin throttles an unfocused/occluded test window — don't trust those.
- `Model::posed_mesh` used to decode every texture to RGBA per call (just for UV sizes): huge per-frame cost.
- Animating by rebuilding meshes re-uploads them every frame (`allocate_and_free_meshes` was the top render
  system). Players are GPU-skinned now: vertices in bone space (`Model::bind_mesh`), identity inverse binds,
  one joint entity per bone; bounds come from the bind pose, so skinned parts get `NoFrustumCulling`.
- `ClusterConfig::None` breaks Bevy 0.20 GPU clustering (zero-size texture errors); use `Single`.
- Profiling: `cargo build -p client --features profile` (bevy trace_chrome) into `CARGO_TARGET_DIR=target-prof`
  (a full separate Bevy build, ~45 min at -j 2); the trace JSON is ~1 GB for 10 s. gdb can't attach to a running
  process (ptrace_scope 1): run it under gdb instead.
- Dev profile now builds our crates at opt-level 3 too (opt 1 cost a lot of fps); rebuilds ~3 min.

## Rounds (2026-10-16)
- Client snapshot state lags the server by a few ticks: tests that check "frozen during freeze time" must use
  the server's phase (`Server::round_phase`), not the client's, or they run into the next phase.
- Engine tests (movement, guns) now need a permissive `MatchConfig` (no freeze, money, friendly fire) or
  freeze time and buy zones get in the way: `tests/common::setup` does that; `setup_with` takes real rules.
- Child modules (`server/round.rs` etc.) can use the parent's private fields, but `fn`s in their `impl Server`
  blocks must be `pub(super)` for `server/mod.rs` to call them.
- `info_map_parameters bombradius` (de_nuke 350): C4 damage = radius, reach = 3.5 × radius, through walls.
- Head height: in the CS models the head centre is ~+19 over the origin, i.e. at eye level (+17), and leans
  forward of the origin by 1-12 units depending on the weapon pose. Shooting someone's back at eye level can
  hit the upper chest first. Hitboxes are posed per weapon (`Target::weapon`).
