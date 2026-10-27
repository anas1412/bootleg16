#!/usr/bin/env python3
"""Checks our data_center against the layout it takes its structure from (docs/MAPS.md): the
same walks, sightlines and door widths measured on both with net/examples/mapstats.rs, and
how far ours is off (the rule: within 5 %). Numbers only; the reference's geometry stays where
it is. data_center keeps the reference's frame, so the same points work on both.

    systemd-run --user --scope -q -p MemoryMax=3G -- cargo build --release -p net --example mapstats -j 2
    python3 tools/compare_maps.py [reference.bsp] [ours.bmap]
Exit status 1 if anything is off by more than 5 %.
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOL = ROOT / "target/release/examples/mapstats"
REFERENCE = sys.argv[1] if len(sys.argv) > 1 else str(ROOT / "gamedata/maps/de_dust2.bsp")
OURS = sys.argv[2] if len(sys.argv) > 2 else str(ROOT / "godot/maps/data_center.bmap")

# Walks go through the points in order (t / ct: a side's spawns, a / b: a site); sightlines
# (~) are an eye-height line from the first point, as far as nothing is in the way; widths (^)
# are wall to wall at waist height along that angle, the narrowest near the point. Names are
# our callouts.
CHECKS = [
    "Strays to A, by the aisle=t/600,500,40/850,900,40/1400,1500,40/a",
    "Strays to A, by the walkway=t/-380,800,40/100,1450,56/448,2016,136/a",
    "Strays to B, by the crawlspace=t/-1900,300,40/-1728,900,40/b",
    "Strays to B, low crawl and lobby=t/-800,1400,-72/-1300,2208,40/b",
    "Strays to the security doors=t/-384,1632,-88",
    "Bureau to the security doors=ct/-384,1632,-88",
    "Bureau to B, by the lobby=ct/-600,2300,-88/-1300,2208,40/b",
    "Strays to the aisle doors=t/850,900,40",
    "Bureau to the aisle doors, by A=ct/a/850,900,40",
    "Bureau rotates A to B=a/b",
    "Strays rotate aisle doors to crawlspace=850,900,40/-1728,900,40",
    "Strays to the walkway=t/-200,1488,56",
    "Bureau to the walkway, by the doors=ct/-384,1632,-88/-200,1488,56",
    "main aisle (long)~1440,780,40/1440,3200,40",
    "spine (mid)~-400,-300,40/-400,2600,40",
    "across the security doors~-480,1700,-88/-300,1560,-88",
    "walkway (cat)~-200,1488,56/700,1488,56",
    "crawlspace exit into B~-1984,1300,40/-1984,3100,40",
    "through the archive doors (B)~-1000,2208,0/-2200,2208,0",
    "control room to the A ramp~1500,2240,40/0,2240,40",
    "aisle doors^850,900,40^90",
    "security doors (mid)^-384,1632,-88^0",
    "archive doors (B)^-1310,2208,40^90",
    "crawlspace neck^-1728,900,40^0",
    "crawlspace up to B^-1984,1500,40^0",
    "low crawl^-800,1400,-72^90",
    "walkway^200,1488,56^90",
    "main aisle^1440,1600,40^0",
    "lobby to control room^-100,2150,-88^90",
]


def measure(map_path):
    out = subprocess.run([str(TOOL), map_path, *CHECKS], capture_output=True, text=True, check=True).stdout
    found = {}
    for line in out.splitlines():
        m = re.match(r"\s+(?:sight |width )?(.+?): (\d+) units", line)
        if m:
            found[m.group(1)] = int(m.group(2))
    return found


def main():
    ref, ours = measure(REFERENCE), measure(OURS)
    off = 0
    print(f"{'':44} {'reference':>9} {'ours':>9}")
    for name in ref:
        a, b = ref[name], ours.get(name)
        if b is None:
            print(f"{name:44} {a:9} {'?':>9}  missing")
            off += 1
            continue
        d = (b / a - 1) * 100 if a else 0
        bad = abs(d) > 5
        off += bad
        print(f"{name:44} {a:9} {b:9}  {d:+5.1f} %{'  <<' if bad else ''}")
    print(f"{'all within 5 %' if off == 0 else f'{off} off by more than 5 %'} (walks: nav path units, 250 a second with a knife; sightlines and widths: units)")
    sys.exit(1 if off else 0)


main()
