//! Bots and a match on our own map format (the exported Godot test arena), no Valve files.
mod common;
use common::*;
use net::bot::BotMap;

#[test]
fn bots_find_their_way_from_spawn_to_both_bomb_sites() {
    let (map, hash) = arena();
    let bots = BotMap::from_map(map, hash);
    assert_eq!(bots.sites.len(), 2, "both sites");
    let none = std::collections::HashSet::new();
    let from = bots.nav.nearest(bots.spawns.0).expect("a walkable point by the Strays spawn");
    for (i, site) in bots.sites.iter().enumerate() {
        let to = bots.nav.nearest(*site).unwrap_or_else(|| panic!("no walkable point at site {i}"));
        // site A is up on the platform: only the stairs (or the ladder) lead there
        assert!(!bots.nav.path(from, to, &none).is_empty(), "no way to site {i} at {site:?}");
    }
}

/// Yaw and pitch from `eye` to `at`.
fn aim(eye: [f32; 3], at: [f32; 3]) -> (f32, f32) {
    let d = [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]];
    (d[1].atan2(d[0]).to_degrees(), -(d[2] / (d[0] * d[0] + d[1] * d[1]).sqrt()).atan().to_degrees())
}

#[test]
fn shooting_the_window_breaks_it_for_everyone_and_you_walk_through() {
    use sim::pmove::Input;
    let mut g = setup_arena(0, 0.0);
    g.server.set_give(vec![sim::weapons::Id::Ak47]);
    g.connect_as(Some(net::protocol::Team::T), Some(net::protocol::Team::T));
    let a = g.a.id().unwrap();
    // the window: Godot (12, 1.25, 0), 4 wide along x; A stands 3 m south of it
    let to = |x: f32, y: f32, z: f32| [x / 0.0254, -z / 0.0254, y / 0.0254];
    g.server.teleport(a, to(12.0, 0.92, 3.0));
    for _ in 0..32 {
        g.tick(Input::default(), Input::default());
    }
    let pane = to(12.0, 1.25, 0.0);
    // whole: it stops a line through it (bullets) for A's own prediction too
    assert!(g.a.world.trace_line(to(12.0, 1.25, 1.0), to(12.0, 1.25, -1.0)).fraction < 1.0);
    let (yaw, pitch) = aim(g.server.player(a).unwrap().eye(), pane);
    for _ in 0..64 {
        g.tick(Input { yaw, pitch, attack: true, ..Default::default() }, Input::default());
    }
    for _ in 0..16 {
        g.tick(Input::default(), Input::default());
    }
    assert!(g.a.broken.contains(&1) && g.b.broken.contains(&1), "both see it broken: {:?} {:?}", g.a.broken, g.b.broken);
    // gone from everyone's world: a line passes, and A walks through where it was
    assert!(g.a.world.trace_line(to(12.0, 1.25, 1.0), to(12.0, 1.25, -1.0)).fraction >= 1.0);
    for _ in 0..256 {
        g.tick(Input { forward: 400.0, yaw: 90.0, ..Default::default() }, Input::default());
    }
    assert!(g.server.player(a).unwrap().origin[1] > 0.0 / 0.0254 + 16.0, "didn't get through: {:?}", g.server.player(a).unwrap().origin);
}

#[test]
fn walking_into_the_door_opens_it_for_everyone_and_you_pass() {
    use sim::pmove::Input;
    let mut g = setup_arena(20, 0.0);
    g.connect_as(Some(net::protocol::Team::T), Some(net::protocol::Team::T));
    let a = g.a.id().unwrap();
    let to = |x: f32, y: f32, z: f32| [x / 0.0254, -z / 0.0254, y / 0.0254];
    // A just south of the door (Godot x 0, z 12), walking north (Godot -z, yaw 90)
    g.server.teleport(a, to(0.0, 0.92, 14.0));
    for _ in 0..32 {
        g.tick(Input::default(), Input::default());
    }
    assert!(g.a.world.trace_line(to(0.0, 1.0, 13.0), to(0.0, 1.0, 11.0)).fraction < 1.0, "the door is shut");
    for _ in 0..(3 * 128) {
        g.tick(Input { forward: 400.0, yaw: 90.0, ..Default::default() }, Input::default());
    }
    let y = g.server.player(a).unwrap().origin[1];
    assert!(y > -11.0 / 0.0254 + 16.0, "didn't get through the door: at Godot z {:.2}", -y * 0.0254);
    // B's copy of the world has the door open too (and A's prediction did)
    let open = |w: &sim::collision::World| w.doors.doors[0].transform().1[1].abs() > 45.0;
    assert!(open(&g.b.world) && open(&g.a.world), "the door isn't open for everyone");
}
