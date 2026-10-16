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

/// Test players accept found matches straight away.
fn accepting(mut c: LobbyClient) -> LobbyClient {
    c.auto_accept = true;
    c
}

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
fn play(server: &mut Server, bsp: &goldsrc::bsp::Bsp, found: [(SocketAddr, u64, [u8; 32]); 2]) -> [Team; 2] {
    let mut cs: Vec<net::Client> = found
        .iter()
        .map(|(addr, ticket, key)| {
            assert_eq!(*addr, server.local_addr());
            let mut c = net::Client::connect(*addr, "p", sim::collision::World::from_bsp(bsp)).unwrap();
            c.ticket = *ticket;
            c.key = Some(*key);
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
    let (mut a, mut b) = (accepting(LobbyClient::new(lobby, "a").unwrap()), accepting(LobbyClient::new(lobby, "b").unwrap()));
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
    let (mut a, mut b) = (accepting(LobbyClient::new(lobby, "a").unwrap()), accepting(LobbyClient::new(lobby, "b").unwrap()));
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
    let mut a = accepting(LobbyClient::new(lobby, "a").unwrap());
    a.steam = login(76561197960287930);
    let mut b = accepting(LobbyClient::new(lobby, "b").unwrap());
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
    // Someone else claims A's Steam id on this unchecked (trust) lobby: they get in, but
    // without it, and A stays (a claim must never push anyone out).
    let mut a2 = accepting(LobbyClient::new(lobby, "a").unwrap());
    a2.steam = login(76561197960287930);
    until(&mut server, &mut [&mut a2, &mut b], "claimer in", |c| c[0].online());
    assert_eq!(a2.state.as_ref().unwrap().members[0].steam, 0);
    assert_eq!(b.state.as_ref().unwrap().members.len(), 2);
}

#[test]
fn a_checked_steam_sign_in_replaces_the_older_one() {
    // With Steam checks, a second sign-in Steam confirmed ends the first session.
    let mut m = Master::bind(SocketAddr::from(LOCAL), vec![]).unwrap();
    m.steam_mode = net::master::SteamMode::Verify;
    let login = || -> Option<Box<dyn FnMut() -> Option<net::master::SteamLogin> + Send + Sync>> { Some(Box::new(|| Some(net::master::SteamLogin { steam_id: 5, ticket: vec![9] }))) };
    let (mut first, mut second) = (accepting(LobbyClient::new(m.local_addr(), "x").unwrap()), accepting(LobbyClient::new(m.local_addr(), "x").unwrap()));
    first.steam = login();
    second.steam = login();
    let start = Instant::now();
    let mut checked = 0;
    while checked < 2 || !second.online() {
        assert!(start.elapsed() < Duration::from_secs(5));
        m.step();
        first.tick();
        if checked >= 1 {
            second.tick();
        }
        for (session, id, _) in m.take_steam_checks() {
            m.steam_checked(id, Some(session)); // what Steam would say for a good ticket
            checked += 1;
        }
    }
    // The first one is gone: its next poll gets Expired and it signs in afresh.
    let code = first.state.as_ref().unwrap().code.clone();
    while first.state.as_ref().unwrap().code == code {
        assert!(start.elapsed() < Duration::from_secs(10));
        m.step();
        first.tick();
        second.tick();
        let _ = m.take_steam_checks();
    }
}

#[test]
fn answers_never_outgrow_requests() {
    // The biggest state the lobby sends, against the smallest request it answers.
    let long = "W".repeat(24);
    let addr: SocketAddr = "[2001:db8::1]:27015".parse().unwrap();
    let state = net::master::LobbyState {
        you: u32::MAX,
        code: "ABC-DEF".into(),
        leader: u32::MAX,
        mode: net::master::Mode::Custom,
        members: (0..net::master::MAX_CUSTOM).map(|i| net::master::Member { id: u32::MAX - i as u32, name: long.clone(), group: net::server::Group::B, steam: u64::MAX }).collect(),
        searching: Some(1e9),
        found: Some((addr, u64::MAX, [255; 32])),
        rejoin: Some((addr, u64::MAX, [255; 32])),
        ready: Some(net::master::ReadyNet { accepted: 10, total: 10, left: 20.0, you: true }),
        cooldown: Some(1e9),
        // (The lobby only ever lists pool maps.)
        map: Some("de_inferno".into()),
        veto: Some(net::master::Veto { left: net::config::MAP_POOL.iter().map(|m| m.to_string()).collect(), turn: net::server::Group::B }),
        notice: "Too many players from your address, or the lobby is full. Try again later.".into(),
        invites: (0..net::master::MAX_INVITES).map(|_| net::master::Invite { from: long.clone(), steam: u64::MAX, code: "ABC-DEF".into(), left: 30.0 }).collect(),
        regions: (0..net::master::MAX_REGIONS).map(|_| ("x".repeat(16), addr)).collect(),
    };
    let answer = net::protocol::encode(&net::master::LobbyMsg::State(state)).len();
    // Both travel sealed in the session channel, which adds the same to each.
    let request = net::protocol::encode(&net::master::LobbyMsg::Command { cmd: Command::Poll, pad: vec![0; net::master::PAD] }).len();
    assert!(answer <= request, "answer {answer} bytes, request {request}");
}

#[test]
fn signs_in_again_after_the_lobby_restarts() {
    // A lobby that has never heard of our session answers Expired; the client signs in again.
    let first = Master::bind(SocketAddr::from(LOCAL), vec![]).unwrap();
    let addr = first.local_addr();
    let mut c = accepting(LobbyClient::new(addr, "a").unwrap());
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
    let (mut a, mut b) = (accepting(LobbyClient::new(lobby, "a").unwrap()), accepting(LobbyClient::new(lobby, "b").unwrap()));
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
    let mut a = accepting(LobbyClient::new(lobby, "a").unwrap());
    a.steam = Some(Box::new(|| Some(net::master::SteamLogin { steam_id: 111, ticket: vec![] })));
    let mut b = accepting(LobbyClient::new(lobby, "b \"quoted\"").unwrap());
    b.steam = Some(Box::new(|| Some(net::master::SteamLogin { steam_id: 222, ticket: vec![] })));
    until(&mut server, &mut [&mut a, &mut b], "sign in", |c| c.iter().all(|c| c.online()));
    a.send(Command::Play);
    b.send(Command::Play);
    until(&mut server, &mut [&mut a, &mut b], "match found", |c| c.iter().all(|c| c.found.is_some()));
    let mut cs: Vec<net::Client> = [a.found.unwrap(), b.found.unwrap()]
        .iter()
        .map(|(addr, ticket, key)| {
            let mut c = net::Client::connect(*addr, "p", sim::collision::World::from_bsp(&bsp)).unwrap();
            c.ticket = *ticket;
            c.key = Some(*key);
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
    let mut anon = accepting(LobbyClient::new(m.local_addr(), "bot").unwrap());
    let mut steam = accepting(LobbyClient::new(m.local_addr(), "player").unwrap());
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
    // Not let in, and not even answered: official servers take nothing unencrypted (an
    // answer bigger than the walk-in's hello would make us a flood amplifier).
    assert_eq!(c.status, net::client::Status::Connecting);
    assert!(c.id().is_none());
}

#[test]
fn a_seen_ticket_without_its_key_gets_nobody_in() {
    let Some(bsp) = dust2() else { return };
    let (mut server, lobby) = world(&bsp, 2);
    let (mut a, mut b) = (accepting(LobbyClient::new(lobby, "a").unwrap()), accepting(LobbyClient::new(lobby, "b").unwrap()));
    until(&mut server, &mut [&mut a, &mut b], "sign in", |c| c.iter().all(|c| c.online()));
    a.send(Command::Play);
    b.send(Command::Play);
    until(&mut server, &mut [&mut a, &mut b], "match found", |c| c.iter().all(|c| c.found.is_some()));
    let (addr, ticket, key) = a.found.unwrap();
    // Someone who saw A's ticket (but not the key, which never travels in the clear) tries
    // first; then A joins for real.
    let mut thief = net::Client::connect(addr, "thief", sim::collision::World::from_bsp(&bsp)).unwrap();
    thief.ticket = ticket;
    thief.key = Some([7; 32]);
    let mut real = net::Client::connect(addr, "a", sim::collision::World::from_bsp(&bsp)).unwrap();
    real.ticket = ticket;
    real.key = Some(key);
    for t in 0..512 {
        server.step();
        thief.tick(Input::default());
        if t > 256 {
            real.tick(Input::default());
        }
        std::thread::sleep(Duration::from_micros(150));
    }
    assert!(thief.id().is_none(), "the wrong key gets nowhere");
    assert!(real.id().is_some(), "the seat is still A's");
}

/// A lobby stepped by the test itself (to stand in for Steam's answers), with an official
/// server in its pool.
fn manual_world(bsp: &goldsrc::bsp::Bsp) -> (Master, Server) {
    let mut m = Master::bind(SocketAddr::from(LOCAL), vec!["tok".into()]).unwrap();
    m.match_size = 2;
    m.steam_mode = net::master::SteamMode::Verify;
    let mut server = Server::new(SocketAddr::from(LOCAL), "test", bsp, assets()).unwrap();
    server.set_config(net::config::MatchConfig { master: m.local_addr().to_string(), official_token: "tok".into(), ..Default::default() });
    (m, server)
}

fn steam(id: u64) -> Option<Box<dyn FnMut() -> Option<net::master::SteamLogin> + Send + Sync>> {
    Some(Box::new(move || Some(net::master::SteamLogin { steam_id: id, ticket: vec![1] })))
}

fn step_all(m: &mut Master, server: &mut Server, cs: &mut [&mut LobbyClient], secs: u64, what: &str, done: &dyn Fn(&[&mut LobbyClient]) -> bool) {
    let start = Instant::now();
    while !done(cs) {
        assert!(start.elapsed() < Duration::from_secs(secs), "timed out: {what}");
        m.step();
        server.step();
        cs.iter_mut().for_each(|c| c.tick());
        for (session, id, _) in m.take_steam_checks() {
            m.steam_checked(id, Some(session)); // Steam says yes
        }
    }
}

#[test]
fn not_accepting_a_match_costs_a_cooldown_and_the_other_keeps_their_place() {
    let Some(bsp) = dust2() else { return };
    let (mut m, mut server) = manual_world(&bsp);
    let (mut a, mut b) = (LobbyClient::new(m.local_addr(), "a").unwrap(), accepting(LobbyClient::new(m.local_addr(), "b").unwrap()));
    a.steam = steam(1);
    b.steam = steam(2);
    step_all(&mut m, &mut server, &mut [&mut a, &mut b], 5, "sign in", &|c| c.iter().all(|c| c.online()));
    a.send(Command::Play);
    b.send(Command::Play);
    step_all(&mut m, &mut server, &mut [&mut a, &mut b], 5, "match found", &|c| c[0].state.as_ref().unwrap().ready.is_some());
    a.send(Command::Decline);
    step_all(&mut m, &mut server, &mut [&mut a, &mut b], 5, "check over", &|c| c[0].state.as_ref().unwrap().ready.is_none() && c[1].state.as_ref().unwrap().ready.is_none());
    let (sa, sb) = (a.state.clone().unwrap(), b.state.clone().unwrap());
    assert!(sa.cooldown.is_some() && sa.searching.is_none(), "A: out of the queue, cooling down");
    assert!(sb.cooldown.is_none() && sb.searching.is_some(), "B: still queued");
    a.send(Command::Play);
    step_all(&mut m, &mut server, &mut [&mut a, &mut b], 5, "refused", &|c| !c[0].notice.is_empty());
    assert!(a.state.as_ref().unwrap().searching.is_none(), "can't queue while cooling down");
}

#[test]
fn a_restarted_game_is_offered_its_running_match() {
    let Some(bsp) = dust2() else { return };
    let (mut m, mut server) = manual_world(&bsp);
    let (mut a, mut b) = (accepting(LobbyClient::new(m.local_addr(), "a").unwrap()), accepting(LobbyClient::new(m.local_addr(), "b").unwrap()));
    a.steam = steam(1);
    b.steam = steam(2);
    step_all(&mut m, &mut server, &mut [&mut a, &mut b], 5, "sign in", &|c| c.iter().all(|c| c.online()));
    a.send(Command::Play);
    b.send(Command::Play);
    step_all(&mut m, &mut server, &mut [&mut a, &mut b], 5, "booked", &|c| c.iter().all(|c| c.found.is_some()));
    let seat = a.found.unwrap();
    // A's game crashes and starts again: a new sign-in, same Steam account.
    drop(a);
    let mut again = LobbyClient::new(m.local_addr(), "a").unwrap();
    again.steam = steam(1);
    step_all(&mut m, &mut server, &mut [&mut again, &mut b], 5, "offered", &|c| c[0].state.as_ref().is_some_and(|s| s.rejoin.is_some()));
    assert_eq!(again.state.unwrap().rejoin, Some(seat), "the same seat, key and server");
}

#[test]
fn a_custom_game_vetoes_maps_down_to_one_and_plays_it() {
    let Some(bsp) = dust2() else { return };
    let Some(dir) = game_dir() else { return };
    let (mut server, lobby) = world(&bsp, 2);
    server.set_game_dir(dir.into());
    let (mut a, mut b) = (accepting(LobbyClient::new(lobby, "a").unwrap()), accepting(LobbyClient::new(lobby, "b").unwrap()));
    until(&mut server, &mut [&mut a, &mut b], "sign in", |c| c.iter().all(|c| c.online()));
    b.send(Command::JoinParty { code: a.state.as_ref().unwrap().code.clone() });
    until(&mut server, &mut [&mut a, &mut b], "party", |c| c[0].state.as_ref().unwrap().members.len() == 2);
    a.send(Command::SetMode(Mode::Custom));
    let bid = b.state.as_ref().unwrap().you;
    a.send(Command::Move { member: bid, group: Group::B });
    a.send(Command::StartVeto);
    until(&mut server, &mut [&mut a, &mut b], "veto", |c| c[0].state.as_ref().unwrap().veto.is_some());
    // Captains in turn: A, B, A. B can't ban out of turn.
    b.send(Command::Ban("de_dust2".into()));
    until(&mut server, &mut [&mut a, &mut b], "out of turn", |c| !c[1].notice.is_empty());
    for (who, map) in [(0, "de_dust2"), (1, "de_inferno"), (0, "de_nuke")] {
        let c = if who == 0 { &mut a } else { &mut b };
        c.send(Command::Ban(map.into()));
        let left = 3 - ["de_dust2", "de_inferno", "de_nuke"].iter().position(|m| *m == map).unwrap();
        until(&mut server, &mut [&mut a, &mut b], "ban", |c| c[0].state.as_ref().unwrap().veto.as_ref().map_or(1, |v| v.left.len()) == left);
    }
    assert_eq!(a.state.as_ref().unwrap().map.as_deref(), Some("de_train"));
    a.send(Command::Play);
    until(&mut server, &mut [&mut a, &mut b], "match found", |c| c.iter().all(|c| c.found.is_some()));
    // The server switched to the vetoed map; the players are told so when they join.
    let (addr, ticket, key) = a.found.unwrap();
    let mut p = net::Client::connect(addr, "a", sim::collision::World::from_bsp(&bsp)).unwrap();
    p.ticket = ticket;
    p.key = Some(key);
    for _ in 0..256 {
        server.step();
        p.tick(Input::default());
        std::thread::sleep(Duration::from_micros(150));
    }
    assert_eq!(p.map.as_deref(), Some("de_train"));
}

#[test]
fn only_maps_everyone_has_are_offered() {
    let Some(bsp) = dust2() else { return };
    let (mut server, lobby) = world(&bsp, 2);
    let (mut a, mut b) = (LobbyClient::new(lobby, "a").unwrap(), LobbyClient::new(lobby, "b").unwrap());
    // A has the community map de_mirage too; B only the stock maps.
    a.maps = net::config::MAP_POOL.iter().map(|m| m.to_string()).collect();
    b.maps = net::config::STOCK_MAPS.iter().map(|m| m.to_string()).collect();
    until(&mut server, &mut [&mut a, &mut b], "sign in", |c| c.iter().all(|c| c.online()));
    b.send(Command::JoinParty { code: a.state.as_ref().unwrap().code.clone() });
    until(&mut server, &mut [&mut a, &mut b], "party", |c| c[0].state.as_ref().unwrap().members.len() == 2);
    a.send(Command::SetMode(Mode::Custom));
    let bid = b.state.as_ref().unwrap().you;
    a.send(Command::Move { member: bid, group: Group::B });
    a.send(Command::SetMap(Some("de_mirage".into())));
    until(&mut server, &mut [&mut a, &mut b], "refused", |c| !c[0].notice.is_empty());
    assert_eq!(a.notice, "Not everyone in the party has that map.");
    a.send(Command::StartVeto);
    until(&mut server, &mut [&mut a, &mut b], "veto", |c| c[0].state.as_ref().unwrap().veto.is_some());
    let left = a.state.as_ref().unwrap().veto.clone().unwrap().left;
    assert!(!left.contains(&"de_mirage".to_string()) && left.len() == 4, "{left:?}");
}
