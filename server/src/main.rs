//! Dedicated server. Usage: `server [map] [port] [--give ak47,awp]` (defaults: de_dust2, 27015).
//! Reads the map from the CS 1.6 install in `BOOTLEG16_DIR` (default: Steam's).
//! `--give` hands every player extra weapons on spawn (until there's a buy menu).
use std::net::SocketAddr;
use std::path::PathBuf;

fn main() {
    let mut positional = Vec::new();
    let mut give = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--give" {
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
    println!("bootleg16 server: {map} on {} at {} tick", server.local_addr(), net::protocol::TICK_RATE);
    server.run();
}
