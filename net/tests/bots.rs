//! Matches with test bots, the way players get one: 10 bots queue for casual on the lobby
//! and get booked onto the official server. Then: a whole short match to the end; a round
//! where the bomb is taken to A, planted and defused (on orders); bots leaving pause the
//! match and coming back resumes it.
mod common;
use common::*;
use net::bot::{Bot, BotMap, Difficulty, Profile};
use net::master::{Command, LobbyClient, Master};
use net::protocol::{GameEvent, Team};
use net::server::Server;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Test players accept found matches straight away.
fn accepting(mut c: LobbyClient) -> LobbyClient {
    c.auto_accept = true;
    c
}

const LOCAL: ([u8; 4], u16) = ([127, 0, 0, 1], 0);

/// A short match: 4 rounds (sides swap after 2), quick timers.
fn short(lobby: SocketAddr) -> net::config::MatchConfig {
    net::config::MatchConfig { master: lobby.to_string(), official_token: "tok".into(), max_rounds: 4, overtime_rounds: 0, freeze_time: 1.0, buy_time: 10.0, round_time: 40.0, round_end_delay: 1.0, ..Default::default() }
}

/// A lobby, an official server and 10 bots booked onto it.
fn booked(map: &Arc<BotMap>, cfg: impl Fn(SocketAddr) -> net::config::MatchConfig) -> (Server, Vec<Bot>) {
    let mut m = Master::bind(SocketAddr::from(LOCAL), vec!["tok".into()]).unwrap();
    let lobby = m.local_addr();
    std::thread::spawn(move || m.run());
    let mut server = Server::new(SocketAddr::from(LOCAL), "test", &map.bsp, assets()).unwrap();
    server.set_config(cfg(lobby));
    let mut queue: Vec<LobbyClient> = (0..10).map(|i| accepting(LobbyClient::new(lobby, &format!("bot{i}")).unwrap())).collect();
    let mut bots: Vec<Bot> = Vec::new();
    let mut asked = [false; 10];
    let start = Instant::now();
    while bots.len() < 10 {
        assert!(start.elapsed() < Duration::from_secs(20), "not all bots got a match: {} online, {} booked, notices {:?}", queue.iter().filter(|c| c.online()).count(), bots.len(), queue.iter().map(|c| c.notice.clone()).collect::<Vec<_>>());
        server.step();
        for (i, c) in queue.iter_mut().enumerate() {
            c.tick();
            if c.online() && !asked[i] {
                asked[i] = true;
                c.send(Command::Play);
            }
            if let Some((addr, ticket, key)) = c.found.take() {
                let mut client = net::Client::connect(addr, &format!("bot{i}"), map.world()).unwrap();
                client.ticket = ticket;
                client.key = Some(key);
                bots.push(Bot::new(client, map.clone(), i, Profile::of(Difficulty::Normal)));
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    (server, bots)
}

/// Run (simulated time, as fast as the machine goes) until `done` or `secs` of game time.
fn play(server: &mut Server, bots: &mut [Bot], secs: u32, events: &mut Vec<GameEvent>, done: impl Fn(&[Bot], &[GameEvent]) -> bool) -> f32 {
    let mut ticks = 0u32;
    while !done(bots, events) {
        ticks += 1;
        assert!(ticks < secs * 128, "not done in {secs} game seconds (round {}, {} events)", bots[0].client.round.round, events.len());
        server.step();
        for b in bots.iter_mut() {
            b.tick();
        }
        events.extend(bots[0].take_events());
        bots[1..].iter_mut().for_each(|b| _ = b.take_events());
        if ticks % 8 == 0 {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    ticks as f32 / 128.0
}

#[test]
fn ten_bots_play_a_whole_match() {
    let Some(bsp) = dust2() else { return };
    let map = BotMap::new(bsp);
    let (mut server, mut bots) = booked(&map, short);
    let mut events = Vec::new();
    let secs = play(&mut server, &mut bots, 6 * 60, &mut events, |b, _| b.iter().all(|b| b.client.match_ended));
    let count = |f: fn(&GameEvent) -> bool| events.iter().filter(|e| f(e)).count();
    let (kills, rounds, plants) = (count(|e| matches!(e, GameEvent::Kill { .. })), count(|e| matches!(e, GameEvent::RoundEnd { .. })), count(|e| matches!(e, GameEvent::BombPlanted { .. })));
    println!("{rounds} rounds, {kills} kills, {plants} plants, {secs:.0} game seconds");
    assert!(events.iter().any(|e| matches!(e, GameEvent::MatchOver { .. })));
    assert!(rounds >= 3 && kills > 0, "{rounds} rounds, {kills} kills");
    assert!(!server.is_reserved(), "server free for the next match");
}

#[test]
fn bomb_goes_to_a_and_gets_defused() {
    let Some(bsp) = dust2() else { return };
    let map = BotMap::new(bsp);
    let (mut server, mut bots) = booked(&map, short);
    // Nobody shoots: the terrorists take the bomb to A, the counter-terrorists come to defuse.
    net::bot::command(&mut bots, "all peaceful; all plant a").unwrap();
    let mut events = Vec::new();
    play(&mut server, &mut bots, 90, &mut events, |_, e| e.iter().any(|e| matches!(e, GameEvent::BombPlanted { .. })));
    let at = events.iter().find_map(|e| if let GameEvent::BombPlanted { at, .. } = e { Some(*at) } else { None }).unwrap();
    let a = map.sites[0];
    assert!(((at[0] - a[0]).powi(2) + (at[1] - a[1]).powi(2)).sqrt() < 600.0, "planted at {at:?}, A is {a:?}");
    play(&mut server, &mut bots, 60, &mut events, |_, e| e.iter().any(|e| matches!(e, GameEvent::BombDefused { .. } | GameEvent::BombExploded { .. })));
    assert!(events.iter().any(|e| matches!(e, GameEvent::BombDefused { .. })), "a counter-terrorist should make it in time from CT spawn to A");
}

#[test]
fn bots_leaving_pause_the_match_and_coming_back_resumes_it() {
    let Some(bsp) = dust2() else { return };
    let map = BotMap::new(bsp);
    let (mut server, mut bots) = booked(&map, short);
    let mut events = Vec::new();
    play(&mut server, &mut bots, 30, &mut events, |b, _| b[0].client.round.round >= 1 && b[0].client.round.phase == net::rounds::Phase::Live);
    let t = bots.iter().find(|b| b.client.team() == Team::T).unwrap().index;
    // (Generous limits: with every test suite running at once the machine is busy.)
    net::bot::command(&mut bots, "ct leave").unwrap();
    play(&mut server, &mut bots, 30, &mut events, |b, _| b[t].client.round.paused);
    net::bot::command(&mut bots, "ct rejoin").unwrap();
    play(&mut server, &mut bots, 30, &mut events, |b, _| !b[t].client.round.paused);
}
