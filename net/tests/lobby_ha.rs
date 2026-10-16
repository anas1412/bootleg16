//! The lobby never being a single point of failure: a restarted lobby picks up exactly where
//! it was (sessions, parties), and a standby lobby on another machine takes over when the
//! active one dies; players' games switch over by themselves and carry on, still encrypted.
use net::crypto::LobbyKey;
use net::master::{Command, LobbyClient, Master, Role};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

const LOCAL: ([u8; 4], u16) = ([127, 0, 0, 1], 0);

fn run(ms: &mut [&mut Master], cs: &mut [&mut LobbyClient], secs: u64, what: &str, done: impl Fn(&[&mut Master], &[&mut LobbyClient]) -> bool) {
    let start = Instant::now();
    while !done(ms, cs) {
        assert!(start.elapsed() < Duration::from_secs(secs), "timed out: {what}");
        for m in ms.iter_mut() {
            m.step();
        }
        for c in cs.iter_mut() {
            c.tick();
        }
    }
}

fn code(c: &LobbyClient) -> String {
    c.state.as_ref().map(|s| s.code.clone()).unwrap_or_default()
}

#[test]
fn a_restarted_lobby_carries_on() {
    let file = std::env::temp_dir().join(format!("bootleg16-lobby-{}.state", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let key = LobbyKey::generate().to_hex();
    let mut m = Master::bind(SocketAddr::from(LOCAL), vec![]).unwrap();
    let addr = m.local_addr();
    m.set_key(LobbyKey::from_hex(&key).unwrap());
    m.state_file = Some(file.clone());
    let (mut a, mut b) = (LobbyClient::new(addr, "a").unwrap(), LobbyClient::new(addr, "b").unwrap());
    run(&mut [&mut m], &mut [&mut a, &mut b], 5, "sign in", |_, c| c.iter().all(|c| c.online()));
    b.send(Command::JoinParty { code: code(&a) });
    run(&mut [&mut m], &mut [&mut a, &mut b], 5, "party", |_, c| c[0].state.as_ref().unwrap().members.len() == 2);
    let party = code(&a);
    let wait = Instant::now();
    run(&mut [&mut m], &mut [&mut a, &mut b], 5, "saved", |_, _| wait.elapsed() > Duration::from_millis(2500));
    // The lobby dies and comes back (same key and saved state, same address).
    drop(m);
    let mut m = Master::bind(addr, vec![]).unwrap();
    m.set_key(LobbyKey::from_hex(&key).unwrap());
    assert!(m.load(&file));
    // Same party, nobody signed in again (a new sign-in would mean a new party code).
    let wait = Instant::now();
    run(&mut [&mut m], &mut [&mut a, &mut b], 5, "carry on", |_, _| wait.elapsed() > Duration::from_secs(2));
    assert_eq!(code(&a), party);
    assert_eq!(a.state.as_ref().unwrap().members.len(), 2);
    let _ = std::fs::remove_file(&file);
}

#[test]
fn a_standby_lobby_takes_over_when_the_active_one_dies() {
    let key = LobbyKey::generate().to_hex();
    let token = vec!["tok".to_string()];
    let (mut one, mut two) = (Master::bind(SocketAddr::from(LOCAL), token.clone()).unwrap(), Master::bind(SocketAddr::from(LOCAL), token).unwrap());
    // Both lobbies have the same key (the one built into the game).
    one.set_key(LobbyKey::from_hex(&key).unwrap());
    two.set_key(LobbyKey::from_hex(&key).unwrap());
    let ports = (free_port(), free_port());
    one.pair(ports.0, SocketAddr::from(([127, 0, 0, 1], ports.1))).unwrap();
    two.pair(ports.1, SocketAddr::from(([127, 0, 0, 1], ports.0))).unwrap();
    let lobbies = vec![one.local_addr(), two.local_addr()];
    // One becomes active, the other stands by.
    run(&mut [&mut one, &mut two], &mut [], 15, "roles", |m, _| (m[0].role == Role::Active) != (m[1].role == Role::Active));
    let one_active = one.role == Role::Active;
    let (mut a, mut b) = (LobbyClient::with_lobbies(lobbies.clone(), "a").unwrap(), LobbyClient::with_lobbies(lobbies, "b").unwrap());
    run(&mut [&mut one, &mut two], &mut [&mut a, &mut b], 15, "sign in", |_, c| c.iter().all(|c| c.online()));
    b.send(Command::JoinParty { code: code(&a) });
    run(&mut [&mut one, &mut two], &mut [&mut a, &mut b], 10, "party", |_, c| c[0].state.as_ref().unwrap().members.len() == 2);
    let party = code(&a);
    let wait = Instant::now();
    run(&mut [&mut one, &mut two], &mut [&mut a, &mut b], 10, "standby in sync", |_, _| wait.elapsed() > Duration::from_secs(3));
    // The active lobby dies.
    let mut survivor = if one_active { drop(one); two } else { drop(two); one };
    run(&mut [&mut survivor], &mut [&mut a, &mut b], 20, "take over", |m, _| m[0].role == Role::Active);
    // The players' games find it and carry on: same party, same session.
    b.send(Command::LeaveParty);
    let leave_at = Instant::now();
    run(&mut [&mut survivor], &mut [&mut a, &mut b], 20, "players carry on", |_, c| leave_at.elapsed() > Duration::from_secs(1) && c[0].state.as_ref().is_some_and(|s| s.members.len() == 1) && code(&c[0]) == party);
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}
