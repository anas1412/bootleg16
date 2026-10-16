//! Master server (the server list). Usage: `master [port] [--tokens file]`.
//! `--tokens`: one secret per line; game servers announcing with one are marked official.
use std::net::SocketAddr;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let port = args.iter().find_map(|a| a.parse().ok()).unwrap_or(net::master::MASTER_PORT);
    let tokens: Vec<String> = args
        .iter()
        .position(|a| a == "--tokens")
        .and_then(|i| args.get(i + 1))
        .map(|f| std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}")).lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    let mut m = net::master::Master::bind(SocketAddr::from(([0, 0, 0, 0], port)), tokens).unwrap_or_else(|e| panic!("binding port {port}: {e}"));
    println!("bootleg16 master on {}", m.local_addr());
    m.run();
}
