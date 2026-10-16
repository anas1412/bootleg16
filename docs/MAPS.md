# Maps

The five competitive maps are rebuilt in the bootleg16 world (Bureau vs Strays).
Each keeps its classic layout and timings, so players' knowledge transfers. The
geometry, textures, props and names are new. Valve map names stay out of the
game and code.

| New map | Layout of | Setting | Light |
|---|---|---|---|
| **Kiln** キルン | Dust2 | Desert solar-furnace town: sandstone blocks under mirror arrays, cable runs, water tanks | Hard noon sun, white-hot sky |
| **Oasis** オアシス | Mirage | Market city on a dry lake: canopies, holo shop signs, the "palace" is a closed luxury hotel | Late afternoon, long shadows |
| **Lantern** ランタン | Inferno | Hillside old town, Jiufen style: stairs, paper lanterns, tea houses; banana is a noodle alley | Dusk, lanterns on |
| **Maglev** マグレブ | Train | Maglev freight depot: container trains on rails, gantry cranes, rain | Night, sodium and LED |
| **Fusion** フュージョン | Nuke | Bureau fusion plant: clean white halls, reactor hall below, vents | Overcast, cold interior |

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
- **One hero landmark per map,** visible from spawn: Kiln's mirror tower, Oasis's hotel dome, Lantern's bell tower, Maglev's crane, Fusion's cooling stack.

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

**Mirage/Oasis:** not in the stock 1.6 install. The 1.6 community `de_mirage` (by BubkeZ, 2010) was installed 2026-10-16 from the user's download and loads in the client.

**Order:** Kiln (Dust2 is our test map) → Lantern → Maglev → Fusion → Oasis.
