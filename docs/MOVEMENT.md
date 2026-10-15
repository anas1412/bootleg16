# Player movement (CS 1.6)

Code: `sim/src/pmove.rs` (movement), `sim/src/collision.rs` (hull traces).
Engine-free and deterministic: same inputs give the same result, so the server
and client can run it identically. It runs at 128 ticks/s (64 also works).

## Constants and where they come from

| Value | Number | Source |
|---|---|---|
| Gravity | 800 | engine cvar default (`hw.so`/`engine_i486.so`) |
| Accelerate | 5 | CS forces it every round: `cs.so` `RestartRound @ab424` |
| Friction | 4 | same, `@ab439` |
| Stop speed | 75 | same, `@ab44e` |
| Air accelerate | 10 | engine cvar default |
| Edge friction | 2 | engine cvar default |
| Step size | 18 | engine cvar default |
| Max velocity | 2000 | engine cvar default |
| Jump speed | 268.328 (45-unit jump) | `cs.so` `PM_Jump @1274bc` |
| Jump stamina | 1315.79, drains 1000/s | `PM_Jump @1274fc` |
| Stamina slowdown | `(100 − stamina × 0.019) / 100` on jump height and landing speed | `PM_Jump`, `PM_WalkMove` |
| Bunny-hop cap | over 1.2 × max speed → scaled to 0.8 of the cap | `PM_PreventMegaBunnyJumping` |
| Crouch speed | × 0.333 | `PM_Duck` |
| Air wish-speed cap | 30 | `PM_AirAccelerate` |
| Walkable floor | normal z ≥ 0.7 | `PM_CatagorizePosition`, `PM_WalkMove` |
| Standing / crouched box | 32×32×72 / 32×32×36 | map collision hulls 1 and 3 |
| Eye height (from box centre) | 17 standing, 12 crouched | `PM_Duck` (17, and 12 − 18 = −6 mid-crouch) |
| Crouch time | 0.4 s on the ground, instant in the air | `PM_Duck` (0.6 = 1 − 0.4, spline scale 2.5) |
| Shift-walk | × 0.52 → 130 u/s with a knife | client cvar `cl_movespeedkey` (`client.so`) |
| Footsteps | only at 150 u/s or faster | `PM_UpdateStepSound` |
| Ladder climb / jump off | 200 / 270 u/s | `PM_LadderMove` |
| Swimming | 0.8 × max speed, sink 60 u/s, swim up 100 u/s | `PM_WaterMove`, `PM_Jump` |
| Fall damage | (speed − 500) × 100/600 × 1.25 HP; lethal ≈ 980 u/s | `FlPlayerFallDamage @a5200`, `CBasePlayer::PostThink` |

Weapon move speeds (AK 221, AWP 210, ...) are in `docs/RULES.md`.

## Doors

`sim/src/doors.rs`: `func_door` (sliding) and `func_door_rotating`, plus
`trigger_multiple` zones that open named doors (de_nuke's glass doors).
GoldSrc rules: named doors open only from their trigger, unnamed ones on touch,
use-only doors with E; rotating doors swing away from the opener; a closing
door that hits a player goes back; `wait`, `speed`, `lip`, toggle and one-way
come from the map.

## Not done yet

- Water jump-out (climbing out of water onto a ledge, `PM_CheckWaterJump`).
- Footstep interval is 300 ms ⚠ (HL convention, not yet read from `PM_UpdateStepSound`).
- Doors don't push players and have no sounds; buttons and breakables come with weapons.
- Ladder face direction is taken from the ladder box's thin side ⚠ (fine for the
  stock maps' straight ladders).

## Tests

- `cargo test -p sim`: flat-floor checks for landing, run speed, friction,
  45-unit jump, crouch speed and timing, bunny-hop cap, fall damage, shift-walk,
  and determinism.
- `sim/tests/maps.rs` (skipped if CS 1.6 isn't installed): every spawn on
  de_dust2, de_inferno, de_nuke and de_train lands, then runs and jumps without
  getting stuck; nuke's glass doors open from their trigger and close again;
  inferno's rotating door opens on touch; all 16 de_train ladders can be climbed.
- `cargo test -p client`: a rendered door sits exactly where its collision is.
