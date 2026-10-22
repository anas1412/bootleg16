//! Dead bodies: they stay where the player died, E picks one up, and carried in front of you
//! it takes most of a bullet.
mod common;
use common::*;
use net::protocol::Team;
use sim::pmove::Input;

/// Yaw and pitch from `eye` to `at`.
fn aim(eye: [f32; 3], at: [f32; 3]) -> (f32, f32) {
    let d = [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]];
    (d[1].atan2(d[0]).to_degrees(), -(d[2] / (d[0] * d[0] + d[1] * d[1]).sqrt()).atan().to_degrees())
}

#[test]
fn a_body_is_picked_up_with_e_and_shields_its_carrier() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.server.set_rules(net::server::Rules::practice(0));
    g.server.set_give(vec![sim::weapons::Id::Ak47]);
    g.connect_as(Some(Team::T), Some(Team::T));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let idle = Input::default();
    // B dies: their body lies where they were, for both players
    let spot = g.server.player(b).unwrap().origin;
    g.server.slay(b);
    for _ in 0..16 {
        g.tick(idle, idle);
    }
    assert_eq!(g.a.bodies.len(), 1, "A sees the body");
    let body = g.a.bodies[0].origin;
    assert!(dist(body, spot) < 80.0 && g.a.bodies[0].carrier.is_none());
    assert_eq!(g.a.world.boxes.len(), 1, "A bumps into it");
    // A stands next to it, looks down at it and presses E
    let (lo, hi) = g.a.bodies[0].lying().rest_box();
    let stand = [(lo[0] + hi[0]) / 2.0 - 50.0, (lo[1] + hi[1]) / 2.0, body[2]];
    g.server.teleport(a, stand);
    for _ in 0..16 {
        g.tick(idle, idle);
    }
    let a_at = g.server.player(a).unwrap();
    let (lo, hi) = g.a.bodies[0].lying().rest_box();
    let (yaw, pitch) = aim(a_at.eye(), [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2] + 6.0]);
    let look = Input { yaw, pitch, ..idle };
    g.tick(Input { use_key: true, ..look }, idle);
    for _ in 0..16 {
        g.tick(look, idle);
    }
    assert_eq!(g.a.bodies[0].carrier, Some(a), "A carries it");
    assert!(g.a.state.movement.carrying && g.a.world.boxes.is_empty());
    // B (back by now) shoots A from in front, through the body: little gets through
    let hurt = |g: &mut Game, shield: bool| -> f32 {
        let a_at = g.server.player(a).unwrap().origin;
        g.server.teleport(b, [a_at[0] + 300.0, a_at[1], a_at[2]]);
        let face_b = Input { yaw: 0.0, ..idle };
        for _ in 0..64 {
            g.tick(face_b, idle);
        }
        assert_eq!(g.a.state.movement.carrying, shield);
        let before = g.server.vitals(a).unwrap().health;
        let (yaw, pitch) = aim(g.server.player(b).unwrap().eye(), [a_at[0], a_at[1], a_at[2] + 4.0]);
        g.tick(face_b, Input { yaw, pitch, attack: true, ..idle });
        for _ in 0..32 {
            g.tick(face_b, Input { yaw, pitch, ..idle });
        }
        before - g.server.vitals(a).unwrap().health
    };
    while !g.server.vitals(b).unwrap().alive() {
        g.tick(look, idle);
    }
    let shielded = hurt(&mut g, true);
    // E again puts it down; the same shot now hurts in full
    g.tick(Input { use_key: true, yaw: 0.0, ..idle }, idle);
    g.tick(Input { yaw: 0.0, ..idle }, idle);
    let open = hurt(&mut g, false);
    assert!(open > 15.0 && shielded < open * 0.5, "shielded {shielded}, open {open}");
}

#[test]
fn a_body_killed_against_a_wall_lies_clear_of_it() {
    let Some(bsp) = dust2() else { return };
    let world = sim::collision::World::from_bsp(&bsp);
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::T));
    let b = g.b.id().unwrap();
    // B stands with their back to the nearest wall, then dies
    let at = g.server.player(b).unwrap().origin;
    let (dir, hit) = (0..16)
        .map(|k| {
            let a = (k as f32 * 22.5f32).to_radians();
            let d = [a.cos(), a.sin(), 0.0];
            // (at the height a body lies: 6 over the floor)
            let low = [at[0], at[1], at[2] - 30.0];
            (d, world.trace_line(low, [low[0] + d[0] * 512.0, low[1] + d[1] * 512.0, low[2]]))
        })
        .min_by(|x, y| x.1.fraction.total_cmp(&y.1.fraction))
        .unwrap();
    assert!(hit.fraction < 1.0, "no wall near the spawn");
    // (24 + a little: their box clear of even a corner)
    g.server.teleport(b, [hit.end[0] - dir[0] * 26.0, hit.end[1] - dir[1] * 26.0, at[2]]);
    for _ in 0..8 {
        g.tick(Input::default(), Input { yaw: dir[1].atan2(dir[0]).to_degrees() + 180.0, ..Default::default() });
    }
    g.server.slay(b);
    for _ in 0..256 {
        g.tick(Input::default(), Input::default());
    }
    let body = g.a.bodies[0].lying();
    let moved = body.out_of_walls(&world).origin;
    assert!(dist(moved, body.origin) < 1.0, "its head or arms are in the wall: would move from {:?} to {moved:?}", body.origin);
    // every way of falling there, facing either way: moved clear, and some had to be
    let mut pushed = 0;
    for death in 0..sim::bodies::DEATHS.len() as u8 {
        let toward = dir[1].atan2(dir[0]).to_degrees();
        for yaw in [toward, toward + 90.0, toward + 180.0, toward + 270.0] {
            let l = sim::bodies::Lying { death, yaw, ..body };
            let out = l.out_of_walls(&world);
            assert!(dist(out.out_of_walls(&world).origin, out.origin) < 1.0, "death {death} yaw {yaw} still in the wall");
            pushed += (dist(out.origin, l.origin) > 1.0) as u32;
        }
    }
    assert!(pushed > 4, "only {pushed} needed moving: is the wall there?");
}

/// B dies, comes back (practice), and A is given a body to work with.
fn with_a_body() -> Option<(Game, u8, u8)> {
    let bsp = dust2()?;
    let mut g = setup(&bsp, 0, 0.0);
    g.server.set_rules(net::server::Rules::practice(0));
    g.server.set_give(vec![sim::weapons::Id::Ak47]);
    g.connect_as(Some(Team::T), Some(Team::T));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    g.server.slay(b);
    while !g.server.vitals(b).unwrap().alive() || g.a.bodies.is_empty() {
        g.tick(Input::default(), Input::default());
    }
    Some((g, a, b))
}

#[test]
fn a_carried_body_takes_a_knife_from_the_front() {
    let Some((mut g, a, b)) = with_a_body() else { return };
    let idle = Input::default();
    let (lo, hi) = g.a.bodies[0].lying().rest_box();
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2] + 6.0];
    g.server.teleport(a, [mid[0] - 50.0, mid[1], mid[2] + 30.0]);
    for _ in 0..16 {
        g.tick(idle, idle);
    }
    let (yaw, pitch) = aim(g.server.player(a).unwrap().eye(), mid);
    g.tick(Input { yaw, pitch, use_key: true, ..idle }, idle);
    g.tick(Input { yaw: 0.0, ..idle }, idle);
    assert!(g.server.player(a).unwrap().carrying, "A picked it up");
    // B, knife out, slashes A from the front (the body between them), then from behind
    let slash = |g: &mut Game, from: f32| -> f32 {
        let at = g.server.player(a).unwrap().origin;
        g.server.teleport(b, [at[0] + from * 44.0, at[1], at[2]]);
        g.server.set_health(a, 100.0);
        let k = Input { slot: 3, yaw: if from > 0.0 { 180.0 } else { 0.0 }, ..idle };
        for _ in 0..128 {
            g.tick(Input { yaw: 0.0, ..idle }, k);
        }
        g.tick(Input { yaw: 0.0, ..idle }, Input { attack: true, ..k });
        for _ in 0..16 {
            g.tick(Input { yaw: 0.0, ..idle }, k);
        }
        100.0 - g.server.vitals(a).unwrap().health
    };
    let (front, back) = (slash(&mut g, 1.0), slash(&mut g, -1.0));
    assert!(back > 5.0 && front < back * 0.5, "front {front}, back {back}");
}
