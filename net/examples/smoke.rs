//! Smoke test for a deployment: two fake players party up on the lobby, start a custom game,
//! and join the server it books. `cargo run -p net --example smoke -- <lobby-host:port> [--local]`
//! (`--local`: join through 127.0.0.1, for testing on the hosting machine behind a home router).
use net::lobby::{Command, LobbyClient, Mode};
use std::time::{Duration, Instant};

fn main() {
    let lobby = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:27010".into());
    let lobby = std::net::ToSocketAddrs::to_socket_addrs(&lobby).expect("lobby address").next().unwrap();
    let mut a = LobbyClient::new(lobby, "smoke-a").unwrap();
    let mut b = LobbyClient::new(lobby, "smoke-b").unwrap();
    let wait = |a: &mut LobbyClient, b: &mut LobbyClient, what: &str, done: &dyn Fn(&LobbyClient, &LobbyClient) -> bool| {
        let start = Instant::now();
        while !done(a, b) {
            assert!(start.elapsed() < Duration::from_secs(10), "FAILED: {what} (notice: {:?} {:?})", a.notice, b.notice);
            a.tick();
            b.tick();
            std::thread::sleep(Duration::from_millis(10));
        }
        println!("ok: {what}");
    };
    wait(&mut a, &mut b, "lobby answers", &|a, b| a.online() && b.online());
    b.send(Command::JoinParty { code: a.state.as_ref().unwrap().code.clone() });
    wait(&mut a, &mut b, "party by code", &|a, _| a.state.as_ref().unwrap().members.len() == 2);
    a.send(Command::SetMode(Mode::Custom));
    let bid = b.state.as_ref().unwrap().you;
    a.send(Command::Move { member: bid, group: net::server::Group::B });
    a.send(Command::Play);
    wait(&mut a, &mut b, "match booked", &|a, b| a.found.is_some() && b.found.is_some());
    let mut players: Vec<net::Client> = [a.found.unwrap(), b.found.unwrap()]
        .iter()
        .map(|(addr, ticket, key)| {
            let local = std::env::args().any(|a| a == "--local");
            let addr = if local { std::net::SocketAddr::from(([127, 0, 0, 1], addr.port())) } else { *addr };
            let mut c = net::Client::connect(addr, "smoke", sim::collision::World::flat_floor(-100_000.0)).unwrap();
            c.ticket = *ticket;
            c.key = Some(*key);
            c
        })
        .collect();
    let start = Instant::now();
    while players.iter().any(|c| c.id().is_none() || c.team() == net::protocol::Team::Spectator) {
        assert!(start.elapsed() < Duration::from_secs(10), "FAILED: joining {} ({:?})", a.found.unwrap().0, players.iter().map(|c| &c.status).collect::<Vec<_>>());
        players.iter_mut().for_each(|c| _ = c.tick(sim::pmove::Input::default()));
        std::thread::sleep(Duration::from_millis(8));
    }
    println!("ok: both joined {} as {:?} / {:?}", a.found.unwrap().0, players[0].team(), players[1].team());
    players.iter_mut().for_each(|c| c.disconnect());
}
