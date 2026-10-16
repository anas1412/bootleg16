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
**Not started.** We play on the CS 1.6 maps until the game itself is finished.
This is the plan for after that.

## Build plan (same structure, new build)
1. **Measure:** a tool reads the base BSP and records numbers only:
   - floor plan, floor/ceiling heights;
   - door and corridor widths, site zones, spawns.

   No Valve geometry or textures carry over.
2. **Greybox:** a generator rebuilds the structure from our own modular blocks.
   - It writes a TrenchBroom `.map`, which VHLT compiles to BSP v30.
   - The existing `goldsrc` loader, collision hulls and `sim` movement work unchanged.
3. **Verify** against the base, each within ±5 %:
   - walk times spawn → each site;
   - main sightline lengths;
   - door widths.
4. **Art pass:**
   - a modular kit per map, CC0 PBR textures overriding WAD textures by name, glTF props;
   - renderer upgrade: fog, bloom, colour grading.
   - Landmark pieces come from an artist or an asset store (CC0/Fab).
5. **Hand polish** in TrenchBroom, which opens the same `.map`.

**Tools needed then:** VHLT (build from source), plus TrenchBroom and Blender for hand work. None are installed yet.

**Mirage base:** not in the stock 1.6 install. The 1.6 community `de_mirage` (by BubkeZ, 2010) was installed 2026-10-16 from the user's download and loads in the client.

**Order:** data_center (Dust2 is our test map) → high_rise → port → bank.
