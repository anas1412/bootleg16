# Weapons: exact CS 1.6 behaviour specification

Single source for implementing weapons, shooting, damage, armor, grenades and
the knife in bootleg16. It describes the **original** CS 1.6 server behaviour.
Prices, ammo prices and the round economy live in `RULES.md` and are not
repeated here.

**Not in bootleg16 (decided 2026-10-16):** the Glock and FAMAS burst modes (right click does
nothing on them), the tactical shield, and the shotguns. Their specs below stay for reference.

## 0. Sources, legend, license

Inputs:

1. `cs.so` (the real server binary). `tools/re/` extractions
   (`BINARY_EXTRACT.txt`) plus extra direct disassembly done while writing this
   document. **The binary always wins.**
2. ReGameDLL_CS, a reverse-engineered open-source reimplementation. Only its
   original code paths were used. Everything that ReGameDLL adds, "fixes" or
   makes optional was ignored (cvars such as `refill_bpammo_weapons`,
   `knife_wall_sparks`, `hegrenade_penetration`, `ff_damage_reduction_*`; the
   float accuracy divisors; kick-back rewrites of pistols/snipers/shotguns;
   changed idle/reload timings; the `GetNextAttackDelay` creep correction).
3. `RULES.md` for prices, damage, range modifier, penetration, max distance.

Legend used in tables and text:

| Mark | Meaning |
|---|---|
| (no mark) | ReGameDLL original code path and binary agree, or binary-only value |
| `†` | Confirmed by me through extra direct disassembly of `cs.so` (not in `BINARY_EXTRACT.txt`) |
| `(S)` | Only in ReGameDLL source; the binary has no constant to compare |
| ⚠ | Neither source confirms it, or it is inferred; verify before relying on it |

License note: this file contains facts (numbers, tables, formulas, behaviour)
written in my own words. No source code is copied. Short field names such as
`m_flAccuracy` are used only to name things.

Conventions: lengths in Quake units; angles in degrees; times in seconds;
"speed" means horizontal speed `|velocity.xy|` at the moment of firing (after
that frame's movement). "Air" means the player does not have the on-ground flag.
"Ducking" means the ducking flag is set (fully ducked or transitioning, as the
movement code sets it). Pitch punch is **negative = view moves up**.

---

## 1. Shared algorithms

### 1.1 Time model, timers, frame order

Per player there is one timer, `next_attack` (blocks all weapon logic), and per
weapon: `next_primary`, `next_secondary`, `idle_timer`.

- All delays are relative: after an action a timer is set to the delay. Every
  server frame each timer is reduced by the frame time. Clamps: `next_primary`
  not below -1.0, `next_secondary` and `idle_timer` not below -0.001,
  `next_attack` not below -0.001 (`†` ReGameDLL source, original branch).
- A weapon can act when its timer is `<= 0`. **No credit is given for the
  overshoot**: the next shot sets the timer to the full cycle time again. So
  the real fire interval is the cycle time rounded up to the next frame.
  (`GetNextAttackDelay(d)` in the binary returns just `d`; the ReGameDLL
  "creep" correction does not exist in the binary. `†`)
- Implementation advice for a fixed tick server: `ready_at = now + delay`,
  `can_act = (now >= ready_at)`. Same behaviour, quantised to the tick.
- While `next_attack > 0` the whole weapon update is skipped (this is how the
  reload and the deploy delay block firing).
- Weapon update runs once per player frame, in this order:
  1. pending burst bullet (Glock or FAMAS, section 2.5);
  2. sniper zoom resume (AWP, Scout, section 1.10);
  3. shield special case (reload cancel when the shield is toggled);
  4. **reload completion** if the reload flag is set and `next_attack <= 0`;
  5. exactly one of: secondary attack, primary attack, reload request, or the
     "no fire button" branch (accuracy decay, auto reload, idle).
  Branches 5 are mutually exclusive in that priority: secondary button first,
  then primary, then reload, then idle.
- Primary attack is refused during freeze time, while defusing, and when
  `can_shoot` is false. Secondary is refused for weapons that have none
  (AK-47, XM1014, MAC-10, Elite, Five-seveN, MP5, M249, M3, TMP, Deagle,
  P228, P90, C4, Galil have no secondary; UMP45 has none in the latest
  ReGameDLL only, so it still has a harmless one in the original (S)). With a
  shield equipped every weapon has the shield secondary.
- Firing underwater: the binary sets no "no fire underwater" flag on any gun
  (all `iFlags` are 0 for guns; grenades and C4 carry 0x18) `†`. Guns fire
  under water.
- Dry fire (empty clip, button pressed): play empty click, set
  `next_primary = 0.2` for every gun. Shotguns: try to reload first, then
  click if still empty, with `next_primary = 0.15` `†` (see Discrepancies).

### 1.2 KickBack (recoil)

Called once per shot by automatic rifles and SMGs with 7 parameters:
`up_base, lat_base, up_mod, lat_mod, up_max, lat_max, dir_change`.
The player's current punch angle (a 3-vector, see 1.3) is modified. The weapon
has persistent `shots_fired` (already incremented for this shot, so >= 1) and
`direction` (0 or 1; starts at 0 on weapon creation; **never reset** by
deploy, reload or anything else).

```
n = shots_fired
if n == 1:  up = up_base;            lat = lat_base
else:       up = n*up_mod + up_base; lat = n*lat_mod + lat_base

punch.pitch = punch.pitch - up
if punch.pitch < -up_max: punch.pitch = -up_max          # hard clamp (absolute)

if direction == 1:
    punch.yaw = punch.yaw + lat
    if punch.yaw >  lat_max: punch.yaw =  lat_max
else:
    punch.yaw = punch.yaw - lat
    if punch.yaw < -lat_max: punch.yaw = -lat_max

# direction flip: random integer in [0, dir_change] inclusive, flip when it is 0
if random_int(0, dir_change) == 0: direction = 1 - direction
```

Notes:
- Flip chance per shot is `1/(dir_change+1)` (so 7 means 12.5 %). The RNG is the
  plain server RNG (not the shared prediction seed).
- `n` is not capped while firing (cap 15 happens only on button release,
  section 1.8), so `up` keeps growing and is limited only by `up_max`.
- The clamps compare against the **current** (already decayed) punch, so the
  recoil saturates at `up_max` / `lat_max` while spraying.
- The parameter set is chosen per shot from the state at fire time:
  **rifles** (AK-47, M4A1, Galil, FAMAS, AUG, SG552) test in the order
  *moving (speed > 0, any speed), air, ducking, standing*;
  **SMGs and M249** (MAC-10, TMP, MP5, UMP45, P90, M249) test in the order
  *air, moving (speed > 0), ducking, standing*. So a jumping rifle with any
  horizontal speed uses its "moving" set, while a jumping SMG uses its "air" set.
  All parameter values are in section 2.4.

### 1.3 Punch angle: decay and effect

- Each player movement step (per usercmd, with that command's frame time) the
  punch vector is shortened along its direction:
  `len = |punch|; len = max(0, len - (10 + 0.5*len) * dt); punch = punch/|punch| * len`.
  Constants 10 and 0.5 are confirmed in the binary extract; the form is the
  standard GoldSrc `PM_DropPunchAngle`. So decay is >= 10 deg/s and faster for
  large punches (a 5 degree punch fully decays in well under 0.5 s).
- The punch is added to the player's view angles for rendering (view moves,
  crosshair does not).
- **Bullet direction** uses `forward_vector(view_angles + punch)` where both
  pitch and yaw punch count (roll does not change the forward vector). So the
  recoil physically moves where bullets go; that is the "pattern".
- Sources of punch: weapon kick (1.2, pistol/sniper/shotgun fixed values in
  section 2), taking damage (1.7), shield hits.

### 1.4 Bullet direction and spread (players)

```
aim    = forward(view_angles + punch)
src    = origin + view_offset            # eye position
seed   = player.random_seed              # one shared value per usercmd
x = shared_rand(seed+0, -0.5, 0.5) + shared_rand(seed+1, -0.5, 0.5)
y = shared_rand(seed+2, -0.5, 0.5) + shared_rand(seed+3, -0.5, 0.5)
dir    = aim + x*spread*right + y*spread*up      # NOT normalised
end    = src + dir * max_distance
```

- Distribution: each axis is the sum of two uniforms, i.e. **triangular** on
  (-1, 1) times `spread`. No rejection and no circular clamp for players
  (corners reach `sqrt(2)*spread`). The `spread` number is therefore directly
  the half-width of the cone in units of "tangent" (x and y offsets), not
  degrees.
- Pellets / non-player shooters use the older method, section 1.6.
- `shared_rand(seed, lo, hi)` is a deterministic function of its arguments (it
  re-seeds a small generator with `seed + bits(lo) + bits(hi)`, discards two
  draws, then takes a 16 bit value / 65536 scaled to the range). It exists so
  client prediction matches the server. Our server does not need the exact
  generator unless a client predicts spread; keep the distribution and the
  "same inputs give same output" property. ⚠ (see Open questions).
- The weapon's spread value is computed in `PrimaryAttack` from the accuracy
  **before** this shot updates it (the shot then updates accuracy for the
  next one). See 1.8 for the order.
- Return value of the call is `(x*spread, y*spread)`; it is sent to clients so
  they can draw the decal/tracer. Not needed server side.

### 1.5 FireBullets3 (single hitscan bullet with penetration)

Parameters: origin, aim, spread, max distance (8192 for most guns, 4096 for the
USP, P228, Deagle, Five-seveN), penetration count, bullet type, base damage,
range modifier, attacker, `pistol` flag (decals only), shared seed.

Bullet types (id, penetration power, max penetration distance) `†` all read
from the binary tables:

| Id | Ammo | Power | Max pen. distance | Used by |
|---|---|---:|---:|---|
| 1 | 9mm | 21 | 800 | Glock, Elite, MP5, TMP |
| 9 | .45 ACP | 15 | 500 | USP, MAC-10, UMP45 |
| 10 | .338 Magnum | 45 | 8000 | AWP |
| 11 | 7.62mm NATO | 39 | 5000 | AK-47, Scout, G3SG1 |
| 12 | 5.56mm NATO | 35 | 4000 | M4A1, Galil, FAMAS, AUG, SG552, SG550, M249 |
| 13 | .50 AE | 30 | 1000 | Deagle |
| 14 | 5.7mm | 30 | 2000 | Five-seveN, P90 |
| 15 | .357 SIG | 25 | 800 | P228 |

Algorithm (all damage values are **integers**; every "multiply" truncates back
to int; the penetration power is also an int and truncates):

```
state: dmg = base damage (int)             power = type power (int)
       pen_left = penetration count        dist = max distance
       dmg_mod = 0.5                       # persists across loop iterations
pen_start = pen_left
compute dir and end (section 1.4)

while pen_left != 0:
    trace src -> end, against everything (world and entities, players included)
    tex = surface material at the hit (living players/monsters count as flesh)
    # texture adjustments (persistent: power keeps being reduced each loop)
    METAL:    power = int(power*0.15); dmg_mod = 0.2
    CONCRETE: power = int(power*0.25)                    # dmg_mod unchanged
    GRATE:    power = int(power*0.5);  dmg_mod = 0.4
    VENT:     power = int(power*0.5);  dmg_mod = 0.45
    TILE:     power = int(power*0.65); dmg_mod = 0.3
    COMPUTER: power = int(power*0.4);  dmg_mod = 0.45
    WOOD:                              dmg_mod = 0.6
    others (dirt, glass, slosh, flesh, unknown): no change
    if nothing hit (fraction == 1): break
    pen_left -= 1
    seg  = fraction * dist                           # distance of THIS segment only
    dmg  = int(dmg * range_mod ^ (seg / 500))
    if seg > type max pen distance: pen_left = 0
    if the hit hitgroup is SHIELD: apply shield effect (1.7), STOP (no damage)
    if hit entity is not a BSP solid (a player, monster, etc.) or pen_left == 0:
        power = 42; dmg_mod = 0.75; dist_mod = 0.75
    else:
        dist_mod = 0.5                                # walls
    src  = hit_point + dir * power                    # restart this far past the hit
    dist = (dist - seg) * dist_mod
    end  = src + dir * dist
    hit_entity.trace_attack(attacker, dmg, dir)       # uses dmg BEFORE the next line
    dmg  = int(dmg * dmg_mod)
    apply the accumulated damage to the hit entity now (one TakeDamage per hit)
```

Consequences:
- **Range falloff** is `range_mod ^ (segment_distance / 500)`, measured per
  segment from the segment start, not from the muzzle. After a wall the
  distance counter restarts at the exit point and the remaining max distance
  is halved (walls) or reduced to 75 % (players).
- Bullets **pass through players**: each player hit costs one penetration,
  multiplies the carried damage by 0.75 and shortens the remaining distance by
  25 %; the next segment restarts 42 units behind the hit point. A bullet can
  therefore hit several players in a line.
- A wall thinner than `power` units lets the bullet continue from open air.
  If the restart point is still inside solid, the next trace reports an
  immediate hit (fraction 0) with the same wall, again costing a penetration
  and damage (and, for CONCRETE/METAL etc., again reducing `power`). Thick
  walls therefore burn the penetration count. Reproduce this with the engine
  trace behaviour "start inside solid => fraction 0".
- Penetration count (RULES.md): 1 for all 9mm/.45/.357/5.7 pistols and SMGs,
  2 for Deagle and all rifles/M249/SG550, 3 for AWP, Scout, G3SG1.
- No dedicated sky rule exists; treat sky as a normal solid (the segment
  simply ends there and nothing is behind it).
- Entities such as breakables take `trace_attack` like anything else and the
  bullet continues (breakable brush entities are BSP solids, so wall rules
  apply).
- The shooter's own hitbox is not hit (trace ignores the shooter).
- `trace_attack` on a player applies hit group multipliers (1.7).
- Hit resolution order = nearest hit along the segment. Lag compensation is
  handled elsewhere (`Server::rewind`).

### 1.6 Older multi-pellet fire (shotguns only)

M3 and XM1014 use the old `FireBullets`, not FireBullets3:

```
for each pellet (M3 9, XM1014 6):
    repeat: x = U(-.5,.5)+U(-.5,.5); y = U(-.5,.5)+U(-.5,.5)  until x*x + y*y <= 1
            # plain server RNG (not the shared seed), circular rejection
    dir = aim + x*cone.x*right + y*cone.y*up     # cone = (0.0675,0.0675) M3, (0.0725,0.0725) XM1014
    trace src -> src + dir*range                 # range 3000 (M3) / 3048 (XM1014)  †
    if hit: pellet_dmg = int((1 - hit_fraction) * 20)     # linear falloff to 0 at max range
            entity.trace_attack(attacker, pellet_dmg, dir)  # hit group multipliers per pellet
apply the summed damage once per victim
```

- No penetration, no range-modifier power law, no wall pass-through. Damage per
  pellet: 20 at distance 0, falling linearly to 0 at 3000 / 3048 units. The
  base 20 is from ReGameDLL (`(S)`, not separately extracted from the binary ⚠)
  and matches `M3_DAMAGE = XM1014_DAMAGE = 20`.
- Pellets that hit the same player are summed first, then armor is applied once
  to the sum (so the armor ratio is computed on the total).
- Binary check: pellet count/range/cone `†` (9 and 3000/0.0675; 6 and 3048/0.0725).

### 1.7 Damage to players (TraceAttack and TakeDamage)

**Hit group multiplier** (applied to the float damage in `trace_attack`; `†`
all constants confirmed in the binary):

| Hit group | Multiplier | Armor applies (needs kevlar) | Notes |
|---|---:|---|---|
| Generic (0) | 1.0 | yes | also used for all explosion damage |
| Head (1) | **4.0** | only with helmet+vest | helmet does NOT reduce the x4 |
| Chest (2) | 1.0 | yes | |
| Stomach (3) | 1.25 | yes | |
| Left arm / right arm (4, 5) | 1.0 | yes | |
| Left leg / right leg (6, 7) | 0.75 | **never** | |
| Shield | damage becomes 0 | n/a | bullet stops, see below |

Side effects (only when the hit "bleeds"):
- Head: if victim has helmet+vest: no blood, sparks (streak splash), no view
  punch. Otherwise victim punch pitch = max(-12, -0.5 * damage_after_x4);
  punch roll = damage_after_x4 * random(-1, 1) clamped to +-9.
- Chest and stomach (no armor): victim punch pitch = max(-4, -0.1 * damage).
- Arms and chest/stomach with any kevlar: no blood.
- Shield hit: damage 0, victim punch pitch/roll from `damage * random(-0.15,
  0.15)` (bugged in the original: the pitch value is forced to -4 when it is
  not below 4, i.e. nearly always -4), roll clamped to +-5. A shield-hit also
  makes later bullet damage the same frame invalid (`m_LastHitGroup == SHIELD`
  refuses bullet damage).

**TakeDamage (bullets, knife; not explosions)**, in order:

1. If the attacker is a teammate or the victim himself (not a fall):
   `damage *= 0.35`. (Friendly fire on/off is a server setting handled in
   rules; with it off the damage is refused.)
2. Armor ratio base: `ratio = 0.5`. If the victim is the VIP: `ratio *= 0.5`.
   If the victim carries a shield: `ratio += 0.2`.
3. `ratio *= weapon_factor` of the **attacker's active weapon** (table below;
   no active weapon: factor 1).
4. If victim armor value != 0, damage is not fall/drown, and the hit group is
   armored (table above):
   ```
   health_part = ratio * damage                       # float
   armor_loss  = (damage - health_part) * 0.5          # ARMOR_BONUS = 0.5
   if armor_loss > armor:                              # not enough armor
       health_part = damage - armor * 2                # armor absorbs 2x its value
       armor = 0
   else:
       armor -= armor_loss    (if armor_loss < 0 it is treated as 1)
   damage = health_part
   if armor <= 0: kevlar (and helmet) removed
   ```
5. `health -= int(damage)` (truncated).
6. Flinch/knockback: if the victim is not ducking, the hit group is not a leg
   and the attacker's active weapon is one of Scout, AUG, SG550, Galil, FAMAS,
   AWP, M3, M4A1, G3SG1, Deagle, SG552, AK-47 (large flinch): if victim speed
   < 300 then `victim.velocity += normalize(victim.origin - attacker.origin) * 170`
   and `velocity_modifier = 0.65`; otherwise (small flinch) `velocity_modifier
   = 0.5` and no push. The modifier recovers `+0.01` per player frame while on
   ground and **multiplies the victim's velocity every such frame** (this is
   the "hit slow-down"). ⚠ Rate is per server frame in the original (about 100
   per second); implement as roughly +1.0 per second.
7. ARMOR_RATIO and ARMOR_BONUS constants (both 0.5) are not in the provided
   source files; they are confirmed from the binary's 0.5 constants and from the
   known per-gun results (AK-47 gives 77.5 % health damage) `†`.

Weapon factor and resulting health fraction (ratio = 0.5 * factor) `†`:

| Weapon | Factor | Health part | Weapon | Factor | Health part |
|---|---:|---:|---|---:|---:|
| AWP | 1.95 | 0.975 | P90, Five-seveN, M249, Deagle | 1.5 | 0.75 |
| Scout, Knife | 1.7 | 0.85 | SG550 | 1.45 | 0.725 |
| G3SG1 | 1.65 | 0.825 | AUG, M4A1 (both modes), FAMAS, SG552 | 1.4 | 0.70 |
| AK-47, Galil | 1.55 | 0.775 | P228 | 1.25 | 0.625 |
| Glock, Elite | 1.05 | 0.525 | MAC-10 | 0.95 | 0.475 |
| USP, MP5, TMP, UMP45, M3, XM1014 | 1.0 | 0.50 | | | |

**Explosions (HE grenade, C4)** use a separate path:
- hit group forced to Generic, so armor applies if the victim has kevlar
  (helmet not needed); `ratio = 0.5` (VIP 0.25); no weapon factor; no shield
  term; `bonus = 0.5`.
- `health_part = ratio*damage; armor_loss = (damage-health_part)*0.5`; if
  `armor_loss > armor` then `health_part = damage - armor` (armor value
  absorbs 1x, unlike bullets) and armor becomes 0; else armor reduces by
  `armor_loss`.
- No x0.35 friendly-fire reduction in this path: team damage and self damage
  are full when friendly fire is on (self damage is allowed even if it is off).
- No hit group multipliers.

**Knife armor**: knife uses the bullet path with factor 1.7 (health fraction
0.85); head hit with a knife is x4 like any other. Knife damage type is
bullet-like (it uses the same trace_attack).

### 1.8 Accuracy, shots fired, delayed fire

Per weapon there are: `accuracy` (float), `shots_fired` (int), `delay_fire`
(bool), `decrease_at` (time).

Order inside one shot for **automatic** weapons:
1. `PrimaryAttack` builds the spread from the **current** accuracy and the
   movement state (tables in section 2), then calls the weapon's fire routine.
2. Fire routine: `delay_fire = true; shots_fired += 1`; **then** recompute
   accuracy from the new `shots_fired` (formula per weapon, integer division);
   then (if the clip is empty: dry fire and return) consume a round, fire,
   apply recoil.
   So the spread used for shot N reflects accuracy computed after shot N-1.
3. Integer arithmetic matters: `shots^3 / D` and `shots^2 / D` are **integer
   divisions** (the binary uses integer magic-number division; only the MP5 uses
   a float divide, 220.1) `†`. The step shape is why accuracy jumps.

When no fire button is held (the idle branch):
```
if delay_fire:                 # set true by every automatic shot
    delay_fire = false
    if shots_fired > 15: shots_fired = 15
    decrease_at = now + 0.4
if weapon is a pistol (Glock, USP, P228, Deagle, Five-seveN, Elite):
    shots_fired = 0                              # this is what allows the next click
else if shots_fired > 0 and decrease_at < now:
    decrease_at = now + 0.0225
    shots_fired -= 1                             # accuracy value itself is NOT recomputed here
```
- Accuracy is only recomputed at the next shot from the decayed `shots_fired`.
  So after releasing, `shots_fired` falls by 1 per 22.5 ms after a 0.4 s grace.
- `delay_fire` is cleared on reload (and `shots_fired = 0`, accuracy reset
  as listed in section 2).
- **Semi-auto pistols**: the fire routine does `shots_fired += 1` and returns
  immediately (no shot) if it is now > 1. `shots_fired` returns to 0 only when
  both fire buttons are up. Hence exactly one shot per click, however long the
  button is held. Burst mode Glock does not use this gate (cycle time 0.5
  limits it instead).
- Pistol and sniper accuracy uses time-based recovery instead (see 2.2, 2.6).
- `DefaultDeploy` sets `decrease_at = now` and resets the zoom (1.9).

### 1.9 Deploy, holster, reload, idle

- **Deploy** (all guns via the common deploy): `next_attack = 0.75`,
  `idle_timer = 1.5`, zoom reset to 90 (and the saved zoom to 90, no resume
  pending) `†`. Exceptions: **AWP** `next_attack = next_primary = 1.45`,
  `next_secondary = 1.0`; **Scout** `next_attack = next_primary = 1.25`,
  `next_secondary = 1.0`. Per-gun deploy also resets accuracy and
  `shots_fired` (values in section 2).
- **Holster** cancels a reload in progress: the reload flag is cleared, so no
  ammo moves and no reload time is refunded. Knife and grenades also set
  `next_attack = 0.5` on holster (immediately overwritten by the next deploy).
- **Reload request** is ignored if ammo reserve is 0, the clip is full, a
  reload is already running, `next_primary` is not strictly in the past, a
  Glock/FAMAS burst is pending, or a shield is drawn.
- **Reload start** (`DefaultReload`): `next_attack = reload_time`,
  `idle_timer = reload_time + 0.5`, reload flag set `†`. The clip is not
  touched yet.
- **Reload completion** (weapon update, when `next_attack <= 0`): `moved =
  min(clip_size - clip, reserve)`; clip += moved; reserve -= moved. So ammo is
  added **at the end** of the reload and nothing is lost if cancelled.
- Auto reload: when no fire button is down, the clip is empty and
  `next_primary < now`, the weapon reloads by itself. Holding fire on an empty
  clip only dry-fires.
- Rifles/SMGs/pistols also reset `accuracy`, `shots_fired`, `delay_fire` on a
  successful reload start (exact values per weapon in section 2).
- **Shotgun (special) reload**, M3 and XM1014, state machine:
  - Start (state 0, clip not full, reserve > 0, `next_primary` <= 0): play
    start animation, state = 1, `next_attack = next_secondary = idle_timer =
    next_primary = 0.55`.
  - State 1 (when idle timer <= 0): state = 2, play insert animation and sound,
    `idle_timer = shell_time` (M3 0.45, XM1014 0.3 `†`).
  - State 2 (next update call): `clip += 1; reserve -= 1;` state = 1.
  - The weapon idle handler keeps calling reload while clip < max and reserve
    > 0; when full (or no reserve) it plays the pump animation, state = 0,
    `idle_timer = 1.5`.
  - Per shell cycle = shell_time. The first shell lands 0.55 + shell_time
    after the start (M3 1.00 s, XM1014 0.85 s). Because `next_primary` is only
    set to 0.55 at the start, the player can **interrupt by firing** after
    0.55 s; firing sets the state back to 0.
  - Sound: 1.0 volume, pitch 85 + random(0..31).
- **Idle**: idle timer expiry only plays idle animations; it matters for
  gameplay only for shotgun reload steps, grenade throws and C4. Idle timer
  after each shot is set per weapon (listed in section 2 as "idle").

### 1.10 Speeds, scoping, zoom

`max_speed` (units/s) comes from the active weapon; special cases: freeze time
1, VIP 227, spectator 900, no weapon 240. Movement code then applies ducking
and walking factors (see `MOVEMENT.md`). Shield: speed 180 while the shield is
drawn (weapon speed otherwise, 250), see 2.9.

| Weapon | Normal | Scoped |
|---|---:|---:|
| Knife, Glock, USP, P228, Deagle, Five-seveN, Elite, MAC-10, TMP, MP5, UMP45, HE, flash, smoke, C4 | 250 | n/a |
| AK-47 | 221 | n/a |
| M4A1 (silenced or not) | 230 | n/a |
| Galil, FAMAS | 240 | n/a |
| SG552 | 235 | 200 |
| AUG | 240 | 240 (no change) |
| M249 | 220 | n/a |
| P90 | 245 | n/a |
| M3 | 230 | n/a |
| XM1014 | 240 | n/a |
| Scout | 260 | 220 |
| AWP | 210 | 150 |
| G3SG1, SG550 | 210 | 150 |

(Scoped/normal numbers for AWP, Scout, G3SG1, SG550, SG552, M4A1 are binary
confirmed; the 250 values for pistols/knife/grenades are the source
constants, only the Deagle's 250 is in the binary extract `(S)`.)
C4 while planting: speed 1 (player frozen) in the original flow.

Scope/zoom (secondary attack; each press 0.3 s lockout `†`):

| Weapon | Cycle of field of view | Notes |
|---|---|---|
| AWP | 90 -> 40 -> 10 -> 90 | zoom sound |
| Scout | 90 -> 40 -> 15 -> 90 | |
| G3SG1, SG550 | 90 -> 40 -> 15 -> 90 | |
| AUG, SG552 | 90 <-> 55 | toggles; also changes cycle time (2.4) |

- Max speed is re-evaluated after every zoom change.
- **Fire un-scopes AWP and Scout**: on a shot with zoom != 90, remember the
  current zoom, set zoom to 90 and mark "resume". When `next_primary <= 0`
  again, the saved zoom is restored. G3SG1 and SG550 stay zoomed when firing.
- **Reload un-scopes every scoped weapon**: AWP/Scout/G3/SG550/AUG/SG552 reset
  to 90 on a successful reload start (they internally force the last zoom step
  then toggle, which lands on 90). Death/deploy also reset zoom to 90.
- **Unscoped inaccuracy** (added to the spread): Scout +0.025, AWP +0.08,
  G3SG1 +0.025, SG550 +0.025. When scoped, these are not added. (AUG/SG552/
  FAMAS have no scope spread change.)

---

## 2. Per-weapon data

### 2.1 Master table

Clip and reserve (ammo "max carry") are binary values `†`. Damage, range
modifier, penetration and max distance agree with `RULES.md` unless noted in
the Discrepancies section. "Cycle" = fire interval (time added to
`next_primary`; see 1.1 for quantisation).

| Weapon | Clip | Reserve | Damage | Range mod | Pen | Max dist | Bullet | Cycle (s) | Reload (s) | Speed |
|---|---:|---:|---:|---:|---:|---:|---|---|---:|---:|
| AK-47 | 30 | 90 | 36 | 0.98 | 2 | 8192 | 7.62 | 0.0955 | 2.45 | 221 |
| M4A1 unsilenced | 30 | 90 | 32 | 0.97 | 2 | 8192 | 5.56 | 0.0875 | 3.05 | 230 |
| M4A1 silenced | 30 | 90 | 33 | 0.95 | 2 | 8192 | 5.56 | 0.0875 | 3.05 | 230 |
| Galil | 35 | 90 | 30 | 0.98 | 2 | 8192 | 5.56 | 0.0875 | 2.45 | 240 |
| FAMAS auto | 25 | 90 | 34 | 0.96 | 2 | 8192 | 5.56 | 0.0825 | 3.3 | 240 |
| FAMAS burst (shot 1 / shots 2-3) | 25 | 90 | 34 / 30 | 0.96 | 2 | 8192 | 5.56 | 0.55 / see 2.5 | 3.3 | 240 |
| SG552 | 30 | 90 | 33 | 0.955 | 2 | 8192 | 5.56 | 0.0825 (0.135 scoped) | 3.0 | 235 / 200 |
| AUG | 30 | 90 | 32 | 0.96 | 2 | 8192 | 5.56 | 0.0825 (0.135 scoped) | 3.3 | 240 |
| AWP | 10 | 30 | 115 | 0.99 | 3 | 8192 | .338 | 1.45 | 2.5 | 210 / 150 |
| Scout | 10 | 90 | 75 | 0.98 | 3 | 8192 | 7.62 | 1.25 | 2.0 | 260 / 220 |
| SG550 | 30 | 90 | 70 | 0.98 | 2 | 8192 | 5.56 | 0.25 | 3.35 | 210 / 150 |
| G3SG1 | 20 | 90 | 80 | 0.98 | 3 | 8192 | 7.62 | 0.25 | 3.5 | 210 / 150 |
| M249 | 100 | 200 | 32 | 0.97 | 2 | 8192 | 5.56 | 0.1 | 4.7 | 220 |
| MP5 | 30 | 120 | 26 | 0.84 | 1 | 8192 | 9mm | 0.075 | 2.63 | 250 |
| UMP45 | 25 | 100 | 30 | 0.82 | 1 | 8192 | .45 | 0.1 | 3.5 | 250 |
| P90 | 50 | 100 | 21 | 0.885 | 1 | 8192 | 5.7 | 0.066 | 3.4 | 245 |
| MAC-10 | 30 | 100 | 29 | 0.82 | 1 | 8192 | .45 | 0.07 | 3.15 | 250 |
| TMP | 30 | 120 | 20 | 0.85 | 1 | 8192 | 9mm | 0.07 | 2.12 | 250 |
| Glock-18 semi | 20 | 120 | 25 | 0.75 | 1 | 8192 | 9mm | 0.15 | 2.2 | 250 |
| Glock-18 burst (shot 1 / shots 2-3) | 20 | 120 | 25 / 18 | 0.75 / 0.9 | 1 | 8192 | 9mm | 0.5 | 2.2 | 250 |
| USP unsilenced | 12 | 100 | 34 | 0.79 | 1 | 4096 | .45 | 0.15 | 2.7 | 250 |
| USP silenced | 12 | 100 | 30 | 0.79 | 1 | 4096 | .45 | 0.15 | 2.7 | 250 |
| P228 | 13 | 52 | 32 | 0.8 | 1 | 4096 | .357 SIG | 0.15 | 2.7 | 250 |
| Deagle | 7 | 35 | 54 | 0.81 | 2 | 4096 | .50 AE | 0.225 | 2.2 | 250 |
| Five-seveN | 20 | 100 | 20 | 0.885 | 1 | 4096 | 5.7 | 0.15 | 2.7 | 250 |
| Elite (dual) | 30 | 120 | 36 | 0.75 | 1 | 8192 | 9mm | 0.075 | 4.5 | 250 |
| M3 | 8 | 32 | 9 pellets x 20 | linear | 0 | 3000 | buckshot | 0.875 | shell by shell | 230 |
| XM1014 | 7 | 32 | 6 pellets x 20 | linear | 0 | 3048 | buckshot | 0.25 | shell by shell | 240 |
| Knife | n/a | n/a | see 2.7 | n/a | n/a | 48 / 32 | n/a | see 2.7 | n/a | 250 |
| HE / Flash / Smoke | n/a | 1 / 2 / 1 | see 2.8 | | | | | | | 250 |
| C4 | n/a | 1 | see 2.9 | | | | | | | 250 |

Other notes:
- Elite reload is binary `4.5`; Glock shield reload time and the M4A1
  unsilenced reload time are computed values in the binary (extract shows `?`);
  ReGameDLL uses 3.05 for both M4A1 states and a single Glock value (S).
- Default clip handed out with a fresh weapon = clip size. How much reserve
  comes with a purchase is not in these sources, see Open questions. Spawn
  loadout (original): CT knife + USP with clip 12 and **24** reserve (12 for the
  VIP); T knife + Glock with clip 20 and **40** reserve (S).
- Weapon switching/deploy: see 1.9. Deploy time is 0.75 s for every weapon
  except AWP 1.45 and Scout 1.25.

### 2.2 Spread tables

"acc" = `m_flAccuracy` as it is **before** this shot (see 1.8). Shown values
are the spread number passed to bullet direction (1.4). Binary constants for
all rows `†` unless marked.

**Rifles (state test: air; speed > 140; else)** (no separate crouch value):

| Weapon | Air | Speed > 140 | Else (standing or crouched) |
|---|---|---|---|
| AK-47 | 0.04 + 0.4*acc | 0.04 + 0.07*acc | 0.0275*acc |
| M4A1 unsilenced | 0.035 + 0.4*acc | 0.035 + 0.07*acc | 0.02*acc |
| M4A1 silenced | 0.035 + 0.4*acc | 0.035 + 0.07*acc | 0.025*acc |
| Galil | 0.04 + 0.3*acc | 0.04 + 0.07*acc | 0.0375*acc |
| FAMAS (burst mode) | 0.03 + 0.3*acc | 0.03 + 0.07*acc | 0.02*acc |
| FAMAS (auto mode) | same as burst **plus 0.01** | same plus 0.01 | same plus 0.01 |
| AUG | 0.035 + 0.4*acc | 0.035 + 0.07*acc | 0.02*acc |
| SG552 | 0.035 + 0.45*acc | 0.035 + 0.075*acc | 0.02*acc |
| M249 | 0.045 + 0.5*acc | 0.045 + 0.095*acc | 0.03*acc |

**SMGs (state test: air; else)**; P90 also has a speed branch (> 170):

| Weapon | Air | Speed > 170 (P90 only) | Else |
|---|---|---|---|
| MAC-10 | 0.375*acc | n/a | 0.03*acc |
| TMP | 0.25*acc | n/a | 0.03*acc |
| MP5 | 0.2*acc | n/a | 0.04*acc |
| UMP45 | 0.24*acc | n/a | 0.04*acc |
| P90 | 0.3*acc | 0.115*acc | 0.045*acc |

(Moving and crouching do not change SMG spread other than through `acc`.)

**Pistols (state test: air; speed > 0; ducking; standing)**; value is
`factor * (1 - acc)`:

| Weapon | Air | Moving (> 0) | Ducking | Standing |
|---|---:|---:|---:|---:|
| Glock semi | 1.0 | 0.165 | 0.075 | 0.1 |
| Glock burst | 1.2 | 0.185 | 0.095 | 0.3 |
| USP unsilenced | 1.2 | 0.225 | 0.08 | 0.1 |
| USP silenced | 1.3 | 0.25 | 0.125 | 0.15 |
| P228 | 1.5 | 0.255 | 0.075 | 0.15 |
| Deagle | 1.5 | 0.25 | 0.115 | 0.13 |
| Five-seveN | 1.5 | 0.255 | 0.075 | 0.15 |
| Elite | 1.3 | 0.175 | 0.08 | 0.1 |

**Snipers**:

| Weapon | Air | Moving | Ducking | Standing | Scope rule |
|---|---|---|---|---|---|
| AWP | 0.85 | 0.25 if speed > 140; 0.1 if speed > 10 | 0.0 | 0.001 | **+0.08 when not scoped** |
| Scout | 0.2 | 0.075 if speed > 170 | 0.0 | 0.007 | +0.025 when not scoped |
| G3SG1 | 0.45 | 0.15 (any speed > 0) | 0.035 | 0.055 | the whole value is multiplied by `(1 - acc)` inside the fire routine, **after** adding +0.025 when not scoped |
| SG550 | `0.45*(1-acc)` | 0.15 (flat, no acc) | `0.04*(1-acc)` | `0.05*(1-acc)` | +0.025 added after, flat, when not scoped |

(AWP/Scout order of tests: air, then the speed thresholds, then ducking, else
standing. Scout and AWP spread does not use accuracy.)

### 2.3 Accuracy per shot (automatic weapons)

Accuracy (`acc`) after shot with `shots_fired = n`:

| Weapon | Formula (integer division where shown "div") | Cap | Initial (deploy) | Reset on reload |
|---|---|---|---|---|
| AK-47 | `(n^3 div 200) + 0.35` | 1.25 | 0.2 | 0.2 |
| Galil | `(n^3 div 200) + 0.35` | 1.25 | 0.2 | 0.2 |
| M4A1 | `(n^3 div 220) + 0.3` | 1.0 | 0.2 | 0.2 |
| SG552 | `(n^3 div 220) + 0.3` | 1.0 | 0.2 | 0.2 |
| AUG | `(n^3 div 215) + 0.3` | 1.0 | 0.2 | **0** |
| FAMAS | `(n^3 div 215) + 0.3` | 1.0 | 0.2 | **0** |
| M249 | `(n^3 div 175) + 0.4` | 0.9 | 0.2 | 0.2 |
| MAC-10 | `(n^3 div 200) + 0.6` | 1.65 | 0.15 | **0** |
| TMP | `(n^3 div 200) + 0.55` | 1.4 | 0.2 | 0.2 |
| MP5 | `(n^2 / 220.1) + 0.45` (float divide) | 0.75 | 0.0 | 0 |
| UMP45 | `(n^2 div 210) + 0.5` | 1.0 | 0.0 | 0 |
| P90 | `(n^2 div 175) + 0.45` | 1.0 | 0.2 | 0.2 |

`†` all divisors confirmed (magic numbers in the binary; extra check for 220
and 210). Because of integer division the accuracy is a **step** function.
The effective accuracy values by shots fired (accuracy that the *next* shot
will use):

- AK-47, Galil: n=1..5 -> 0.35; n >= 6 -> 1.25 (6^3 = 216 gives div 1).
- M4A1, SG552: n=1..6 -> 0.3; n >= 7 -> 1.0.
- AUG, FAMAS: n=1..5 -> 0.3; n >= 6 -> 1.0.
- M249: n=1..5 -> 0.4; n >= 6 -> 0.9.
- MAC-10: n=1..5 -> 0.6; n=6,7 -> 1.6; n >= 8 -> 1.65.
- TMP: n=1..5 -> 0.55; n >= 6 -> 1.4.
- MP5: n=1 -> 0.4545, rising slowly (n=5: 0.5635); reaches the 0.75 cap at n=8
  (0.7408) -> 0.75 from n=9.
- UMP45: n=1..14 -> 0.5; n >= 15 -> 1.0.
- P90: n=1..13 -> 0.45; n >= 14 -> 1.0 (14^2 = 196 div 175 = 1).
The very first shot after deploy/reload uses the initial value (e.g. AK stand
spread `0.0275*0.2 = 0.0055`). After the decay in 1.8 the next shot starts
low again.

**Pistol time-based accuracy** (evaluated in the fire routine, only if the
weapon has fired before: `last_fire != 0`, which is never reset; skipped for
the very first ever shot), `dt = now - last_fire`:

```
acc = acc - (K - dt) * S ;  acc = clamp(acc, lo, hi) ;  last_fire = now
```
(ReGameDLL source; constants `†` from the binary.)

| Weapon | K | S | lo | hi | Initial/Reload value | Idle after shot |
|---|---:|---:|---:|---:|---:|---|
| Glock | 0.325 | 0.275 | 0.6 | 0.9 | 0.9 | 2.5 |
| USP | 0.3 | 0.275 | 0.6 | 0.92 | 0.92 | 2.0 |
| P228 | 0.325 | 0.3 | 0.6 | 0.9 | 0.9 | 2.0 |
| Deagle | 0.4 | 0.35 | 0.55 | 0.9 | 0.9 | 1.8 |
| Five-seveN | 0.275 | 0.25 | 0.725 | 0.92 | 0.92 | 2.0 |
| Elite | 0.325 | 0.275 | 0.55 | 0.88 | 0.88 | 2.0 |

Cycle time actually stored after the pistol's fixed subtraction `†`:
Glock semi 0.2 - 0.05 = 0.15; USP 0.225 - 0.075 = 0.15; P228 and Five-seveN
0.2 - 0.05 = 0.15; Deagle 0.3 - 0.075 = 0.225; Elite 0.2 - 0.125 = 0.075.
The spread value was computed by `PrimaryAttack` from the accuracy left by the
previous shot; the time-based update in the fire routine only affects the next
shot (consistent with 1.8). A long pause raises the accuracy toward its cap by
the same formula (large `dt` makes `K - dt` negative, so `acc` rises).

**Sniper accuracy**:
- G3SG1: at each shot, `acc = 0.98` if never fired, else `min(0.98, dt*0.3 +
  0.55)` where dt is time since the previous shot. Reset on reload 0.2;
  deploy 0.2. Spread = `(1 - acc) * (base [+0.025 unscoped])` (computed after
  the update).
- SG550: `acc = min(0.98, dt*0.35 + 0.65)` (only when it has fired before;
  otherwise unchanged). **The original never initialises SG550 accuracy**
  (stays 0 until the first update; ReGameDLL "fixes" this to 0.9, ignore):
  first shot's `(1-acc)` is 1.0. Spread uses the accuracy **before** the update.
  No reset on reload or deploy (original).

### 2.4 Recoil parameter sets (KickBack)

Format: `(up_base, lat_base, up_mod, lat_mod, up_max, lat_max, dir_change)`.
All 4 sets per weapon confirmed against the binary `†`. The "standing" direction
change values for AUG, FAMAS, M249, M4A1, MP5, UMP45 and Galil are also
present in the disassembly's last call; see order note.

**Rifles: order of selection = moving, air, ducking, standing** (moving is
any horizontal speed > 0):

| Weapon | Moving | Air | Ducking | Standing |
|---|---|---|---|---|
| AK-47 | 1.5, 0.45, 0.225, 0.05, 6.5, 2.5, 7 | 2, 1, 0.5, 0.35, 9, 6, 5 | 0.9, 0.35, 0.15, 0.025, 5.5, 1.5, 9 | 1, 0.375, 0.175, 0.0375, 5.75, 1.75, 8 |
| M4A1 (both modes) | 1, 0.45, 0.28, 0.045, 3.75, 3, 7 | 1.2, 0.5, 0.23, 0.15, 5.5, 3.5, 6 | 0.6, 0.3, 0.2, 0.0125, 3.25, 2, 7 | 0.65, 0.35, 0.25, 0.015, 3.5, 2.25, 7 |
| Galil | same as M4A1 | | | |
| FAMAS | 1, 0.45, 0.275, 0.05, 4, 2.5, 7 | 1.25, 0.45, 0.22, 0.18, 5.5, 4, 5 | 0.575, 0.325, 0.2, 0.011, 3.25, 2, 8 | 0.625, 0.375, 0.25, 0.0125, 3.5, 2.25, 8 |
| AUG | same as FAMAS | | | |
| SG552 | 1, 0.45, 0.28, 0.04, 4.25, 2.5, 7 | 1.25, 0.45, 0.22, 0.18, 6, 4, 5 | 0.6, 0.35, 0.2, 0.0125, 3.7, 2, 10 | 0.625, 0.375, 0.25, 0.0125, 4, 2.25, 9 |

**SMGs and M249: order of selection = air, moving, ducking, standing**:

| Weapon | Air | Moving | Ducking | Standing |
|---|---|---|---|---|
| M249 | 1.8, 0.65, 0.45, 0.125, 5, 3.5, 8 | 1.1, 0.5, 0.3, 0.06, 4, 3, 8 | 0.75, 0.325, 0.25, 0.025, 3.5, 2.5, 9 | 0.8, 0.35, 0.3, 0.03, 3.75, 3, 9 |
| MAC-10 | 1.3, 0.55, 0.4, 0.05, 4.75, 3.75, 5 | 0.9, 0.45, 0.25, 0.035, 3.5, 2.75, 7 | 0.75, 0.4, 0.175, 0.03, 2.75, 2.5, 10 | 0.775, 0.425, 0.2, 0.03, 3, 2.75, 9 |
| TMP | 1.1, 0.5, 0.35, 0.045, 4.5, 3.5, 6 | 0.8, 0.4, 0.2, 0.03, 3, 2.5, 7 | 0.7, 0.35, 0.125, 0.025, 2.5, 2, 10 | 0.725, 0.375, 0.15, 0.025, 2.75, 2.25, 9 |
| MP5 | 0.9, 0.475, 0.35, 0.0425, 5, 3, 6 | 0.5, 0.275, 0.2, 0.03, 3, 2, 10 | 0.225, 0.15, 0.1, 0.015, 2, 1, 10 | 0.25, 0.175, 0.125, 0.02, 2.25, 1.25, 10 |
| UMP45 | 0.125, 0.65, 0.55, 0.0475, 5.5, 4, 10 | 0.55, 0.3, 0.225, 0.03, 3.5, 2.5, 10 | 0.25, 0.175, 0.125, 0.02, 2.25, 1.25, 10 | 0.275, 0.2, 0.15, 0.0225, 2.5, 1.5, 10 |
| P90 | 0.9, 0.45, 0.35, 0.04, 5.25, 3.5, 4 | 0.45, 0.3, 0.2, 0.0275, 4, 2.25, 7 | 0.275, 0.2, 0.125, 0.02, 3, 1, 9 | 0.3, 0.225, 0.125, 0.02, 3.25, 1.25, 8 |

Other recoil (fixed, not KickBack in the original):

| Weapon | Pitch punch per shot | Yaw punch |
|---|---|---|
| USP, P228, Deagle, Five-seveN, Elite, Scout, AWP | `pitch -= 2.0` (flat, no clamp, added to current punch) `†` | none |
| Glock-18 | **none** (the binary's Glock fire code has no punch; burst follow-up bullets have none either) `†` | none |
| G3SG1 | `pitch -= rand(0.75, 1.75) + punch.pitch*0.25` (shared rand, seed+4) `†` | `yaw += rand(-0.75, 0.75)` (seed+5) |
| SG550 | `pitch -= rand(0.75, 1.25) + punch.pitch*0.25` (seed+4) `†` | `yaw += rand(-0.75, 0.75)` (seed+5) |
| M3 | ground: `pitch -= randint(4, 6)` (S); air: `randint(8, 11)` `†` (seed+1, shared) | none |
| XM1014 | ground: `pitch -= randint(3, 5)` (S); air: `randint(7, 10)` `†` (seed+1) | none |
| Knife, grenades, C4 | none | none |

(`punch.pitch*0.25` is negative after a kick, so successive sniper shots
subtract less.)

### 2.5 Burst, silencer, alternate fire modes

**Glock-18** (secondary toggles mode; 0.3 s lockout): semi-auto (default; the
click rules of 1.8) or 3-round burst. In **burst mode**:
- First bullet: normal fire routine (damage 25, range mod 0.75, spread from
  the burst column), cycle time 0.5 (no semi subtraction), accuracy update as
  usual.
- The next two bullets are fired by the weapon update on **consecutive weapon
  updates with no delay test** (a nonzero "shoot at" value is only a flag; the
  stored time value 0.1 is never compared for the Glock) `†`. In practice the
  burst is 3 bullets within about 2 frames.
- Follow-up bullets: damage **18**, range mod **0.9**, spread **0.05** fixed
  (ignores movement and accuracy), penetration 1, 9mm, max distance 8192,
  aimed along the current view + punch. No recoil kick, no accuracy update.
  If the clip empties mid-burst the burst aborts.
- Reload is blocked while a burst is pending; `next_primary` 0.5 applies from
  the first shot.

**FAMAS** (secondary toggles auto/burst, 0.3 s lockout): in **burst mode** the
shot uses `cycle = 0.55`, spread without the +0.01 (auto adds +0.01 and cycle
0.0825), and sets the follow-up flag.
- First bullet: damage 34 (the binary uses 34 for **every** FAMAS main shot,
  auto or burst) `†`, range mod 0.96, one KickBack.
- Follow-ups: scheduled at +0.05 s, then +0.1 s (so at t = 0, 0.05, 0.15 s). Damage
  **30**, range mod 0.96, penetration 2, 5.56mm, spread = the first bullet's
  spread, no recoil kick. Burst length is 3 bullets total.

**M4A1**: secondary toggles the silencer (2.0 s: `next_primary = next_secondary =
idle = 2.0`). Silenced: damage 33, range mod 0.95, standing spread 0.025*acc
(other spread terms equal), quieter sound. Unsilenced: damage 32, range mod
0.97, standing spread 0.02*acc. Same cycle, recoil, reload (3.05 s).

**USP**: secondary toggles the silencer (3.0 s, ReGameDLL adds a different
value only in its fixed build; use 3.0 `†`). Silenced: damage 30, spreads
from the silenced column in 2.2, no muzzle flash. Unsilenced: damage 34.
Range mod 0.79 both. Deploy chooses the silenced/unsilenced draw animation.

**Elite**: alternates left and right gun each shot (left uses a start point
5 units to the left along the view `right` vector, right 5 units to the right
`(S)`); side after deploy decided by clip parity (even clip -> left first).
Cycle 0.075.

**AUG / SG552**: secondary toggles scope zoom 90 <-> 55; cycle becomes 0.135
when zoomed (0.0825 unzoomed) `†`. Spread and recoil unchanged.

### 2.6 Snipers

- AWP and Scout bolt-action: after the shot un-scope and resume (1.10); AWP
  eject brass 0.55 s, Scout 0.56 s (cosmetic). Idle after shot: AWP 2.0, Scout
  1.8, G3SG1/SG550 1.8.
- Auto-snipers (G3SG1, SG550): semi-automatic cadence is just the 0.25 cycle
  (no click gate), accuracy by time since last shot (2.3).
- Scoped bullets are not more penetrating than unscoped ones.

### 2.7 Knife

Primary (slash): trace 48 units from the eye along the view; if no hit, a
head-hull sweep of the same length; if still nothing it is a miss. When the
hit entity is the world or a BSP model, the best intersection is re-searched
(8 corner traces with the duck hull, closest wins). Secondary (stab): same with
32 units.

| Action | Damage | Next primary | Next secondary | Notes |
|---|---:|---:|---:|---|
| Slash miss | none | 0.35 | 0.5 | slash sound |
| Slash hit | 15 | 0.4 | 0.5 | see note on 20 |
| Stab miss | none | 1.0 | 1.0 | |
| Stab hit | 65 (x3 = 195 backstab) | 1.1 | 1.1 | backstab only for stabs |
| Shield version: slash miss | | 1.0 | 1.2 | animation differs |

- Slash damage is 15, or 20 "when the previous swing was more than 0.4 s ago";
  in the binary the 20 branch exists, **but the check runs after the delay for
  this swing was already stored, so it can never be true**; effective slash
  damage is always 15 `†` (see Discrepancies).
- Stab backstab: compare the horizontal facing of attacker and victim; if the
  dot product of the attacker's forward 2D (normalised) and the victim's
  forward 2D is **> 0.8** the damage is multiplied by 3 (65 -> 195). Applies
  only to player victims.
- All knife damage goes through `trace_attack` (hit groups; head x4) and
  `TakeDamage` (armor factor 1.7 -> health fraction 0.85, teammate x0.35).
- Idle after a hit/miss: 2.0 (cosmetic). Speed 250 (180 with shield drawn).
  Holster/deploy: standard deploy 0.75.
- No knife damage falloff, no penetration.

### 2.8 Grenades

Common (HE, flash, smoke):

- Press attack: if no pin pulled and ammo > 0, pull pin (idle timer 0.5).
  The throw happens in the weapon update **after** the button is released and
  the idle timer is up. On the throw: shout "fire in the hole", compute the
  throw, `next_primary = 0.5` (relative), `idle = 0.75`. When the last grenade
  is thrown the weapon retires after the animation.
- **Throw velocity** `†` (extract constants 80/90, 100/90, 10, 90, 6, 750):
  ```
  angle = view_angles + punch                 # pitch is angle.x, negative = looking up
  if angle.pitch < 0: pitch' = -10 + pitch * (80/90)
  else:               pitch' = -10 + pitch * (100/90)
  speed = min(750, (90 - pitch') * 6)
  forward = forward_vector(angles with pitch')
  start   = eye_position + forward * 16
  velocity = forward * speed + player_velocity
  ```
- Fuse (time until detonation): **1.5 s** from release for all three `†`.
  Also if a player dies while holding the pin with attack pressed the grenade
  is thrown from the corpse (S).
- Projectile physics: bounce movement, own gravity scale and friction:
  HE gravity 0.55, friction 0.7; flash and smoke gravity 0.5, friction 0.8 `†`.
  Bounce sound/velocity-loss logic is `BounceTouch` (not extracted, ⚠).
- Carry limits: HE 1, flash 2, smoke 1 `†` (iMaxAmmo); weapon slot items
  "exhaustible".
- Shield owners: secondary draws/holsters the shield (0.4 s), cannot throw
  while shield drawn.
- Hold/throw cadence: after a throw the weapon redraws; next throw possible
  after `next_primary` and the draw.

**HE grenade** `†`:
- Damage value 100, radius = damage * 3.5 = **350** units (the monster
  radius-damage routine uses `damage * 3.5` when damage > 80).
- For each damageable entity within 350 units (players only matter): skip if
  the blast and the victim are on different sides of water (blast in water:
  victim must be in water; blast in air: victim must not be fully submerged);
  `dmg = 100 - distance * (100/350)`; distance is from the blast point (1 unit
  above the detonation point) to the victim's **origin**; no damage if <= 0.
  There is **no line-of-sight or wall check** in cstrike (the line-of-sight
  variant is Condition Zero only), so HE damages through walls/thin cover. ⚠
  Verify in-game if this ever feels wrong, but it is what the binary does.
- Damage type explosion (armor path in 1.7). Result truncated to int.
- Detonation: floor trace 8 units above to 40 units below the grenade origin
  to place the blast; the grenade explodes only when the fuse ends (no impact
  fuse).
- Explosion sound/visual: cosmetic, plus a "danger" sound used by bots.

**Flashbang** `†` (all constants from the binary `RadiusFlash`):
- On detonation: `radius = 1500`, `power = 4.0`.
- Affected: players only (not dead, `takedamage` on), within 1500 of the blast
  (1 unit above), with the same water rule as HE, and with **line of sight**
  from the blast to the victim body target in both directions (two traces
  ignoring monsters, one each way; any obstruction rejects).
- `adj = max(0, 4.0 - dist * (4.0/1500))` with `dist` measured from the blast to
  the trace end point (eye/body target).
- Facing test: `dot(blast - victim_eye, victim_forward)`:
  - facing the flash (`dot >= 0`): `fade = 3.0*adj`, `hold = adj / 1.5`,
    `alpha = 255`
  - not facing (`dot < 0`): `fade = 1.75*adj`, `hold = adj / 3.5`,
    `alpha = 200`
- Max (point blank, facing): fade 12 s, hold 2.667 s, alpha 255.
- Screen fade: color white (255,255,255), `fade`, `hold`, `alpha`, to the
  victim; spectators observing the victim get the same fade unless the
  fade-to-black option is on (`0` default).
- "Blind" bookkeeping for gameplay (bots): `blind_until = now + 0.33*fade`,
  hold time `hold`, fade time `fade`, alpha.
- **Already blinded** (partially decoded, ⚠): for a facing hit, any remaining
  hold time of the previous flash is added to the new hold; and when the
  previous flash is still active the new fade/alpha become the maximum of old
  and new. Non-facing hits skip the hold addition.
- Teammates and the thrower are flashed too (no team check).

**Smoke grenade**: detonates after the 1.5 s fuse, then emits smoke puffs once
per second; the emitter counter runs from 0 until it passes 20 (21 emissions)
so the cloud lasts about **21 s**, then the entity is removed `†` (derived
from the emitter loop; exact visual radius/opacity are client-side effect
parameters not extracted ⚠). Smoke has no damage.

### 2.9 C4, shield

**C4** (T): carried in slot, ammo 1. To plant: press and hold attack while on
the ground and inside a bomb zone; arming takes **3.0 s** (`C4_ARMING_ON_TIME`)
during which max speed is 1 and a progress bar runs; the plant animation
starts 0.75 s before completion. Release or leaving the zone cancels
(`next_primary` 1.0 on release, 1.5 when leaving the zone); a failed press
(not in a zone or in the air) sets `next_primary = 1.0`. After success: bomb
entity created at the planter's origin, planter loses C4. Between presses
`next_primary` 0.3. Bomb timer, blast damage and defuse times: out of scope
here; see `RULES.md` (⚠ blast damage/radius and defuse times not extracted
yet).

**Tactical shield** (CT; `RULES.md` gives price 2,200): secondary on a shield-
equipped player toggles drawn state: `next_primary = next_secondary = 0.4`,
idle 0.6; max speed 180 while drawn, 250 otherwise. While drawn the player
cannot reload or throw grenades. Bullets hitting the shield hit group are
fully blocked (1.5). Armor base ratio gets +0.2 for a shield owner (1.7).
Shield-compatible weapons: the pistols, knife, grenades. (Detailed
hitbox/angle rules for the shield are not covered here ⚠.)

### 2.10 Idle timers after shots (cosmetic), for completeness

| Weapon | Idle after shot | Weapon | Idle after shot |
|---|---:|---|---:|
| AK-47 | 1.9 | M249 | 1.6 |
| M4A1 | 1.5 | MAC-10, MP5, TMP, UMP45, P90 | 2.0 |
| Galil | 1.28 | AWP | 2.0 |
| FAMAS | 1.1 | Scout, G3SG1, SG550 | 1.8 |
| AUG | 1.9 | M3 | 2.5 (0.875 if the clip is now empty) |
| SG552 | 2.0 | XM1014 | 2.25 (0.75 if empty) |

These are `(S)` except where the extract shows the constant (M3 2.5/0.875 and
XM1014 2.25/0.75 `†`).

---

## 3. Discrepancies (source vs binary vs RULES.md)

Binary wins in every row.

| # | Item | ReGameDLL source (original branch) | Binary | RULES.md | Used here |
|---|---|---|---|---|---|
| 1 | FAMAS damage | 30 auto, 34 burst | **34 for every main shot** (auto and burst first shot); burst follow-ups 30 | 34 | 34 / follow-up 30 |
| 2 | Glock burst follow-up bullets | damage 25 (same as main) | **damage 18**, range mod 0.9 `†` | 25, 0.75 (main only) | 18, 0.9 |
| 3 | Shotgun dry-fire delay | `GetNextAttackDelay(1)` | **0.15** | not listed | 0.15 |
| 4 | Knife slash fast damage (20) | 20 if >0.4 s since last | 20 branch exists but its condition is always false (checked after the timer is set) `†` | not listed | 15 only |
| 5 | USP damage | 34 / 30 sil | 34 unsilenced, 30 silenced `†` | USP 30 (this is the silenced value only) | 34 / 30 |
| 6 | M4A1 which call is silenced | silenced = 33 / 0.95 | first call (33, 0.95) = silenced branch `†` | listed both, unresolved | silenced 33/0.95, unsilenced 32/0.97 (resolves RULES.md warning) |
| 7 | SG550 punch range | rewritten in ReGameDLL | `rand(0.75, 1.25)` `†` | not listed | 0.75..1.25 (G3SG1 0.75..1.75) |
| 8 | SG550 initial accuracy | ReGameDLL "fix" sets 0.9 | original never sets it (0) | not listed | 0 |
| 9 | `GetNextAttackDelay` | extra "creep" compensation in a late build | returns just the delay `†` | not listed | plain delay |
| 10 | Accuracy divisors | float in the fixed build | integer magic division `†` (MP5 float 220.1) | not listed | integer |
| 11 | Pistol/sniper/shotgun recoil | rewritten through KickBack (new code) | flat `-= 2`, G3/SG550 shared random, shotguns shared random ints | not listed | flat, as binary |
| 12 | Elite semi cycle | 0.078 in fixed build | 0.125 subtracted `†` | not listed | 0.2 - 0.125 = 0.075 |
| 13 | Item flags (fire underwater) | M3/XM1014/Galil have `NOFIREUNDERWATER` | flags are 0 for guns `†` | not listed | guns can fire underwater |
| 14 | Famas in RULES.md | n/a | n/a | "FAMAS 34" ok for auto and burst first shot; follow-ups (30) were missing | see 1 |
| 15 | Glock/Famas follow-up timing | Glock: nonzero test only; Famas: time test | confirmed `†` | not listed | as binary |

Notes that are not discrepancies but are easy to get wrong:
- Rifles test "moving" before "air" for recoil; SMGs test "air" before
  "moving".
- `(1 - acc)` pistol spreads use the accuracy of the **previous** shot.
- Range falloff uses per-segment distance (1.5).

## 4. Open questions

1. **Shared random generator** and where `random_seed` comes from (per usercmd):
   not in the inputs. If our client predicts spread, match the shared-seed
   scheme (seed offsets +0..+3 for x/y; +4/+5 for sniper recoil; +1 shotgun
   recoil). Otherwise any generator with the triangular distribution is fine.
2. **Reserve ammo at purchase and at pickup**: the buy path is not in the
   provided files. Likely clip + full reserve, but the exact reserve given
   with a bought gun, and ammo clip prices to refill, must be confirmed.
3. **Hit slowdown rate** (`velocity_modifier` +0.01 per player frame): the
   original is per frame (~100 Hz); our tick-rate dependent equivalent needs a
   decision (suggest +1.0 per second).
4. **Grenade bounce/roll physics**: `BounceTouch`/`SlideTouch` constants,
   explosion on world impact, sounds; smoke cloud radius and opacity; flash
   "already flashed" combination rule (only partially decoded).
5. **C4**: blast damage/radius, defuse times (5/10 s per `RULES.md` ⚠),
   `mp_c4timer` use, and the T-money rules are outside this file.
6. **Surface material table** (which textures map to METAL, CONCRETE, TILE,
   WOOD, VENT, GRATE, COMPUTER, GLASS, DIRT): comes from the game's
   `materials.txt` by texture name prefix, not in the inputs. Without it every
   surface would behave as "no modifier".
7. **Hit group layout** (which hitbox belongs to which group) comes from the
   player models (`sim::hitbox`); this file only gives the multipliers.
8. **Knife swing/hull** details (`head_hull`, duck hull corners, trace flags
   for `func_breakable`) and exact aim of the knife: partially specified.
9. **M3/XM1014 ground recoil ranges** (4-6, 3-5) are source-only; air ranges are
   binary confirmed.
10. **Shield** (hitbox, angle checks, which weapons may be used) is only
    summarised.
11. **Dry-fire and sound/volume values** (e.g. gun loudness for bot hearing) are
    not covered.
12. **Armor/helmet purchase interactions** (partial armor repair) belong to the
    buy rules, not here.
13. `FL_ONGROUND` / `FL_DUCKING` semantics at the exact frame of firing (for
    example the first frame after landing, or mid-duck) are decided by the
    movement code, see `MOVEMENT.md`.
