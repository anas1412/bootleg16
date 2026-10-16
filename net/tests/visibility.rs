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
