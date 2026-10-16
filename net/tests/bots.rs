//! A whole match with test bots, the way players get one: 10 bots queue for casual on the
//! lobby, get booked onto the official server, play a short match to the end (buying,
//! fighting, rounds, halftime) and are sent home; the server is free again afterwards.
mod common;
use common::*;
use net::bot::Bot;
use net::master::{Command, LobbyClient, Master};
use net::protocol::GameEvent;
use net::server::Server;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const LOCAL: ([u8; 4], u16) = ([127, 0, 0, 1], 0);

#[test]
fn ten_bots_play_a_whole_match() {
    let Some(bsp) = dust2() else { return };
    let mut m = Master::bind(SocketAddr::from(LOCAL), vec!["tok".into()]).unwrap();
    let lobby = m.local_addr();
    std::thread::spawn(move || m.run());
    let mut server = Server::new(SocketAddr::from(LOCAL), "test", &bsp, assets()).unwrap();
    // A short match: 4 rounds (sides swap after 2), quick timers.
    server.set_config(net::config::MatchConfig {
        master: lobby.to_string(),
        official_token: "tok".into(),
        max_rounds: 4,
        overtime_rounds: 0,
        freeze_time: 1.0,
        buy_time: 10.0,
        round_time: 25.0,
        round_end_delay: 1.0,
        ..Default::default()
    });
    let spawns = net::bot::spawns_of(&bsp);
    let built = Instant::now();
    let nav = std::sync::Arc::new(net::bot::Nav::of_map(&bsp, &sim::collision::World::from_bsp(&bsp)));
    println!("nav: {} points in {:.1} s", nav.points.len(), built.elapsed().as_secs_f32());
    let mut queue: Vec<LobbyClient> = (0..10).map(|i| LobbyClient::new(lobby, &format!("bot{i}")).unwrap()).collect();
    let mut bots: Vec<Bot> = Vec::new();
    let mut events = Vec::new();
    let start = Instant::now();
    let mut asked = vec![false; 10];
    // Queue up and get booked.
    while bots.len() < 10 {
        assert!(start.elapsed() < Duration::from_secs(20), "not all bots got a match");
        server.step();
        for (i, c) in queue.iter_mut().enumerate() {
            c.tick();
            if c.online() && !asked[i] {
                asked[i] = true;
                c.send(Command::Play);
            }
            if let Some((addr, ticket)) = c.found.take() {
                let mut client = net::Client::connect(addr, &format!("bot{i}"), sim::collision::World::from_bsp(&bsp)).unwrap();
                client.ticket = ticket;
                bots.push(Bot::new(client, nav.clone(), spawns, i as u64 + 1));
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    // Play it out (simulated time: as fast as the machine goes).
    let mut ticks = 0u32;
    while !bots.iter().all(|b| b.client.match_ended) {
        ticks += 1;
        assert!(ticks < 128 * 60 * 6, "the match didn't end in 6 game minutes ({} rounds, {} events, status {:?} {:?}, team {:?})", bots[0].client.round.round, events.len(), bots[0].client.status, bots[0].client.reject_reason, bots[0].client.team());
        server.step();
        for b in bots.iter_mut() {
            b.tick();
        }
        if std::env::var_os("BOTS_DEBUG").is_some() && ticks % (128 * 10) == 0 {
            let teams: Vec<_> = bots.iter().map(|b| format!("{:?}", b.client.team())).collect();
            println!("t={}s phase {:?} stage {:?} round {} players {} teams {teams:?}", ticks / 128, server.round_phase(), bots[0].client.round.stage, bots[0].client.round.round, server.player_count());
            let at: Vec<_> = bots.iter().map(|b| { let o = b.client.me().origin; format!("({:.0},{:.0})", o[0], o[1]) }).collect();
            println!("  at {at:?} sees {:?}", bots.iter().map(|b| b.client.others().len()).collect::<Vec<_>>());
        }
        events.extend(bots[0].client.take_events());
        bots[1..].iter_mut().for_each(|b| _ = b.client.take_events());
        if ticks % 8 == 0 {
            std::thread::sleep(Duration::from_micros(200));
        }
    }
    let kills = events.iter().filter(|e| matches!(e, GameEvent::Kill { .. })).count();
    let rounds = events.iter().filter(|e| matches!(e, GameEvent::RoundEnd { .. })).count();
    println!("{rounds} rounds, {kills} kills, {:.0} game seconds", ticks as f32 / 128.0);
    assert!(events.iter().any(|e| matches!(e, GameEvent::MatchOver { .. })));
    assert!(rounds >= 3, "{rounds} rounds");
    assert!(kills > 0, "bots should fight");
    assert!(!server.is_reserved(), "server free for the next match");
}
