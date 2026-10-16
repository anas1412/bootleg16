//! Dedicated server. Usage: `server [map] [port] [--config server.cfg] [--give ak47,awp]`
//! (defaults: de_dust2, 27015). Match rules come from the config file (see `config/server.cfg`);
//! without `--config`, `server.cfg` in the current directory is used if it exists.
//! Reads the map from the CS 1.6 install in `BOOTLEG16_DIR` (default: Steam's).
//! `--give` hands every player extra weapons on spawn.
//! `--practice <dummies>` runs the practice room (infinite ammo, quick respawn, standing dummies).
//! There are no bots: matches are people only.
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::path::PathBuf;

fn main() {
    let mut positional = Vec::new();
    let mut give = Vec::new();
    let mut practice: Option<u8> = None;
    let mut config: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--practice" {
            practice = Some(args.next().and_then(|n| n.parse().ok()).unwrap_or(5));
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
    let dir = std::env::var_os("BOOTLEG16_DIR").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".steam/steam/steamapps/common/Half-Life")
    });
    let path = dir.join("cstrike/maps").join(format!("{map}.bsp"));
    let bsp = goldsrc::bsp::Bsp::load(&path).unwrap_or_else(|e| panic!("loading {}: {e}", path.display()));
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let assets = net::server::Assets::load(&dir).unwrap_or_else(|e| panic!("loading game files: {e}"));
    let mut server = net::Server::new(addr, &map, &bsp, assets).unwrap_or_else(|e| panic!("binding {addr}: {e}"));
    server.set_give(give);
    let config = config.or_else(|| Some(PathBuf::from("server.cfg")).filter(|p| p.exists()));
    if let Some(path) = config {
        let cfg = net::config::MatchConfig::load(&path).unwrap_or_else(|e| panic!("{e}"));
        println!("match rules from {}", path.display());
        server.set_config(cfg);
    }
    if let Some(n) = practice {
        server.set_rules(net::server::Rules::practice(n));
    }
    println!("bootleg16 server: {map} on {} at {} tick", server.local_addr(), net::protocol::TICK_RATE);
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
