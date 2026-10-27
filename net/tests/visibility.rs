//! Anti-wallhack over localhost UDP: enemies behind walls aren't sent (smoke hides no one); sounds are sent only
//! within hearing range. On our own test arena.
mod common;
use common::*;
use net::protocol::{GameEvent, Team};
use sim::pmove::Input;

#[test]
fn enemies_behind_walls_are_not_sent_but_in_the_open_they_are() {
    let bsp = arena_map();
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    // The Bureau spawn is behind a long wall from the Strays spawn.
    for _ in 0..64 {
        g.tick(Input::default(), Input::default());
    }
    assert!(!g.a.others().iter().any(|o| o.id == b), "the CT at his spawn was sent to the T");
    // Far away and walking: the T doesn't hear (or get) those footsteps either.
    let mut steps = 0;
    for _ in 0..256 {
        g.tick(Input::default(), Input { forward: 400.0, ..Default::default() });
        steps += g.a.take_events().iter().filter(|e| matches!(e, GameEvent::Footstep { player, .. } if *player == b)).count();
    }
    assert_eq!(steps, 0, "footsteps from across the map");
    // Right in front of him in the open spawn area: sent, and his steps are heard.
    let o = g.server.player(a).unwrap().origin;
    g.server.teleport(b, [o[0] + 150.0, o[1], o[2]]);
    let mut steps = 0;
    for _ in 0..256 {
        g.tick(Input::default(), Input { side: 400.0, ..Default::default() });
        steps += g.a.take_events().iter().filter(|e| matches!(e, GameEvent::Footstep { player, .. } if *player == b)).count();
    }
    assert!(g.a.others().iter().any(|o| o.id == b), "the CT in plain view wasn't sent");
    assert!(steps > 0, "no footsteps from someone next to you");
    // It works both ways.
    assert!(g.b.others().iter().any(|o| o.id == a));
}

#[test]
fn smoke_does_not_hide_anyone_from_the_server() {
    // CS 1.6 sends players behind smoke as usual (the cloud only hides them on screen).
    let bsp = arena_map();
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let o = g.server.player(a).unwrap().origin;
    g.server.add_smoke([o[0] + 200.0, o[1], o[2] - 30.0]);
    for _ in 0..(128 * 3) {
        g.server.teleport(b, [o[0] + 400.0, o[1], o[2]]);
        g.tick(Input::default(), Input::default());
    }
    assert!(g.a.others().iter().any(|p| p.id == b), "hidden by the smoke");
}

#[test]
fn the_dead_only_see_what_their_living_teammates_see() {
    let bsp = arena_map();
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let mut mate = net::Client::connect(g.server.local_addr(), "mate", bsp.world()).unwrap();
    let tick = |g: &mut Game, mate: &mut net::Client| {
        g.tick(Input::default(), Input::default());
        mate.tick(Input::default());
    };
    for _ in 0..128 {
        tick(&mut g, &mut mate);
    }
    mate.join_team(Some(Team::Ct));
    for _ in 0..128 {
        tick(&mut g, &mut mate);
    }
    let (t, ct, m) = (g.a.id().unwrap(), g.b.id().unwrap(), mate.id().unwrap());
    // B (a CT) dies; their living teammate is at CT spawn (joining mid-round you'd sit out
    // the round, as in CS: bring them in), the T across the map at his.
    g.server.set_health(m, 100.0);
    g.server.teleport(m, g.server.player(ct).unwrap().origin);
    g.server.set_health(ct, 0.0);
    for _ in 0..64 {
        tick(&mut g, &mut mate);
    }
    assert_eq!((mate.team(), g.b.vitals.alive()), (Team::Ct, false), "setup");
    assert!(!mate.others().iter().any(|o| o.id == t), "setup: the living teammate can't see the T either");
    assert!(!g.b.others().iter().any(|o| o.id == t), "dead, but shown an enemy no teammate can see (ghosting)");
    // The teammate walks up to the T: now B sees him too (watching that teammate).
    let o = g.server.player(t).unwrap().origin;
    g.server.teleport(m, [o[0] + 150.0, o[1], o[2]]);
    for _ in 0..64 {
        tick(&mut g, &mut mate);
    }
    assert!(mate.others().iter().any(|o| o.id == t), "setup: the teammate next to him sees him");
    assert!(g.b.others().iter().any(|o| o.id == t), "a teammate sees him, so the dead do too");
    // Team IDs: a teammate's health comes along, an enemy's never does.
    assert_eq!(g.b.others().iter().find(|o| o.id == m).and_then(|o| o.mate).map(|x| x.health), Some(100));
    assert!(mate.others().iter().find(|o| o.id == t).is_some_and(|o| o.mate.is_none()), "an enemy's health was sent");
}

#[test]
fn a_whole_team_dead_sees_nothing_of_the_enemy() {
    let bsp = arena_map();
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (t, ct) = (g.a.id().unwrap(), g.b.id().unwrap());
    // The CT right in front of the T, then dies: their team is all dead, the screen goes black.
    let o = g.server.player(t).unwrap().origin;
    g.server.teleport(ct, [o[0] + 150.0, o[1], o[2]]);
    for _ in 0..64 {
        g.tick(Input::default(), Input::default());
    }
    assert!(g.b.others().iter().any(|o| o.id == t));
    g.server.set_health(ct, 0.0);
    for _ in 0..128 {
        g.tick(Input::default(), Input::default());
    }
    assert!(!g.b.others().iter().any(|o| o.id == t && o.alive), "nobody alive on their team: nothing of the enemy");
}

#[test]
fn a_dropped_bomb_is_sent_to_cts_only_in_their_sight() {
    let bsp = arena_map();
    let mut g = setup(&bsp, 0, 0.0);
    g.server.set_rules(net::server::Rules::practice(0)); // practice hands the Strays the bomb
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    for _ in 0..128 * 5 {
        g.tick(Input::default(), Input::default());
        if g.a.bomb == net::protocol::BombNet::Carried(a) {
            break;
        }
    }
    assert_eq!(g.a.bomb, net::protocol::BombNet::Carried(a), "the terrorist has the bomb");
    // The CT at his spawn across the map isn't told who has it.
    assert_eq!(g.b.bomb, net::protocol::BombNet::None);
    // The T drops it at his spawn: he knows where it is, the CT across the map doesn't.
    for i in 0..128 {
        let drop = Input { slot: if i < 8 { 5 } else { 0 }, drop: (96..104).contains(&i), ..Default::default() };
        g.tick(drop, Input::default());
    }
    assert!(matches!(g.a.bomb, net::protocol::BombNet::Dropped(_)), "{:?}", g.a.bomb);
    assert_eq!(g.b.bomb, net::protocol::BombNet::None, "the CT saw a bomb across the map");
    // Standing over it, the CT sees it.
    let net::protocol::BombNet::Dropped(at) = g.a.bomb else { unreachable!() };
    g.server.teleport(b, [at[0] + 100.0, at[1], at[2] + 40.0]);
    for _ in 0..64 {
        g.tick(Input::default(), Input::default());
    }
    assert!(matches!(g.b.bomb, net::protocol::BombNet::Dropped(_)), "{:?}", g.b.bomb);
}
