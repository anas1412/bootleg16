//! Our own test map (`godot/maps/test_arena.tscn`, exported to `.bmap` by
//! `godot/tools/export_map.gd`): the simulation plays on what the Godot exporter writes.
use sim::collision::{Contents, Hull, World};
use sim::mapfile::MapFile;
use sim::pmove::{step, Input, MoveVars, Player};
use sim::zones::Side;

const DT: f32 = 1.0 / 128.0;

fn arena() -> (MapFile, World) {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../godot/maps/test_arena.bmap");
    let (map, _) = MapFile::load(std::path::Path::new(path)).expect("export it: godot --headless --path godot --script res://tools/export_map.gd -- res://maps/test_arena.tscn");
    let w = map.world();
    (map, w)
}

fn run(p: &mut Player, input: Input, secs: f32, w: &World) {
    for _ in 0..(secs / DT) as usize {
        step(p, &input, DT, w, &MoveVars::default());
    }
}

/// Godot metres (x, y up, z) as GoldSrc units.
fn at(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x / 0.0254, -z / 0.0254, y / 0.0254]
}

#[test]
fn you_spawn_onto_the_floor_and_run() {
    let (map, w) = arena();
    let spawn = map.spawns.iter().find(|s| s.side == sim::mapfile::SideDef::T).unwrap();
    let mut p = Player::new(spawn.origin);
    run(&mut p, Input::default(), 1.0, &w);
    assert!(p.on_ground && (p.origin[2] - 36.0).abs() < 0.1, "{:?}", p.origin);
    // east along the open south side (north is the door, which needs the doors running)
    run(&mut p, Input { forward: 400.0, yaw: 0.0, ..Default::default() }, 2.0, &w);
    assert!((p.speed_2d() - 250.0).abs() < 2.0 && p.origin[0] > spawn.origin[0] + 300.0, "{:?}", p.origin);
}

#[test]
fn the_ramp_is_walked_up() {
    let (_, w) = arena();
    // at the foot of the ramp (Godot x -14, its low end towards -z), walking up it towards +z
    let mut p = Player::new(at(-14.0, 1.0, 6.0));
    run(&mut p, Input::default(), 0.5, &w);
    let z0 = p.origin[2];
    run(&mut p, Input { forward: 400.0, yaw: -90.0, ..Default::default() }, 1.0, &w);
    assert!(p.origin[2] > z0 + 30.0, "didn't climb: {z0} -> {:?}", p.origin);
}

#[test]
fn glass_stops_bullets_not_sight_and_the_zones_are_where_they_were_placed() {
    let (map, w) = arena();
    // the glass panel at Godot (12, 1.25, 6)
    let (a, b) = (at(12.0, 1.2, 8.0), at(12.0, 1.2, 4.0));
    let t = w.trace_line(a, b);
    assert!(t.fraction < 1.0 && !w.sight_blocked(a, b), "{t:?}");
    // a concrete wall blocks both
    let (a, b) = (at(0.0, 1.0, 19.0), at(0.0, 1.0, 22.0));
    assert!(w.trace_line(a, b).fraction < 1.0 && w.sight_blocked(a, b));
    assert_eq!(w.point_contents(at(0.0, -0.5, 0.0)), Contents::Solid);
    // bomb site A is on the platform, B by the east wall; buying at your spawn
    assert!(w.zones.in_bomb_site(at(7.0, 2.0 + 0.92, -10.4), Hull::Standing));
    assert_eq!(w.zones.sites_a_first(&map.name).len(), 2);
    assert!(w.zones.in_buy_zone(Side::T, at(0.0, 0.92, 16.0), Hull::Standing));
    assert!(!w.zones.in_buy_zone(Side::Ct, at(0.0, 0.92, 16.0), Hull::Standing));
    assert!(w.ladder_at(Hull::Standing, at(9.8, 0.92, -10.4)).is_some());
}

