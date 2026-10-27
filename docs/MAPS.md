# Maps

Four competitive maps in the bootleg16 world (Bureau vs Strays). Each takes the
structure of a classic layout, so players' knowledge transfers. The setting,
story, geometry, textures, props and names are new. Valve map names stay out of
the game and code.

Map names are working names for now; final names are chosen later.

## Story
In the 2040s, governments have outsourced policing, energy and transport to the
**Bureau**, a private security conglomerate. Rent, debt and power bills have
locked a generation out.
- **Strays:** a leaderless network of young people who believe the system can't
  be reformed, only broken. They bomb the places that keep it running.
  Motto: *"Nothing left to lose."*
- **Bureau:** the system is the only thing between people and chaos, and Strays
  attacks hurt ordinary people first. Motto: *"Order is mercy."*

Strays plant, Bureau defends. Each map's two bomb sites are fictional
Bureau-owned targets inside a real place.

**Neither side is simply right.** Loading screens and voice lines show the cost
on both sides. Each character has a personal reason for being on their side. The
movement is fictional, not a real party or protest group.

## The four maps
| Map | Pillar | Real place (look only) | Layout from |
|---|---|---|---|
| **port** | Supply | Container port, Rotterdam | Train |
| **data_center** | Surveillance | Server farm, Iceland | Dust2 |
| **bank** | Money | Financial district, Canary Wharf, London | Mirage |
| **high_rise** | Housing | Luxury tower under construction, Kowloon, Hong Kong | Inferno |

### Why each layout fits
- **port ← Train:**
  - Train is parallel rows of train cars as cover, with lanes between them; container stacks in rows are the same structure.
  - Train's upper routes (Ivy, the overpass) become the crane gantries.
- **data_center ← Dust2:**
  - Long A is the main server aisle;
  - short/catwalk is a raised-floor walkway;
  - mid with its doors is a central corridor with security doors;
  - B tunnels are the cable crawlspace under the floor.
- **bank ← Mirage:**
  - Mirage's A site is an open plaza watched from ramp, palace, connector and stairs. The bank atrium has the same angles from a mezzanine and glass bridges.
  - Mid with its window is the trading floor behind glass.
  - The B apartments are office corridors leading to the vault.
- **high_rise ← Inferno:**
  - Inferno is tight and on two levels: apartments, balconies over the lanes, stairs everywhere, one narrow choke (banana). The unfinished tower has the same: bare flats, open balconies, stairwells.
  - Banana is the narrow scaffolding run.

**Only the structure is copied.** Light, weather, textures, colours and
architecture come from the real place, never from the original map.

## Rules for every map
- **Layout:**
  - same lanes, sites, chokes and rotation times as the base layout (±5 %);
  - callouts are adapted to the setting.
- **Readability first:**
  - plain mid-value walls;
  - player silhouettes must pop against everything;
  - no busy textures at head height;
  - bomb sites marked with a large painted letter plus a site-colored light.
- **Factions:** Bureau defends, Strays attack. Spawns are styled to match: Bureau vans and barriers vs Strays bikes and graffiti.
- **One hero landmark per map,** visible from spawn:
  - port: the container cranes;
  - data_center: the steam from the cooling towers;
  - bank: the glass atrium;
  - high_rise: the tower crane.

## Status
- **data_center:** grey box built and in the match pool (2026-10-27). Matches the
  reference within 5 % on every walk, sightline and door measured (below). Art pass next.
- high_rise, port, bank: not started.

## Build plan (same structure, new build)
1. **Measure:** `net/examples/mapstats.rs` reads the base BSP and prints numbers only:
   - spawns, buy zones, site boxes;
   - walking distances (the bots' navigation: stairs, drops and doors count), sightlines,
     door and corridor widths at named points;
   - `--plan`: the floor heights on a 16-unit grid, to read the layout from. It stays in a
     scratch folder; no Valve geometry or textures carry over.
2. **Greybox:** a Godot tool script lays the structure out again from our own blocks
   (`godot/tools/build_data_center.gd`: areas with a floor height or a ramp, every cell no
   area covers is wall, cover and door frames as boxes) and writes the map scene;
   `godot/tools/export_map.gd` writes its `.bmap`.
3. **Verify** against the base, each within ±5 % (`tools/compare_maps.py`):
   - walk times spawn → each site, the routes between, the rotations;
   - main sightline lengths;
   - door widths.
4. **Art pass:**
   - a modular kit per map (Quaternius sci-fi pieces, Kenney kits, Poly Haven textures), lighting;
   - renderer: fog, bloom, colour grading. Budget: 60 fps at 1080p medium on a GTX 1650.
   - Landmark pieces come from an artist or an asset store (CC0/Fab).
5. **Hand polish** in the Godot editor.

**Mirage base:** not in the stock 1.6 install. The 1.6 community `de_mirage` (by BubkeZ, 2010) was installed 2026-10-16 from the user's download and loads in the client.

**Order:** data_center (Dust2 is our test map) → high_rise → port → bank.

## data_center
The structure of Dust2, laid out in its frame (x east, y north, GoldSrc units), so the same
points measure both. Strays spawn south at the loading dock, the Bureau north in the
control room.

| Ours | Dust2 | What it is |
|---|---|---|
| Dock | T spawn | loading dock, 128 up; ramps east and down to the spine |
| Intake | outside tunnels | yard down from the dock to the crawlspace |
| Crawlspace (mouth, neck, cross, up) | upper B tunnels | cable crawlspace under the floor, roofed 144 high |
| Low crawl | lower tunnels | crawlspace branch down to the spine |
| Hall B (archive hall) | B site | |
| Archive doors | B doors | two half-open panels, slanted gap |
| Lobby | CT mid | from the spine's doors west down to B, east to the control room |
| Control room | CT spawn | under the walkway deck |
| Spine | mid | central corridor, ramps down 128 northwards; a rail 32 high to its side, climbable only at the south end |
| Security doors | mid doors | two half-open panels, slanted gap ~70 wide |
| Walkway | catwalk / short | raised-floor walkway from the spine up to A's deck |
| Yard, aisle doors | outside long, long doors | 320-wide passage |
| Main aisle, sump | long A, pit | the main server aisle |
| Hall A (core hall): deck, ramp, back | A site | |

**Changing it:** edit the layout in `godot/tools/build_data_center.gd`, then
```
godot --headless --path godot --script res://tools/build_data_center.gd
godot --headless --path godot --script res://tools/export_map.gd -- res://maps/data_center.tscn
python3 tools/compare_maps.py
```
(the map scene is generated: edits made to it in the editor are lost on the next build).

**Measured against Dust2** (`tools/compare_maps.py`, 2026-10-27; walks are bot-navigation
path lengths, seconds at 250 units/s with a knife):

| Check | Dust2 | data_center | Off |
|---|---|---|---|
| Strays spawn → A | 4749 (19.0 s) | 4661 (18.6 s) | −1.9 % |
| Strays spawn → B | 4425 (17.7 s) | 4525 (18.1 s) | +2.3 % |
| Bureau spawn → A | 1859 (7.4 s) | 1824 (7.3 s) | −1.9 % |
| Bureau spawn → B | 2367 (9.5 s) | 2363 (9.5 s) | −0.2 % |
| Strays spawn → Bureau spawn | 3939 | 3874 | −1.7 % |
| Strays → A by the aisle (long) | 5323 | 5178 | −2.7 % |
| Strays → A by the walkway (short) | 4820 | 4813 | −0.1 % |
| Strays → B by the crawlspace | 4624 | 4684 | +1.3 % |
| Strays → B by low crawl and lobby | 5473 | 5465 | −0.1 % |
| Strays → security doors | 2800 | 2727 | −2.6 % |
| Bureau → security doors | 1139 | 1147 | +0.7 % |
| Bureau → B by the lobby | 2446 | 2442 | −0.2 % |
| Strays → aisle doors | 2784 | 2819 | +1.3 % |
| Bureau → aisle doors by A | 3897 | 3741 | −4.0 % |
| Bureau rotates A → B | 3464 | 3497 | +1.0 % |
| Strays rotate aisle doors → crawlspace | 4213 | 4218 | +0.1 % |
| Strays → walkway | 2681 | 2625 | −2.1 % |
| Bureau → walkway by the doors | 2871 | 2967 | +3.3 % |
| Sight: main aisle | 1796 | 1796 | 0 % |
| Sight: spine | 1900 | 1892 | −0.4 % |
| Sight: across the security doors | 228 | 228 | 0 % |
| Sight: walkway | 616 | 600 | −2.6 % |
| Sight: crawlspace exit into B | 1740 | 1772 | +1.8 % |
| Sight: through the archive doors | 338 | 337 | −0.3 % |
| Sight: control room to the A ramp | 956 | 940 | −1.7 % |
| Width: aisle doors | 320 | 320 | 0 % |
| Width: security doors | 112 | 114 | +1.8 % |
| Width: archive doors | 110 | 109 | −0.9 % |
| Width: crawlspace | 128 | 128 | 0 % |
| Width: low crawl | 160 | 160 | 0 % |
| Width: walkway | 160 | 160 | 0 % |
| Width: main aisle | 320 | 320 | 0 % |
| Width: lobby to control room | 160 | 160 | 0 % |

**Indoors and out:** the loading dock, the intake yard, the yards outside the aisle doors
and the Strays' end of the spine are outdoors; everything else is the building. Each area's
ceiling is at Dust2's height over it (`CEILINGS` in the generator): a roof with ceiling lamps
indoors, an invisible sky outdoors that stops grenades as Dust2's sky does. Room above eye
height, Dust2 and ours alike: dock 228, main aisle 324, spine 416, A 224, B 356, control
room 164, crawlspace 164. 108 fps at 1080p on Medium on the GTX 1650 (bench, at the spawn).

**Dressing** (looks only: meshes the exporter skips, so the `.bmap` doesn't change): server
racks along the hall and aisle walls (`shaders/rack.gdshader`, 2.1 m, mid-dark, a few small
status lights), raised-floor tiles with vent tiles indoors (`raised_floor.gdshader`), cable
trays under the high roofs, cable bundles low along the crawlspace walls (`cables.gdshader`),
fan condensers along the outdoor walls (`condenser.gdshader`), and the landmark: three cooling
towers west of the dock, steaming over its wall. 119 fps at 1080p Medium at the spawn.

**Every block is something** (`BLOCKS`, `PROPS`, `prop.gdshader`): delivery crates at the
dock, a network cabinet and a UPS on the walkway, the sump's pump, boxes of servers in the
aisle, the core servers on A, a tape library and archive boxes on B, a Bureau barrier by the
control room, hazard-striped kerbs on the ledges, metal security-door panels with a status
light. Frames, headers and the crawl's column are the building. The map's boxes are drawn
as one solid (a CSG union, built in a quarter of a second at load): no two faces in the same
place, so nothing flickers.

Spawns: 10 a side (Dust2 has 20); sites the same size as Dust2's (A 256 × 320, B 384 × 384).
Not in the grey box yet: B's window from the lobby, most small cover, ladders.
