//! Reserved matches over localhost UDP: only ticket holders get in, on their teams; a knife
//! round decides sides (winners vote), then round 1; a dropped player gets their seat back.
mod common;
use common::*;
use net::protocol::{GameEvent, Stage, Team};
use net::server::{Group, MatchSetup};
use sim::pmove::Input;

#[test]
fn knife_round_then_winners_pick_sides() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, net::config::MatchConfig::default());
    g.server.reserve(MatchSetup { players: vec![(111, Group::A), (222, Group::B)], knife_round: true });
    g.a.ticket = 111;
    g.b.ticket = 222;
    let mut ev = Vec::new();
    let run = |g: &mut Game, n: usize, ia: Input, ib: Input, ev: &mut Vec<GameEvent>| {
        for _ in 0..n {
            g.tick(ia, ib);
            ev.extend(g.a.take_events());
            g.b.take_events();
        }
    };
    run(&mut g, 128, Input::default(), Input::default(), &mut ev);
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    assert_eq!(g.a.team(), Team::T, "group A starts as T");
    assert_eq!(g.b.team(), Team::Ct);
    assert_eq!(g.a.round.stage, Stage::Knife, "everyone's in: knife round");
    assert_eq!(g.server.player_state(a).unwrap().loadout.active_id(), sim::weapons::Id::Knife);
    // Someone without a ticket is turned away.
    let mut stranger = net::Client::connect(g.server.local_addr(), "x", sim::collision::World::from_bsp(&bsp)).unwrap();
    for _ in 0..64 {
        g.server.step();
        stranger.tick(Input::default());
        std::thread::sleep(std::time::Duration::from_micros(150));
    }
    assert_eq!(stranger.status, net::client::Status::Rejected);
    // After freeze time, A knifes B (put right in front of them).
    run(&mut g, 3 * 128, Input::default(), Input::default(), &mut ev);
    let o = g.server.player(a).unwrap().origin;
    for t in 0..800 {
        if t % 32 == 0 {
            g.server.teleport(b, [o[0] + 30.0, o[1], o[2]]);
        }
        run(&mut g, 1, Input { yaw: 0.0, pitch: 5.0, attack: t % 64 < 2, ..Default::default() }, Input::default(), &mut ev);
        if ev.iter().any(|e| matches!(e, GameEvent::KnifeWon { .. })) {
            break;
        }
    }
    assert!(ev.iter().any(|e| *e == GameEvent::KnifeWon { winners: Team::T }), "{ev:?}");
    assert!(matches!(g.a.round.stage, Stage::SideVote { winners: Team::T, .. }));
    // The winner votes to switch: sides swap and round 1 starts with pistols.
    g.a.side_vote(true);
    run(&mut g, 64, Input::default(), Input::default(), &mut ev);
    assert!(ev.iter().any(|e| *e == GameEvent::SidesChosen { switched: true }), "{ev:?}");
    assert_eq!(g.a.team(), Team::Ct);
    assert_eq!(g.a.round.stage, Stage::Match);
    assert_eq!(g.a.round.round, 1);
    assert_eq!(g.server.player_state(a).unwrap().loadout.active_id(), sim::weapons::Id::Usp, "CT pistol");

    // B drops and comes back with the same ticket: same seat (now T), same money.
    let money = g.server.money(b).unwrap();
    g.b.disconnect();
    run(&mut g, 16, Input::default(), Input::default(), &mut ev);
    let mut back = net::Client::connect(g.server.local_addr(), "b", sim::collision::World::from_bsp(&bsp)).unwrap();
    back.ticket = 222;
    for _ in 0..128 {
        g.server.step();
        g.a.tick(Input::default());
        back.tick(Input::default());
        std::thread::sleep(std::time::Duration::from_micros(150));
    }
    let id = back.id().expect("rejoined");
    assert_eq!(back.team(), Team::T);
    assert_eq!(g.server.money(id), Some(money));
}

#[test]
fn abandoned_match_frees_the_server() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, net::config::MatchConfig::default());
    g.server.reserve(MatchSetup { players: vec![(111, Group::A), (222, Group::B)], knife_round: false });
    // Nobody with a ticket shows up (a and b have none): after the wait, the server is free again.
    for _ in 0..92 * 128 {
        g.server.step();
    }
    assert!(!g.server.is_reserved());
}
