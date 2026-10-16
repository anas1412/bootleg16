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
    let mut m = Master::bind(SocketAddr::from(LOCAL), vec!["tok".into()]).unwrap();
    m.match_size = match_size;
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
