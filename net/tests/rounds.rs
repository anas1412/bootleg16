//! The round system end to end over localhost UDP: freeze time, round wins and money,
//! planting, defusing and the explosion. Skipped when de_dust2 isn't installed.
mod common;
use common::*;
use net::config::MatchConfig;
use net::protocol::{GameEvent, Team};
use net::rounds::{EndReason, Phase};
use sim::pmove::Input;

/// Short timers so the tests run fast.
fn quick() -> MatchConfig {
    MatchConfig { freeze_time: 1.0, buy_time: 5.0, round_end_delay: 1.0, c4_timer: 5.0, defuse_time: 2.0, ..Default::default() }
}

fn wait(g: &mut Game, secs: f32, ia: Input, ib: Input, events: &mut Vec<GameEvent>) {
    for _ in 0..(secs * 128.0) as u32 {
        g.tick(ia, ib);
        events.extend(g.a.take_events());
        g.b.take_events();
    }
}

/// A standing spot in the middle of a bomb site (players drop to the floor).
fn site_spot(bsp: &goldsrc::bsp::Bsp) -> [f32; 3] {
    let z = sim::zones::Zones::from_bsp(bsp);
    let (lo, hi) = z.bomb_sites[0];
    let p = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2] + 40.0];
    assert!(z.in_bomb_site(p));
    p
}

#[test]
fn freeze_time_then_a_win_pays_both_teams_and_starts_the_next_round() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, quick());
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let mut ev = Vec::new();
    // connect_as ran ~1 s: we're in the first round. Wait for the next freeze to test it cleanly.
    assert_ne!(g.a.round.phase, Phase::Warmup, "both teams have a player: the match started");
    // Live: the terrorist shoots the CT (put right in front of him): terrorists win by elimination.
    while g.a.round.phase != Phase::Live {
        wait(&mut g, 0.05, Input::default(), Input::default(), &mut ev);
    }
    let o = g.server.player(a).unwrap().origin;
    g.server.teleport(b, [o[0] + 64.0, o[1], o[2]]);
    ev.clear();
    for i in 0..60 {
        // Semi-auto pistol: press and release; aim at the body.
        let fire = Input { yaw: 0.0, pitch: 8.0, attack: i % 2 == 0, ..Default::default() };
        wait(&mut g, 0.05, fire, Input::default(), &mut ev);
        if ev.iter().any(|e| matches!(e, GameEvent::RoundEnd { .. })) {
            break;
        }

    }
    assert!(ev.iter().any(|e| *e == GameEvent::RoundEnd { winner: Team::T, reason: EndReason::Eliminated }), "{ev:?}");
    // T: 800 + 300 kill + 3250 win; CT: 800 + 1400 first loss.
    wait(&mut g, 1.2, Input::default(), Input::default(), &mut ev);
    assert_eq!(g.server.money(a), Some(4350));
    assert_eq!(g.server.money(b), Some(2200));
    // The next round started and brought the CT back.
    assert!(ev.iter().any(|e| matches!(e, GameEvent::RoundStart { round: 2 })), "{ev:?}");
    assert!(g.server.vitals(b).unwrap().alive());
    assert_eq!(g.a.round.score, [1, 0]);
    // The scoreboard: the T is the round's MVP, both played a round, the history has it, and
    // money shows for your own team only.
    let me = g.a.scores.iter().find(|s| s.id == a).unwrap();
    let them = g.a.scores.iter().find(|s| s.id == b).unwrap();
    assert_eq!((me.mvps, me.rounds, them.rounds), (1, 1, 1));
    assert_eq!(me.kills, 1);
    assert!(me.damage >= 100, "{}", me.damage);
    assert_eq!(me.money, Some(4350));
    assert_eq!(them.money, None, "the other team's money is hidden");
    assert_eq!(g.a.round_history, vec![(Team::T, EndReason::Eliminated)]);
    assert!(me.ping < 100, "{}", me.ping);
    // Ammo isn't bought: the new round tops everyone's reserve up.
    let full = sim::weapons::gun(sim::weapons::Id::Glock).unwrap().reserve;
    assert_eq!(g.server.player_state(a).unwrap().loadout.secondary.unwrap().reserve, full);
    // Freeze time of round 2 (1 s): holding forward goes nowhere (falling onto the floor is fine).
    assert_eq!(g.server.round_phase(), Phase::Freeze);
    let start = g.server.player(a).unwrap().origin;
    wait(&mut g, 0.3, Input { forward: 400.0, ..Default::default() }, Input::default(), &mut ev);
    let now = g.server.player(a).unwrap().origin;
    let moved = dist([now[0], now[1], 0.0], [start[0], start[1], 0.0]);
    assert!(moved < 1.0, "moved {moved} during freeze time");
}

/// Plant in a site (3 s holding attack with the C4 out), then either defuse or let it blow.
fn plant(g: &mut Game, bsp: &goldsrc::bsp::Bsp, ev: &mut Vec<GameEvent>) -> [f32; 3] {
    let a = g.a.id().unwrap();
    while g.a.round.phase != Phase::Live {
        wait(g, 0.05, Input::default(), Input::default(), ev);
    }
    assert!(g.server.player_state(a).unwrap().loadout.c4, "the only terrorist carries the bomb");
    let spot = site_spot(bsp);
    g.server.teleport(a, spot);
    wait(g, 0.5, Input::default(), Input::default(), ev);
    // Outside a site nothing happens; inside, 3 s of holding attack plants it.
    wait(g, 0.1, Input { slot: 5, ..Default::default() }, Input::default(), ev);
    wait(g, 0.5, Input::default(), Input::default(), ev);
    ev.clear();
    wait(g, 3.3, Input { attack: true, ..Default::default() }, Input::default(), ev);
    let at = ev.iter().find_map(|e| match e {
        GameEvent::BombPlanted { at, .. } => Some(*at),
        _ => None,
    });
    at.expect("planted")
}

#[test]
fn bomb_defused_pays_the_planters_bonus() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, quick());
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let mut ev = Vec::new();
    let at = plant(&mut g, &bsp, &mut ev);
    let money_t = g.server.money(a).unwrap();
    // The CT stands next to it, looks down at it and holds use for the defuse time (2 s here).
    g.server.teleport(b, [at[0] + 30.0, at[1], at[2] + 40.0]);
    wait(&mut g, 0.3, Input::default(), Input::default(), &mut ev);
    let eye = g.server.player(b).unwrap().eye();
    let d = [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]];
    let look = Input { yaw: d[1].atan2(d[0]).to_degrees(), pitch: -(d[2].atan2((d[0] * d[0] + d[1] * d[1]).sqrt())).to_degrees(), use_key: true, ..Default::default() };
    ev.clear();
    wait(&mut g, 2.5, Input::default(), look, &mut ev);
    assert!(ev.iter().any(|e| matches!(e, GameEvent::BombDefused { .. })), "{ev:?}");
    assert!(ev.iter().any(|e| *e == GameEvent::RoundEnd { winner: Team::Ct, reason: EndReason::BombDefused }));
    assert_eq!(g.server.money(a).unwrap(), money_t + 1400 + 800, "loss bonus plus the planted bonus");
}

#[test]
fn bomb_explodes_wins_the_round_and_kills_nearby() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, quick());
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let b = g.b.id().unwrap();
    let mut ev = Vec::new();
    let at = plant(&mut g, &bsp, &mut ev);
    g.server.teleport(b, [at[0] + 100.0, at[1], at[2] + 40.0]);
    ev.clear();
    wait(&mut g, 5.5, Input::default(), Input::default(), &mut ev);
    assert!(ev.iter().any(|e| matches!(e, GameEvent::BombExploded { .. })), "{ev:?}");
    assert!(ev.iter().any(|e| *e == GameEvent::RoundEnd { winner: Team::T, reason: EndReason::BombExploded }));
    assert!(ev.iter().any(|e| matches!(e, GameEvent::Kill { victim, weapon: Some(sim::weapons::Id::C4), .. } if *victim == b)), "{ev:?}");
    // dust2's crates at that site go with it, and are back next round.
    assert!(ev.iter().any(|e| matches!(e, GameEvent::Broke { .. })), "no crates broke: {ev:?}");
    assert!(!g.a.broken.is_empty());
    wait(&mut g, 2.0, Input::default(), Input::default(), &mut ev);
    assert!(g.a.broken.is_empty(), "crates not restored: {:?}", g.a.broken);
}

#[test]
fn time_running_out_pays_living_terrorists_nothing() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, MatchConfig { round_time: 2.0, ..quick() });
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let mut ev = Vec::new();
    while g.a.round.phase != Phase::Live {
        wait(&mut g, 0.05, Input::default(), Input::default(), &mut ev);
    }
    let (t, ct) = (g.server.money(a).unwrap(), g.server.money(b).unwrap());
    ev.clear();
    wait(&mut g, 2.5, Input::default(), Input::default(), &mut ev);
    assert!(ev.iter().any(|e| *e == GameEvent::RoundEnd { winner: Team::Ct, reason: EndReason::TimeUp }), "{ev:?}");
    assert_eq!(g.server.money(a), Some(t), "the living T gets no loss bonus");
    assert_eq!(g.server.money(b), Some(ct + 3250));
}

#[test]
fn everything_bought_this_round_sells_back() {
    use net::protocol::BuyItem;
    use sim::weapons::Id;
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, MatchConfig { freeze_time: 3.0, buy_time: 20.0, start_money: 16000, ..quick() });
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let a = g.a.id().unwrap();
    let mut ev = Vec::new();
    wait(&mut g, 0.5, Input::default(), Input::default(), &mut ev);
    let money = g.server.money(a).unwrap();
    for item in [BuyItem::Weapon(Id::Ak47), BuyItem::KevlarHelmet, BuyItem::Weapon(Id::HeGrenade), BuyItem::Weapon(Id::Deagle)] {
        g.a.buy(item);
        wait(&mut g, 0.1, Input::default(), Input::default(), &mut ev);
    }
    let l = g.server.player_state(a).unwrap().loadout.clone();
    assert_eq!((l.primary.map(|p| p.id), l.secondary.map(|p| p.id), l.grenades[0]), (Some(Id::Ak47), Some(Id::Deagle), 1), "bought");
    assert!(g.server.money(a).unwrap() < money);
    g.a.undo_buys();
    wait(&mut g, 0.2, Input::default(), Input::default(), &mut ev);
    let l = g.server.player_state(a).unwrap().loadout.clone();
    assert_eq!(g.server.money(a), Some(money), "every dollar back");
    assert_eq!((l.primary.map(|p| p.id), l.secondary.map(|p| p.id), l.grenades[0]), (None, Some(Id::Glock), 0), "the Glock is back off the floor");
    assert_eq!(g.server.vitals(a).unwrap().armor, 0.0);
    // A gun that's been fired is yours: no refund.
    g.a.buy(BuyItem::Weapon(Id::Ak47));
    wait(&mut g, 1.0, Input::default(), Input::default(), &mut ev);
    wait(&mut g, 0.3, Input { attack: true, ..Default::default() }, Input::default(), &mut ev);
    let after = g.server.money(a).unwrap();
    g.a.undo_buys();
    wait(&mut g, 0.2, Input::default(), Input::default(), &mut ev);
    assert_eq!(g.server.money(a), Some(after));
    assert_eq!(g.server.player_state(a).unwrap().loadout.primary.map(|p| p.id), Some(Id::Ak47));
}

#[test]
fn practice_hands_the_strays_the_bomb_and_only_a_ct_defuses() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, quick());
    g.server.set_rules(net::server::Rules::practice(0));
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let mut ev = Vec::new();
    wait(&mut g, 0.3, Input::default(), Input::default(), &mut ev);
    assert!(g.server.player_state(a).unwrap().loadout.c4, "the Strays (T) get the bomb in practice");
    assert!(!g.server.player_state(b).unwrap().loadout.c4, "the Bureau (CT) don't");
    // plant it in a site
    let spot = site_spot(&bsp);
    g.server.teleport(a, spot);
    wait(&mut g, 0.5, Input::default(), Input::default(), &mut ev);
    wait(&mut g, 0.1, Input { slot: 5, ..Default::default() }, Input::default(), &mut ev);
    wait(&mut g, 0.5, Input::default(), Input::default(), &mut ev);
    ev.clear();
    wait(&mut g, 3.3, Input { attack: true, ..Default::default() }, Input::default(), &mut ev);
    let at = ev.iter().find_map(|e| match e {
        GameEvent::BombPlanted { at, .. } => Some(*at),
        _ => None,
    });
    let at = at.expect("planted");
    // the T can't defuse it (CS 1.6: only a CT); the CT can
    let look_from = |g: &mut Game, id: u8| {
        let eye = g.server.player(id).unwrap().eye();
        let d = [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]];
        Input { yaw: d[1].atan2(d[0]).to_degrees(), pitch: -(d[2].atan2((d[0] * d[0] + d[1] * d[1]).sqrt())).to_degrees(), use_key: true, ..Default::default() }
    };
    g.server.teleport(a, [at[0] + 30.0, at[1], at[2] + 40.0]);
    wait(&mut g, 0.3, Input::default(), Input::default(), &mut ev);
    let look = look_from(&mut g, a);
    ev.clear();
    wait(&mut g, 0.5, look, Input::default(), &mut ev);
    assert!(!ev.iter().any(|e| matches!(e, GameEvent::BombDefused { .. } | GameEvent::Defusing { .. })), "a T defused: {ev:?}");
    g.server.teleport(b, [at[0] - 30.0, at[1], at[2] + 40.0]);
    wait(&mut g, 0.1, Input::default(), Input::default(), &mut ev);
    let look = look_from(&mut g, b);
    ev.clear();
    wait(&mut g, 2.5, Input::default(), look, &mut ev);
    assert!(ev.iter().any(|e| matches!(e, GameEvent::BombDefused { .. })), "{ev:?}");
    // a fresh bomb 3 s later
    wait(&mut g, 3.5, Input::default(), Input::default(), &mut ev);
    assert!(g.server.player_state(a).unwrap().loadout.c4, "handed another one");
}

#[test]
fn a_dead_cts_defuse_kit_drops_and_another_ct_picks_it_up() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup_with(&bsp, 0, 0.0, quick());
    g.connect_as(Some(Team::Ct), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let mut ev = Vec::new();
    while g.a.round.phase == Phase::Over {
        wait(&mut g, 0.05, Input::default(), Input::default(), &mut ev);
    }
    g.a.buy(net::protocol::BuyItem::DefuseKit);
    wait(&mut g, 0.3, Input::default(), Input::default(), &mut ev);
    assert!(g.a.me_info.defuse_kit, "bought a kit");
    let at = g.server.player(a).unwrap().origin;
    g.server.slay(a);
    wait(&mut g, 0.3, Input::default(), Input::default(), &mut ev);
    assert_eq!(g.b.kits.len(), 1, "the kit lies where A died");
    assert!(!g.b.me_info.defuse_kit);
    g.server.teleport(b, at);
    wait(&mut g, 0.3, Input::default(), Input::default(), &mut ev);
    assert!(g.b.me_info.defuse_kit, "B picked it up");
    assert!(g.b.kits.is_empty());
}
