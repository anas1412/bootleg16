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
