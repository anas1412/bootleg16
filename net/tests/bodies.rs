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
    let stand = [body[0] - 50.0, body[1], body[2]];
    g.server.teleport(a, stand);
    for _ in 0..16 {
        g.tick(idle, idle);
    }
    let a_at = g.server.player(a).unwrap();
    let (yaw, pitch) = aim(a_at.eye(), [body[0], body[1], body[2] - 30.0]);
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
