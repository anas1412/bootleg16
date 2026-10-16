//! Joining a match: what the next match is, starting the built-in server, the connecting
//! screen and its timeout, and leaving again.
use super::{AppState, MenuError, Settings};
use crate::game::Walker;
use crate::game_dir;
use crate::ui::{t, Kit, Type, BG0, TX1};
use bevy::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// What the next match is.
#[derive(Resource, Clone)]
pub struct GameConfig {
    /// Map to run (built-in server). For a remote server the server says which map.
    pub map: String,
    pub server: ServerChoice,
    pub give: Vec<sim::weapons::Id>,
    /// Our seat in a reserved match (from the lobby); 0 = none.
    pub ticket: u64,
    /// The seat's encryption key (from the lobby).
    pub key: Option<[u8; 32]>,
}

#[derive(Clone)]
pub enum ServerChoice {
    /// A server inside the game.
    Local(net::server::Rules),
    Remote(String),
}

/// The server running inside the game, stopped when the match ends.
#[derive(Resource)]
pub struct LocalServer(pub Arc<AtomicBool>);

/// A connection waiting for the server's welcome.
#[derive(Resource)]
pub struct PendingClient(pub Option<net::Client>);

/// Start the built-in server if needed and open the connection.
pub(super) fn start_connecting(mut commands: Commands, config: Res<GameConfig>, settings: Res<Settings>, steam: Option<Res<crate::steam::Steam>>, mut next: ResMut<NextState<AppState>>) {
    let root = game_dir();
    let fail = |commands: &mut Commands, next: &mut NextState<AppState>, msg: String| {
        commands.insert_resource(MenuError(msg));
        next.set(AppState::Menu);
    };
    let addr = match &config.server {
        ServerChoice::Remote(a) => match std::net::ToSocketAddrs::to_socket_addrs(a).ok().and_then(|mut it| it.next()) {
            Some(addr) => addr,
            None => return fail(&mut commands, &mut next, format!("Can't find server '{a}'")),
        },
        ServerChoice::Local(rules) => {
            let path = root.join("cstrike/maps").join(format!("{}.bsp", config.map));
            let bsp = match goldsrc::bsp::Bsp::load(&path) {
                Ok(b) => b,
                Err(e) => return fail(&mut commands, &mut next, format!("Can't load {}: {e}", config.map)),
            };
            let assets = match net::server::Assets::load(&root) {
                Ok(a) => a,
                Err(e) => return fail(&mut commands, &mut next, format!("Missing game files: {e}")),
            };
            let mut server = match net::Server::new(([127, 0, 0, 1], 0).into(), &config.map, &bsp, assets) {
                Ok(s) => s,
                Err(e) => return fail(&mut commands, &mut next, format!("Can't start the server: {e}")),
            };
            server.set_give(config.give.clone());
            server.set_rules(rules.clone());
            let addr = server.local_addr();
            let stop = Arc::new(AtomicBool::new(false));
            let flag = stop.clone();
            std::thread::spawn(move || server.run_until(&flag));
            commands.insert_resource(LocalServer(stop));
            addr
        }
    };
    // The world gets filled in once the server tells us the map.
    // with Steam you play under your Steam name
    let name = steam.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| settings.name.clone());
    let mut client = match net::Client::connect(addr, &name, sim::collision::World::flat_floor(-100_000.0)) {
        Ok(c) => c,
        Err(e) => return fail(&mut commands, &mut next, format!("Network error: {e}")),
    };
    let ping = std::env::var("BOOTLEG16_FAKE_PING").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let loss = std::env::var("BOOTLEG16_FAKE_LOSS").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    client.simulate_network(ping, loss);
    client.ticket = config.ticket;
    client.key = config.key;
    commands.insert_resource(PendingClient(Some(client)));
}

#[derive(Component)]
pub(super) struct Waited(f32);

pub(super) fn connecting_screen(mut commands: Commands, config: Res<GameConfig>, kit: Res<Kit>) {
    let what = match &config.server {
        ServerChoice::Remote(a) => format!("Connecting to {a}..."),
        ServerChoice::Local(_) => format!("Loading {}...", config.map),
    };
    commands.spawn((Camera2d, DespawnOnExit(AppState::Connecting)));
    commands
        .spawn((
            DespawnOnExit(AppState::Connecting),
            Waited(0.0),
            Node { width: percent(100), height: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
            BackgroundColor(BG0),
        ))
        .with_children(|p| {
            p.spawn(t(&kit, Type::H3, what, TX1));
        });
}

/// Wait for the welcome (or give up after 10 s).
pub(super) fn poll_connection(mut commands: Commands, mut pending: ResMut<PendingClient>, mut config: ResMut<GameConfig>, mut next: ResMut<NextState<AppState>>, mut waited: Query<&mut Waited>, time: Res<Time>) {
    let Some(client) = pending.0.as_mut() else { return };
    client.tick(sim::pmove::Input::default());
    if let Ok(mut w) = waited.single_mut() {
        w.0 += time.delta_secs();
        if w.0 > 10.0 {
            commands.insert_resource(MenuError("The server didn't answer.".into()));
            next.set(AppState::Menu);
            return;
        }
    }
    match client.status {
        net::client::Status::Connected { .. } => {
            if let Some(m) = client.map.clone() {
                config.map = m;
            }
            next.set(AppState::InGame);
        }
        net::client::Status::Rejected => {
            let why = client.reject_reason.clone().unwrap_or_default();
            commands.insert_resource(MenuError(format!("Server refused: {why}")));
            next.set(AppState::Menu);
        }
        net::client::Status::Connecting => {}
    }
}

/// Leaving a match: say goodbye, stop the built-in server, back to the menu.
pub(super) fn leave_game(mut commands: Commands, walker: Option<ResMut<Walker>>, server: Option<Res<LocalServer>>) {
    if let Some(mut w) = walker {
        w.client.disconnect();
    }
    if let Some(s) = server {
        s.0.store(true, Ordering::Relaxed);
    }
    commands.remove_resource::<Walker>();
    commands.remove_resource::<LocalServer>();
}

