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
| Eye height (from box centre) | 17 standing, 12 crouched | `PM_Duck` (17); 12 ⚠ from the HL SDK convention |

Weapon move speeds (AK 221, AWP 210, ...) are in `docs/RULES.md`.

## Not done yet

- Crouching is instant. CS 1.6 takes ~0.4 s to crouch. ⚠ Extract the timing from `PM_Duck`.
- Ladders, water and fall damage (`PM_LadderMove`, `PM_WaterMove`, `PM_CheckFalling`).
- Moving brush entities (doors). They collide, but as if they never move.
- Footsteps, view bob, view punch.

## Tests

- `cargo test -p sim`: flat-floor checks for landing, run speed, friction,
  45-unit jump, crouch speed, bunny-hop cap and determinism.
- `sim/tests/dust2.rs`: every de_dust2 spawn lands, then runs and jumps for 4 s
  without getting stuck or falling out (skipped if CS 1.6 isn't installed).
