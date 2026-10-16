//! Test bots against a real deployment: `bots <lobby host:port> [count] [--local]`.
//! Each bot signs in to the lobby, queues for casual, plays the match it gets in real time and
//! queues again. Needs a test lobby (`./host.sh --test`): official lobbies only let in Steam
//! players. `--local`: reach the game servers through 127.0.0.1 (on the hosting machine).
use net::bot::{Bot, Nav};
use net::master::{Command, LobbyClient};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lobby = std::net::ToSocketAddrs::to_socket_addrs(args.get(1).map(String::as_str).unwrap_or("127.0.0.1:27010")).expect("lobby address").next().unwrap();
    let count: usize = args.get(2).and_then(|n| n.parse().ok()).unwrap_or(10);
    let local = args.iter().any(|a| a == "--local");
    let dir = std::env::var("BOOTLEG16_DIR").unwrap_or_else(|_| format!("{}/.steam/steam/steamapps/common/Half-Life", std::env::var("HOME").unwrap_or_default()));
    let bsp = goldsrc::bsp::Bsp::load(format!("{dir}/cstrike/maps/de_dust2.bsp")).expect("de_dust2.bsp (set BOOTLEG16_DIR)");
    let world = sim::collision::World::from_bsp(&bsp);
    let nav = Arc::new(Nav::of_map(&bsp, &world));
    let spawns = net::bot::spawns_of(&bsp);
    let mut lobbies: Vec<LobbyClient> = (0..count).map(|i| LobbyClient::new(lobby, &format!("bot{i}")).unwrap()).collect();
    let mut queued = vec![false; count];
    let mut bots: Vec<Option<Bot>> = (0..count).map(|_| None).collect();
    let tick = Duration::from_secs_f64(1.0 / 128.0);
    let mut next = Instant::now();
    let mut matches = 0;
    loop {
        for i in 0..count {
            lobbies[i].tick();
            if !lobbies[i].notice.is_empty() {
                println!("bot{i}: {}", std::mem::take(&mut lobbies[i].notice));
            }
            if bots[i].as_ref().is_some_and(|b| b.client.match_ended) {
                bots[i] = None;
                queued[i] = false;
                matches += 1;
                if i == 0 {
                    println!("match over ({matches} bot-matches played)");
                }
            }
            if bots[i].is_none() && lobbies[i].online() && !queued[i] {
                queued[i] = true;
                lobbies[i].send(Command::Play);
            }
            if let Some((mut addr, ticket)) = lobbies[i].found.take() {
                if local {
                    addr.set_ip([127, 0, 0, 1].into());
                }
                let mut client = net::Client::connect(addr, &format!("bot{i}"), sim::collision::World::from_bsp(&bsp)).unwrap();
                client.ticket = ticket;
                if i == 0 {
                    println!("bots booked on {addr}");
                }
                bots[i] = Some(Bot::new(client, nav.clone(), spawns, i as u64 + 1));
            }
            if let Some(b) = &mut bots[i] {
                b.tick();
                b.client.take_events();
            }
        }
        next += tick;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
}
