//! Movement on the real de_dust2 collision hulls. Skipped when CS 1.6 isn't installed.
use goldsrc::bsp::Bsp;
use sim::collision::{Hull, World};
use sim::pmove::{step, Input, MoveVars, Player};

fn dust2() -> Option<Bsp> {
    let home = std::env::var("HOME").ok()?;
    let dir = std::env::var("BOOTLEG16_DIR").unwrap_or(format!("{home}/.steam/steam/steamapps/common/Half-Life"));
    Bsp::load(format!("{dir}/cstrike/maps/de_dust2.bsp")).ok()
}

#[test]
fn every_spawn_lands_and_can_run() {
    let Some(bsp) = dust2() else { return eprintln!("de_dust2 not installed, skipping") };
    let world = World::from_bsp(&bsp);
    let spawns: Vec<[f32; 3]> = bsp
        .entity_list()
        .iter()
        .filter(|e| matches!(e.get("classname").map(String::as_str), Some("info_player_start" | "info_player_deathmatch")))
        .map(|e| {
            let v: Vec<f32> = e["origin"].split_whitespace().map(|x| x.parse().unwrap()).collect();
            [v[0], v[1], v[2]]
        })
        .collect();
    assert!(spawns.len() >= 10, "found {} spawns", spawns.len());
    let dt = 1.0 / 128.0;
    for (i, &s) in spawns.iter().enumerate() {
        assert!(!world.is_solid(Hull::Standing, s), "spawn {i} at {s:?} is inside a wall");
        let mut p = Player::new(s);
        for _ in 0..128 {
            step(&mut p, &Input::default(), dt, &world, &MoveVars::default());
        }
        assert!(p.on_ground, "spawn {i} never landed: {:?}", p.origin);
        // Run in a circle for 4 seconds: must never end up inside geometry or fall out of the map.
        for t in 0..512 {
            let input = Input { forward: 400.0, yaw: t as f32 * 0.7, jump: t % 90 == 0, ..Default::default() };
            step(&mut p, &input, dt, &world, &MoveVars::default());
            assert!(!world.is_solid(p.hull(), p.origin), "spawn {i}: stuck in solid at tick {t}: {:?}", p.origin);
        }
        assert!(p.origin[2] > s[2] - 300.0, "spawn {i}: fell out of the map to {:?}", p.origin);
    }
}
