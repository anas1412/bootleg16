//! Test bots against a real deployment: `bots <lobby host:port> [count] [--local]
//! [--difficulty easy|normal|hard|expert] [--deathmatch]`. Each bot signs in to the lobby, queues for
//! casual, plays the match it gets in real time and queues again. Needs a test lobby
//! (`./host.sh --test`): official lobbies only let in Steam players. `--local`: reach the game
//! servers through 127.0.0.1 (on the hosting machine).
//!
//! Orders (see net/src/bot/control.rs): type them into its terminal, or from anywhere on the
//! same machine: `bots --order "t plant b; ct hold"` (sent to UDP 127.0.0.1:27099).
use net::bot::{control, Bot, BotMap, Mode, Order, Profile};
use net::lobby::{Command, LobbyClient};
use std::net::UdpSocket;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

const CONTROL_PORT: u16 = 27099;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--order") {
        let order = args.get(i + 1).expect("--order \"<who> <order>\"");
        UdpSocket::bind("127.0.0.1:0").unwrap().send_to(order.as_bytes(), ("127.0.0.1", CONTROL_PORT)).unwrap();
        return;
    }
    let lobby = std::net::ToSocketAddrs::to_socket_addrs(args.get(1).map(String::as_str).unwrap_or("127.0.0.1:27010")).expect("lobby address").next().unwrap();
    let count: usize = args.get(2).and_then(|n| n.parse().ok()).unwrap_or(10);
    let local = args.iter().any(|a| a == "--local");
    // Deathmatch: take seats on the deathmatch server instead of queueing for matches.
    let deathmatch = args.iter().any(|a| a == "--deathmatch");
    let difficulty = args.iter().position(|a| a == "--difficulty").and_then(|i| args.get(i + 1)).and_then(|d| Profile::parse(d)).unwrap_or(net::bot::Difficulty::Normal);
    let dir = net::gamefiles::data_dir().to_string_lossy().into_owned();
    // Each map's navigation, worked out the first time a match is on it.
    let mut maps: std::collections::HashMap<String, std::sync::Arc<BotMap>> = std::collections::HashMap::new();
    let mut load = |name: &str| -> Option<std::sync::Arc<BotMap>> {
        if !maps.contains_key(name) {
            // our own map format first, else a GoldSrc BSP
            let map = match sim::mapfile::MapFile::load(std::path::Path::new(&format!("{dir}/maps/{name}.bmap"))) {
                Ok((map, hash)) => BotMap::from_map(map, hash),
                Err(_) => BotMap::new(goldsrc::bsp::Bsp::load(format!("{dir}/maps/{name}.bsp")).ok()?, name),
            };
            maps.insert(name.to_string(), map);
        }
        maps.get(name).cloned()
    };

    // Orders from the terminal and from the control port.
    let (tx, orders) = channel::<String>();
    let stdin_tx = tx.clone();
    std::thread::spawn(move || {
        for line in std::io::stdin().lines().map_while(Result::ok) {
            let _ = stdin_tx.send(line);
        }
    });
    if let Ok(udp) = UdpSocket::bind(("127.0.0.1", CONTROL_PORT)) {
        std::thread::spawn(move || {
            let mut buf = [0u8; 512];
            while let Ok(n) = udp.recv(&mut buf) {
                let _ = tx.send(String::from_utf8_lossy(&buf[..n]).trim().to_string());
            }
        });
        println!("orders: type them here, or send to udp 127.0.0.1:{CONTROL_PORT}");
    }

    let mut lobbies: Vec<LobbyClient> = (0..count)
        .map(|i| {
            let mut c = LobbyClient::new(lobby, &format!("bot{i}")).unwrap();
            c.auto_accept = true; // bots accept every match they're found
            c.maps = net::config::pool_maps_in(std::path::Path::new(&dir));
            c
        })
        .collect();
    let mut queued = vec![false; count];
    let mut bots: Vec<Option<Bot>> = (0..count).map(|_| None).collect();
    // Connecting: until the server says which map, a bot can't find its way yet.
    let mut joining: Vec<Option<net::Client>> = (0..count).map(|_| None).collect();
    // Standing orders, kept for bots between matches.
    let mut modes = vec![Mode::Normal; count];
    let mut profiles = vec![Profile::of(difficulty); count];
    let mut sites = vec![None; count];
    let tick = Duration::from_secs_f64(1.0 / 128.0);
    let mut next = Instant::now();
    let mut matches = 0;
    let mut kills = 0;
    loop {
        while let Ok(line) = orders.try_recv() {
            for part in line.split(';').map(str::trim).filter(|p| !p.is_empty()) {
                match control::parse(part) {
                    Ok((who, order)) => {
                        let mut n = 0;
                        for (i, slot) in bots.iter_mut().enumerate() {
                            let hit = match slot {
                                Some(b) => b.is(&who),
                                None => matches!(who, net::bot::Who::All) || who == net::bot::Who::Bot(i),
                            };
                            if !hit {
                                continue;
                            }
                            n += 1;
                            match &order {
                                Order::Mode(m) => modes[i] = *m,
                                Order::Difficulty(d) => profiles[i] = Profile::of(*d),
                                Order::Site(s) => sites[i] = *s,
                                _ => {}
                            }
                            if let Some(b) = slot {
                                b.order(order.clone());
                            }
                        }
                        println!("{part}: {n} bot(s)");
                    }
                    Err(e) => println!("{part}: {e}"),
                }
            }
        }
        for i in 0..count {
            lobbies[i].tick();
            if !lobbies[i].notice.is_empty() {
                println!("bot{i}: {}", std::mem::take(&mut lobbies[i].notice));
            }
            if let Some(b) = bots[i].as_ref().filter(|b| b.client.status == net::client::Status::Rejected) {
                println!("bot{i} refused by the server: {}", b.client.reject_reason.clone().unwrap_or_default());
                bots[i] = None;
                queued[i] = false;
                continue;
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
                lobbies[i].send(if deathmatch { Command::Deathmatch } else { Command::Play });
            }
            if let Some((mut addr, ticket, key)) = lobbies[i].found.take() {
                if local {
                    addr.set_ip([127, 0, 0, 1].into());
                }
                let mut client = net::Client::connect(addr, &format!("bot{i}"), sim::collision::World::flat_floor(-100_000.0)).unwrap();
                client.ticket = ticket;
                client.key = Some(key);
                if i == 0 {
                    println!("bots booked on {addr}");
                }
                joining[i] = Some(client);
            }
            if let Some(c) = &mut joining[i] {
                c.tick(sim::pmove::Input::default());
                if let Some(name) = c.map.clone() {
                    let mut client = joining[i].take().unwrap();
                    match load(&name) {
                        Some(map) => {
                            client.world = map.world();
                            if i == 0 {
                                println!("map: {name}");
                            }
                            let mut b = Bot::new(client, map, i, profiles[i]);
                            b.mode = modes[i];
                            b.site = sites[i];
                            bots[i] = Some(b);
                        }
                        None => println!("bot{i}: no {name}.bmap or .bsp here (BOOTLEG16_DIR)"),
                    }
                }
            }
            if let Some(b) = &mut bots[i] {
                b.tick();
                for e in b.take_events() {
                    // One bot reports the match as it goes.
                    if i == 0 {
                        report(&e, &mut kills);
                    }
                }
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

/// A line per round (and per plant) from what one bot saw.
fn report(e: &net::protocol::GameEvent, kills: &mut u32) {
    use net::protocol::GameEvent;
    match e {
        GameEvent::Kill { .. } => {
            *kills += 1;
            if *kills % 10 == 0 {
                println!("{kills} kills so far");
            }
        }
        GameEvent::KillLimit { player } => println!("player {player} reached the kill limit: scores start over"),
        GameEvent::BombPlanted { at, .. } => println!("bomb planted at {:.0} {:.0}", at[0], at[1]),
        GameEvent::RoundEnd { winner, reason } => {
            println!("round over: {winner:?} win ({reason:?}), {kills} kills");
            *kills = 0;
        }
        GameEvent::MatchOver { winner } => println!("match over: {winner:?}"),
        _ => {}
    }
}
