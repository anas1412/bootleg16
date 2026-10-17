//! Server + two clients over real localhost UDP. Skipped when de_dust2 isn't installed.
mod common;
use common::*;
use sim::pmove::Input;
use std::time::Duration;

/// Run A along a path, then stand still; check the server, B's view and A's prediction agree.
/// `unlag_tolerance`: with packet loss the client blends across missing snapshots,
/// so its view of a turning player differs slightly from the server's full history.
fn run_and_check(g: &mut Game, label: &str, unlag_tolerance: f32) {
    let id = g.a.id().unwrap();
    let mut last = g.server.player(id).unwrap().origin;
    let mut travelled = 0.0;
    let mut worst_correction = 0.0f32;
    let mut worst_unlag = 0.0f32;
    for t in 0..256 {
        // Run in wide circles with jumps, bouncing off whatever walls are near the spawn.
        let input = Input { forward: 400.0, yaw: t as f32 * 2.0, jump: t % 70 == 0, ..Default::default() };
        g.tick(input, Input::default());
        worst_correction = worst_correction.max(g.a.last_correction);
        // Lag compensation: rewinding to B's view time puts A exactly where B is drawing A.
        if t > 64 {
            if let Some(seen) = g.b.others().into_iter().find(|r| r.id == id) {
                let rewound = g.server.rewind(g.b.view_tick()).into_iter().find(|p| p.id == id).unwrap().origin;
                worst_unlag = worst_unlag.max(dist(seen.origin, rewound));
            }
        }
        let now = g.server.player(id).unwrap().origin;
        travelled += dist(now, last);
        last = now;
    }
    for _ in 0..128 {
        g.tick(Input::default(), Input::default());
    }
    let server_a = *g.server.player(g.a.id().unwrap()).unwrap();
    assert!(travelled > 200.0, "{label}: A only travelled {travelled} units on the server");
    // A's prediction ends exactly where the server says (same code, same inputs).
    assert!(dist(g.a.me().origin, server_a.origin) < 0.01, "{label}: prediction {:?} vs server {:?}", g.a.me().origin, server_a.origin);
    assert!(worst_unlag < unlag_tolerance, "{label}: rewound position off by {worst_unlag} from what B saw");
    // Mid-run corrections stay tiny: prediction was right the whole way.
    assert!(worst_correction < 1.0, "{label}: worst correction {worst_correction}");
    // B sees A where the server has them.
    let seen = g.b.others().into_iter().find(|r| Some(r.id) == g.a.id()).expect("B sees A");
    assert!(dist(seen.origin, server_a.origin) < 1.0, "{label}: B sees A at {:?}, server {:?}", seen.origin, server_a.origin);
}

#[test]
fn two_clients_on_lan() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    // Teammates: always sent to each other (enemies only when in sight, see tests/visibility.rs).
    g.connect_as(Some(net::protocol::Team::T), Some(net::protocol::Team::T));
    run_and_check(&mut g, "lan", 0.05);
}

#[test]
fn a_64_tick_server_makes_clients_step_at_64() {
    let Some(bsp) = dust2() else { return };
    let rate = net::tick::TickRate::new(64).unwrap();
    let cfg = net::config::MatchConfig { freeze_time: 0.0, buy_time: 1e6, round_time: 1e6, start_money: 16_000, friendly_fire: true, tick_rate: rate, ..Default::default() };
    let mut g = setup_with(&bsp, 0, 0.0, cfg);
    g.connect_as(Some(net::protocol::Team::T), Some(net::protocol::Team::T));
    assert_eq!((g.server.tick_rate(), g.a.rate(), g.b.rate()), (rate, rate, rate), "clients follow the server's Welcome");
    // Prediction and the server agree to the unit, so both use the same step size.
    run_and_check(&mut g, "lan at 64", 0.05);
}

#[test]
fn two_clients_at_100ms_ping_with_5_percent_loss() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 100, 0.05);
    g.connect_as(Some(net::protocol::Team::T), Some(net::protocol::Team::T));
    run_and_check(&mut g, "100ms/5%", 2.0);
}

#[test]
fn speed_hack_gains_nothing() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.connect();
    let id = g.a.id().unwrap();
    let mut last = g.server.player(id).unwrap().origin;
    let mut travelled = 0.0;
    // A hacked client sends inputs 3x faster than real time for 2 seconds,
    // running in circles so walls don't cut the run short.
    for t in 0..256 {
        let run = Input { forward: 400.0, yaw: t as f32 * 2.0, ..Default::default() };
        g.server.step();
        for _ in 0..3 {
            g.a.tick(run);
        }
        g.b.tick(Input::default());
        std::thread::sleep(Duration::from_micros(150));
        let now = g.server.player(id).unwrap().origin;
        travelled += dist(now, last);
        last = now;
    }
    let moved = travelled;
    // Honest max: 2 s at 250 u/s = 500 units (+ the 16-input burst allowance). Walls may cut it short.
    assert!(moved < 540.0, "speed hack moved {moved} units in 2 s");
    assert!(moved > 300.0, "player should still move at normal speed, moved {moved}");
}

#[test]
fn ak_kills_another_player_and_everyone_hears_about_it() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 60, 0.0);
    g.server.set_give(vec![sim::weapons::Id::Ak47]);
    g.connect_as(Some(net::protocol::Team::T), Some(net::protocol::Team::T));
    // The give applies on spawn; respawn both by waiting? They spawned before set_give took
    // effect only if connect happened first, so check what A holds.
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    assert_eq!(g.server.player_state(a).unwrap().loadout.active_id(), sim::weapons::Id::Ak47);
    let target = g.server.player(b).unwrap().origin;
    let eye = g.server.player(a).unwrap().eye();
    let d = [target[0] - eye[0], target[1] - eye[1], target[2] + 8.0 - eye[2]];
    let yaw = d[1].atan2(d[0]).to_degrees();
    let pitch = -(d[2] / (d[0] * d[0] + d[1] * d[1]).sqrt()).atan().to_degrees();
    let mut kills = Vec::new();
    for _ in 0..384 {
        g.tick(Input { yaw, pitch, attack: true, ..Default::default() }, Input::default());
        kills.extend(g.b.take_events().into_iter().filter(|e| matches!(e, net::protocol::GameEvent::Kill { .. })));
        let _ = g.a.take_events();
    }
    assert!(!kills.is_empty(), "B never died; health {:?}", g.server.vitals(b));
    assert!(matches!(kills[0], net::protocol::GameEvent::Kill { killer, victim, weapon: Some(sim::weapons::Id::Ak47), .. } if killer == a && victim == b));
    // A's own predicted ammo matches the server's.
    let server_clip = g.server.player_state(a).unwrap().loadout.active_gun().unwrap().clip;
    for _ in 0..64 {
        g.tick(Input { yaw, pitch, ..Default::default() }, Input::default());
    }
    let server_clip_now = g.server.player_state(a).unwrap().loadout.active_gun().unwrap().clip;
    assert_eq!(g.a.state.loadout.active_gun().unwrap().clip, server_clip_now, "predicted clip matches (was {server_clip})");
}

#[test]
fn he_hurts_and_flash_blinds_across_the_network() {
    use net::protocol::GameEvent;
    use sim::weapons::Id;
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 40, 0.0);
    g.server.set_give(vec![Id::HeGrenade, Id::Flashbang]);
    g.connect_as(Some(net::protocol::Team::T), Some(net::protocol::Team::T));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let target = g.server.player(b).unwrap().origin;
    let eye = g.server.player(a).unwrap().eye();
    let d = [target[0] - eye[0], target[1] - eye[1]];
    let yaw = d[1].atan2(d[0]).to_degrees();
    // Aim a little down so the grenade lands near B; B looks at A for the flash.
    let look = |pitch| Input { yaw, pitch, ..Default::default() };
    let b_looks = Input { yaw: yaw + 180.0, ..Default::default() };
    let mut events = Vec::new();
    let run = |g: &mut Game, events: &mut Vec<GameEvent>, input: Input, ticks: usize| {
        for _ in 0..ticks {
            g.tick(input, b_looks);
            events.extend(g.b.take_events());
        }
    };
    run(&mut g, &mut events, Input { slot: 4, ..look(30.0) }, 1);
    run(&mut g, &mut events, look(30.0), 128);
    assert_eq!(g.a.state.loadout.active_id(), Id::HeGrenade);
    run(&mut g, &mut events, Input { attack: true, ..look(30.0) }, 80); // pull the pin
    run(&mut g, &mut events, look(30.0), 300); // release, 1.5 s fuse
    let health = g.server.vitals(b).unwrap().health;
    assert!(events.iter().any(|e| matches!(e, GameEvent::Explosion { .. })), "no explosion");
    assert!(health < 100.0, "B unhurt by the HE: {health}");
    // A now holds the flashbang (the HE is gone).
    assert_eq!(g.a.state.loadout.active_id(), Id::Flashbang);
    run(&mut g, &mut events, Input { attack: true, ..look(10.0) }, 80);
    run(&mut g, &mut events, look(10.0), 300);
    let flashed = events.iter().find_map(|e| match e {
        GameEvent::Flash { at, blinded } => blinded.iter().find(|(id, _)| *id == b).map(|x| (*at, x.1)),
        _ => None,
    });
    let (at, blind) = flashed.expect("B should be blinded");
    // Full white only if the flash went off in front of B (it may fly past them).
    let eye_b = g.server.player(b).unwrap().eye();
    let (f, _, _) = sim::weapons::bullets::view_vectors(0.0, yaw + 180.0);
    let in_front = (at[0] - eye_b[0]) * f[0] + (at[1] - eye_b[1]) * f[1] + (at[2] + 1.0 - eye_b[2]) * f[2] >= 0.0;
    assert_eq!(blind.alpha, if in_front { 255 } else { 200 }, "flash at {at:?}");
    assert!(blind.hold > 0.0 && blind.fade > blind.hold);
}

#[test]
fn idle_players_keep_their_ammo() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 40, 0.0);
    g.connect();
    for _ in 0..1280 {
        g.tick(Input::default(), Input::default());
    }
    for c in [&g.a, &g.b] {
        let gun = c.state.loadout.active_gun().expect("pistol out");
        let server = g.server.player_state(c.id().unwrap()).unwrap().loadout.active_gun().unwrap().clip;
        assert_eq!((gun.clip, server), (sim::weapons::gun(gun.id).unwrap().clip, sim::weapons::gun(gun.id).unwrap().clip), "{:?}", gun.id);
    }
}

#[test]
fn throwing_grenades_does_not_touch_anyone_elses_ammo() {
    use sim::weapons::Id;
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.server.set_give(vec![Id::Smoke, Id::HeGrenade]);
    g.connect();
    let base = Input { yaw: 270.0, pitch: -30.0, ..Default::default() };
    for t in 0..2000u32 {
        let a = match t {
            1 | 121 => Input { slot: 4, ..base },
            241..=340 => Input { attack: true, ..base },
            _ => base,
        };
        g.tick(a, Input::default());
    }
    let b = g.b.state.loadout.active_gun().unwrap();
    assert_eq!(b.clip, sim::weapons::gun(b.id).unwrap().clip, "B's {:?} lost ammo", b.id);
    let sb = g.server.player_state(g.b.id().unwrap()).unwrap().loadout.active_gun().unwrap().clip;
    assert_eq!(sb, b.clip);
}

#[test]
fn weapon_menu_gives_the_weapon() {
    use sim::weapons::Id;
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 60, 0.0);
    g.connect();
    g.a.buy(net::protocol::BuyItem::Weapon(Id::Awp));
    g.a.buy(net::protocol::BuyItem::Weapon(Id::Flashbang));
    g.a.buy(net::protocol::BuyItem::KevlarHelmet);
    for _ in 0..64 {
        g.tick(Input::default(), Input::default());
    }
    let server = &g.server.player_state(g.a.id().unwrap()).unwrap().loadout;
    assert_eq!(server.active_id(), Id::Awp);
    assert_eq!(server.grenades, [0, 1, 0]);
    assert_eq!(g.a.state.loadout.active_id(), Id::Awp, "the client sees it too");
    assert_eq!((g.a.vitals.armor, g.a.vitals.helmet), (100.0, true));
}

#[test]
fn dropped_guns_land_and_everyone_sees_them() {
    use sim::weapons::Id;
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 40, 0.0);
    g.server.set_give(vec![Id::Ak47]);
    g.connect();
    let a = g.a.id().unwrap();
    g.tick(Input { drop: true, ..Default::default() }, Input::default());
    for _ in 0..256 {
        g.tick(Input::default(), Input::default());
    }
    assert_ne!(g.server.player_state(a).unwrap().loadout.active_id(), Id::Ak47);
    assert_eq!(g.b.ground.iter().map(|x| x.0).collect::<Vec<_>>(), vec![Id::Ak47], "B sees the AK on the ground");
    // It came to rest on the floor (not inside it, not floating).
    let at = g.b.ground[0].1;
    assert_ne!(g.a.world.point_contents([at[0], at[1], at[2] + 1.0]), sim::collision::Contents::Solid, "not inside a wall");
    assert!(g.a.world.trace_line(at, [at[0], at[1], at[2] - 4.0]).fraction < 1.0, "resting on something: {at:?}");
}


#[test]
fn practice_room_dummies_and_infinite_ammo() {
    use sim::weapons::Id;
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.server.set_rules(net::server::Rules::practice(3));
    g.server.set_give(vec![Id::Ak47]);
    g.connect();
    assert_eq!(g.server.player_count(), 5, "2 players + 3 dummies");
    assert!(g.a.practice);
    let me = g.a.team();
    let dummies: Vec<_> = g.a.scores.iter().filter(|s| s.bot).collect();
    assert_eq!(dummies.len(), 3);
    assert!(dummies.iter().all(|d| d.team != me && d.team != net::protocol::Team::Spectator), "dummies are on the other side");
    // Dummies stand still.
    let d = dummies[0].id;
    let before = g.server.player(d).unwrap().origin;
    for _ in 0..200 {
        g.tick(Input::default(), Input::default());
    }
    let after = g.server.player(d).unwrap().origin;
    assert!((before[0] - after[0]).abs() + (before[1] - after[1]).abs() < 1.0, "dummy moved");
    // Infinite reserve: spray a whole clip and the reserve stays full.
    for _ in 0..512 {
        g.tick(Input { attack: true, ..Default::default() }, Input::default());
    }
    let gun = *g.server.player_state(g.a.id().unwrap()).unwrap().loadout.active_gun().unwrap();
    assert_eq!((gun.id, gun.reserve), (Id::Ak47, 90));
}

#[test]
fn welcome_carries_the_servers_map_file_hash() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.connect();
    // The client compares this with its own file before playing (client/src/game.rs).
    assert_eq!(g.a.map_hash, bsp.hash);
    assert_ne!(bsp.hash, 0);
}

#[test]
fn kill_feed_survives_packet_loss() {
    let Some(bsp) = dust2() else { return };
    // 40% of packets lost: plain events would often vanish.
    let mut g = setup(&bsp, 50, 0.4);
    g.server.set_give(vec![sim::weapons::Id::Ak47]);
    g.connect_as(Some(net::protocol::Team::T), Some(net::protocol::Team::T));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let target = g.server.player(b).unwrap().origin;
    let eye = g.server.player(a).unwrap().eye();
    let d = [target[0] - eye[0], target[1] - eye[1], target[2] + 8.0 - eye[2]];
    let yaw = d[1].atan2(d[0]).to_degrees();
    let pitch = -(d[2] / (d[0] * d[0] + d[1] * d[1]).sqrt()).atan().to_degrees();
    let (mut kills_a, mut kills_b) = (0, 0);
    for t in 0..1200 {
        let fire = t < 400;
        g.tick(Input { yaw, pitch, attack: fire, ..Default::default() }, Input::default());
        kills_a += g.a.take_events().iter().filter(|e| matches!(e, net::protocol::GameEvent::Kill { .. })).count();
        kills_b += g.b.take_events().iter().filter(|e| matches!(e, net::protocol::GameEvent::Kill { .. })).count();
    }
    let deaths = g.a.scores.iter().find(|s| s.id == b).map(|s| s.deaths).unwrap_or(0);
    assert!(deaths >= 1, "B never died (health {:?})", g.server.vitals(b));
    assert!(kills_a >= 1 && kills_a == kills_b, "kills seen: A {kills_a}, B {kills_b}");
}

#[test]
fn characters_picked_show_on_the_scoreboard() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(net::protocol::Team::T), Some(net::protocol::Team::Ct));
    let a = g.a.id().unwrap();
    g.a.set_looks(2, 9); // 9: no such class, clamped to the last
    for _ in 0..256 {
        g.tick(Input::default(), Input::default());
    }
    assert_eq!(g.b.scores.iter().find(|s| s.id == a).map(|s| s.looks), Some([2, net::protocol::CLASSES - 1]));
}
