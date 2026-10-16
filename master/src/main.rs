//! The lobby (parties, matchmaking, custom games). Usage: `master [port] [--tokens file]`.
//! `--tokens`: one secret per line; only game servers announcing with one get matches.
//! `--reports file`: where player reports are appended (one JSON object per line).
//! `--key file`: the lobby's long-term key (made on first start; keep it, its public half is
//! built into the game). `--print-public file`: print that public half and exit.
//! Built with `--features steam`, it checks Steam logins with Steam (`--steam-app <id>`,
//! default 480, Valve's test app until we have our own), and only lets in players signed in
//! with Steam unless started with `--allow-anonymous` (test lobbies: smoke tests, bots).
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
    if let Some(path) = flag("--print-public") {
        println!("{}", net::crypto::to_hex(&load_key(path).public()));
        return;
    }
    let mut m = net::master::Master::bind(SocketAddr::from(([0, 0, 0, 0], port)), tokens).unwrap_or_else(|e| panic!("binding port {port}: {e}"));
    m.reports = flag("--reports").map(std::path::PathBuf::from);
    match flag("--key") {
        Some(path) => m.set_key(load_key(path)),
        None => println!("no --key: a new key this run (games built with the old one can't sign in)"),
    }
    println!("lobby public key: {}", net::crypto::to_hex(&m.public_key()));
    println!("bootleg16 lobby on {}", m.local_addr());
    #[cfg(feature = "steam")]
    {
        let app: u32 = flag("--steam-app").and_then(|a| a.parse().ok()).unwrap_or(480);
        let mut steam = steam::Checker::start(app);
        m.steam_mode = net::master::SteamMode::Verify;
        m.require_steam = !args.iter().any(|a| a == "--allow-anonymous");
        if !m.require_steam {
            println!("test lobby: players without Steam (bots, smoke tests) are let in");
        }
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

/// The key in this file, or a new one written there (readable by us only).
fn load_key(path: &str) -> net::crypto::LobbyKey {
    if let Ok(hex) = std::fs::read_to_string(path) {
        return net::crypto::LobbyKey::from_hex(&hex).unwrap_or_else(|| panic!("{path}: not a lobby key"));
    }
    let key = net::crypto::LobbyKey::generate();
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    writeln!(f, "{}", key.to_hex()).unwrap();
    println!("made a new lobby key in {path}");
    key
}
