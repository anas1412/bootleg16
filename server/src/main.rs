//! Dedicated server. Usage: `server [map] [port] [--config server.cfg] [--give ak47,awp] [--tick 64]`
//! (defaults: de_dust2, 27015). Match rules come from the config file (see `config/server.cfg`);
//! without `--config`, `server.cfg` in the current directory is used if it exists.
//! Reads the map from `maps/<map>.bmap` (ours, built in Godot) or `maps/<map>.bsp` under the
//! game data folder (`BOOTLEG16_DIR`).
//! `--give` hands every player extra weapons on spawn.
//! `--practice <dummies>` runs the practice room (infinite ammo, quick respawn, standing dummies).
//! There are no bots: matches are people only.
//! `--play <recording.bdem> [port] [--speed N] [--from TICK]`: replay a match recording; join
//! it with the game (`client --connect 127.0.0.1:<port>`) to watch it as a spectator.
//! `--reports <reports.jsonl>` / `--review <reports.jsonl> <n> [port] [--demos dir]`: see `review`.
mod review;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;

fn main() {
    let mut positional = Vec::new();
    let mut give = Vec::new();
    let mut practice: Option<u8> = None;
    let mut config: Option<PathBuf> = None;
    let mut tick: Option<u32> = None;
    let all: Vec<String> = std::env::args().collect();
    if let Some(i) = all.iter().position(|a| a == "--play") {
        return play(&all, i);
    }
    if let Some(i) = all.iter().position(|a| a == "--reports") {
        return review::list(all.get(i + 1).expect("--reports <reports.jsonl>"));
    }
    if let Some(i) = all.iter().position(|a| a == "--review") {
        let file = all.get(i + 1).expect("--review <reports.jsonl> <n>");
        let n = all.get(i + 2).and_then(|n| n.parse().ok()).expect("--review <reports.jsonl> <n>");
        let demos = all.iter().position(|a| a == "--demos").and_then(|i| all.get(i + 1)).map_or("demos", |d| d.as_str());
        let (demo, from) = review::find(file, n, std::path::Path::new(demos));
        let port = all.iter().skip(i + 3).filter(|a| !a.starts_with("--")).find_map(|a| a.parse().ok()).unwrap_or(net::protocol::DEFAULT_PORT);
        return watch(&demo, port, 1, Some(from));
    }
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--practice" {
            practice = Some(args.next().and_then(|n| n.parse().ok()).unwrap_or(5));
        } else if a == "--tick" {
            tick = args.next().and_then(|n| n.parse().ok());
            assert!(tick.is_some(), "--tick 64 or --tick 128");
        } else if a == "--config" {
            config = args.next().map(PathBuf::from);
        } else if a == "--give" {
            for w in args.next().unwrap_or_default().split(',') {
                give.push(sim::weapons::data::by_name(w).unwrap_or_else(|| panic!("unknown weapon {w}")));
            }
        } else {
            positional.push(a);
        }
    }
    let map = positional.first().cloned().unwrap_or_else(|| "de_dust2".into());
    let port: u16 = positional.get(1).and_then(|p| p.parse().ok()).unwrap_or(net::protocol::DEFAULT_PORT);
    let dir = net::gamefiles::data_dir();
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let assets = net::server::Assets::load(&dir).unwrap_or_else(|e| panic!("loading game files: {e}"));
    // our own map format (built in Godot) if there is one, else a GoldSrc BSP
    let ours = dir.join("maps").join(format!("{map}.bmap"));
    let mut server = if ours.exists() {
        let (file, hash) = sim::mapfile::MapFile::load(&ours).unwrap_or_else(|e| panic!("loading {}: {e}", ours.display()));
        net::Server::with_world(addr, &map, file.world(), hash, assets)
    } else {
        let path = dir.join("maps").join(format!("{map}.bsp"));
        let bsp = goldsrc::bsp::Bsp::load(&path).unwrap_or_else(|e| panic!("loading {}: {e}", path.display()));
        net::Server::new(addr, &map, &bsp, assets)
    }
    .unwrap_or_else(|e| panic!("binding {addr}: {e}"));
    server.set_game_dir(dir.clone());
    server.set_give(give);
    let config = config.or_else(|| Some(PathBuf::from("server.cfg")).filter(|p| p.exists()));
    if let Some(path) = config {
        let cfg = net::config::MatchConfig::load(&path).unwrap_or_else(|e| panic!("{e}"));
        println!("match rules from {}", path.display());
        server.set_config(cfg);
    }
    if let Some(hz) = tick {
        server.set_rate(net::tick::TickRate::new(hz).unwrap_or_else(|| panic!("--tick must be 64 or 128")));
    }
    if let Some(n) = practice {
        server.set_rules(net::server::Rules::practice(n));
    }
    println!("bootleg16 server: {map} on {} at {} tick", server.local_addr(), server.tick_rate().hz());
    // SIGTERM / Ctrl-C (docker stop, updates): finish the current match, then exit.
    on_stop_signal();
    server.run_draining(&STOP);
}

static STOP: AtomicBool = AtomicBool::new(false);

/// Set STOP on SIGINT / SIGTERM (std has no signal API; libc's `signal` is always linked).
fn on_stop_signal() {
    extern "C" fn handler(_: i32) {
        STOP.store(true, Ordering::Relaxed);
    }
    extern "C" {
        fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
    }
    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;
    unsafe {
        signal(SIGINT, handler);
        signal(SIGTERM, handler);
    }
}

/// Replay a recording for the game to watch.
fn play(args: &[String], i: usize) {
    let file = args.get(i + 1).expect("--play <recording.bdem>");
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u32>().ok());
    let port = args.iter().skip(1).filter(|a| !a.starts_with("--")).filter_map(|a| a.parse::<u16>().ok()).next().unwrap_or(net::protocol::DEFAULT_PORT);
    watch(std::path::Path::new(file), port, flag("--speed").unwrap_or(1), flag("--from"));
}

fn watch(file: &std::path::Path, port: u16, speed: u32, from: Option<u32>) {
    let mut replay = net::replay::Replay::open(SocketAddr::from(([127, 0, 0, 1], port)), file).unwrap_or_else(|e| panic!("{e}"));
    replay.speed = speed;
    if let Some(tick) = from {
        replay.skip_to(tick);
    }
    let h = replay.header();
    println!("replaying {} ({}, recorded on {}): watch with  client --connect {}", file.display(), h.map, h.server, replay.local_addr());
    replay.run();
}
