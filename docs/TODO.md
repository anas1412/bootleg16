# Open items

Everything not done yet, in one place. Tick items off (or delete them) as they
land, and add new ones here instead of leaving them only in chat.
⚠ = a value or behaviour still to be checked against CS 1.6's own code (`cs.so`).

## Milestone 1: full 5v5 Competitive match (current goal)

### Weapons (in progress)
- [ ] Server-side shooting: hitscan through map + players, damage, range falloff, penetration
- [ ] Hit groups (head/chest/stomach/arms/legs) and armor (kevlar/helmet) damage rules
- [ ] Fire rate, ammo, reloads, deploy times, weapon switching, dropping/picking up
- [ ] Spread/inaccuracy (moving, jumping, crouching) and recoil patterns from `cs.so`
- [x] Lag compensation history: server rewinds players to what the shooter saw (`Server::rewind`)
- [x] Bullet line traces through exact geometry (`World::trace_line`)
- [x] Hit zones from the real player model's 21 hitboxes, posed (`sim::hitbox`)
- [ ] ⚠ Hitbox pitch: which end of the 9 aim poses is "looking up"
- [ ] Hitboxes follow running/walking leg animation (gait) and per-weapon aim poses; per-model (T vs CT) hitboxes
- [ ] Knife, grenades (HE damage/radius, flash, smoke), shotguns (pellets, `FireBullets`)
- [ ] Weapon move speeds into `Player::max_speed`
- [ ] Breakables (`func_breakable`: dust2 glass/boxes), `func_button`
- [ ] Health, death, spectating
- [ ] ⚠ Shotgun stats (separate fire function), M4A1 silenced-vs-unsilenced call sites
- [ ] ⚠ Pistol/knife move speeds other than Deagle (set in each weapon's `Spawn`)

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
- [ ] Player models and animations (from the install's MDL files) instead of orange boxes
- [ ] First-person weapon models (viewmodels)

### HUD and UI
- [ ] HUD: health, armor, ammo, money, round timer, score, kill feed
- [ ] Scoreboard, buy menu UI, team select, main menu, settings (sensitivity, FOV, keys)

### Networking (`docs/NETWORKING.md`)
- [ ] Anti-wallhack: don't send players the client can't see (server-side visibility)
- [ ] Map check on join (same map file), map change / map vote
- [ ] Stat heuristics for aimbots, match replays, reports
- [ ] 64-tick server option (tick rate is a constant 128 today)
- [ ] Delta-compressed snapshots (bandwidth ≈ 40 KB/s per client at 10 players)
- [ ] Encryption/authentication, NAT traversal, server browser
- [ ] Prediction error smoothing (corrections are applied instantly)

### Sound
- [ ] No audio yet: footsteps (events exist), weapons, doors, ambience, radio

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
- [ ] Standalone release: own assets, own name (`bootleg1.6` is a codename;
      pick e.g. Oxidestrike/Fuseline), legal check before selling
