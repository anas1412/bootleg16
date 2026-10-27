# Open items

Everything not done yet, in one place. Tick items off (or delete them) as they
land, and add new ones here instead of leaving them only in chat.
⚠ = a value or behaviour still to be checked against CS 1.6's own code (`cs.so`).

## Milestone 1: full 5v5 Competitive match (current goal)

### Weapons (`docs/WEAPONS.md`)
- [x] All 22 guns: fire rate, ammo, reloads (shotguns shell by shell), deploy, switching
- [x] Accuracy, spread, KickBack recoil and punch, fire modes (silencers, bursts), scopes
- [x] Server-side bullets: FireBullets3 penetration, range falloff, materials, hit zones,
      pass-through players; shotgun pellets; armor and hit group damage; kills
- [x] Lag compensation; client predicts its own weapons (ammo/recoil) on the command clock
- [x] Knife slash/stab with backstab
- [x] HUD (health/armor/ammo/weapon), crosshair, tracers, kill feed
- [x] Grenades: throw formula, GoldSrc bounce physics, 1.5 s fuse; HE (100 dmg, 350 radius,
      armor 1x), flash (1500 radius, line of sight, facing rules), smoke (21 s, pops on the ground)
- [x] Grenade models in flight, HE fireball, flash white-out, smoke cloud, HUD counts
- [x] Smoke: soft sprite puffs that billow and thin out; smoke blocks the server's sight checks
- [x] Flash "already flashed" combine rule and the HE/flash water rule (`grenades::combine`, `blast_reaches`)
- [x] Grenade bounce sounds
- [x] Flash white-out and HE explosion checked on screen (HE: CS's fexplo/eexplo/steam1 sprites, explosions.rs)
- [x] Tagging: hits slow you (0.5, or 0.65 large flinch) and kick the view, as cs.so (`player::hit_by`)
- [x] Knife: GoldSrc line then crouch-box sweep (`trace_melee`); the 8-corner search only moves the impact point, skipped
- [x] Breakables: shot/blast damage, dust2/train crates break with the bomb, restored each round
- [x] `func_button` / `func_rot_button` (E or touch, fires its target: sim/src/doors.rs)
- [x] Breakable gibs: CS's gib models (gibs.rs)
- [x] CS 1.6 keys: B buy menu (CS layout per side, 0 cancel), Q last weapon, wheel, G drop,
      `,`/`.` ammo, Tab scoreboard; drop/pickup on the ground; gun drops on death
- [x] Ground guns cleared at round start
- [x] First-person weapon models (viewmodels) and third-person weapon models
- [x] Weapon sounds
- [x] Bullet holes + dust for every bullet/pellet (server sends wall hits), blood spray (bigger on headshots), blood on the wall behind
- [x] Own bullet holes predicted locally; CS decal textures from decals.wad (holes, blood, scorch, infodecal)
- [x] Kill feed, round and bomb news reliable (numbered, resent until acked); team join resent until confirmed

### Rounds and economy (`docs/RULES.md`)
- [x] Teams (T/CT spawns), round flow: warmup, freeze time, buy time, round time, round end (net/src/rounds.rs, server/round.rs)
- [x] Buy zones (func_buyzone, or 200 units around own spawns), buy time, prices, team-only items, money
- [x] Bomb: C4 to one T each round, drop (G/death) and pick up, plant (3 s in a site, frozen), defuse (use, 10/5 s kit, frozen), 40 s timer, explosion (map bombradius, ×3.5 reach), win conditions
- [x] MR12 + halftime swap + MR3 overtime at $10,000 (unit tests)
- [x] Loss bonus: per-team ladder (default) and 1.6 shared mode (config)
- [x] All rule values in a server config file (config/server.cfg, `server --config`)
- [x] Round HUD: score, clock, freeze/bomb states, money +/-, announcements, plant/defuse bars; bomb models, beeps, sounds
- [x] Shop trimmed to the chosen list (docs/RULES.md 4); ammo not sold, refilled each round
- [x] Defuse reach as CS `PlayerUse`: 64 from your centre to the bomb's, within 45 degrees of your view (no wall check in CS either)
- [x] Team money (CS2): `$` column on the scoreboard, teammates' money in the buy menu header (red under a rifle)
- [x] Hitboxes follow each weapon's aim pose (head moved up to ~7 units between poses; the old test simply aimed 9 units over the head)
- [x] Hitboxes follow the drawn animation (sim::anim, shared by client and server: gait, spine twist, jump, shoot/reload/flinch), rewound with lag compensation like positions
- [x] Defuse times 10 s / 5 s with kit (ReGameDLL `DefuseBombStart`)
- [x] Living Ts get no money when time runs out (after `#Target_Saved`)
- [x] Team buy restrictions match `CanBuyWeaponByMaptype` (de_ maps)

### Players and bots
- [x] ~~Bots~~: decided 2026-10-16: no bots, ever. Only standing dummies in practice.
- [x] Player models drawn from the install's MDL files (static aim pose per stance)
- [x] T vs CT models per team (4 classes each, picked by id)
- [x] Player animations: gait (idle/walk/run/crouch) + upper body (aim pitch blend, shoot, flinch), death animation by hit place, corpses (16 kept)
- [x] Legs turned toward movement direction (gaityaw) with the spine twisting back, jump animation, others' reload animation, corpses cleared each round (no water maps: no swim; CS has no ladder gait)
- [x] Models lit from the map's lightmaps like GoldSrc (studio.rs)

### HUD and UI
- [x] HUD: health, armor, ammo, grenades, kill feed (with names), notices
- [x] Crosshair gap = real spread of the next shot; none on snipers; sniper scope overlay
- [x] Scoreboard by team, clickable buy panel with prices (free in practice), team select (M, 1/2/5/6)
- [x] Main menu: practice (map, standing dummies), join server, settings (name, sensitivity, volume); Esc menu
- [x] FPS + ping counter (top left); Max FPS and VSync settings
- [x] CS2 "Show Team ID Through Walls": pip + name over teammates through walls (client/src/teammates.rs; Settings: off / pips / pips + names). Safe: hidden enemies are never sent
- [x] Team IDs: health bar under the name (red at 25 and under), mic above it while they talk
- [x] Settings: key binds (crosshair style done; FOV stays fixed at 90° at 4:3 for fairness, as in CS)
- [x] Viewmodel GPU-skinned (mirrored bones through a reflection transform)
- [ ] Try drawing the viewmodel without a second camera (depth range trick)
- [x] Spawn view angle from the spawn point

### Networking (`docs/NETWORKING.md`)
- [x] Anti-wallhack: enemies out of line of sight aren't sent (net/src/server/visibility.rs): rays from your eyes to their grown box, 0.15 s look-ahead, 0.25 s grace; teammates/dead/spectators see all; footsteps and shots still sent within hearing range
- [x] Visibility: closed doors, crates and smoke hide players; glass and open doors don't
- [x] Map check on join: the server sends its map file's hash (FNV-1a) in Welcome; a client with a different file goes back to the menu
- [x] Match recordings (net/src/demo.rs, replay with `server --play`), reports with recording + tick
- [x] Aim checks (net/src/server/aimcheck.rs): inhuman reactions and snap hits flagged for review, never auto-ban
- [ ] Tune the aim checks against real players: thresholds come from bot tests only (honest bots never trip them); check flags against recordings once people play, before trusting them
- [x] A review tool for reports: `server --reports reports.jsonl`, `server --review reports.jsonl <n>` (from 10 s before)
- [ ] de_mirage (community map, BubkeZ): on hold (2026-10-16); loads, plays and has its A/B table; add to MAP_POOL when wanted
- [x] de_train: A is the big middle yard, B the lower yard by the CT spawn (from the radar)
- [x] Delta-compressed snapshots (net/src/delta.rs): 10 players moving, 16.5 to 5.3 KB/s per client
- [x] Official servers only (decided 2026-10-16), no community servers, no server list:
      casual queue (solo or party of 2-5, party on one team, no mid-match joins, reconnect to your match),
      custom games (invite up to 10, Team A/B lobby, knife round then winners pick side); invite codes until accounts
- [x] Reserved-match mode on the game server (only the match's players, on their teams) + knife round + side vote
- [x] Matchmaker / lobby service (parties, codes, queue, free-server pool); Play page replaces the server list
- [x] Lobby: OS randomness for tickets, regions/ping in the queue, standby lobby, saved state, hidden skill matchmaking, ready check, cooldowns, vote kick, reconnect
- [ ] Accounts: Steam (sign-in done with Valve's test app 480); our own App ID before release
- [x] One-command hosting: ./host.sh (Docker, fleets, standby lobby, firewall, rolling updates)
- [ ] Home hosting without port forwarding: UPnP from the server, relay through the lobby as fallback
- [x] Host button: a match or deathmatch inside the game on UDP 27015, friends join via Steam "Join game" (rich presence `+connect <public ip>`, found by STUN) or by typing the address
- [x] Bodies: server-side, all match; a step-over bump; E carries one as a front shield (75% of a bullet stopped, 0.6x speed)
- [ ] Bodies in match recordings (replays show none yet: `replay.rs`)
- [ ] Host: without port forwarding (Steam Datagram Relay / SteamNetworkingSockets would remove it), and Steam "Join game" when the friend's game isn't running launches App 480 (Spacewar), not us, until we have our own App ID
- [x] Server protection: join cookie (stateless, no spoofed slots), per-IP packet budget, max 6 players per IP, 1400-byte cap, padded info/list requests (no amplification), lobby rate limits
- [x] Kernel flood filtering: ./host.sh firewall (nftables)
- [ ] Host-side DDoS filtering: choose a provider with game DDoS protection (OVH Game, Path.net); a user decision
- [x] Encryption/authentication (X25519 + ChaCha20-Poly1305, per-seat keys, sealed server <-> lobby)
- [x] Prediction error smoothing (corrections glide out over ~0.1 s, big jumps snap: client game.rs)

### Voice
- [x] Voice chat, two push-to-talk keys (decided 2026-10-16): team voice (K) reaches your team
      anywhere; proximity voice (X) reaches everyone alive nearby, enemies too, fading out by
      `voice_range` and panned when you see the speaker. Opus 24 kbit/s, server relays.
      Settings page: on/off, rebind keys (incl. mouse buttons), mic, mic volume, voice volume, mic test.
      **No radio commands/callouts**: decided 2026-10-15, voice replaces them.
- [x] Voice backend: per-player volume and mute (`VoiceSettings::set_player_gain`), open mic + noise gate, proximity voice through walls muffled
- [x] Voice UI: per-player mute/volume on the scoreboard, open mic and gate on the Voice page
- [ ] Voice: only checked automatically (relay test, codec round trip); needs a real two-person test

### Sound
- [x] Gunshots (silenced variants, random pitch), reload/draw/pin sounds from model events,
      footsteps by surface/ladder/water, hits (flesh/kevlar/helmet/headshot), deaths,
      grenades (bounce, HE, flash, smoke), knife, dry fire, zoom; 3D with GoldSrc falloff
- [x] Doors, ambience (`ambient_generic`, triggered ones too), `env_spark`, bullet impact and
      ricochet sounds (`EV_HLDM_PlayTextureSound`/`GunshotDecalTrace`), knife hits, jump/land
- [x] Gunshot volumes and attenuations from client.so (`EV_Fire*`, `fire_attenuation`); footstep, jump and landing volumes from `PM_UpdateStepSound`/`PM_CheckFalling`, left and right feet
- [x] Muzzle flash at the model's muzzle attachment (own view model and other players)
- [ ] One mixer for everything: game sounds, map sounds, UI tones and voice each play their own
      way now (falloff written twice). One play path, one listener
- [ ] Volume buses with Settings sliders: master, effects, ambience, UI, assistant, voice
- [ ] Sound channels as GoldSrc (per player: weapon, body, voice, item): a new sound cuts the old. Done for gunshots (weapon channel, 2026-10-21); body, voice and item still overlap. Note (2026-10-21, HUD_StudioEvent): CS 1.6 plays the gun model's sounds (event 5004) as one-shots at a place, so a reload sound that has started keeps playing after a switch in CS too; later ones never fire because the animation changed (we do the same)
      one on its channel (switching weapons mid-reload still plays the reload now)
- [ ] Voice limit: cap how many sounds play at once, drop the quietest/oldest (a 10-player
      fight can spawn hundreds now)
- [ ] Listener is the camera: falloff is worked out from your own body, panning from the camera,
      so spectating hears from the wrong place
- [ ] Volume changes reach sounds already playing (only map ambience does now)
- [x] Room reverb (2026-10-21, `client/src/reverb.rs`, not heard in game yet): our maps have no `env_sound`, so CS 1.6 plays them dry; ours is automatic like Source/CS2 (rays from the ears every 0.2 s: open sky dry, enclosed spaces echo, bigger rings longer), on game sounds and voice chat. Tune `room_of` by ear.
- [x] Footsteps compared with CS 1.6 (2026-10-21): timings, volumes and the surface table match; the surface lookup was fixed (`2035856`). Was: Footsteps may use the wrong sound (reported in play, not confirmed): check the surface
      material lookup and the files per material against CS

### Movement leftovers (`docs/MOVEMENT.md`)
- [x] Door sounds (`movesnd`/`stopsnd`), button clicks: client/src/map_sounds.rs
- [x] Footstep timing as `PM_UpdateStepSound` (3D speed over 150; 300 ms, ladder 350, wading 600, +100 crouched or on a ladder; 400 ms wait after moving slowly)

### Rendering
- [x] Sky: the map's skybox (TGA faces resampled into a cubemap)
- [x] Lighting through GoldSrc's light and texture gamma tables (level.rs)
- [x] Translucent brushes (`rendermode` 1/2/3/5 by `renderamt`)
- [x] Decals, sprites (env_glow, explosions), sparks, muzzle flashes
- [x] Map props in the pool maps: env_glow, env_spark, infodecal (no env_model/cycler in them)
- [ ] Switching weapons briefly shows a character model on screen (a third-person model
      drawn in view for a frame or so)
- [x] Smoke as CS 1.6 (`5bed563`, not yet seen on screen). Was: Smoke grenade cloud is far too transparent: should block sight like CS 1.6's

### UI (Interference v1, the Bevy client: docs/UI.md; its mockups were removed, v2 is redone for Godot)
- [x] Scoreboard data: `Score` has assists, MVPs, headshots, damage and rounds (ADR), server-measured
      ping, money (own team only)
- [x] Scoreboard columns for them (K D A ADR HS% MVP $ PING)
- [x] Round history from the server (`ServerMsg::History`, `Client::round_history`); the scoreboard
      should switch to it (UI session)
- [x] Seen on screen (faked moments in screenshot runs): low-health heartbeat, reload line, money
      delta, vote panel, voted out announcement; also hit wash, meter trail, uplink, voice rows,
      status icons, match found, Practice
- [ ] ⚠ Timer ticks: not yet seen on screen
- [x] AI assistant lines: 6 per trigger per assistant (601), incl. mvp, flashed, team_out, clutch_won
- [ ] AI assistant: generated lines through OpenCode, free models first (plan in design notes)
- [x] Character select screen on the main menu (CS 1.6 class portraits for now); the Maps page was removed (2026-10-21, no point for it)
- [ ] Character select with our own models (when they exist)

## Later (after milestone 1)

- [ ] Smoke on the GPU (one mesh per cloud, puffs moved in a vertex shader): tried 2026-10-22, it
      rendered but the cloud came out see-through (a player inside showed as a silhouette) even
      with the standard material's fragment path; the old per-puff smoke stays until that's
      understood. The attempt: one quad per puff, puff data in NORMAL/UV_1/TANGENT, `now` uniform.

- [ ] (low priority) Callouts: place names per map area (GoldSrc maps have none: our own zone data per
      map), shown under the radar ("A LONG"), on teammates' voice rows and in the assistant's
      relay lines; bomb site A/B letters and dead teammates' last position on the radar
- [ ] Custom player/weapon skins shared through the server (hash-cached, size
      limits, strict parsing, fixed hitboxes, server and client on/off)
- [ ] In-game map editor (Forge-style: place, test instantly, share)
- [ ] First original map `rust2`: Dust2-style flow, new geometry, own/CC0 textures
- [ ] Engine works from a neutral map form so editor maps and BSPs both load
- [ ] Modernized look: new maps and character models replace the CS assets
      (UI direction E in `design/mockups/e.html`; update `docs/UI.md` to E)
- [ ] Four maps per `docs/MAPS.md`: port (Train structure), data_center (Dust2),
      bank (Mirage), high_rise (Inferno). Nuke dropped.
- [ ] Renderer: PBR texture overrides by WAD name, glTF props, fog/bloom/grading
- [ ] Factions Bureau (cyan) vs Strays (pink) replace CT/T in UI and code
- [ ] Characters picked once in the main menu, independent of team; outfit
      follows the team: Bureau = suits for everyone (women too), Strays = street-tactical
      (bulky, layered; Bureau silhouette narrow). Each keeps a signature trait in both.
      Roster v1, named by archetype, all adult: Gyaru, Femme Fatale, Tsundere (F);
      Yakuza, Idol, Hacker (M).
      Character select mocked in `design/mockups/e.html?s=char`.
      Everyone has the same height, hitbox, eye height and speed (decided 2026-10-16): a
      shorter character would be harder to hit; looks may differ a few percent only.
- [ ] Standalone release: own assets, own name (`bootleg16` is a codename;
      pick e.g. Oxidestrike/Fuseline), legal check before selling

  - Own player/weapon models later (Blender + Blender MCP, glTF; 1 weapon end to end first); keep the CS 1.6 models until the game is finished (decided 2026-10-16)
## Tick rate (64 or 128)
- Done: per-server rate (`tick_rate` in server.cfg, `server --tick 64`), clients follow the Welcome, snapshots stay 64/s, demos and replays use the recording's own rate. Protocol v32.
- Aim checks (`aimcheck.rs`): reaction times are whole ticks (15.6 ms steps at 64) and the flick window (`FLICK_TICKS`, 8 inputs) is twice as long in time at 64. Retune against real players.
- Lobby: servers at different rates must not share a queue unless the lobby knows each server's rate.
- Not done: sim and net test loops still count ticks as 128 (`net/tests/`, `sim` tests); only the multiplayer test runs at 64. README/NETWORKING.md need `--tick`.

## Compared with CS 1.6 (decompiled with REA, 2026-10-21)
- Done: footsteps (`PM_UpdateStepSound`, `PM_PlayStepSound`, `PM_CatagorizeTextureType`), smoke (`CreateSmokePop`, `CreateGasSmoke`, `CPartSmokeGrenade`), view bob and stair smoothing (`V_CalcBob`, `V_CalcNormalRefdef`: we had neither).
- Decided (2026-10-21): keep our crosshair (the real spread), not CS's. For reference: CS 1.6's (`CHudAmmo::DrawCrosshair`) is not the real spread: a per-weapon gap that each shot pushes out (capped at 15), shrinking by 1.3% + 0.1 per frame, doubled in the air, x1.5 when moving faster than the weapon's limit, halved crouched; its lines grow with it and it fades to 120 alpha while firing. Ours shows the real spread of the next shot. Pick one (the HUD belongs to the UI session).
- Not seen on screen yet: the new smoke, bob and stair smoothing.

## Window and settings (2026-10-18)
- Display: a window at a chosen size (Settings, Window size: 720p to 4K) or borderless (no frame, screen-sized; Settings or Alt+Enter), never the compositor's fullscreen mode (it took KDE down on the NVIDIA+Intel laptop, see GOTCHAS). Borderless is untested on that laptop: default is still a 1920x1080 window until it is, Settings has a row (`client/src/window.rs`). Default window 1920x1080.
- The crash on a big window was NVIDIA's Vulkan on Wayland ("explicit sync is used, but no buffer is attached", driver 615, KDE): fullscreen or maximizing killed the game. Fixed by setting `__NV_DISABLE_EXPLICIT_SYNC=1` at the start of `main` (Linux). Windows is not affected as far as we know.
- Hover and clicks dead after a resize: the 3D menu (two 2D cameras drawn into images, shown on tilted surfaces) kept stale UI layout when the images were resized. Removed: the menu is one flat UI again (`menu/stage3d.rs` is gone; Esc back to the main menu lives in `menu/mod.rs`). The Persona-style swing into 3D is dropped for now; bring it back only as something simple that does not render the UI into images.
- The in-game HUD visor still draws the HUD into an image and bends it (`hud/visor.rs`); it has no hover, so it was not touched. Remove it too if it causes trouble.
- Test hooks: `BOOTLEG16_RESIZE=w,h` (resize 4 s in) or `drag` (a new size every frame).

## Buy menu weapon pictures (add-on art)
- The add-on's weapon pictures (`640hud*.spr`) have each weapon's name drawn into the image, and some sheets keep Valve's striped panels. `hud/icons.rs` `clean_gun_sheet` strips the stripes and punches out the names, which leaves speckle where the name overlapped the gun. Interim only, not shippable.
- Proper fix: render the weapon's own `w_*.mdl` (the add-on has them) into a small image per weapon (3D icon, no baked text). The UI session owns the buy menu: agree the size and style with it first.

## Future plans: own art and maps (after the game is playable online; decided 2026-10-17)
Priority now: playable online. These wait. Everything below replaces what is still Valve's or the add-on's.
- **Maps (the real rights blocker):** the maps and radar overviews in `gamedata/` are still Valve's. Plan is `docs/MAPS.md`: measure the base map (numbers only), generate a greybox `.map`, compile with VHLT or SDHLT, polish in TrenchBroom. First map: `data_center` (Dust2 structure). Install notes: TrenchBroom from Flathub or AUR `trenchbroom-appimage`; compiler `hlcsg`, `hlbsp`, `hlvis`, `hlrad` in `tools/vhlt/` (no verified download yet). Compile with `-nowadtextures`; the game finds textures by name in `gamedata/*.wad`. Maps embed their own textures (Inferno: all 97), so retexturing a Valve map is not enough. A small tool to build our own `.wad` from PNG textures is needed.
- **AI-assisted art (Higgsfield, connected as an MCP):** concept art, skyboxes, seamless textures, posters and site letters, effect sprites (smoke, explosion, muzzle flash, decals), loading and menu backgrounds, character portraits, store art. Needs: a locked style prompt per batch, a seamless-tile check, a green-screen step for sprites. Check Higgsfield's terms (commercial use, ownership) before shipping. Steam asks for disclosure of AI-generated content.
- **3D models:** props via image to 3D (Meshy, Tripo or Hunyuan3D), cleaned in Blender (fewer polygons, UVs, glTF export). Hero weapons and characters need an artist or CC0 packs, not AI meshes (same hitbox, animations, first-person quality). Engine gap: the game loads only GoldSrc `.mdl`; a glTF loader and render path is needed first. Test the pipeline on one prop.
- **Still Valve's or unverified:** radar images (`overviews/`), anything in the add-on that is Valve's (`cstrike.wad`, `halflife.wad`, `decay.wad`, ...): decide per file; the add-on's author is unknown to us. `gamedata/` stays out of git.
- **Map textures are Valve's originals (2026-10-22):** the add-on's WADs were replacement textures in other sizes and colours (dust2 sign panels stood out, Inferno looked wrong), so `cs_bdog`, `cs_cbble`, `cs_dust`, `cstrike`, `decals`, `de_vertigo`, `halflife`, `itsitaly`, `liquids`, `tempdecal` and `xeno.wad` were replaced from a CS 1.6 install (every texture of the four maps is found at the size the map asks). `barney`, `decay`, `opfor` are still the add-on's. Same blocker as the maps.
- **Weapon icons to redo:** Galil, Scout and AWP look generic (see icons section); regenerate with each gun's main feature in the prompt.

## Reported in play (2026-10-17), not fixed yet
- **Hands:** on some view models a hand is missing (AK-47: the left hand shows after the animation-name fix, the right hand is not confirmed). The add-on models split the arms into several body parts (`leftarm1`, `leftarm2`, `rightarm1`...): check which ones `spawn_skinned` draws and whether some are alternatives.
- **Tracer lines:** the yellow bullet tracer lines when shooting do not start or end where the shots go (muzzle origin vs the shot's start, and the end point). Check `client/src/` tracer code against the viewmodel muzzle attachment and the server's shot origin.
- **Collision:** "collision whatsoever" (details not given yet): ask for the places. The world collision is `sim::collision` (BSP hulls); the added map data is Valve's maps unchanged, so suspect model or door collision first.
- **Animation names:** done with `tools/sequence_names.py` (rules in `tools/sequence_names.txt`); run it again on any new `v_*.mdl` added to `gamedata/models/`.

## Hosting at scale (decided 2026-10-18: later, not now)
Now: one server box, the game server as a systemd service. Only when we run several hosts or need the pool to grow and shrink on its own:
- [ ] Kubernetes on each host (k3s), with Agones on top (game server fleets: a Fleet keeps N ready servers, the matchmaker allocates one, crashed servers are replaced, autoscaler keeps a buffer). The server calls the Agones SDK (Ready, Allocated, Shutdown).
- [ ] k3d only on the dev laptop, to test manifests before they reach a real server. Not on servers.
- [ ] Decide between Agones and our own small allocator in the lobby (less to learn, but we maintain pool, crash replacement and scaling ourselves).

## Game data rules (2026-10-17: we are making a game, not a mod)
- **Rights: CSO (Nexon) assets in use (2026-10-22):** the six female characters, `sound/player/female/`, `weapons/knife_girl_draw.wav` and every first-person model `models/v_*.mdl`, the eight `characters/f_*` (models `models/player/f_*`, portraits) (CS 1.6 guns with CSO female arms, from the 2011 CSO female pack). Replace before shipping.
- One naming rule: every file and folder name in `gamedata/` is lowercase (`tools/normalize_names.py`); the game lowercases its requests (`net::gamefiles::paths`, map WAD names). Player models carry their own textures (`tools/merge_model_textures.py`); animation names follow `tools/sequence_names.txt`. Run the three tools on any new data.
- **Rights: the add-on contains sound packs from other games.** Folders under `sound/weapons/` are named for Call of Duty / Modern Warfare 2019 (`galil_arm_cod17`, `mw2019_grenades`, `scout_mw2019`), a Minimi, a FAMAS F1 and a "Slayer5530's G3SG-1" pack. These cannot ship: find which the game uses (`client/src/sound.rs`) and replace them with our own or CC0 sounds; delete the unused ones from the data folder.
- **Weapon sounds are normalised** (2026-10-17): `tools/sound_names.py` (rules `tools/sound_names.txt`: one folder per weapon under `sound/weapons/`, renamed in the models' events too) and `tools/normalize_sounds.py` (shots peak at about -2 dBFS, reloads and bolts about -9 dBFS). Run both on any new weapon sounds. The USP model also names sounds the Source way (`Weapon_USP.Deploy`...): 8 of 17 are not found, so those USP reload/deploy sounds are silent until mapped or replaced. `v_galil` names `magmove.wav` and `v_sg550` `sg550_bipod.wav`, which don't exist.

## Godot port (plan: ~/.claude/plans/why-are-you-rushing-parsed-thacker.md)
- [x] P0: `gdbridge` GDExtension (Session: practice server + predicted client), `client_core` (view maths), box-brush `World::from_boxes`, `Server::with_world`, Godot test room, Linux export
- [ ] P0: Windows export needs `gdbridge.dll` built on Windows (CI, P6)
- [x] P1: map format (.bmap: convex brushes, glass, sky, water, ladders, zones, breakables, doors) made in Godot (`godot/tools/export_map.gd`), server, bots and client on it; the network tests play on our test arena
- [ ] P1 leftovers: buttons and triggers in .bmap; convex shapes other than boxes in the exporter; the BSP format tests (`sim/tests/maps.rs`, nuke glass) go with the BSP loader in P6
- [x] P2: identity marks, design system (`design/system/tokens.json` -> `godot/ui/tokens.gd` + the page), Godot menu (Play, Settings with graphics presets), pause, lobby connection (`Lobby` class + `LobbyLink`: party, casual queue, custom, codes, invites, ready check, rejoin)
- [ ] Matches can't start for Godot players yet: `MAP_POOL` (net/src/config.rs) is Valve's maps and the Godot game only has `test_arena.bmap`, so the lobby finds no map everyone has. Moves to our own maps in P3 (de_datacenter)
- [ ] Godot: player name setting (until Steam names), Steam in the Godot client (overlay invites, presence, auth ticket), first-launch graphics preset from `scripts/bench.gd`
- [ ] P4: tests still load Valve's player model (hitboxes) and `materials.txt`
- [ ] Factions bound to characters (user, 2026-10-27): team = your character's faction (Bureau defends, Strays attack), no halftime swap, no knife round for sides, no team select; queue parties on one faction, custom games across factions; per-map side balance from bot-match win rates
