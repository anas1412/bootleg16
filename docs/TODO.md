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
- [ ] Flash "already flashed" combine rule (simplified to max); HE water rule; grenade bounce sounds
- [ ] ⚠ Flash white-out and HE fireball not yet checked on screen (only the smoke was)
- [x] Tagging: hits slow you (0.5, or 0.65 large flinch) and kick the view, as cs.so (`player::hit_by`)
- [ ] Knife: GoldSrc hull sweep and 8-corner search (approximated by 5 rays now)
- [ ] Shield
- [x] Breakables: shot/blast damage, dust2/train crates break with the bomb, restored each round
- [ ] `func_button`; breakable gibs (only coloured debris now); bullets lose no damage through glass
- [x] CS 1.6 keys: B buy menu (CS layout per side, 0 cancel), Q last weapon, wheel, G drop,
      `,`/`.` ammo, Tab scoreboard; drop/pickup on the ground; gun drops on death
- [ ] hud_fastswitch 0 (CS default: number keys show a selection, mouse1 confirms); only fastswitch 1 now
- [x] Ground guns cleared at round start
- [ ] First-person weapon models (viewmodels) and third-person weapon models
- [x] Weapon sounds
- [x] Bullet holes + dust for every bullet/pellet (server sends wall hits), blood spray (bigger on headshots), blood on the wall behind
- [ ] Own bullet holes predicted locally (now they come back from the server, one ping late); CS decal textures from decals.wad; sparks on metal
- [x] Kill feed, round and bomb news reliable (numbered, resent until acked); team join resent until confirmed
- [ ] ⚠ Shotgun ground-recoil ranges (source-only); FAMAS/Glock follow-up timings per spec

### Rounds and economy (`docs/RULES.md`)
- [x] Teams (T/CT spawns), round flow: warmup, freeze time, buy time, round time, round end (net/src/rounds.rs, server/round.rs)
- [x] Buy zones (func_buyzone, or 200 units around own spawns), buy time, prices, team-only items, money
- [x] Bomb: C4 to one T each round, drop (G/death) and pick up, plant (3 s in a site, frozen), defuse (use, 10/5 s kit, frozen), 40 s timer, explosion (map bombradius, ×3.5 reach), win conditions
- [x] MR12 + halftime swap + MR3 overtime at $10,000 (unit tests)
- [x] Loss bonus: per-team ladder (default) and 1.6 shared mode (config)
- [x] All rule values in a server config file (config/server.cfg, `server --config`)
- [x] Round HUD: score, clock, freeze/bomb states, money +/-, announcements, plant/defuse bars; bomb models, beeps, sounds
- [x] Shop trimmed to the chosen list (docs/RULES.md 4); ammo not sold, refilled each round
- [ ] Defuse needs only reach + looking roughly at it; CS traces +use to the bomb entity
- [ ] Buy menu: "you can't buy now" feedback; CS2-style team money on the scoreboard
- [x] Hitboxes follow each weapon's aim pose (head moved up to ~7 units between poses; the old test simply aimed 9 units over the head)
- [ ] Hitboxes don't follow shoot/reload/flinch upper-body animations or gait (aim pose frame 0 only)
- [ ] ⚠ Defuse times (10 s / 5 s with kit)
- [ ] ⚠ Living Ts get no money when time runs out (`MarkLivingPlayersOnTeamAsNotReceivingMoneyNextRound` call sites)
- [ ] ⚠ Team buy restrictions (`CanBuyWeaponByMaptype`)
- [ ] ⚠ Hostage money formulas (only for cs_ maps, not milestone 1)

### Players and bots
- [x] ~~Bots~~: decided 2026-10-16: no bots, ever. Only standing dummies in practice.
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
- [x] Main menu: practice (map, standing dummies), join server, settings (name, sensitivity, volume); Esc menu
- [x] FPS + ping counter (top left); Max FPS and VSync settings
- [x] CS2 "Show Team ID Through Walls": pip + name over teammates through walls (client/src/teammates.rs; Settings: off / pips / pips + names). Safe: hidden enemies are never sent
- [ ] Team IDs: health/equipment like CS2's option 2; fade with distance
- [ ] Settings: FOV, key binds, crosshair style
- [x] Viewmodel GPU-skinned (mirrored bones through a reflection transform)
- [ ] Try drawing the viewmodel without a second camera (depth range trick)
- [x] Spawn view angle from the spawn point

### Networking (`docs/NETWORKING.md`)
- [x] Anti-wallhack: enemies out of line of sight aren't sent (net/src/server/visibility.rs): rays from your eyes to their grown box, 0.15 s look-ahead, 0.25 s grace; teammates/dead/spectators see all; footsteps and shots still sent within hearing range
- [ ] Visibility ignores doors and glass (only the static world blocks sight): safe, but leaks players behind closed doors; smoke doesn't hide either
- [x] Map check on join: the server sends its map file's hash (FNV-1a) in Welcome; a client with a different file goes back to the menu
- [ ] Map change / map vote; downloading the server's map
- [ ] Deathmatch is de_dust2 only for now (decided 2026-10-16); later: other maps, or a new map each time someone reaches the kill limit; show the server's map on the Practice page
- [x] Match recordings (net/src/demo.rs, replay with `server --play`), reports with recording + tick
- [x] Aim checks (net/src/server/aimcheck.rs): inhuman reactions and snap hits flagged for review, never auto-ban
- [ ] Tune the aim checks against real players: thresholds come from bot tests only (honest bots never trip them); check flags against recordings once people play, before trusting them
- [ ] A review tool for reports: list reports.jsonl, open the recording at the tick
- [ ] 64-tick server option (tick rate is a constant 128 today)
- [ ] Delta-compressed snapshots (bandwidth ≈ 40 KB/s per client at 10 players)
- [x] Official servers only (decided 2026-10-16), no community servers, no server list:
      casual queue (solo or party of 2-5, party on one team, no mid-match joins, reconnect to your match),
      custom games (invite up to 10, Team A/B lobby, knife round then winners pick side); invite codes until accounts
- [x] Reserved-match mode on the game server (only the match's players, on their teams) + knife round + side vote
- [x] Matchmaker / lobby service (parties, codes, queue, free-server pool); Play page replaces the server list
- [x] Lobby: OS randomness for tickets, regions/ping in the queue, standby lobby, saved state, hidden skill matchmaking, ready check, cooldowns, vote kick, reconnect
- [ ] Accounts: Steam (sign-in done with Valve's test app 480); our own App ID before release
- [x] One-command hosting: ./host.sh (Docker, fleets, standby lobby, firewall, rolling updates)
- [ ] Home hosting without port forwarding: UPnP from the server, relay through the master as fallback
- [x] Server protection: join cookie (stateless, no spoofed slots), per-IP packet budget, max 6 players per IP, 1400-byte cap, padded info/list requests (no amplification), master list rate limit
- [x] Kernel flood filtering: ./host.sh firewall (nftables)
- [ ] Host-side DDoS filtering: choose a provider with game DDoS protection (OVH Game, Path.net); a user decision
- [x] Encryption/authentication (X25519 + ChaCha20-Poly1305, per-seat keys, sealed server <-> lobby)
- [ ] Prediction error smoothing (corrections are applied instantly)

### Voice
- [x] Voice chat, two push-to-talk keys (decided 2026-10-16): team voice (K) reaches your team
      anywhere; proximity voice (X) reaches everyone alive nearby, enemies too, fading out by
      `voice_range` and panned when you see the speaker. Opus 24 kbit/s, server relays.
      Settings page: on/off, rebind keys (incl. mouse buttons), mic, mic volume, voice volume, mic test.
      **No radio commands/callouts**: decided 2026-10-15, voice replaces them.
- [ ] Voice: mute individual players (scoreboard), noise gate / open-mic option, walls muffling proximity voice
- [ ] Voice: only checked automatically (relay test, codec round trip); needs a real two-person test

### Sound
- [x] Gunshots (silenced variants, random pitch), reload/draw/pin sounds from model events,
      footsteps by surface/ladder/water, hits (flesh/kevlar/helmet/headshot), deaths,
      grenades (bounce, HE, flash, smoke), knife, dry fire, zoom; 3D with GoldSrc falloff
- [ ] Doors, ambience (`ambient_generic`), bullet ricochets/impacts,
      knife hit sounds, jump/land variants, menu sounds; other players' reload sounds
- [ ] ⚠ Silenced shot loudness/attenuation and footstep volumes are estimates
- [x] Muzzle flash at the model's muzzle attachment (own view model and other players)

### Movement leftovers (`docs/MOVEMENT.md`)
- [ ] Water jump-out onto ledges (`PM_CheckWaterJump`)
- [ ] Doors pushing players; door sounds
- [ ] ⚠ Footstep interval (300 ms is the HL convention; read `PM_UpdateStepSound`)
- [ ] ⚠ Ladder face direction from the ladder box (fine for stock ladders)

### Rendering
- [x] Sky: the map's skybox (TGA faces resampled into a cubemap)
- [ ] Lighting looks flatter than the original (overbright/gamma tuning)
- [x] Translucent brushes (`rendermode` 1/2/3/5 by `renderamt`)
- [ ] Decals, sprites, particles, muzzle flashes
- [ ] Static props/models placed in maps (`env_model`, `cycler_sprite` …)

### UI (Interference: docs/UI.md, design/mockups/ui is the spec)
- [ ] Scoreboard: the server's `Score` only has name, kills, deaths, alive, team, bot.
      Add per player: assists, ping, MVPs, teammates' money (own team only), headshot %,
      ADR (average damage per round). Then show them as columns.
- [ ] Scoreboard: round history is rebuilt from events seen since you joined; the server
      should send the full list so late joiners and reconnects see it too
- [ ] Rewrite docs/UI.md for Interference (tokens, components, motion; the mockup is the spec)
- [ ] ⚠ Not yet seen on screen in a real match: hit wash/heartbeat, reload line, money
      delta, timer ticks, vote panel, voted out / surrendered / kill limit announcements
      (the uplink, voice rows, status icons, match found and Practice were checked)
- [ ] AI assistant: generated lines through OpenCode, free models first (plan in design notes)
- [ ] Main menu character card + character select (waits for the new models)
- [ ] Map pool screen (waits for the new maps)

## Later (after milestone 1)

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