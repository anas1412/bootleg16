# Handoff: make the tick rate a per-server setting (bootleg16)

For a Claude Code session running on the user's own machine (blackbox), in the repo
`/home/blackbox/bootleg16`, branch `main`. Read this top to bottom once, then do the work.

The previous session ran in a sandbox VM with no Rust toolchain and no GitHub credentials,
so nothing below has been compiled with cargo and nothing has been pushed. You can do both.

## Goal

Today the server and client assume 128 ticks per second in about 70 places. Make the tick
rate a per-server setting: default 128, changeable to 64 at any time (config entry plus a
`--tick 64` flag). Clients learn the rate from the server. At 128 every behaviour stays
exactly as it is today.

Why: one match runs on one CPU core (one thread, fixed-rate loop in
`net/src/server/running.rs`; `host.sh` starts one game server per core). Halving the rate
should roughly double the matches per core. 128 stays the default and the target for
competitive play. This is a user decision, already made: "default 128, switchable to 64".

User's design wish: unification, modularity, centralization. One timing value, built once,
passed in. No scattered literals. No global constant for the live rate.

## State at handoff (2026-10-17)

- `main` has one new local commit, `ff885ba`: adds `net/src/tick.rs` and `pub mod tick;` in
  `net/src/lib.rs`. `TickRate` has `new(hz)` (only 64 or 128 allowed), `hz()`, `tick()`
  (seconds per step), `secs(seconds)` (whole ticks, rounded), and `Default` (128). It has 5
  unit tests. Nothing uses it yet. No behaviour change.
- That commit is NOT pushed. It was never compiled with cargo. The 5 tests passed only when
  the file was compiled standalone with `rustc --test`.
- `docs/tick-rate-survey.md` (untracked) is the site-by-site survey. Its content is
  reproduced below, so you do not need it. Commit it if you want to keep it.
- `docs/ContextHUD.md` (untracked) belongs to the UI session. Do not touch it, do not commit it.
- Harmless leftovers from the sandbox, delete them: local branch `tick-rate-setting`
  (`git branch -D tick-rate-setting`), `.git/tick-verify.bundle`, and `.git/stale-*lock*`
  files (renamed stale git locks).
- The other repo docs are accurate: `docs/Context.md` (engine handoff, rules, systems),
  `docs/TODO.md`, `docs/GOTCHAS.md`, `docs/NETWORKING.md`, `docs/MOVEMENT.md`,
  `docs/RULES.md`.

## Do these first

1. `git status` and `git log --oneline -3`. Expect `ff885ba` on top of `1160e8d`.
2. Clean the leftovers above.
3. Verify the existing commit:
   `systemd-run --user --scope -q -p MemoryMax=3G -- cargo test -p net --lib tick`
   If it fails, fix it before anything else.
4. `git push origin main`.

## Rules (CLAUDE.md and docs/Context.md, they override defaults)

- Work on `main`. No feature branches (user's instruction).
- Build with `systemd-run --user --scope -q -p MemoryMax=3G -- cargo build -p <crate> -j 2`.
  An uncapped build froze the PC.
- Do not change Cargo features, profiles or `.cargo/config.toml`: that rebuilds Bevy
  (30-45 min). The net and sim crates do not use Bevy, so building them is quick.
- Run only the tests for what changed. Net tests run a real server and clients over localhost
  and skip themselves when de_dust2 is not installed.
- No god files. One module per concern, split a file past about 400 lines.
- Commits: no Claude co-author line and no "Generated with" or session trailer. This
  overrides the harness default. Never `git commit -a`: add files by name. Two sessions can
  share one working tree.
- Wayland only: run the client with `env -u DISPLAY`. Do not `pkill` by name.
- The UI session ("Game HUD/UI design concepts") owns menus, HUD, `status_screens.rs`,
  `design/mockups`, `ui/`. This work should not need any of them. If it does, message that
  session first.
- Game copy has no em dashes.
- Everything must work on Windows and Linux.
- Open items go in `docs/TODO.md`, lessons and traps in `docs/GOTCHAS.md`.
- The user wants short answers in simple terms. Keep your reports brief.

## Design to follow

- `net/src/tick.rs` (exists) is the single source of truth. Keep it in the `net` crate. If
  `sim` needs the rate, pass the `dt` value in (as `pmove::step(..., dt, ...)` already does),
  do not make `sim` depend on `net`.
- Keep `net::protocol::TICK_RATE` and `TICK` as the 128 default while converting, then remove
  them once nothing reads them (step 8 below). Never let new code read them for the live rate.
- Write time values in seconds, convert with `rate.secs(x)`. Never write a count of ticks for
  something that is really a duration. Example: `TIMEOUT_TICKS = 5 * TICK_RATE` becomes a
  5.0 s constant and `rate.secs(TIMEOUT_SECS)` at the use site.
- The server holds one `TickRate`, built once from config (`config/server.cfg`, a new
  `tick_rate` entry, and `server --tick 64`; reject anything but 64 or 128). It passes it to
  `Match::new(cfg, rate.hz())` (already takes a rate), the run loop, snapshots, and so on.
- The client takes the rate from `Welcome.tick_rate` (already sent by
  `net/src/server/handle.rs:65`, ignored by the client at `net/src/client.rs:277`). It stores a
  `TickRate`, and sets Bevy's fixed timestep on connect with `Time::<Fixed>::set_timestep_hz`.
  Remove the startup `Time::<Fixed>::from_hz(128.0)` in `client/src/main.rs:93-94`.
- Prediction needs client and server to use the exact same step size. This is the most
  important invariant.
- Bump `PROTOCOL_VERSION` (currently 31) in `net/src/protocol.rs` when you change what the
  client does with Welcome, so mismatched builds are turned away.
- Decision already recommended, take it unless it causes trouble: keep **64 snapshots per
  second at both rates**. `SNAPSHOT_EVERY = tick_rate / 64` (2 at 128, 1 at 64). Bandwidth,
  interpolation delay and `net/tests/bandwidth.rs` stay unchanged.

## The work, in order

Do each step as its own small commit on `main`. After each: build `net` (and `sim`/`server`
if touched), run the tests for what changed, commit by file name. Behaviour at 128 must not
change. Push after each step or at the end, as the user prefers.

**Step 1. Done** (`ff885ba`). Verify and push, as above.

**Step 2. Convert time-based values to seconds x rate.** One file per commit. Files, with the
survey item numbers in brackets. Keep the 128 values identical.
- `net/src/protocol.rs`: TIMEOUT_TICKS [3], MAX_UNLAG_TICKS [4], keep `Welcome.tick_rate` [5].
- `net/src/socket.rs`: fake-ping delay [6]. Keep ping in ms and convert (integer division is coarse at 64).
- `net/src/client.rs`: INTERP_TICKS [7], HELLO_EVERY [8], prediction and clock `TICK` uses
  [9], clock jump threshold [10], resend loops [11, 12], [13].
- `net/src/server/mod.rs`: HEARTBEAT_EVERY [25], BACKLOG_TICKS [26], DUMMY_RESPAWN_TICKS [27],
  REPICK_TICKS [28], `Match::new` calls [29], once-a-second tidy [30], history length [31],
  anim time [33], MAX_INPUT_BUDGET [34].
- `net/src/server/` others: `handle.rs` ping ms [35] and input queue cap [36]; `packets.rs`
  once-a-second tick and seat retention [37, 38]; `players.rs` and `hits.rs`, `ground.rs`,
  `buy.rs`, `bomb.rs`, `surrender.rs`, `reserve.rs`, `admin.rs` (all use `TICK` for movement,
  grenade physics, gravity, command clocks, timers) [39, 40]; `bomb.rs` c4 timer [41];
  `surrender.rs` and `reserve.rs` vote windows [42]; `running.rs` drain window [43];
  `snapshot.rs` SENT_KEPT and score cadence [45, 46]; `recording.rs` [48]; `visibility.rs`
  grace [49]; `aimcheck.rs` [50]; `guard.rs` cookies [51]; `lobby_link.rs` [52].
- `net/src/rounds.rs` already takes `tick_rate` [55]: just feed it the server's value.

**Step 3. Snapshot policy.** Make `SNAPSHOT_EVERY` derived from the rate (see Design), with
a seconds-based interpolation delay (about 31 ms). Check `net/tests/bandwidth.rs` and
`visibility` tests still pass at 128.

**Step 4. Server loop and client.** `net/src/server/running.rs:40` real-time loop uses
`1.0 / rate` [44]. Add the `tick_rate` config entry and `--tick` flag, banner in
`server/src/main.rs:72` [64]. Client: set the fixed timestep from `Welcome.tick_rate`
[14], keep `client/src/game.rs` calling `client.tick(input)` once per fixed step [16].
Bump `PROTOCOL_VERSION`.

**Step 5. Demos and replay (a real bug at 64).** `net/src/demo.rs:83` writes `TICK_RATE` in
the header, write the server's rate [56]. `net/src/replay.rs` ignores the header rate and
runs at the global rate even though it sends `h.tick_rate` in Welcome [57]: make it use
`h.tick_rate`. `server/src/review.rs:55,70` lead-in seconds use the demo header's rate [66].
`server/src/main.rs:8` `--from TICK` is a tick number in the demo's own ticks: document it
[65]. Add a regression test: record a 64-tick demo and replay it at the right speed.

**Step 6. Bots and aimcheck.** `net/src/bot/brain.rs`: noise memory `5 * 128` [59], re-route
`128 * 20` [60], tracked time `/ 128` [61], stuck check `t % 64` [62], strafe cycle
`(t / 128) % 4` [63]: convert to seconds. `aimcheck.rs` stores reaction times in ticks and
converts to ms [50]: correct once the rate is runtime, but a tick is 15.6 ms at 64 Hz, so the
thresholds get coarser. Do not retune them now, add a note to `docs/TODO.md` (the aim checks
already have an open "tune against real players" item).

**Step 7. Tests.** Literal 128 and 64 loop counts and `ticks < secs * 128` in `net/tests/`
(`common/mod.rs`, `multiplayer.rs`, `reserved.rs`, `leavers.rs`, `deathmatch.rs`,
`rounds.rs`, `visibility.rs`, `recording.rs`, `bots.rs`) [68]: convert to seconds, then run
the key ones (multiplayer, rounds, bots, leavers, recording) at both 64 and 128. They do not
fail on their own when the rate changes: loops finish early and still pass, so parametrize
them. `sim` tests use `DT = 1/128` and `128 * seconds` loops (`sim/src/pmove.rs`,
`grenades.rs`, `player.rs`, `weapons/tests.rs`, `sim/tests/maps.rs`) [18-24]: convert to
seconds so a 64 Hz run means the same thing. `client/src/level.rs:218,220` test [15].
`net/tests/bandwidth.rs` stays if snapshots stay 64/s [69]. Examples in `net/examples/`
[67] can keep 128.

**Step 8. Cleanup and docs.** Remove `TICK_RATE`/`TICK` from `protocol.rs` once unused.
Update `docs/NETWORKING.md` (inputs per second, snapshots per second, interpolation ms),
`docs/MOVEMENT.md:6` ("runs at 128 ticks/s (64 also works)": verify), `docs/RULES.md:51`
(already says "128 (64 option)"), `README.md` hosting section (new `--tick` option and
`host.sh`, which may need a `--tick` passthrough). Tick off the item in `docs/TODO.md`,
add what you learned to `docs/GOTCHAS.md`.

## Not checked by the survey (check these yourself)

- The body of `sim/src/pmove.rs::step`: grep for any per-step constant that silently assumes
  1/128 (survey item 17). This is the main open check.
- `sim::doors::update`, the weapon fire-control code and `tagging` for the same assumption.
- `sim/src/pmove.rs:742-744`: a jump test runs `for _ in 0..128` steps after the jump with
  `DT = 1/128` (1 s). At 64 Hz the `DT` must be 1/64, then 128 steps is 2 s: still enough, but
  write it as `secs / DT` with `secs = 1.0`.

## Risks to watch

- Client and server must agree on the rate, or prediction breaks (`net/tests/multiplayer.rs`
  asserts exact agreement). A client built before this change talking to a 64 Hz server: the
  protocol version bump turns it away.
- Demos recorded at 64 replay at 128 speed until step 5.
- Lag compensation and the 1 s history window must be in seconds, or they double in ticks.
- Interpolation delay in ticks would double from 31 ms to 62 ms at 64 Hz (item 7).
- `MAX_INPUT_BUDGET` (16 inputs) and the input queue cap are counts: 125 ms and 1 s at 128,
  250 ms and 2 s at 64 if left alone.
- Granularity: a tick is 15.6 ms at 64 Hz (7.8 ms at 128). Ping resolution and aimcheck
  reaction times get coarser.
- Lobby and hosting: servers at different rates should not be mixed in one queue unless the
  lobby knows each server's rate. Out of scope here, add it to `docs/TODO.md`.

## Done means

- Default run is byte-for-byte the same behaviour at 128: existing net and sim tests pass.
- `server --tick 64` runs a match at 64 Hz, a client connecting to it plays at 64 Hz with
  correct prediction (no rubber-banding on `net/tests/multiplayer.rs` at 64), and a 64 Hz demo
  replays at the right speed.
- No remaining literal 128 or 1.0 / 128.0 for the live rate. Quick check:
  `grep -rnE "128|TICK_RATE|\bTICK\b" --include=*.rs net sim server client`.
- Everything is committed on `main` and pushed, with a short note in `docs/TODO.md` and
  `docs/GOTCHAS.md`.

## Report back

Short, plain: what you changed per step, test results at 128 and 64, anything you decided
differently from this plan and why, and what is left. Update `docs/Context.md` if the engine
handoff needs it.
