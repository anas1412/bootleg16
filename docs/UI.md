# UI design system

Mockups: `design/mockups/d.html` (serve `design/` and open `?s=menu|hud|buy|score|system`).
Tokens live in `design/mockups/system.css`. The Bevy UI must use the same values.

## Audit of the first D pass, and the fix for each issue
| # | Problem | Fix |
|---|---------|-----|
| 1 | Palette read as Valorant: cream text plus red-orange on black. | Olive-graphite base (1.6 VGUI heritage), cool off-white text, one amber accent (1.6 HUD). |
| 2 | Two HUD languages: 1.6 bitmap digits at the bottom, flat panels at the top. | One numeric style everywhere: JetBrains Mono, tabular. |
| 3 | Edge margins varied (24/28/34/40 px). | 32 px safe margin on every edge at 1080p. |
| 4 | Proportional digits in Space Grotesk made numbers jump width (100 to 99). | Every number is mono and tabular, no exceptions. |
| 5 | Three corner treatments (double 18 px cut, 10 px cut, square). | One signature: a single 12 px notch, top-right, on panels and buttons. |
| 6 | Accent overused: kill icons, chat, buyzone, money, CTA, selection. | Accent only for: you, selected/focus, primary action. |
| 7 | Menu bloat: meta header, registration marks, footer hints, hairline grid. | Removed. Wordmark, nav, profile chip only. |
| 8 | "Featured de_dust" card on the menu was out of place. | Removed. Play leads to its own flow. |
| 9 | Unaffordable buy items used low-opacity red text, which was unreadable. | The card stays solid; gun and name drop to tertiary; price stays in `bad` at full opacity. |
| 10 | Selected buy card used a cream fill (Valorant tell). | Raised surface plus a 2 px accent outline. |
| 11 | Chat and labels had low contrast on bright sand. | Bottom scrim plus a text shadow token; labels use `tx-2`, never `tx-3`, over the game. |
| 12 | Timer at the bottom and score at the top split the round state. | Round state lives together at top centre: score, timer, round, alive. |

## Tokens
- **Surfaces:** `bg-0 #0B0D0B` (page), `bg-1 #121511` (panel), `bg-2 #1A1E18` (raised), `bg-3 #242A21` (hover/selected). `hud rgba(11,13,11,.78)` for panels over the game.
- **Lines:** `line rgba(233,238,228,.10)`, `line-2 rgba(233,238,228,.22)`.
- **Text:** `tx-1 #EEF0EA`, `tx-2 #A3AA9C`, `tx-3 #6A7164`.
- **Accent:** `acc #FFA000`, with `acc-ink #1A1200` for text on accent.
- **Teams:** `ct #8CBEF0`, `t #E86A4A`.
- **States:** `ok #7CCB6A` (money, online), `bad #FF5252` (low HP, cannot afford).
- **Type:**
  - Space Grotesk for words: display 128, h2 44, h3 28, body 18, all 700/500.
  - JetBrains Mono for numbers and labels: num-l 56, num-m 32, num-s 18, label 13 caps with +0.14 em tracking.
- **Space:** 4 px grid; component gaps of 8, 16 and 24; 32 px HUD safe margin.
- **Shape:** notch 12 px top-right; 1 px `line` stroke; no radius, no blur.
- **HUD placement (kept from 1.6):**
  - radar and money top-left;
  - round state top-centre;
  - kill feed top-right;
  - HP and armor bottom-left;
  - ammo bottom-right.
