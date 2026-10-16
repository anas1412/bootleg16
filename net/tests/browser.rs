//! Server list over localhost UDP: game servers announce to the master, the game fetches the
//! list with pings, and matchmaking picks an official server.
mod common;
use common::*;
use std::net::SocketAddr;
use std::time::Duration;

#[test]
fn servers_show_up_in_the_list_and_matchmaking_picks_an_official_one() {
    let Some(bsp) = dust2() else { return };
    let mut master = net::master::Master::bind(SocketAddr::from(([127, 0, 0, 1], 0)), vec!["secret".into()]).unwrap();
    let m = master.local_addr();
    let mk = |name: &str, token: &str| {
        let mut s = net::Server::new(SocketAddr::from(([127, 0, 0, 1], 0)), "de_dust2", &bsp, assets()).unwrap();
        s.set_config(net::config::MatchConfig { name: name.into(), master: m.to_string(), official_token: token.into(), ..Default::default() });
        s
    };
    let mut official = mk("official eu 1", "secret");
    let mut community = mk("someone's server", "wrong");
    // A few ticks: both send their first heartbeat; the master takes them in.
    for _ in 0..4 {
        official.step();
        community.step();
    }
    master.step();
    // Fetch on another thread while the servers keep running (they answer the pings).
    let fetch = std::thread::spawn(move || net::browser::fetch(m, Duration::from_millis(800)).unwrap());
    let t = std::time::Instant::now();
    while t.elapsed() < Duration::from_secs(1) {
        master.step();
        official.step();
        community.step();
        std::thread::sleep(Duration::from_millis(2));
    }
    let list = fetch.join().unwrap();
    assert_eq!(list.len(), 2, "{list:?}");
    assert!(list.iter().all(|l| l.ping_ms.is_some() && l.compatible), "{list:?}");
    let pick = net::browser::find_match(&list).expect("an official server");
    assert_eq!(pick.entry.name, "official eu 1");
    assert!(list.iter().any(|l| l.entry.name == "someone's server" && !l.entry.official));
}
