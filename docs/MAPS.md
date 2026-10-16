# Maps

The five competitive maps are rebuilt in the bootleg16 world (Bureau vs Strays).
Each keeps its classic layout and timings, so players' knowledge transfers. The
geometry, textures, props and names are new. Valve map names stay out of the
game and code.

## Story (2041)
The **Bureau** is a private security agency in suits that guards the world
energy grid. The **Strays** are a street crew of runaways and hackers who found
"Keystone": proof the Bureau causes blackouts to raise power prices. Each map is
one chapter. Strays attack (plant), Bureau defends.

| Ch. | Layout of | Map | Real place | Story beat |
|---|---|---|---|---|
| 1 | Dust2 | **Noor** | Noor solar plant, Ouarzazate, Morocco (desert kasbah town) | Strays hit the plant where the blackouts start and steal the first Keystone logs |
| 2 | Mirage | **Deira** | Old souk district, Dubai (gold souk, wind towers, skyline behind) | The money trail: the Bureau launders it through the souk; the palace is a Bureau-owned hotel |
| 3 | Inferno | **Jiufen** | Hillside old town, Taiwan (lanterns, stairs, tea houses) | Bureau raids the Strays' hacker safehouse; Banana is the market alley, the bell tower a temple bell |
| 4 | Train | **Shinagawa** | Shinagawa freight yard, Tokyo (future maglev terminal), night and rain | Bureau moves the decrypted core by maglev freight; Strays ambush the yard |
| 5 | Nuke | **Cadarache** | ITER fusion site, southern France | Finale: Strays break into the Bureau's showcase reactor to broadcast Keystone |

Each map borrows the real place's light, architecture, signage and language. The
layout stays the original's.

## Rules for every map
- **Layout:**
  - same lanes, sites, chokes and rotation times as the original (±5 %);
  - callouts are kept or close (Long, Short, Mid, B tunnels, Banana, Ramp...).
- **Readability first:**
  - plain mid-value walls;
  - player silhouettes must pop against everything;
  - no busy textures at head height;
  - bomb sites marked with a large painted letter plus a site-colored light.
- **Factions:** Bureau defends (sites are Bureau assets: arrays, reclaimers, reactor), and Strays attack. Spawns are styled to match: Bureau vans and barriers vs Strays bikes and graffiti.
- **One hero landmark per map,** visible from spawn:
  - Noor's solar tower;
  - Deira's hotel dome;
  - Jiufen's temple bell;
  - Shinagawa's maglev gantry;
  - Cadarache's tokamak hall.

## Status
**Not started.** We play on the CS 1.6 maps until the game itself is finished.
This remodel is the plan for after that.

## Remodel plan (same layout, new build)
1. **Measure:** a tool reads the original BSP and records numbers only:
   - floor plan, floor/ceiling heights;
   - door and corridor widths, site zones, spawns.

   No Valve geometry or textures carry over.
2. **Greybox:** a generator rebuilds the layout from our own modular blocks (walls, floors, stairs, arches, crates).
   - It writes a TrenchBroom `.map`, which VHLT compiles to BSP v30.
   - The existing `goldsrc` loader, collision hulls and `sim` movement work unchanged.
3. **Verify** against the original, each within ±5 %:
   - walk times spawn → each site;
   - main sightline lengths;
   - door widths.
4. **Art pass:**
   - a modular kit per map, CC0 PBR textures overriding WAD textures by name, glTF props;
   - renderer upgrade: fog, bloom, colour grading.
   - Landmark pieces come from an artist or an asset store (CC0/Fab).
5. **Hand polish** in TrenchBroom, which opens the same `.map`.

**Tools needed then:** VHLT (build from source), plus TrenchBroom and Blender for hand work. None are installed yet.

**Mirage/Deira:** not in the stock 1.6 install. The 1.6 community `de_mirage` (by BubkeZ, 2010) was installed 2026-10-16 from the user's download and loads in the client.

**Order:** Noor (Dust2 is our test map) → Jiufen → Shinagawa → Cadarache → Deira.
