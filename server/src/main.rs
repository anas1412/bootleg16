//! Dedicated server. Usage: `server [map] [port]` (defaults: de_dust2, 27015).
//! Reads the map from the CS 1.6 install in `BOOTLEG16_DIR` (default: Steam's).
use std::net::SocketAddr;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let map = args.next().unwrap_or_else(|| "de_dust2".into());
    let port: u16 = args.next().and_then(|p| p.parse().ok()).unwrap_or(net::protocol::DEFAULT_PORT);
    let dir = std::env::var_os("BOOTLEG16_DIR").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".steam/steam/steamapps/common/Half-Life")
    });
    let path = dir.join("cstrike/maps").join(format!("{map}.bsp"));
    let bsp = goldsrc::bsp::Bsp::load(&path).unwrap_or_else(|e| panic!("loading {}: {e}", path.display()));
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let mut server = net::Server::new(addr, &map, &bsp).unwrap_or_else(|e| panic!("binding {addr}: {e}"));
    println!("bootleg16 server: {map} on {} at {} tick", server.local_addr(), net::protocol::TICK_RATE);
    server.run();
}
