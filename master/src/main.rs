//! The lobby (parties, matchmaking, custom games). Usage: `master [port] [--tokens file]`.
//! `--tokens`: one secret per line; only game servers announcing with one get matches.
//! Built with `--features steam`, it checks Steam logins with Steam (`--steam-app <id>`,
//! default 480, Valve's test app until we have our own).
use std::net::SocketAddr;

#[cfg(feature = "steam")]
mod steam;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let port = args.iter().find_map(|a| a.parse().ok()).unwrap_or(net::master::MASTER_PORT);
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1));
    let tokens: Vec<String> = flag("--tokens")
        .map(|f| std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{f}: {e}")).lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect())
        .unwrap_or_default();
    let mut m = net::master::Master::bind(SocketAddr::from(([0, 0, 0, 0], port)), tokens).unwrap_or_else(|e| panic!("binding port {port}: {e}"));
    println!("bootleg16 lobby on {}", m.local_addr());
    #[cfg(feature = "steam")]
    {
        let app: u32 = flag("--steam-app").and_then(|a| a.parse().ok()).unwrap_or(480);
        let mut steam = steam::Checker::start(app);
        m.steam_mode = net::master::SteamMode::Verify;
        loop {
            m.step();
            steam.run(&mut m);
        }
    }
    #[cfg(not(feature = "steam"))]
    {
        println!("built without Steam: Steam logins are taken on trust (dev only)");
        m.run();
    }
}
