//! Breakables over localhost UDP: nuke's glass breaks when shot (and stops blocking),
//! dust2's crates break when the bomb goes off at their site, all come back next round.
mod common;
use common::*;
use net::protocol::{GameEvent, Team};
use sim::pmove::Input;

#[test]
fn shooting_nukes_glass_breaks_it_for_everyone() {
    let Some(bsp) = map("de_nuke") else { return };
    let glass = sim::breakables::from_bsp(&bsp).into_iter().find(|b| b.material == sim::breakables::Material::Glass).expect("nuke has glass");
    let m = bsp.models[glass.model];
    // The thin axis is the glass's facing: stand 120 units off it, eyes level with its middle.
    let size: Vec<f32> = (0..3).map(|i| m.maxs[i] - m.mins[i]).collect();
    let axis = if size[0] < size[1] { 0 } else { 1 };
    let map = TestMap::Bsp(bsp);
    let mut g = setup(&map, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::T));
    let a = g.a.id().unwrap();
    let world = map.world();
    let c = glass.center;
    let spot = [-1.0f32, 1.0]
        .iter()
        .map(|s| {
            let mut o = [c[0], c[1], c[2] - 17.0];
            o[axis] += 120.0 * s;
            o
        })
        .find(|o| !world.is_solid(sim::collision::Hull::Standing, *o))
        .expect("room on one side of the glass");
    g.server.teleport(a, spot);
    let yaw = if axis == 0 { if spot[0] > c[0] { 180.0 } else { 0.0 } } else if spot[1] > c[1] { 270.0 } else { 90.0 };
    let mut broke = false;
    for t in 0..400 {
        // Teleporting resets velocity each tick so A stays put while shooting.
        if t % 8 == 0 {
            g.server.teleport(a, spot);
        }
        g.tick(Input { yaw, pitch: 0.0, attack: t % 16 < 2, ..Default::default() }, Input::default());
        broke |= g.b.take_events().iter().any(|e| matches!(e, GameEvent::Broke { model, .. } if *model as usize == glass.model));
        let _ = g.a.take_events();
        if broke && g.b.broken.contains(&(glass.model as u16)) {
            break;
        }
    }
    assert!(broke, "glass never broke");
    assert!(g.b.broken.contains(&(glass.model as u16)), "B's world doesn't know");
    // It no longer blocks a line through it, on the client too.
    let from = [spot[0], spot[1], c[2]];
    assert!(g.b.world.trace_line(from, c).fraction >= 1.0);
}
