# UI design system: Interference (v1, the Bevy client)

**Replaced.** This is the first Interference, still drawn by the frozen Bevy client
(`client/src/ui`). Its mockups were removed on 2026-10-27 and v2 is being designed from
scratch for the Godot client, from the reference board picks (Godot port plan, P2). Don't
build v2 on anything below.

The spec is the mockup, `design/mockups/ui` (serve `design/`, open `mockups/ui/index.html`;
`system.html` is the whole system on one sheet). The code is `client/src/ui`. The two use the
same names: `tokens.css` mirrors `ui/tokens.rs`, and each mockup component in `ui.js` has one
widget in `ui/`. Change both together.

## The idea
The UI is a signal. At rest it is clean. When something changes, it splits into its two
channels and tunes back in: cyan (Bureau, `CH_A`) to one side, pink (Strays, `CH_B`) to the
other. The split only happens in transitions (entering, a hover burst, a choice, a value
stepping), never at rest. **Amber is you**: your selection, your main action, your row.

## Tokens (`ui/tokens.rs`)
- **Surfaces:** `BG0 #07080B` page and clear colour, `BG1 #0E1116` panels and cards,
  `BG2 #151A21` raised (toasts), `BG3 #1D242D` selected and avatars.
  `HUD_BG rgba(7,9,12,.76)` glass over the game. `BACKDROP rgba(7,8,11,.72)` dims behind menus.
- **Text:** `TX1 #EAF2F8`, `TX2 #8E9BA8`, `TX3 #56616D` (menus only, never over the game).
- **You:** `AMBER #FFA000`, `AMBER_INK #170E00` on amber, `AMBER_WASH` .08 and `AMBER_WASH_2`
  .16 under a selection or a hovered main action.
- **Sides:** `CH_A #49C6FF` = `CT_COLOR` (Bureau), `CH_B #FF4F8B` = `T_COLOR` (Strays).
- **States:** `OK #5BE3A0` (money, ready, online), `BAD #FF4040` (low health, can't afford, the
  last seconds). Red always means a problem: waiting or neutral states are grey.
- **Lines:** `LINE` .10 hairlines, `LINE_2` .24 quiet frames, `LINE_3` .45 hovered frames
  (all `rgba(200,230,255,a)`); `HOVER` .04 wash under a hovered row.
- **Space (1080p px, `vh()` scales with the window height):** `GAP_S 8`, `GAP 16`, `PAD 24`,
  `SAFE 32` (HUD from every edge), `BAR 4` (the amber bar); menu pages keep 96 at the sides.
- **Shape:** `RADIUS 6` panels and cards, `RADIUS_S 4` chips, rows and buttons. A soft lift
  shadow under panels (`panels::lift`).
- **Type (`Type`):** Display 150 and H2 52 in Big Shoulders 800; H3 28, Body 18 and Label 13
  (caps, +.18 em) in Chakra Petch 600; Jp 13 in Noto Sans JP 700; NumL 56, NumM 32, NumS 18 in
  JetBrains Mono 700. Every number is mono and tabular.
- **Motion:** `T_HOVER .14`, `T_TUNE .32`, `T_OUT .18`, `T_BURST .24`, `T_SLIDE .28`,
  `T_ROLL .22`, `STAGGER .04`, `T_BLUR .22`, `SPLIT_IN 8` px.

## Components
- **`panels`:** `soft` (radius), `lift`, `hud_panel` (glass, tinted 7% to your side via
  `Tint::Team`), `card` (selected: amber frame on an amber wash), `feed_row` (kill feed and
  voice rows: yours `Tint::You` with the amber bar on the outer side, inside the padding, so
  content lines up), `plate` (the glass a menu over the game sits on, with your side's
  `edge` along the top), `side_card` and `side_bar` (team pick: a card lit in its side's
  colour from the top edge down).
- **`widgets`:** `nav` (numbered list, the selection slides), `chips` / `Choice` (one
  selection per group, it slides), `chip`, `chip_w`, `stepper` (the value rolls),
  `setting_row`, `primary_button` (the one main action: amber frame, 6 px bar on the reading
  edge, key on the far edge), `primary_off` (same shape, quiet, the reason in red),
  `page_title`, `page_title_lead` (the eyebrow starts with a word in its colour, e.g. your
  side), `status_label`.
- **`motion`:** `Anim` (tune in / out, burst, slam, roll, tick) on a node and its whole
  subtree; `Slide`; `Fade` / `FadeBorder` hovers; `Blurs` for menus over the game (the game
  blurs, the shared scrim dims it, a scanline sweeps, the HUD hides; `Covered` says so).
- **`motion::live` (the HUD):** `Edge` (the light on a HUD panel's top edge in your side's
  colour, or a fixed one; `flare`, `alert` red, `beat` heartbeat), `Meter` (drops at once, a
  light trail drains after it), `Count` (numbers count), `Opens` (status icons open and close
  in place), `Side` (which side you're on; the game sets it).
- **`sound`:** `UiSound` blips made in code: the assistant's chirps (each has its own pitch),
  the timer's last seconds, a match found, status icons. Quiet, so footsteps stay clear.

## Screens
- **Menu:** numbered list left, wordmark bottom left, you top right with your running match
  (Reconnect) and invites under it, the page right. Pages swap inside a frame that stays.
  Play is a full screen of its own. A match found covers any page.
- **HUD (kept from 1.6):** radar, money (with status icons) and the voice list top left;
  score, clock and alive ticks top centre, a team vote under them; frame rate and ping, then
  the kill feed top right; health and armor bottom left; weapon and ammo bottom right; the
  assistant's uplink bottom centre (in matches only); announcements upper middle.
- **Over the game:** TX1 and TX2 text only, with a dark text shadow on bare text.

## Copy
Plain words, no em dashes. Side names are Bureau (defends) and Strays (attack); the code still
says Ct and T. Labels in caps with letter spacing.
