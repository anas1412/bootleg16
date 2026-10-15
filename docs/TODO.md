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
- [ ] Smoke looks like a solid grey blob: use CS-style soft sprite puffs; smoke should also hide
      players from the server's visibility checks (anti-wallhack)
- [ ] Flash "already flashed" combine rule (simplified to max); HE water rule; grenade bounce sounds
- [ ] ⚠ Flash white-out and HE fireball not yet checked on screen (only the smoke was)
- [ ] Hit flinch/slow-down (`velocity_modifier`) and victim view punch on hit
- [ ] Knife: GoldSrc hull sweep and 8-corner search (approximated by 5 rays now)
- [ ] Shield
- [ ] Breakables (`func_breakable`: dust2 glass/boxes), `func_button`
- [x] CS 1.6 keys: B buy menu (CS layout per side, 0 cancel), Q last weapon, wheel, G drop,
      `,`/`.` ammo, Tab scoreboard; drop/pickup on the ground; gun drops on death
- [ ] hud_fastswitch 0 (CS default: number keys show a selection, mouse1 confirms); only fastswitch 1 now
- [ ] Ground guns are never cleaned up (CS removes them at round start)
- [ ] First-person weapon models (viewmodels) and third-person weapon models
- [x] Weapon sounds
- [x] Bullet holes + dust for every bullet/pellet (server sends wall hits), blood spray (bigger on headshots), blood on the wall behind
- [ ] Own bullet holes predicted locally (now they come back from the server, one ping late); CS decal textures from decals.wad; sparks on metal
- [ ] Kill feed and other events are sent unreliably (a lost snapshot loses them)
- [ ] ⚠ Shotgun ground-recoil ranges (source-only); FAMAS/Glock follow-up timings per spec

### Rounds and economy (`docs/RULES.md`)
- [ ] Teams (T/CT spawns), round flow: freeze time, buy time, round time, round end
- [ ] Buy menu and buy zones; prices and money rules from RULES.md
- [ ] Bomb: plant, defuse (kit), timer, explosion, win conditions
- [ ] MR12 + halftime swap + MR3 overtime at $10,000
- [ ] Loss bonus: per-team ladder (default) and 1.6 shared mode (config)
- [ ] All rule values in a server config file
- [ ] ⚠ Defuse times (10 s / 5 s with kit)
- [ ] ⚠ Living Ts get no money when time runs out (`MarkLivingPlayersOnTeamAsNotReceivingMoneyNextRound` call sites)
- [ ] ⚠ Team buy restrictions (`CanBuyWeaponByMaptype`)
- [ ] ⚠ Hostage money formulas (only for cs_ maps, not milestone 1)

### Players and bots
- [ ] Bots to fill empty slots (navigation on the four maps)
- [x] Player models drawn from the install's MDL files (static aim pose per stance)
- [x] T vs CT models per team (4 classes each, picked by id)
- [x] Player animations: gait (idle/walk/run/crouch) + upper body (aim pitch blend, shoot, flinch), death animation by hit place, corpses (16 kept)
- [ ] Legs turned toward movement direction (gaityaw), jump/swim/ladder gaits, reload anims for others; class choice menu; corpses cleared each round
- [ ] Light models from the map's lightmaps like GoldSrc (they're unlit now)
- [ ] First-person weapon models (viewmodels)

### HUD and UI
- [x] HUD: health, armor, ammo, grenades, kill feed (with names), notices
- [x] Crosshair gap = real spread of the next shot; none on snipers; sniper scope overlay
- [x] Scoreboard by team, clickable buy panel with prices (free in practice), team select (M, 1/2/5/6)
- [x] Main menu: practice (map, dummies, strafe), join server, settings (name, sensitivity, volume); Esc menu
- [ ] HUD: money, round timer, team score (with the round system)
- [ ] Settings: FOV, key binds, crosshair style
- [ ] Spawn view angle from the spawn point (camera starts at yaw 0 now)

### Networking (`docs/NETWORKING.md`)
- [ ] Anti-wallhack: don't send players the client can't see (server-side visibility)
- [ ] Map check on join (same map file), map change / map vote
- [ ] Stat heuristics for aimbots, match replays, reports
- [ ] 64-tick server option (tick rate is a constant 128 today)
- [ ] Delta-compressed snapshots (bandwidth ≈ 40 KB/s per client at 10 players)
- [ ] Encryption/authentication, NAT traversal, server browser
- [ ] Prediction error smoothing (corrections are applied instantly)

### Voice
- [ ] Proximity voice chat (positional, fades with distance like game sounds). Open: everyone
      nearby, or teammates only? (undecided)
      **No radio commands/callouts**: decided 2026-10-15, voice replaces them.

### Sound
- [x] Gunshots (silenced variants, random pitch), reload/draw/pin sounds from model events,
      footsteps by surface/ladder/water, hits (flesh/kevlar/helmet/headshot), deaths,
      grenades (bounce, HE, flash, smoke), knife, dry fire, zoom; 3D with GoldSrc falloff
- [ ] Doors, ambience (`ambient_generic`), bullet ricochets/impacts,
      knife hit sounds, jump/land variants, menu sounds; other players' reload sounds
- [ ] ⚠ Silenced shot loudness/attenuation and footstep volumes are estimates
- [ ] Muzzle flash (view model event 5001)

### Movement leftovers (`docs/MOVEMENT.md`)
- [ ] Water jump-out onto ledges (`PM_CheckWaterJump`)
- [ ] Doors pushing players; door sounds
- [ ] ⚠ Footstep interval (300 ms is the HL convention; read `PM_UpdateStepSound`)
- [ ] ⚠ Ladder face direction from the ladder box (fine for stock ladders)

### Rendering
- [ ] Sky (currently a flat blue clear colour)
- [ ] Lighting looks flatter than the original (overbright/gamma tuning)
- [ ] Translucent brushes (`rendermode`/`renderamt`, e.g. nuke's glass doors draw opaque)
- [ ] Decals, sprites, particles, muzzle flashes
- [ ] Static props/models placed in maps (`env_model`, `cycler_sprite` …)

## Later (after milestone 1)

- [ ] Custom player/weapon skins shared through the server (hash-cached, size
      limits, strict parsing, fixed hitboxes, server and client on/off)
- [ ] In-game map editor (Forge-style: place, test instantly, share)
- [ ] First original map `rust2`: Dust2-style flow, new geometry, own/CC0 textures
- [ ] Engine works from a neutral map form so editor maps and BSPs both load
- [ ] Standalone release: own assets, own name (`bootleg16` is a codename;
      pick e.g. Oxidestrike/Fuseline), legal check before selling
