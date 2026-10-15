# Match rules: CS2 Competitive format + CS 1.6 balance

Decision (2026-10-15): the **match format** comes from CS2 Competitive; **weapons
and economy** come from CS 1.6, unchanged. CS 1.6's guns and money were balanced
together over ~20 years of competitive play; CS2's economy was tuned for CS2's
guns (per-weapon kill rewards, different prices), so mixing them would break
balance. The round format does not affect weapon balance, so it is safe to take
from CS2.

Every value below lives in a server config file. Nothing is hard-coded.

---

## 1. Where the CS 1.6 numbers come from

All CS 1.6 values were **read from the game's own server library**, not from
wikis:

- File: `Half-Life/cstrike/dlls/cs.so` (32-bit Linux build, shipped **with full
  DWARF debug info**: real type names, enums, struct layouts).
- Tools: `gdb` (types, enums, static tables, struct offsets) and `objdump -d`
  (constants inside functions).
- Facts only: numbers, names, rules. No Valve code is copied. Our code is
  written from scratch in Rust.
- Every value has a source tag so it can be re-checked:
  - `enum` = named enum in debug info
  - `table` = static data table
  - `asm @addr` = constant in the disassembly at that address
  - `cvar` = default value of a server variable
- `tools/re/fb3.py` re-extracts the weapon damage table from the disassembly.

Values tagged ⚠ were **not** confirmed from the binary yet and must be checked
before step 3 (weapons/rounds).

---

## 2. Match format (from CS2 Competitive)

| Setting | Value | Notes |
|---|---|---|
| Players | 5v5 | bots fill empty slots |
| Max rounds | 24 (MR12) | first to 13 wins |
| Halftime | after round 12 | teams swap sides; money resets to start money |
| Tie at 12–12 | overtime | MR3: 6 rounds (3 per half), first to 4; repeats until a winner |
| Overtime start money | $10,000 | CS2's default convar. Pro events use $12,500, but that was tuned for CS2 prices. Keep $10,000 with 1.6 prices, revisit with data |
| Freeze time | 15 s | CS2 matchmaking default. Premier and pro events use 20 s |
| Buy time | 20 s | starts when freeze time ends |
| Round time | 1:55 | CS2 `mp_roundtime_defuse 1.92` |
| Bomb timer | 40 s | CS2 value (CS 1.6 default was 45 s) |
| Defuse time | 10 s, 5 s with kit | ⚠ same in both games by reputation; verify in `cs.so` |
| Tick rate | 128 (64 option) | see the project plan |

For comparison, CS 1.6's own defaults (`cvar`): `mp_freezetime 6`,
`mp_buytime 1.5` (minutes), `mp_roundtime 5` (minutes), `mp_c4timer 45`,
`mp_startmoney 800`, `mp_maxrounds 0`. 1.6 leagues ran MR15. We use MR12.

---

## 3. Economy (from CS 1.6, binary-verified)

### 3.1 Limits

| | Value | Source |
|---|---|---|
| Start money | $800 | `cvar mp_startmoney` |
| Max money | $16,000 | `asm @c375a` (`CBasePlayer::AddAccount` clamps at 0x3e80) |

### 3.2 Round-win rewards (whole winning team)

| Round end | Team | Money | Source |
|---|---|---|---|
| Enemy eliminated (map has a bomb site) | winner | $3,250 | `asm @a9368` (CT), `@a988b` (T) |
| Enemy eliminated (map without a bomb site) | winner | $3,000 | `asm @a9f36`, `@a9f29` |
| Bomb exploded (`#Target_Bombed`) | T | $3,500 | `asm @a9753` |
| Bomb defused (`#Bomb_Defused`) | CT | $3,250 | `asm @a964f` |
| …and in the same round | T (losers) | +$800 planted bonus | `asm @a9656` |
| Time ran out, bomb not planted (`#Target_Saved`) | CT | $3,250 | `asm @accff` |
| All hostages rescued | CT | $2,500 | `asm @a94ed` |
| Time ran out, hostages not rescued | T | $3,250 | `asm @ad418` |
| VIP escaped | CT | $3,500 | `asm @a9d0d` |
| VIP assassinated | T | $3,250 | `asm @a9977` |
| VIP not escaped (time) | T | $3,250 | `asm @ad787` |
| Terrorists escaped | T | $3,150 | `asm @a9aaf` |

Only bomb (de_) maps matter for milestone 1. The rest are documented for later.

**There is no personal plant or defuse bonus in 1.6.** `CBasePlayer::AddAccount`
is never called from the C4 or grenade code. The $800 planted bonus goes to the
whole T team, and only when the bomb is defused.

### 3.3 Loss bonus (losing team), exact 1.6 algorithm

Source: `CHalfLifeMultiplay::RestartRound`, `asm @abdd8–abe37` and `@abe58–abe8a`;
initial value in the constructor, `asm @afa21`.

1.6 keeps **one shared** `m_iLoserBonus` plus a consecutive-loss counter per
team:

```
initial: loser_bonus = 1400, ct_losses = 0, t_losses = 0

on round end (winner W, loser L):
    if W.losses > 1:             loser_bonus = 1500   # a streak was broken
    W.losses = 0
    L.losses += 1
    if L.losses > 1 and loser_bonus <= 2999:
                                 loser_bonus += 500
    every player on L gets loser_bonus
```

Resulting payouts for a losing streak: **$1,400 → $1,900 → $2,400 → $2,900 → $3,400**
(cap). After a team breaks a streak of 2+ losses, the next loss pays $1,500.

Because the bonus is one shared number, alternating
wins and losses give amounts that depend on what the *other* team did. That is
a 1.6 quirk. Options:

- `loss_bonus = "cs16"`: exact 1.6 behaviour (faithful).
- `loss_bonus = "per_team"`: each team has its own ladder using the same 1.6
  amounts ($1,400 + $500 per loss, cap $3,400), reset on a win. This is easier
  for players to predict. CS2 also tracks this per team.

**Decided (2026-10-15): default is `per_team`.** `cs16` stays available as a
config option for purists.

### 3.4 Personal rewards and penalties

| Event | Money | Source |
|---|---|---|
| Kill an enemy | +$300, any weapon | `asm @a7431` (`PlayerKilled`) |
| Kill a teammate | −$3,300 | `asm @a7326` |
| Kill the VIP | +$2,500 | `asm @a6fdd` |
| First touch of a hostage | +$150 player, +$100 team | `asm @150bdf`, `@150be4` |
| Rescue a hostage | +$1,000 | `asm @152b3c` |
| Hurt a hostage | −(20 × value) | `asm @151749` ⚠ exact input not traced |
| Kill a hostage | −(20 × value) − $500 | `asm @15192f` ⚠ |

Unlike CS2, 1.6 pays a flat $300 per kill for every weapon. There are no SMG,
shotgun or AWP kill-reward differences.

⚠ The rule that living T players get no money when time runs out on a bomb map
(`MarkLivingPlayersOnTeamAsNotReceivingMoneyNextRound`) exists in the binary,
but its call sites were inlined and are not traced yet.

---

## 4. Prices (from CS 1.6, `enum` in debug info)

### Weapons (`WeaponCostType`)

| Pistols | $ | SMGs | $ | Rifles | $ | Snipers | $ | Heavy | $ |
|---|---|---|---|---|---|---|---|---|---|
| Glock-18 | 400 | TMP (CT) | 1,250 | Galil (T) | 2,000 | Scout | 2,750 | M3 | 1,700 |
| USP | 500 | MAC-10 (T) | 1,400 | FAMAS (CT) | 2,250 | AWP | 4,750 | XM1014 | 3,000 |
| P228 | 600 | MP5 | 1,500 | AK-47 (T) | 2,500 | SG550 (CT) | 4,200 | M249 | 5,750 |
| Deagle | 650 | UMP45 | 1,700 | M4A1 (CT) | 3,100 | G3SG1 (T) | 5,000 | | |
| Five-SeveN (CT) | 750 | P90 | 2,350 | SG552 (T) | 3,500 | | | | |
| Dual Elites (T) | 800 | | | AUG (CT) | 3,500 | | | | |

Tactical shield (CT): $2,200. ⚠ Team restrictions in the table follow the
well-known 1.6 buy menus; verify against `CanBuyWeaponByMaptype`.

### Equipment (`ItemCostType`)

| Item | $ |
|---|---|
| Kevlar | 650 |
| Kevlar + helmet | 1,000 (helmet alone: 350) |
| Flashbang | 200 |
| HE grenade | 300 |
| Smoke grenade | 300 |
| Defuse kit (CT) | 200 |
| Night vision | 1,250 |

### Ammo per clip (`AmmoCostType`)

| Ammo | $ | Ammo | $ |
|---|---|---|---|
| 9mm | 20 | 5.56mm | 60 |
| .45 ACP | 25 | 7.62mm | 80 |
| .357 SIG | 50 | .338 Magnum (AWP) | 125 |
| 5.7mm | 50 | Buckshot | 65 |
| .50 AE | 40 | | |

---

## 5. Weapon stats (from CS 1.6, `asm`, arguments to `FireBullets3`)

`range mod` = damage multiplier per 500 units of distance. Penetration = how
many surfaces a bullet can pass through. Max distance is 8192 units unless
noted.

| Weapon | Damage | Range mod | Penetration | Move speed |
|---|---|---|---|---|
| AK-47 | 36 | 0.98 | 2 | 221 |
| M4A1 (unsilenced) | 32 | 0.97 | 2 | 230 |
| M4A1 (silenced) | 33 | 0.95 | 2 | 230 |
| Galil | 30 | 0.98 | 2 | 240 |
| FAMAS | 34 | 0.96 | 2 | 240 |
| SG552 | 33 | 0.955 | 2 | 235 |
| AUG | 32 | 0.96 | 2 | 240 |
| AWP | 115 | 0.99 | 3 | 210 (150 scoped) |
| Scout | 75 | 0.98 | 3 | 260 (220 scoped) |
| SG550 | 70 | 0.98 | 2 | 210 (150 scoped) |
| G3SG1 | 80 | 0.98 | 3 | 210 (150 scoped) |
| M249 | 32 | 0.97 | 2 | 220 |
| MP5 | 26 | 0.84 | 1 | 250 |
| UMP45 | 30 | 0.82 | 1 | 250 |
| P90 | 21 | 0.885 | 1 | 245 |
| MAC-10 | 29 | 0.82 | 1 | 250 |
| TMP | 20 | 0.85 | 1 | 250 |
| Glock-18 | 25 | 0.75 | 1 | 250 ⚠ |
| USP | 30 | 0.79 | 1 (max dist 4096) | 250 ⚠ |
| P228 | 32 | 0.80 | 1 (max dist 4096) | 250 ⚠ |
| Deagle | 54 | 0.81 | 2 (max dist 4096) | 250 |
| Five-SeveN | 20 | 0.885 | 1 (max dist 4096) | 250 ⚠ |
| Dual Elites | 36 | 0.75 | 1 | 250 |
| M3 / XM1014 | ⚠ shotguns use a different fire function | | | 230 / 240 |
| Knife, C4 | | | | 250 ⚠ / 250 |

⚠ The M4A1 rows come from the two call sites in `M4A1Fire`. Which call is the
silenced branch still needs confirming. Pistol, knife and grenade speeds are
set in each weapon's `Spawn` (the Deagle's 250 is confirmed); the others are
assumed 250 until checked.

Still to extract before step 3: fire rates, spread/inaccuracy formulas, recoil
(`KickBack` parameters), reload and deploy times, shotgun pellets, grenade
damage and radius, hit-group multipliers, armor ratios.

---

## 6. Keeping it fair over time

- All values above go in a server config file, so tuning needs no rebuild.
- The server logs every match: buys, kills, round results and money per round.
  Balance changes are made from real data, not guesses.
- Any change from 1.6 values is listed in a "changes from 1.6" section here, so
  players know exactly what differs.

---

## Sources

- CS 1.6 server library `cstrike/dlls/cs.so` (local install), via `gdb` and `objdump`.
- CS2 round format: [Counter-Strike Wiki – Premier](https://counterstrike.fandom.com/wiki/Premier),
  [ESL Pro Tour 2023 CS2 rule changes](https://pro.eslgaming.com/tour/2023/10/esl-pro-tour-fall-2023-rule-book-update/),
  [EPIC.LAN CS2 rules](https://tournaments.epiclan.co.uk/tournaments/epic42-cs/rules),
  [csdb.gg – mp_roundtime_defuse](https://csdb.gg/command/mp-roundtime-defuse/),
  [TotalCS – mp_freezetime](https://totalcsgo.com/commands/mpfreezetime),
  [TotalCS – mp_overtime_startmoney](https://totalcsgo.com/commands/mpovertimestartmoney).
