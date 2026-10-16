//! Deathmatch, the way players get it: ask the lobby for a seat, drop into the deathmatch
//! server (encrypted, no booking), fight, come back at once at a random spawn, and when
//! someone reaches the kill limit everyone's score starts over. No seat: no way in.
mod common;
use common::*;
use net::bot::{Bot, BotMap, Difficulty, Profile};
use net::master::{Command, LobbyClient, Master};
use net::protocol::GameEvent;
use net::server::{Rules, Server};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const LOCAL: ([u8; 4], u16) = ([127, 0, 0, 1], 0);

#[test]
fn drop_in_fight_respawn_and_the_kill_limit_resets_scores() {
    let Some(bsp) = dust2() else { return };
    let map = BotMap::new(bsp);
    let mut m = Master::bind(SocketAddr::from(LOCAL), vec!["tok".into()]).unwrap();
    let lobby = m.local_addr();
    std::thread::spawn(move || m.run());
    let mut server = Server::new(SocketAddr::from(LOCAL), "dm", &map.bsp, assets()).unwrap();
    server.set_config(net::config::MatchConfig { master: lobby.to_string(), official_token: "tok".into(), deathmatch: true, max_players: 20, ..Default::default() });
    server.set_rules(Rules { kill_limit: 1, ..Rules::deathmatch() });
    // Two players ask for deathmatch and get seats on it.
    let mut cs: Vec<LobbyClient> = (0..2).map(|i| LobbyClient::new(lobby, &format!("p{i}")).unwrap()).collect();
    let mut asked = [false; 2];
    let mut bots: Vec<Bot> = Vec::new();
    let start = Instant::now();
    while bots.len() < 2 {
        assert!(start.elapsed() < Duration::from_secs(10), "no deathmatch seats");
        server.step();
        for (i, c) in cs.iter_mut().enumerate() {
            c.tick();
            if c.online() && !asked[i] {
                asked[i] = true;
                c.send(Command::Deathmatch);
            }
            if let Some((addr, ticket, key)) = c.found.take() {
                assert_eq!(addr, server.local_addr());
                let mut client = net::Client::connect(addr, &format!("p{i}"), map.world()).unwrap();
                client.ticket = ticket;
                client.key = Some(key);
                bots.push(Bot::new(client, map.clone(), i, Profile::of(Difficulty::Expert)));
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    // A walk-in without a seat gets nowhere.
    let mut walk_in = net::Client::connect(server.local_addr(), "x", map.world()).unwrap();
    // Straight onto a team (no team pick), they fight, die, come back.
    let (mut events, mut ticks) = (Vec::new(), 0);
    while !events.iter().any(|e| matches!(e, GameEvent::KillLimit { .. })) {
        ticks += 1;
        assert!(ticks < 128 * 240, "nobody reached the kill limit ({} events)", events.len());
        server.step();
        walk_in.tick(sim::pmove::Input::default());
        for b in bots.iter_mut() {
            b.tick();
        }
        events.extend(bots[0].take_events());
        bots[1].take_events();
        if ticks % 8 == 0 {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    assert!(walk_in.id().is_none(), "no seat, no way in");
    // Back in the fight straight away, and the scores started over.
    for _ in 0..3 * 128 {
        server.step();
        bots.iter_mut().for_each(|b| b.tick());
        std::thread::sleep(Duration::from_micros(150));
    }
    assert!(bots.iter().all(|b| b.client.vitals.alive()), "everyone respawned");
    assert!(bots[0].client.scores.iter().all(|s| s.kills <= 1), "scores started over: {:?}", bots[0].client.scores);
}
