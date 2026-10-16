//! The lobby over localhost UDP: friends party up by code (custom game, teams A/B) or queue
//! for casual; the lobby reserves the official game server and both get in with tickets.
mod common;
use common::*;
use net::master::{Command, LobbyClient, Master, Mode};
use net::protocol::Team;
use net::server::{Group, Server};
use sim::pmove::Input;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const LOCAL: ([u8; 4], u16) = ([127, 0, 0, 1], 0);

/// A lobby (in its own thread) and an official game server announcing to it.
fn world(bsp: &goldsrc::bsp::Bsp, match_size: usize) -> (Server, SocketAddr) {
    world_with(bsp, match_size, None)
}

fn world_with(bsp: &goldsrc::bsp::Bsp, match_size: usize, reports: Option<std::path::PathBuf>) -> (Server, SocketAddr) {
    let mut m = Master::bind(SocketAddr::from(LOCAL), vec!["tok".into()]).unwrap();
    m.match_size = match_size;
    m.reports = reports;
    let lobby = m.local_addr();
    std::thread::spawn(move || m.run());
    let mut server = Server::new(SocketAddr::from(LOCAL), "test", bsp, assets()).unwrap();
    server.set_config(net::config::MatchConfig { master: lobby.to_string(), official_token: "tok".into(), ..Default::default() });
    (server, lobby)
}

/// Step the server and lobby clients until `done` (or panic after a while).
fn until(server: &mut Server, cs: &mut [&mut LobbyClient], what: &str, done: impl Fn(&[&mut LobbyClient]) -> bool) {
    let start = Instant::now();
    while !done(cs) {
        assert!(start.elapsed() < Duration::from_secs(5), "timed out: {what}");
        server.step();
        for c in cs.iter_mut() {
            c.tick();
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Both join the server they were sent to with their tickets; returns their teams.
fn play(server: &mut Server, bsp: &goldsrc::bsp::Bsp, found: [(SocketAddr, u64); 2]) -> [Team; 2] {
    let mut cs: Vec<net::Client> = found
        .iter()
        .map(|(addr, ticket)| {
            assert_eq!(*addr, server.local_addr());
            let mut c = net::Client::connect(*addr, "p", sim::collision::World::from_bsp(bsp)).unwrap();
            c.ticket = *ticket;
            c
        })
        .collect();
    for _ in 0..256 {
        server.step();
        cs.iter_mut().for_each(|c| _ = c.tick(Input::default()));
        std::thread::sleep(Duration::from_micros(150));
    }
    [cs[0].team(), cs[1].team()]
}

#[test]
fn custom_game_with_a_friend() {
    let Some(bsp) = dust2() else { return };
    let (mut server, lobby) = world(&bsp, 2);
    let (mut a, mut b) = (LobbyClient::new(lobby, "a").unwrap(), LobbyClient::new(lobby, "b").unwrap());
    until(&mut server, &mut [&mut a, &mut b], "sign in", |c| c.iter().all(|c| c.online()));
    // B joins A's party by code; A makes it a custom game and puts B on team B.
    let code = a.state.as_ref().unwrap().code.clone();
    b.send(Command::JoinParty { code: code.to_lowercase() });
    until(&mut server, &mut [&mut a, &mut b], "party", |c| c[0].state.as_ref().unwrap().members.len() == 2);
    let st = a.state.clone().unwrap();
    assert_eq!(st.leader, st.you);
    b.send(Command::Play);
    a.send(Command::SetMode(Mode::Custom));
    until(&mut server, &mut [&mut a, &mut b], "notice", |c| !c[1].notice.is_empty());
    assert_eq!(b.notice, "Only the party leader can do that.");
    let bid = b.state.as_ref().unwrap().you;
    a.send(Command::Move { member: bid, group: Group::B });
    a.send(Command::Play);
    until(&mut server, &mut [&mut a, &mut b], "match found", |c| c.iter().all(|c| c.found.is_some()));
    let teams = play(&mut server, &bsp, [a.found.unwrap(), b.found.unwrap()]);
    assert_eq!(teams, [Team::T, Team::Ct], "A starts T (knife round decides later)");
}

#[test]
fn casual_queue_matches_two_strangers() {
    let Some(bsp) = dust2() else { return };
    let (mut server, lobby) = world(&bsp, 2);
    let (mut a, mut b) = (LobbyClient::new(lobby, "a").unwrap(), LobbyClient::new(lobby, "b").unwrap());
    until(&mut server, &mut [&mut a, &mut b], "sign in", |c| c.iter().all(|c| c.online()));
    a.send(Command::Play);
    until(&mut server, &mut [&mut a, &mut b], "queued", |c| c[0].state.as_ref().unwrap().searching.is_some());
    b.send(Command::Play);
    until(&mut server, &mut [&mut a, &mut b], "match found", |c| c.iter().all(|c| c.found.is_some()));
    let teams = play(&mut server, &bsp, [a.found.unwrap(), b.found.unwrap()]);
    assert!(teams.contains(&Team::T) && teams.contains(&Team::Ct), "{teams:?}");
}

#[test]
fn steam_sign_in_and_lobby_restart() {
    let Some(bsp) = dust2() else { return };
    let (mut server, lobby) = world(&bsp, 2);
    let login = |id: u64| -> Option<Box<dyn FnMut() -> Option<net::master::SteamLogin> + Send + Sync>> { Some(Box::new(move || Some(net::master::SteamLogin { steam_id: id, ticket: vec![1, 2, 3] }))) };
    let mut a = LobbyClient::new(lobby, "a").unwrap();
    a.steam = login(76561197960287930);
    let mut b = LobbyClient::new(lobby, "b").unwrap();
    until(&mut server, &mut [&mut a, &mut b], "sign in", |c| c.iter().all(|c| c.online()));
    // B invites A (in the game, signed in with Steam); A accepts by joining that party.
    b.send(Command::Invite { steam: 76561197960287930 });
    until(&mut server, &mut [&mut a, &mut b], "invite", |c| !c[0].state.as_ref().unwrap().invites.is_empty());
    let inv = a.state.as_ref().unwrap().invites[0].clone();
    assert_eq!((inv.from.as_str(), inv.code.as_str()), ("b", b.state.as_ref().unwrap().code.as_str()));
    a.send(Command::JoinParty { code: inv.code });
    until(&mut server, &mut [&mut a, &mut b], "party", |c| c[1].state.as_ref().unwrap().members.len() == 2);
    let m = &b.state.as_ref().unwrap().members;
    assert_eq!((m[0].steam, m[1].steam), (0, 76561197960287930), "B sees A's Steam id (for the avatar)");
    // A signs in again from somewhere else: the old session ends, B's party is down to B.
    let mut a2 = LobbyClient::new(lobby, "a").unwrap();
    a2.steam = login(76561197960287930);
    until(&mut server, &mut [&mut a2, &mut b], "old session replaced", |c| c[0].online() && c[1].state.as_ref().unwrap().members.len() == 1);
}

#[test]
fn signs_in_again_after_the_lobby_restarts() {
    // A lobby that has never heard of our session answers Expired; the client signs in again.
    let first = Master::bind(SocketAddr::from(LOCAL), vec![]).unwrap();
    let addr = first.local_addr();
    let mut c = LobbyClient::new(addr, "a").unwrap();
    let mut m = first;
    let start = Instant::now();
    while !c.online() {
        assert!(start.elapsed() < Duration::from_secs(5));
        m.step();
        c.tick();
    }
    let code = c.state.as_ref().unwrap().code.clone();
    drop(m);
    let mut m = Master::bind(addr, vec![]).unwrap();
    while c.state.as_ref().unwrap().code == code {
        assert!(start.elapsed() < Duration::from_secs(10), "never signed in again");
        m.step();
        c.tick();
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn queue_books_the_region_players_ping_best() {
    let Some(bsp) = dust2() else { return };
    let (mut eu, lobby) = world(&bsp, 2);
    let mut na = Server::new(SocketAddr::from(LOCAL), "test", &bsp, assets()).unwrap();
    na.set_config(net::config::MatchConfig { master: lobby.to_string(), official_token: "tok".into(), region: "na".into(), ..Default::default() });
    let (mut a, mut b) = (LobbyClient::new(lobby, "a").unwrap(), LobbyClient::new(lobby, "b").unwrap());
    let both = |eu: &mut Server, na: &mut Server, cs: &mut [&mut LobbyClient], what: &str, done: &dyn Fn(&[&mut LobbyClient]) -> bool| {
        let start = Instant::now();
        while !done(cs) {
            assert!(start.elapsed() < Duration::from_secs(5), "timed out: {what}");
            eu.step();
            na.step();
            cs.iter_mut().for_each(|c| c.tick());
            std::thread::sleep(Duration::from_millis(2));
        }
    };
    both(&mut eu, &mut na, &mut [&mut a, &mut b], "both regions listed", &|c| c.iter().all(|c| c.state.as_ref().is_some_and(|s| s.regions.len() == 2)));
    // Let the automatic pings (both ~0 ms on localhost) go out, then say we're far from eu.
    let t = Instant::now();
    both(&mut eu, &mut na, &mut [&mut a, &mut b], "pings", &|_| t.elapsed() > Duration::from_millis(300));
    for c in [&mut a, &mut b] {
        c.send(Command::Pings(vec![("eu".into(), 150), ("na".into(), 25)]));
        c.send(Command::Play);
    }
    both(&mut eu, &mut na, &mut [&mut a, &mut b], "match found", &|c| c.iter().all(|c| c.found.is_some()));
    assert_eq!(a.found.unwrap().0, na.local_addr(), "booked in na, where both ping 25 ms");
}

#[test]
fn reports_are_filed_with_steam_ids() {
    let Some(bsp) = dust2() else { return };
    let path = std::env::temp_dir().join(format!("bootleg16-reports-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let (mut server, lobby) = world_with(&bsp, 2, Some(path.clone()));
    let mut a = LobbyClient::new(lobby, "a").unwrap();
    a.steam = Some(Box::new(|| Some(net::master::SteamLogin { steam_id: 111, ticket: vec![] })));
    let mut b = LobbyClient::new(lobby, "b \"quoted\"").unwrap();
    b.steam = Some(Box::new(|| Some(net::master::SteamLogin { steam_id: 222, ticket: vec![] })));
    until(&mut server, &mut [&mut a, &mut b], "sign in", |c| c.iter().all(|c| c.online()));
    a.send(Command::Play);
    b.send(Command::Play);
    until(&mut server, &mut [&mut a, &mut b], "match found", |c| c.iter().all(|c| c.found.is_some()));
    let mut cs: Vec<net::Client> = [a.found.unwrap(), b.found.unwrap()]
        .iter()
        .map(|(addr, ticket)| {
            let mut c = net::Client::connect(*addr, "p", sim::collision::World::from_bsp(&bsp)).unwrap();
            c.ticket = *ticket;
            c
        })
        .collect();
    for _ in 0..256 {
        server.step();
        cs.iter_mut().for_each(|c| _ = c.tick(Input::default()));
        std::thread::sleep(Duration::from_micros(150));
    }
    let target = cs[1].id().unwrap();
    cs[0].report(target, net::protocol::ReportReason::Cheating);
    cs[0].report(target, net::protocol::ReportReason::Cheating); // once per player
    let start = Instant::now();
    while std::fs::read_to_string(&path).unwrap_or_default().is_empty() {
        assert!(start.elapsed() < Duration::from_secs(5), "no report filed");
        server.step();
        cs.iter_mut().for_each(|c| _ = c.tick(Input::default()));
        std::thread::sleep(Duration::from_millis(2));
    }
    std::thread::sleep(Duration::from_millis(300));
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.lines().count(), 1, "{text}");
    for want in ["\"reason\":\"Cheating\"", "\"steam\":111", "\"steam\":222,\"name\":\"b \\\"quoted\\\"\"", "\"map\":\"test\""] {
        assert!(text.contains(want), "{want} in {text}");
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn official_lobbies_turn_away_players_without_steam() {
    let mut m = Master::bind(SocketAddr::from(LOCAL), vec![]).unwrap();
    m.require_steam = true;
    let mut anon = LobbyClient::new(m.local_addr(), "bot").unwrap();
    let mut steam = LobbyClient::new(m.local_addr(), "player").unwrap();
    steam.steam = Some(Box::new(|| Some(net::master::SteamLogin { steam_id: 7, ticket: vec![] })));
    let start = Instant::now();
    while anon.notice.is_empty() || !steam.online() {
        assert!(start.elapsed() < Duration::from_secs(5));
        m.step();
        anon.tick();
        steam.tick();
    }
    assert_eq!(anon.notice, "Start Steam to play online.");
    assert!(!anon.online());
}

#[test]
fn official_servers_only_take_booked_players() {
    let Some(bsp) = dust2() else { return };
    let (mut server, _lobby) = world(&bsp, 2);
    let mut c = net::Client::connect(server.local_addr(), "walk-in", sim::collision::World::from_bsp(&bsp)).unwrap();
    for _ in 0..256 {
        server.step();
        c.tick(Input::default());
        std::thread::sleep(Duration::from_micros(150));
    }
    assert_eq!(c.status, net::client::Status::Rejected);
    assert!(c.reject_reason.unwrap().contains("lobby"));
}
