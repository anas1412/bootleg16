//! A bullet that gets through a wall hurts someone the server never sent to the shooter.
//! On our own test arena.
mod common;
use common::*;
use net::protocol::Team;
use sim::collision::Contents;
use sim::pmove::Input;
use sim::weapons::bullets::{fire_bullet, view_vectors, Target};

#[test]
fn a_shot_through_a_wall_hurts_someone_who_was_never_sent() {
    let bsp = arena_map();
    let mut g = setup(&bsp, 0, 0.0);
    g.server.set_give(vec![sim::weapons::Id::Ak47]);
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let world = bsp.world();
    let assets = assets();
    let (origin, eye) = (g.server.player(a).unwrap().origin, g.server.player(a).unwrap().eye());
    let ak = g.server.player_state(a).unwrap().loadout.active_gun().map(|_| sim::weapons::Id::Ak47).unwrap();
    // Where A stands relative to the floor, then search the map for a place with a wall in
    // front that this very bullet gets through to a spot where someone could stand.
    let floor = |p: [f32; 3]| world.trace_line(p, [p[0], p[1], p[2] - 200.0]).end[2];
    let (stand, eye_up) = (origin[2] - floor(origin), eye[2] - origin[2]);
    let (mut tried, mut walls, mut stands, mut gridok) = (0, 0, 0, 0);
    let mut found = None;
    'search: for gx in (-2400..2400).step_by(96) {
        for gy in (-2400..2400).step_by(96) {
            for gz in (-700..500).step_by(48) {
            let top = [gx as f32, gy as f32, gz as f32];
            if world.point_contents(top) != Contents::Empty {
                continue;
            }
            let down = world.trace_line(top, [top[0], top[1], top[2] - 60.0]);
            if down.fraction >= 1.0 || down.start_solid {
                continue;
            }
            gridok += 1;
            let me = [top[0], top[1], down.end[2] + stand];
            let eye = [me[0], me[1], me[2] + eye_up];
            if [0.0, 20.0, -30.0].iter().any(|dz| world.point_contents([me[0], me[1], me[2] + dz]) != Contents::Empty) {
                continue;
            }
            stands += 1;
            for yaw in (0..360).step_by(10) {
                let (f, _, _) = view_vectors(0.0, yaw as f32);
                let wall = world.trace_line(eye, std::array::from_fn(|i| eye[i] + f[i] * 600.0));
                if wall.fraction >= 1.0 || wall.start_solid {
                    continue;
                }
                for k in (24..100).step_by(8) {
                    let at: [f32; 3] = std::array::from_fn(|i| wall.end[i] + f[i] * k as f32);
                    let spot = [at[0], at[1], floor([at[0], at[1], eye[2]]) + stand];
                    if (spot[2] - me[2]).abs() > 8.0 || [-30.0, 0.0, 30.0, 50.0].iter().any(|dz| world.point_contents([spot[0], spot[1], spot[2] + dz]) != Contents::Empty) {
                        continue;
                    }
                    let d = [spot[0] - eye[0], spot[1] - eye[1]];
                    let aim = d[1].atan2(d[0]).to_degrees();
                    let target = Target { id: b, origin: spot, yaw: 0.0, pitch: 0.0, crouched: false, weapon: ak, anim: None };
                    tried += 1;
                    let r = fire_bullet(&world, &assets.materials, &assets.hitboxes, eye, &ak_shot(ak, aim), a, &[target]);
                    walls += !r.impacts.is_empty() as u32;
                    // hidden the way the server judges it: no part of his box, grown by its margin, in sight
                    let mut pts = vec![spot, [spot[0], spot[1], spot[2] + 32.0]];
                    for x in [-40.0, 40.0] {
                        for y in [-40.0, 40.0] {
                            for z in [-32.0, 36.0] {
                                pts.push([spot[0] + x, spot[1] + y, spot[2] + z]);
                            }
                        }
                    }
                    let clear = pts.iter().any(|p| !world.sight_blocked(eye, *p));
                    if !clear && r.hits.iter().any(|h| h.victim == b) {
                        found = Some((me, spot, aim));
                        break 'search;
                    }
                }
            }
            }
        }
    }
    let Some((me, spot, yaw)) = found else { panic!("no wall on the map that a bullet gets through to a standing spot ({gridok} floors, {stands} stand spots, {tried} tried, {walls} with impacts; stand {stand} eye_up {eye_up})") };
    g.server.teleport(a, me);
    let eye = [me[0], me[1], me[2] + eye_up];
    let health = g.server.vitals(b).unwrap().health;
    for _ in 0..128 {
        g.server.teleport(b, spot);
        g.tick(Input { yaw, pitch: 0.0, ..Default::default() }, Input::default());
    }
    assert!(world.trace_line(eye, [spot[0], spot[1], eye[2]]).fraction < 1.0, "a wall is between them");
    assert!(!g.a.others().iter().any(|o| o.id == b), "setup: he was sent to A");
    for _ in 0..64 {
        g.server.teleport(b, spot);
        g.tick(Input { yaw, pitch: 0.0, attack: true, ..Default::default() }, Input::default());
    }
    let now = g.server.vitals(b).unwrap();
    assert!(now.health < health, "no damage through the wall: {health} -> {}", now.health);
}

/// A perfectly accurate AK bullet (its numbers from the weapon table).
fn ak_shot(weapon: sim::weapons::Id, yaw: f32) -> sim::weapons::Shot {
    sim::weapons::Shot { weapon, damage: 36, range_mod: 0.98, penetration: 2, distance: 8192.0, bullet: sim::weapons::data::B762, spread: 0.0, angles: [0.0, yaw], seed: 0, pellets: None, silenced: false }
}
