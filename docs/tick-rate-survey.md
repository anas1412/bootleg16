# Tick rate survey (128 today, 64 as a per-server option)

Read-only survey. Nothing was built or changed in code. Line numbers are from the tree as of 2026-10-17.

Scope: every place that assumes 128 ticks per second. "Global" means a `const` in `net/src/protocol.rs` that everything reads. "Derive" means it should come from one per-server value. "Keep" means it is a real constant (a sim tuning value or a test value) that does not change with the rate.

## Root values

1. `net/src/protocol.rs:14-15`: `TICK_RATE = 128`, `TICK = 1/TICK_RATE`. The source of almost every other site. **Derive.** Keep a `DEFAULT_TICK_RATE = 128` and move the live value into a per-server `TickRate` that carries `tick()` (seconds) and `per_sec()`.
2. `net/src/protocol.rs:16-17`: `SNAPSHOT_EVERY = 2`, comment says "64 per second at 128 tick". **Derive, but decide the policy first.** Options: keep 64 snapshots/s at both rates (`SNAPSHOT_EVERY = tick_rate / 64`, so 2 at 128 and 1 at 64), or keep the ratio (32/s at 64). I recommend the first: it keeps bandwidth, interpolation and `tests/bandwidth.rs` (which assumes 64/s) unchanged.
3. `net/src/protocol.rs:21`: `TIMEOUT_TICKS = 5 * TICK_RATE` (5 s). **Derive** from seconds.
4. `net/src/protocol.rs:140`: `MAX_UNLAG_TICKS = TICK_RATE / 2` (0.5 s lag-comp window). **Derive** from seconds.
5. `net/src/protocol.rs:368`: `Welcome.tick_rate` is already sent by the server (`net/src/server/handle.rs:65`). **Keep the field.** The client ignores it (`net/src/client.rs:277` uses `..`).

## net/src/socket.rs

6. `:35` `set_conditions`: `delay_ticks = ping_ms * TICK_RATE / 2000`. Fake-ping delay in ticks. **Derive.** Integer division: at 64 Hz each tick is 15.6 ms, so ping steps are coarse. Keep ping in ms and convert.

## net/src/client.rs (client-side networking)

7. `:14` `INTERP_TICKS = SNAPSHOT_EVERY * 2` (4 ticks = 31 ms). **Derive from time:** two snapshot intervals. If `SNAPSHOT_EVERY` becomes 1 at 64 Hz this stays 2 ticks = 31 ms. Written in ticks it would silently double to 62 ms.
8. `:16` `HELLO_EVERY = TICK_RATE / 4` (retry 4x/s). **Derive** (0.25 s).
9. `:254`, `:335`, `:345`, `:346`, `:372`: prediction runs `player::step` with `TICK` and the clock uses `seq * TICK`. **Derive** via the runtime value. These must match the server's `TICK` exactly.
10. `:267`: render-clock jump threshold `TICK_RATE as f32 / 4.0` (0.25 s). **Derive** from seconds.
11. `:445`: resend looks every `TICK_RATE / 2` (0.5 s). **Derive.**
12. `:459`: resend join every `TICK_RATE / 4` (0.25 s). **Derive.**
13. `:523`: `(ticks - heard) as f32 * TICK`. Fine once `TICK` is runtime.

## client/src/

14. `client/src/main.rs:93-94`: `Time::<Fixed>::from_hz(128.0)`, comment "like the server". **Derive, and not at startup.** The client connects to servers of different rates, so set the timestep from the Welcome `tick_rate` on connect (`Time::<Fixed>::set_timestep_hz`). Otherwise prediction and the fixed-step connection loop (`menu/connect.rs:203`, `menu/mod.rs:79`) run at 128 regardless.
15. `client/src/level.rs:218` and `:220`: `doors.update(1.0 / 128.0, ...)`. Inside a `cargo test -p client` test. **Keep the test, but use the test's own rate** (or the derived `TICK`).
16. `client/src/game.rs` calls `client.tick(input)` once per fixed step. No literal, but it depends on the fixed-step rate in item 14.

## sim/src/ (movement and game rules)

17. `sim/src/pmove.rs` `step(..., dt, ...)` takes `dt` as an argument. The sim has no tick constant in its signature. **Keep** the function. I read its signature and tests, not every line of its body, so before relying on this, grep for any per-step constant inside `step` that assumes 1/128. This is the main open check for item 17.
18. `sim/src/pmove.rs:699`: test-only `DT = 1/128`. `:702` loops `secs / DT`. **Keep** (test value), tie to the same runtime `TICK` if tests are parametrized.
19. `sim/src/pmove.rs:742-744`: the jump test runs `for _ in 0..128` steps after the jump. **See the 64 Hz note below.**
20. `sim/src/pmove.rs:776`: `i % 64` input pattern in a test. **Keep** (test pattern).
21. `sim/src/grenades.rs:205`, `:222`: test `DT = 1/128` and grenade physics. **Keep** (test); runtime physics is driven by the server's `TICK` (item 40).
22. `sim/src/player.rs:119-122`: `tagging(..., 1/128)` and a 64-iteration loop, "recovered after ~0.5 s". Test. **Keep**, but the comment's "0.5 s" only holds at 128 Hz if the step is 1/128. Use `TICK`.
23. `sim/src/weapons/tests.rs:3`, `:23`, `:42`, `:50`, `:244`, `:254`: test `DT = 1/128`, fire-control loops of 128 and 64. **Keep** (tests), convert to seconds.
24. `sim/tests/maps.rs:7`, `:33-34`, `:55`, `:80-86`, `:106`, `:133`, `:350-351`, `:448-460`: test `DT = 1/128` and 128 * seconds loops, including door timing and ladder tests. **Keep** the physics, convert loop counts to seconds, so a 64 Hz run means the same thing.

## net/src/server/ (dedicated server and match)

25. `server/mod.rs:21`: `HEARTBEAT_EVERY = HEARTBEAT_SECONDS * TICK_RATE`. Already seconds-based. **Derive** (the value is only the multiplier).
26. `server/mod.rs:59`: `BACKLOG_TICKS = TICK_RATE / 4` (0.25 s). **Derive** from seconds.
27. `server/mod.rs:65`: `DUMMY_RESPAWN_TICKS = 2 * TICK_RATE` (2 s). **Derive.**
28. `server/mod.rs:86`: `REPICK_TICKS = TICK_RATE / 2` (0.5 s). **Derive.**
29. `server/mod.rs:257`, `:286`; `server/reserve.rs:80`, `:254`; `server/round.rs:52`: `Match::new(cfg, TICK_RATE)`. **Derive**, pass the server's value.
30. `server/mod.rs:378`, `:402`: `rewind` filters history by `tick % SNAPSHOT_EVERY` (good, works at any value). `if tick % TICK_RATE == 0` runs `guard.tidy` once a second. **Derive** (once-per-second, so use `per_sec()`).
31. `server/mod.rs:448`: history is kept for `TICK_RATE` ticks (1 s). **Derive** from seconds.
32. `server/mod.rs:453`: `TIMEOUT_TICKS`. Via item 3.
33. `server/mod.rs:437-443`: `anim` time `tick * TICK`. Follows `TICK`.
34. `server/mod.rs:31-40` area: `MAX_INPUT_BUDGET = 16.0` (inputs a client may bank while packets are delayed). **Derive** to a time (about 125 ms at 128 today; 250 ms at 64 if left alone).
35. `server/handle.rs:72`: ping `ms = ticks * 1000 / TICK_RATE`. **Derive** (correct once the value is runtime).
36. `server/handle.rs:87`: input queue cap `c.queue.len() > TICK_RATE` (1 s of input). **Derive** from seconds.
37. `server/packets.rs:41`: `if tick % TICK_RATE == 0` (once a second). **Derive** via `per_sec()`.
38. `server/packets.rs:46`: seat retention `60 * TICK_RATE` (1 minute). **Derive.**
39. `server/players.rs:28-29`, `:89`, `:91`, `:95`: each queued input runs with `TICK`, and `world.update(TICK)`. **Derive.** These are the rate-critical lines; see Risks.
40. `server/hits.rs:8-10`, `:73`, `:114`, `:147`; `server/ground.rs:11-12`; `server/buy.rs:55`, `:130`; `server/bomb.rs:71`, `:187-188`; `server/surrender.rs:102`; `server/reserve.rs:262`; `server/admin.rs:45`: all use `TICK` for grenade physics, gravity, command clocks, timers. **Derive** via `TICK`.
41. `server/bomb.rs:79`, `:142`: `c4_timer * TICK_RATE` (seconds to ticks). **Derive.**
42. `server/surrender.rs:36`, `:60`, `:75`, `:135`; `server/reserve.rs:143`, `:149`, `:170`, `:202`: vote and abandon windows `* TICK_RATE`. **Derive.**
43. `server/running.rs:31`: drain window `2 * TICK_RATE`. **Derive.**
44. `server/running.rs:40`: the real-time loop `Duration::from_secs_f64(1.0 / TICK_RATE)`. **Derive.** This is what makes the server run at 64 Hz in real time.
45. `server/snapshot.rs:7`: `SENT_KEPT = TICK_RATE / SNAPSHOT_EVERY / 2` (32 at 128, half a second of baselines). **Derive** from seconds.
46. `server/snapshot.rs:12`: scores every `TICK_RATE / 4` (0.25 s). **Derive.**
47. `server/snapshot.rs:36`: `if tick % SNAPSHOT_EVERY == 0` (the snapshot cadence). Derived via item 2.
48. `server/recording.rs:6`: `SCORES_EVERY = TICK_RATE / SNAPSHOT_EVERY` (0.5 s). **Derive** from seconds.
49. `server/visibility.rs:8`: `GRACE_TICKS = TICK_RATE / 4` (0.25 s). **Derive** from seconds.
50. `server/aimcheck.rs:28`: `SAME_FIGHT_TICKS = TICK_RATE / 2`. **Derive.** `:85` and `:156`: reaction times stored in ticks, converted with `* 1000 / TICK_RATE` into ms, compared to `FAST_MS`. **Derive** (correct), but see the granularity risk.
51. `server/guard.rs:10`: comment "inputs at 128/s". `PLAYER_RATE = 600` is per second and needs no change. **Keep**; fix the comment. `:40`, `:50`, `:57`, `:61`: cookies use `tick / TICK_RATE`. **Derive.** `:73-84`: test loop over `TICK_RATE`. **Keep** (test, follows the value).
52. `server/lobby_link.rs:29`: `60 * TICK_RATE` lookup cadence. **Derive.**
53. `server/mod.rs:354`: `64.0 * 64.0` is a distance, not a rate. **Keep.**
54. `server/mod.rs:31`: `MAX_OUTBOX = 64` and `BACKLOG_INPUTS = 4` are counts. **Keep.**

## net/src/ (other)

55. `net/src/rounds.rs:44`, `:79-84`, `:89`: `Match` already takes `tick_rate` and converts seconds with `* self.tick_rate`. **Already derived.** Just pass the server's value (item 29).
56. `net/src/demo.rs:83`: header writes `tick_rate: TICK_RATE` (line 25 is the header field). **Derive** from the server value. The header already carries the rate; it is the replay that ignores it (item 57).
57. `net/src/replay.rs:5`, `:80`, `:127`: replay uses the global `SNAPSHOT_EVERY` and `TICK`. It sends `h.tick_rate` in the Welcome (`:71`) but then runs at the global rate. **Bug at 64 Hz.** A 64-tick demo would play back at 128-tick speed. **Derive** from `h.tick_rate`.
58. `net/src/delta.rs`, `net/src/crypto.rs`, `net/src/lobby/*`: no tick assumption found. **Keep.**
59. `net/src/bot/brain.rs:93`: `5 * 128` (noise memory, 5 s, in ticks). **Derive**, wrong by 2x at 64 otherwise.
60. `net/src/bot/brain.rs:172`: `% (128 * 20)` (re-route every 20 s). **Derive.**
61. `net/src/bot/brain.rs:282`: `tracked = (t - since) / 128` (seconds). **Derive** to `TICK`.
62. `net/src/bot/brain.rs:325`, `:336`: `t % 64 == 0` (stuck check every 0.5 s). **Derive.**
63. `net/src/bot/brain.rs:351`: `(t / 128) % 4` (90-degree strafe cycle, 1 s per step). **Derive.**

## server/src/ (binary)

64. `server/src/main.rs:72`: startup banner prints `TICK_RATE`. **Derive** (server value).
65. `server/src/main.rs:8`: `--from TICK` on `--play` is a tick number. **Keep the flag, document it as "in the demo's own ticks"** or accept seconds. Changing the rate changes what a given tick number means.
66. `server/src/review.rs:55`, `:70`: `LEAD_SECONDS * TICK_RATE`, and a test with `10 * TICK_RATE`. **Derive** from the demo header's rate, not the global.

## net/examples/ and net/tests/

67. `net/examples/thrower.rs:15`, `shooter.rs:18`, `shooter.rs:35`, `:39`, `:43`, `bots.rs:75`: `1.0 / 128.0` and `/ 128` loops. **Keep** (examples), or derive if they should follow the server setting.
68. `net/tests/common/mod.rs:72`, `:80`; `multiplayer.rs:22`, `:32`, `:117`, `:148`, `:218`; `reserved.rs` (many); `leavers.rs` (many); `deathmatch.rs:55`, `:69`, `:125`; `rounds.rs:16`; `visibility.rs` (many); `recording.rs:28`; `bots.rs:65`, `:76`: literal 128/64 loop counts and `ticks < secs * 128`. **Convert to seconds** and run the key ones at both rates.
69. `net/tests/bandwidth.rs:1`, `:28`: "sent 64 times a second", `avg * 64.0`. **Keep** if snapshots stay 64/s (recommended in item 2). Otherwise derive.

## Docs (not code)

70. `docs/NETWORKING.md`: "128 per second" (inputs), "64 times a second" (snapshots), "~31 ms". Update after the policy in item 2 is set.
71. `docs/MOVEMENT.md:6`: "runs at 128 ticks/s (64 also works)". Verify before keeping.
72. `docs/RULES.md:51`: "Tick rate | 128 (64 option)". Already matches the plan.

## Risks

- **Client and server must agree on the rate.** Inputs are one per fixed step on the client and one per tick on the server. A mismatch breaks prediction (`net/tests/multiplayer.rs` asserts exact agreement). The client already gets `Welcome.tick_rate` and ignores it. Item 14 is the fix.
- **Demos.** Replay ignores the header rate (item 57) and uses global `TICK`. Recordings made at 64 would replay at 128 speed. Also `--from TICK` and `review.rs` lead times (items 65-66).
- **Prediction and lag compensation.** `rewind` interpolates between snapshot ticks. If snapshot cadence changes (item 2), it still works, but `MAX_UNLAG` and the 1 s history must be in seconds or the 0.5 s window doubles in ticks.
- **Interpolation delay.** Written in ticks, 31 ms becomes 62 ms at 64 Hz. Must be in seconds (item 7).
- **Input budget and queue caps.** `MAX_INPUT_BUDGET` (16) and the queue cap (`TICK_RATE`) are in inputs. At 64 Hz they'd be 250 ms and 1 s if not converted.
- **Granularity.** At 64 Hz a tick is 15.6 ms (vs 7.8 ms). Aimcheck reaction thresholds (item 50) and ping resolution (item 6) get coarser. The anti-cheat thresholds may need re-tuning.
- **Bots.** Brain timers in ticks (items 59-63) would run at half speed on time if not converted.
- **Tests pass silently.** `ticks < secs * 128` at 64 Hz just passes early, and most loops do not fail when the rate changes. Parametrize the key tests over both rates.
- **Not verified.** I did not read every line of `pmove::step`, `doors::update`, or the weapon fire-control code. Item 17 is the open check.

## 64 Hz note on the pmove 128-iteration loop

`sim/src/pmove.rs:742-744` runs 128 steps after the jump, with `DT = 1/128`, so it simulates 1 s. The test only checks that the jump reaches 45 units and lands. At 64 Hz, the test's `DT` must be 1/64. Then 128 steps is 2 s. That's still enough for the landing, so the test passes, but the comment and intent change. The clean fix is `(secs / DT)` with `secs = 1.0`.

## Proposed order

1. Add `TickRate` (runtime value plus `tick()` and `per_sec()`) with `DEFAULT_TICK_RATE = 128`. Keep `TICK_RATE` as a deprecated alias for now.
2. Convert all time-based consts to seconds * rate at the call sites (items 3, 4, 7, 8, 10-12, 25-28, 36, 38, 42-49, 51, 52). Behaviour stays identical at 128.
3. Decide the snapshot policy (item 2) and make `SNAPSHOT_EVERY` derived. Check `bandwidth.rs` and `visibility` still pass at 128.
4. Client: set `Time<Fixed>` from `Welcome.tick_rate` on connect (item 14). Remove the startup `from_hz(128.0)`.
5. Demos and replay: use the header rate (items 56-57, 65-66). Add a regression test for a 64-tick demo.
6. Bots and aimcheck literals (items 50, 59-63).
7. Tests: convert literal loop counts to seconds and run the multiplayer, bots, and rounds tests at 64 and 128.
8. Docs (items 70-72), once the policy is final.
