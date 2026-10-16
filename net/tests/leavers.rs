//! Leaving a reserved match: one leaver changes nothing until they've been gone 5 minutes;
//! then their team may vote to surrender. A side with nobody left pauses the match, and
//! forfeits if it stays empty.
mod common;
use common::*;
use net::protocol::{GameEvent, Team};
use net::server::{Group, MatchSetup};
use sim::pmove::Input;

/// Fast-forward: the server and the given clients, no sleeping (localhost is quick enough
/// with a short pause every few ticks).
fn run(g: &mut Game, extra: &mut [&mut net::Client], ticks: u32, events: &mut Vec<GameEvent>) {
    for t in 0..ticks {
        g.server.step();
        g.a.tick(Input::default());
        events.extend(g.a.take_events());
        g.b.tick(Input::default());
        for c in extra.iter_mut() {
            c.tick(Input::default());
        }
        if t % 8 == 0 {
            std::thread::sleep(std::time::Duration::from_micros(150));
        }
    }
}

#[test]
fn surrender_after_a_teammate_abandons() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, net::config::MatchConfig { round_time: 1e6, ..Default::default() });
    g.server.reserve(MatchSetup { players: vec![(1, Group::A), (2, Group::A), (3, Group::B)], knife_round: false });
    g.a.ticket = 1;
    g.b.ticket = 3;
    let mut mate = net::Client::connect(g.server.local_addr(), "mate", sim::collision::World::from_bsp(&bsp)).unwrap();
    mate.ticket = 2;
    let mut ev = Vec::new();
    run(&mut g, &mut [&mut mate], 256, &mut ev);
    assert_eq!((g.a.team(), mate.team(), g.b.team()), (Team::T, Team::T, Team::Ct));
    // A's teammate leaves: nothing happens, and a surrender can't be called yet.
    mate.disconnect();
    run(&mut g, &mut [], 128, &mut ev);
    g.a.call_surrender();
    run(&mut g, &mut [], 64, &mut ev);
    assert!(g.a.round.vote.is_none(), "too early: they may come back");
    // Five minutes later they count as abandoned: A calls the vote, which passes (A is the
    // whole connected team), and B's team wins the match.
    run(&mut g, &mut [], 300 * 128, &mut ev);
    assert_eq!(g.a.round.abandoned, [true, false]);
    g.a.call_surrender();
    run(&mut g, &mut [], 128, &mut ev);
    assert!(ev.iter().any(|e| *e == GameEvent::Surrendered { team: Team::T }), "{ev:?}");
    assert!(ev.iter().any(|e| *e == GameEvent::MatchOver { winner: Some(Team::Ct) }));
}

#[test]
fn an_empty_side_pauses_then_forfeits() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, net::config::MatchConfig { freeze_time: 1.0, round_time: 30.0, ..Default::default() });
    g.server.reserve(MatchSetup { players: vec![(1, Group::A), (2, Group::B)], knife_round: false });
    g.a.ticket = 1;
    g.b.ticket = 2;
    let mut ev = Vec::new();
    run(&mut g, &mut [], 3 * 128, &mut ev);
    let round = g.a.round.round;
    assert!(round >= 1);
    g.b.disconnect();
    // A minute later: still the same round, and the clock hasn't run out (paused).
    run(&mut g, &mut [], 60 * 128, &mut ev);
    assert_eq!(g.a.round.round, round, "paused, no rounds handed out");
    assert!(!ev.iter().any(|e| matches!(e, GameEvent::MatchOver { .. })));
    // Nobody came back for five minutes: the side forfeits.
    run(&mut g, &mut [], 241 * 128, &mut ev);
    assert!(ev.iter().any(|e| *e == GameEvent::MatchOver { winner: Some(Team::T) }), "{ev:?}");
}
