//! A practice match on this machine: our own server running the real match rules (CS2 timers,
//! a short match) on a map, with bots filling both teams around you, 5 v 5. The server and the
//! bots share one thread at the server's tick rate; you join it over localhost like anyone.
use net::bot::{Bot, BotMap, Difficulty, Profile};
use net::protocol::Team;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Players a side.
pub const SIDE: usize = 5;
/// Rounds in a short match (CS2: first to 9, sides swap after 8, 8 all is a draw).
pub const ROUNDS: u32 = 16;
/// The bots' names (no clan tags, no em dashes).
const NAMES: [&str; 2 * SIDE - 1] = ["Ash", "Bishop", "Cole", "Dex", "Echo", "Flint", "Gale", "Hex", "Ivo"];

/// The bots' view of the last map played (working it out takes a moment, so a second
/// match on the same map file starts at once).
static MAPS: Mutex<Option<(u64, Arc<BotMap>)>> = Mutex::new(None);

/// The match's rules: CS2's, with a short match's rounds and no overtime.
pub fn config() -> net::config::MatchConfig {
    net::config::MatchConfig { max_rounds: ROUNDS, overtime_rounds: 0, name: "practice".into(), ..Default::default() }
}

/// Starts the server and its bots on a map (the text of its `.bmap`); you join `you`'s side
/// and the bots take the other nine seats. Returns the server's address and the switch that
/// stops it all.
pub fn start(text: &str, you: Team) -> Result<(SocketAddr, Arc<AtomicBool>), String> {
    start_with(text, you, config())
}

pub fn start_with(text: &str, you: Team, cfg: net::config::MatchConfig) -> Result<(SocketAddr, Arc<AtomicBool>), String> {
    let file = sim::mapfile::MapFile::parse(text)?;
    let assets = net::server::Assets::load(&net::gamefiles::data_dir())?;
    let hash = sim::mapfile::hash(text.as_bytes());
    let mut server = net::Server::with_map(([127, 0, 0, 1], 0).into(), &file, hash, assets).map_err(|e| format!("the practice server didn't start ({e})"))?;
    server.set_config(cfg);
    let addr = SocketAddr::from(([127, 0, 0, 1], server.local_addr().port()));
    let stop = Arc::new(AtomicBool::new(false));
    let s = stop.clone();
    std::thread::spawn(move || run(server, file, hash, addr, you, &s));
    Ok((addr, stop))
}

fn bot_map(file: sim::mapfile::MapFile, hash: u64) -> Arc<BotMap> {
    let mut last = MAPS.lock().unwrap();
    if let Some((h, map)) = &*last {
        if *h == hash {
            return map.clone();
        }
    }
    let map = BotMap::from_map(file, hash);
    *last = Some((hash, map.clone()));
    map
}

/// The server and the bots, a tick at a time, until `stop`. Your join waits in the socket
/// while the bots' map is worked out.
fn run(mut server: net::Server, file: sim::mapfile::MapFile, hash: u64, addr: SocketAddr, you: Team, stop: &AtomicBool) {
    let map = bot_map(file, hash);
    let them = if you == Team::Ct { Team::T } else { Team::Ct };
    let mut bots: Vec<Bot> = NAMES
        .iter()
        .enumerate()
        .filter_map(|(i, name)| {
            let client = net::Client::connect(addr, name, map.world()).ok()?;
            let mut b = Bot::new(client, map.clone(), i, Profile::of(Difficulty::Normal));
            b.name = name.to_string();
            // you take one of your side's seats
            b.client.join_team(Some(if i < SIDE - 1 { you } else { them }));
            Some(b)
        })
        .collect();
    let tick = Duration::from_secs_f64(server.tick_rate().tick() as f64);
    let mut next = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        server.step();
        for b in &mut bots {
            b.tick();
            b.take_events();
        }
        next += tick;
        let now = Instant::now();
        if next > now {
            std::thread::sleep(next - now);
        } else {
            next = now;
        }
    }
    for b in &mut bots {
        b.client.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// You join data_center's practice match: nine bots fill the teams 5 v 5, the round
    /// starts with everyone's start money, and bots buy in freeze time.
    #[test]
    fn a_practice_match_fills_both_teams_with_bots() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../godot/maps/data_center.bmap")).unwrap();
        let cfg = net::config::MatchConfig { freeze_time: 3.0, ..config() };
        let (addr, stop) = start_with(&text, Team::T, cfg).unwrap();
        let world = sim::mapfile::MapFile::parse(&text).unwrap().world();
        let mut me = net::Client::connect(addr, "you", world).unwrap();
        me.join_team(Some(Team::T));
        let start = Instant::now();
        let mut live = false;
        while !live {
            assert!(start.elapsed() < Duration::from_secs(60), "the match never went live: {:?}, {} players", me.round.phase, me.scores.len());
            me.tick(sim::pmove::Input::default());
            live = me.round.phase == net::rounds::Phase::Live;
            std::thread::sleep(Duration::from_millis(8));
        }
        let side = |t| me.scores.iter().filter(|s| s.team == t).count();
        assert_eq!((side(Team::T), side(Team::Ct)), (SIDE, SIDE), "5 v 5");
        assert_eq!(me.team(), Team::T);
        assert_eq!(me.me_info.money, 800, "CS2's start money");
        // the bots on your side bought something in freeze time (their money shows to you)
        assert!(me.scores.iter().any(|s| s.team == Team::T && s.money.is_some_and(|m| m < 800)), "no bot bought: {:?}", me.scores.iter().map(|s| s.money).collect::<Vec<_>>());
        stop.store(true, Ordering::Relaxed);
    }
}
