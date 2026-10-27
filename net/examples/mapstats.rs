//! A map's numbers (no geometry): bounds, spawns, buy zones, bomb sites, walking distances
//! from each spawn to each site (bot navigation, so stairs and drops count) and through named
//! routes, and how far you see down named sightlines. Used to measure a reference layout and
//! check our own map against it (docs/MAPS.md).
//!
//! Usage: mapstats <map.bsp | map.bmap> [route...] [--plan out.txt]
//! - route: `name=x,y,z/x,y,z/...` walks through those points in order (the first and last can
//!   be `t`, `ct`, `a` or `b`: a spawn or a site); `name~x,y,z/x,y,z` is a sightline (eye
//!   height added to both ends), its length to the first thing in the way;
//!   `name^x,y,z^deg` is a width (a door, a corridor): wall to wall at waist height along the
//!   direction `deg` (0 = along x), the narrowest within 32 units of that point.
//! - --plan: the floor heights on a 16-unit grid as text, to read the layout from (stays in a
//!   scratch folder: it is the reference's floor plan).
use net::bot::BotMap;
use sim::collision::{Hull, World};
use sim::zones::Side;
use std::collections::HashSet;

/// Running with a knife (CS 1.6 `CKnife::GetMaxSpeed`), units a second.
const RUN: f32 = 250.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: mapstats <map.bsp | map.bmap> [route...] [--plan out.txt]");
    let name = std::path::Path::new(path).file_stem().unwrap().to_string_lossy().into_owned();
    let map = if path.ends_with(".bmap") {
        let (m, h) = sim::mapfile::MapFile::load(std::path::Path::new(path)).unwrap();
        BotMap::from_map(m, h)
    } else {
        BotMap::new(goldsrc::bsp::Bsp::load(path).unwrap(), &name)
    };
    let world = map.world();
    let (lo, hi) = bounds(&map);
    println!("map {name}: {:.0} x {:.0} x {:.0} units ({lo:.0?} .. {hi:.0?})", hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]);

    let spawn_centre = |side| centroid(&map.zones.spawns(side));
    for (label, side) in [("t", Side::T), ("ct", Side::Ct)] {
        let s = map.zones.spawns(side);
        println!("spawns {label}: {} around {:.0?}", s.len(), centroid(&s));
    }
    for (i, (l, h)) in map.zones.bomb_sites.iter().enumerate() {
        println!("site {i}: {:.0} x {:.0} at {:.0?} .. {:.0?}", h[0] - l[0], h[1] - l[1], l, h);
    }
    println!("sites A first: {:.0?}", map.sites);

    let point = |s: &str| -> [f32; 3] {
        match s {
            "t" => spawn_centre(Side::T),
            "ct" => spawn_centre(Side::Ct),
            "a" => map.sites[0],
            "b" => map.sites[1],
            _ => {
                let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().expect("x,y,z")).collect();
                [v[0], v[1], v[2]]
            }
        }
    };
    let walk = |points: &[[f32; 3]]| -> Option<f32> {
        let mut total = 0.0;
        for w in points.windows(2) {
            total += walk(&map, w[0], w[1])?;
        }
        Some(total)
    };
    println!("walks (nav path, seconds at {RUN} u/s with a knife):");
    for (from, to) in [("t", "a"), ("t", "b"), ("ct", "a"), ("ct", "b"), ("t", "ct")] {
        report(&format!("{from} -> {to}"), walk(&[point(from), point(to)]));
    }
    let mut plan = None;
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if a == "--plan" {
            plan = args.get(i + 1).cloned();
            i += 2;
            continue;
        }
        if let Some((label, pts)) = a.split_once('=') {
            let pts: Vec<[f32; 3]> = pts.split('/').map(point).collect();
            report(label, walk(&pts));
        } else if let Some((label, pts)) = a.split_once('~') {
            let pts: Vec<[f32; 3]> = pts.split('/').map(point).collect();
            let eye = |p: [f32; 3]| [p[0], p[1], p[2] + 28.0];
            let (from, to) = (eye(pts[0]), eye(pts[1]));
            let t = world.trace_line(from, to);
            println!("  sight {label}: {:.0} units (of {:.0})", t.fraction * dist(from, to), dist(from, to));
        } else if let Some((label, rest)) = a.split_once('^') {
            let (p, deg) = rest.split_once('^').expect("name^x,y,z^deg");
            let p = point(p);
            let (s, c) = deg.parse::<f32>().expect("degrees").to_radians().sin_cos();
            // the narrowest within 32 units either side along the way through (a door's leaf)
            let across = |at: [f32; 3]| -> f32 {
                let reach = |sign: f32| world.trace_line(at, [at[0] + sign * c * 4096.0, at[1] + sign * s * 4096.0, at[2]]).fraction * 4096.0;
                reach(1.0) + reach(-1.0)
            };
            let narrowest = (-4..=4).map(|k| across([p[0] - s * k as f32 * 8.0, p[1] + c * k as f32 * 8.0, p[2]])).fold(f32::MAX, f32::min);
            println!("  width {label}: {narrowest:.0} units");
        }
        i += 1;
    }
    if let Some(out) = plan {
        floor_plan(&world, lo, hi, &out);
    }
}

fn report(label: &str, d: Option<f32>) {
    match d {
        Some(d) => println!("  {label}: {d:.0} units, {:.2} s", d / RUN),
        None => println!("  {label}: no way through"),
    }
}

/// The length of the bots' path from one point to another (None: no way).
fn walk(map: &BotMap, from: [f32; 3], to: [f32; 3]) -> Option<f32> {
    let (a, b) = (map.nav.nearest(from)?, map.nav.nearest(to)?);
    let path = map.nav.path(a, b, &HashSet::new());
    if path.is_empty() {
        return None;
    }
    let p = &map.nav.points;
    Some(path.windows(2).map(|w| dist(p[w[0] as usize], p[w[1] as usize])).sum())
}

fn bounds(map: &BotMap) -> ([f32; 3], [f32; 3]) {
    match &map.source {
        net::bot::Source::Bsp(b) => (b.models[0].mins, b.models[0].maxs),
        net::bot::Source::Map(m, _) => m.bounds(),
    }
}

fn centroid(ps: &[[f32; 3]]) -> [f32; 3] {
    let n = ps.len().max(1) as f32;
    let mut c = [0.0; 3];
    for p in ps {
        for i in 0..3 {
            c[i] += p[i] / n;
        }
    }
    c
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The floors a player stands on at each spot (16 units apart): a text grid, first line
/// `x0 y0 step columns rows`, then one row per line from the lowest y, spots separated by
/// spaces, each its floor heights from the top (`z/z`; `-` where nobody stands).
fn floor_plan(world: &World, lo: [f32; 3], hi: [f32; 3], out: &str) {
    let step = 16.0f32;
    let (w, h) = (((hi[0] - lo[0]) / step) as usize + 1, ((hi[1] - lo[1]) / step) as usize + 1);
    let mut text = format!("{} {} {step} {w} {h}\n", lo[0], lo[1]);
    for r in 0..h {
        let row: Vec<String> = (0..w)
            .map(|c| {
                let (x, y) = (lo[0] + c as f32 * step, lo[1] + r as f32 * step);
                // as the bots' navigation: each stretch of open space from the top, down to
                // its floor
                let mut floors = Vec::new();
                let mut z = hi[2] - 40.0;
                while z > lo[2] + 40.0 {
                    if world.is_solid(Hull::Standing, [x, y, z]) {
                        z -= 16.0;
                        continue;
                    }
                    let t = world.trace(Hull::Standing, [x, y, z], [x, y, lo[2]]);
                    if t.start_solid {
                        z -= 16.0;
                        continue;
                    }
                    if t.fraction >= 1.0 {
                        break;
                    }
                    if t.normal[2] > 0.7 {
                        floors.push(format!("{:.0}", t.end[2] - 36.0));
                    }
                    z = t.end[2] - 80.0;
                }
                if floors.is_empty() { "-".into() } else { floors.join("/") }
            })
            .collect();
        text += &row.join(" ");
        text += "\n";
    }
    std::fs::write(out, text).unwrap();
    println!("plan: {out} ({w} x {h}, {step} units apart)");
}
