//! Anti-wallhack over localhost UDP: enemies out of sight aren't sent; sounds are sent only
//! within hearing range. Skipped when de_dust2 isn't installed.
mod common;
use common::*;
use net::protocol::{GameEvent, Team};
use sim::pmove::Input;

#[test]
fn enemies_behind_walls_are_not_sent_but_in_the_open_they_are() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    // T and CT spawns on dust2 are far apart with the whole map in between.
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
fn smoke_hides_enemies_from_the_server_too() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let o = g.server.player(a).unwrap().origin;
    g.server.teleport(b, [o[0] + 400.0, o[1], o[2]]);
    for _ in 0..64 {
        g.tick(Input::default(), Input::default());
    }
    assert!(g.a.others().iter().any(|p| p.id == b), "in plain view first");
    // A smoke between them: thrown straight ahead (smokes pop when they stop rolling).
    g.server.add_smoke([o[0] + 200.0, o[1], o[2] - 30.0]);
    for _ in 0..(128 * 3) {
        g.server.teleport(b, [o[0] + 400.0, o[1], o[2]]);
        g.tick(Input::default(), Input::default());
    }
    assert!(!g.a.others().iter().any(|p| p.id == b), "seen through the smoke");
}

#[test]
fn the_dead_only_see_what_their_living_teammates_see() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let mut mate = net::Client::connect(g.server.local_addr(), "mate", sim::collision::World::from_bsp(&bsp)).unwrap();
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
    let Some(bsp) = dust2() else { return };
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
