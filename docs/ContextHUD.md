# HUD handoff

For the next session taking over the in-game HUD and the menu screens. Read this, then
`docs/UI.md` (rules) and `docs/TODO.md` (open items).

## Spec and how to check

- Design spec: `design/mockups/ui/` (`screens.js` per screen, `components.css`). Open
  `index.html?bare&still#<screen>` in a browser; screen names are the keys in `SCREENS`.
- Published copy: https://claude.ai/artifact/DycSLbUmmEed1hNjpoagiS
- Build: `systemd-run --user --scope -q -p MemoryMax=3G -- cargo build -p client -j 2`
- Screenshot a moment in game: `BOOTLEG16_SCREENSHOT=<png> BOOTLEG16_SCREENSHOT_AT=<secs>`.
  Other switches: `BOOTLEG16_MENU=1` (buy menu), `BOOTLEG16_TEAM=ct|t`, `BOOTLEG16_SPECTATE=1`,
  `BOOTLEG16_VOICE=1` (voice rows, teammate tags talking), `BOOTLEG16_LOOK=yaw,pitch`.
- Test match: run `target/debug/server de_dust2 27116`, then two bots with
  `target/debug/examples/shooter 127.0.0.1:27116 ct 90 0` and `... t 90 0`, then the client
  with `--connect 127.0.0.1:27116`. Warmup ends only when both sides have players.
- Don't `pkill` by name: it can kill your own screenshot job.

## Layout (1080p design units; `SAFE` = 32 from the edge)

| Where | What | File |
|---|---|---|
| Top left | Radar, 256 square | `radar.rs` |
| Under radar | Money, buy-zone icon | `round_hud/money.rs` |
| Under money (`VOICES_TOP`) | Voice rows: who talks, level | `voice/rows.rs` |
| Top centre | Score, clock (CS 1.6 warning from 30 s, silent), Round n · Buy 0:14 | `round_hud/mod.rs` |
| Top centre, 190 down | Vote panel | `match_flow.rs` |
| Top right | FPS and ping | `hud/mod.rs` |
| Under FPS (`FEED_TOP`) | Kill feed | `killfeed` |
| Bottom left | Health, armor | `hud/mod.rs` |
| Bottom right | Weapon, clip and reserve | `hud/mod.rs` |
| Over the weapon panel | What you carry: guns, knife, grenades with counts, C4 (red in a site), kit | `hud/owned.rs` |
| Bottom centre, 260 up | Notice line (spectating, dead) | `hud/mod.rs` |
| Low middle, 320 up | Short hints | `hud/hint.rs` |
| Bottom centre, `SAFE` up | Spectate plate | `spectate.rs` |
| Under the score, 190 down | Reconnecting panel | `status_screens.rs` |
| Centre | Crosshair, scope | `hud/crosshair.rs` |
| Over the world | Teammate tags | `teammates.rs` |
| Over everything | Buy menu, Esc menu, scoreboard, notices, loading | `buy.rs`, `scoreboard.rs`, `status_screens.rs` |

## Rules already settled

- HUD panels are HUD glass with an edge light in your side's colour.
- HUD roots carry `HudLayer` or `Visor` and go through the visor (the HUD curve setting).
  Menus, buy menu and scoreboard stay on the flat camera, so clicks work.
- Spectating shows the watched player's health, armor, weapon, clip and reserve
  (`Remote::mate.ammo`). Your money stays on. The crosshair hides.
- Teammate tags need names on (`team_ids >= 2`). The mic shows above the name only while
  that teammate talks. The tag hangs from the head (`HEAD` in `teammates.rs`).
- The flash overlay in `effects.rs` must stay `Pickable::IGNORE`, or every click in the
  buy menu dies. Test buy clicks after any overlay change.
- Menus over the game are one state in `overlay.rs` (`Overlay`: None, Menu, Teams, Buy): Esc
  closes what's open or opens the Esc menu (titled Menu: no pause, as in CS 1.6), M team
  pick, B the shop. The mouse is freed there too. You can still move while shopping.
- Shop: stays open after a buy; double-click buys (0.4 s); numpad works; cards you own say
  OWNED (no price); what you can still buy and at what price comes from the server's
  `net::server::buy::cost_now` (full armor makes the helmet $350); Undo buys is greyed until
  you buy (`Client::undo_buys`); buy time shows in the header and the top bar
  (`round_hud::buy_time`).
- Loading stays up until the first frame is drawn at the spawn. Its steps and progress come
  from `loading.rs`.
- Errors show as a notice (`notice.rs`), not a red line on the menu. Retry only when a
  retry can work.
- Every fix works on Windows and Linux.

## Ownership

- This session: menu pages, HUD, buy menu, spectate, teammate tags, `status_screens.rs`, mockups.
- The other session: netcode, sim, server, `net/`, `play.rs`, `steam.rs`, `match_flow.rs`,
  `level.rs`, `models.rs`, `players.rs`, `impacts.rs`, `effects.rs`, `viewmodel.rs`,
  `studio.rs`, `map_sounds.rs`, `sound.rs`, and the loading and notice logic in
  `loading.rs`, `notice.rs`, `menu/connect.rs`, `game.rs`.
- Ask before editing their files. Their sound rework was in progress when this was written.

## Open

- Reconnecting: the 30 s give-up path is not yet checked on screen.
- Mockup vs game: shop card layout differs (price position, ammo line), the mockup's Esc
  menu has a Settings item the game lacks.
- Weapon switch briefly shows a character model; smoke too transparent (rendering, `docs/TODO.md`).
- Release build and offline bundle (`tools/bundle.sh`): on hold until the user is satisfied.
- Alt+Tab / Alt+Enter crash reported by the user: parked. Never stress-test window modes.
- Dead-code warnings in `ui/kit.rs`, `ui/motion/` and `ui/tokens.rs` left by the old connecting screen.
