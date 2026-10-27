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
    let bsp = arena_map();
    let mut g = setup_with(&bsp, 0, 0.0, net::config::MatchConfig { round_time: 1e6, ..Default::default() });
    g.server.reserve(MatchSetup { players: vec![(1, Group::A), (2, Group::A), (3, Group::B)], knife_round: false, keys: Vec::new(), map: String::new() });
    g.a.ticket = 1;
    g.b.ticket = 3;
    let mut mate = net::Client::connect(g.server.local_addr(), "mate", bsp.world()).unwrap();
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
    let bsp = arena_map();
    let mut g = setup_with(&bsp, 0, 0.0, net::config::MatchConfig { freeze_time: 1.0, round_time: 30.0, ..Default::default() });
    g.server.reserve(MatchSetup { players: vec![(1, Group::A), (2, Group::B)], knife_round: false, keys: Vec::new(), map: String::new() });
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

#[test]
fn a_team_votes_a_teammate_out() {
    let bsp = arena_map();
    let mut g = setup_with(&bsp, 0, 0.0, net::config::MatchConfig { round_time: 1e6, ..Default::default() });
    g.server.reserve(MatchSetup { players: vec![(1, Group::A), (2, Group::A), (3, Group::B)], knife_round: false, keys: Vec::new(), map: String::new() });
    g.a.ticket = 1;
    g.b.ticket = 3;
    let mut mate = net::Client::connect(g.server.local_addr(), "mate", bsp.world()).unwrap();
    mate.ticket = 2;
    let mut ev = Vec::new();
    run(&mut g, &mut [&mut mate], 256, &mut ev);
    let target = mate.id().unwrap();
    // A calls the vote; with the target not voting, A alone is the majority: out.
    g.a.call_kick(target);
    run(&mut g, &mut [&mut mate], 128, &mut ev);
    assert!(ev.iter().any(|e| *e == GameEvent::VotedOut { player: target }), "{ev:?}");
    assert_eq!(mate.status, net::client::Status::Rejected);
    // They can't come back, and A's team may now surrender (a teammate is gone for good).
    let mut back = net::Client::connect(g.server.local_addr(), "mate", bsp.world()).unwrap();
    back.ticket = 2;
    run(&mut g, &mut [&mut back], 256, &mut ev);
    assert!(back.id().is_none());
    assert_eq!(g.a.round.abandoned, [true, false]);
    // B can't vote someone off A's team.
    g.b.call_kick(g.a.id().unwrap());
    run(&mut g, &mut [], 64, &mut ev);
    assert!(g.a.round.vote.is_none() && g.a.id().is_some());
}

#[test]
fn a_dropped_player_rejoins_their_seat() {
    let bsp = arena_map();
    let mut g = setup_with(&bsp, 0, 0.0, net::config::MatchConfig { round_time: 1e6, ..Default::default() });
    g.server.reserve(MatchSetup { players: vec![(1, Group::A), (2, Group::B)], knife_round: false, keys: Vec::new(), map: String::new() });
    g.a.ticket = 1;
    g.b.ticket = 2;
    let mut ev = Vec::new();
    run(&mut g, &mut [], 256, &mut ev);
    let before = g.server.player_count();
    // A's network goes dark for 6 s: the server drops them (5 s timeout), A hears nothing.
    g.a.simulate_network(0, 1.0);
    run(&mut g, &mut [], 6 * 128, &mut ev);
    assert_eq!(g.server.player_count(), before - 1, "dropped");
    assert!(g.a.silence() > 5.0);
    g.a.simulate_network(0, 0.0);
    // A rejoins: back in, on their own side.
    g.a.rejoin();
    run(&mut g, &mut [], 256, &mut ev);
    assert!(matches!(g.a.status, net::client::Status::Connected { .. }), "{:?}", g.a.status);
    assert!(g.a.silence() < 0.1);
    assert_eq!(g.server.player_count(), before);
    assert_eq!(g.a.team(), Team::T);
}
