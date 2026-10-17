//! Joining a match: what the next match is, then the steps (`loading::Step`), each slow one
//! on its own thread so the loading screen keeps moving: start the built-in server (or find
//! the address), say hello until welcomed (10 s), load the map, enter. Any failure goes back
//! to the menu with a `Notice`; Esc cancels. Also leaving again.
use super::{AppState, Settings};
use crate::game::Walker;
use crate::game_dir;
use crate::loading::{load_map, Job, Loading, MapData, Progress, Step};
use crate::notice::Notice;
use crate::ui::{UiAction, UiActions};
use bevy::prelude::*;
use std::net::SocketAddr;
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

/// The step in progress: the server's address being found (or the server started), or the
/// map being loaded.
#[derive(Resource)]
pub(super) enum Work {
    Server(Job<Result<SocketAddr, Notice>>),
    Hello { waited: f32 },
    Map(Job<Result<MapData, Notice>>, Progress),
}

/// Start: the built-in server, or look the address up.
pub(super) fn start_connecting(mut commands: Commands, config: Res<GameConfig>) {
    commands.remove_resource::<PendingClient>();
    commands.remove_resource::<Notice>();
    commands.spawn((Camera2d, DespawnOnExit(AppState::Connecting)));
    let practice = matches!(config.server, ServerChoice::Local(_));
    commands.insert_resource(Loading {
        target: match &config.server {
            ServerChoice::Remote(a) => a.clone(),
            ServerChoice::Local(_) => config.map.clone(),
        },
        map: practice.then(|| config.map.clone()),
        practice,
        step: if practice { Step::StartingServer } else { Step::Contacting },
        progress: 0.0,
        elapsed: 0.0,
    });
    let job = match config.server.clone() {
        ServerChoice::Remote(a) => Job::spawn(move || {
            std::net::ToSocketAddrs::to_socket_addrs(&a).ok().and_then(|mut it| it.next()).ok_or_else(|| Notice::error("Can't find the server", format!("No server at '{a}'. Check the address.")).with_retry())
        }),
        ServerChoice::Local(rules) => {
            // The stop flag exists before the thread: cancelling stops a server still starting.
            let stop = Arc::new(AtomicBool::new(false));
            commands.insert_resource(LocalServer(stop.clone()));
            let (map, give) = (config.map.clone(), config.give.clone());
            Job::spawn(move || {
                let root = game_dir();
                let bsp = goldsrc::bsp::Bsp::load(&root.join("cstrike/maps").join(format!("{map}.bsp"))).map_err(|e| Notice::error("Map missing", format!("Can't load {map} ({e}).")))?;
                let assets = net::server::Assets::load(&root).map_err(|e| Notice::error("Game files missing", format!("{e}. Verify your game files.")))?;
                let mut server = net::Server::new(([127, 0, 0, 1], 0).into(), &map, &bsp, assets).map_err(|e| Notice::error("Couldn't start the game", format!("The built-in server didn't start ({e}).")).with_retry())?;
                server.set_give(give);
                server.set_rules(rules);
                let addr = server.local_addr();
                std::thread::spawn(move || server.run_until(&stop));
                Ok(addr)
            })
        }
    };
    commands.insert_resource(Work::Server(job));
}

/// Back to the menu with a notice.
fn fail(commands: &mut Commands, next: &mut NextState<AppState>, notice: Notice) {
    commands.insert_resource(notice);
    next.set(AppState::Menu);
}

/// Move the steps along (the connection itself ticks at the tick rate, `tick_connection`).
#[allow(clippy::too_many_arguments)]
pub(super) fn advance(
    mut commands: Commands,
    mut work: ResMut<Work>,
    mut loading: ResMut<Loading>,
    mut pending: Option<ResMut<PendingClient>>,
    config: Res<GameConfig>,
    settings: Res<Settings>,
    steam: Option<Res<crate::steam::Steam>>,
    mut next: ResMut<NextState<AppState>>,
    time: Res<Time>,
) {
    loading.elapsed += time.delta_secs();
    match &mut *work {
        Work::Server(job) => {
            let addr = match job.poll() {
                Ok(None) => return,
                Ok(Some(Ok(addr))) => addr,
                Ok(Some(Err(n))) => return fail(&mut commands, &mut next, n),
                Err(()) => return fail(&mut commands, &mut next, Notice::error("Couldn't start the game", "Something went wrong getting ready.").with_retry()),
            };
            // With Steam you play under your Steam name.
            let name = steam.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| settings.name.clone());
            let mut client = match net::Client::connect(addr, &name, sim::collision::World::flat_floor(-100_000.0)) {
                Ok(c) => c,
                Err(e) => return fail(&mut commands, &mut next, Notice::error("Network error", format!("Can't open a connection ({e}).")).with_retry()),
            };
            let ping = std::env::var("BOOTLEG16_FAKE_PING").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
            let loss = std::env::var("BOOTLEG16_FAKE_LOSS").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
            client.simulate_network(ping, loss);
            client.ticket = config.ticket;
            client.key = config.key;
            commands.insert_resource(PendingClient(Some(client)));
            loading.step = Step::Contacting;
            loading.progress = 0.15;
            *work = Work::Hello { waited: 0.0 };
        }
        Work::Hello { waited } => {
            let Some(client) = pending.as_mut().and_then(|p| p.0.as_mut()) else { return };
            *waited += time.delta_secs();
            match client.status {
                net::client::Status::Connected { .. } => {
                    let map = client.map.clone().unwrap_or_default();
                    let hash = client.map_hash;
                    loading.map = Some(map.clone());
                    loading.step = Step::LoadingMap;
                    loading.progress = 0.25;
                    let progress = Progress::default();
                    let p = progress.clone();
                    *work = Work::Map(Job::spawn(move || load_map(map, hash, p)), progress);
                }
                net::client::Status::Rejected => {
                    let why = client.reject_reason.clone().unwrap_or_default();
                    fail(&mut commands, &mut next, Notice::error("Server refused you", why).with_retry());
                }
                net::client::Status::Connecting if *waited > 10.0 => {
                    let body = match &config.server {
                        ServerChoice::Remote(a) => format!("{a} didn't answer in 10 seconds."),
                        ServerChoice::Local(_) => "The built-in server didn't answer.".to_string(),
                    };
                    fail(&mut commands, &mut next, Notice::error("Can't reach the server", body).with_retry());
                }
                net::client::Status::Connecting => loading.progress = 0.15 + 0.1 * (*waited / 10.0),
            }
        }
        Work::Map(job, progress) => {
            loading.progress = 0.25 + 0.65 * progress.get();
            let mut map = match job.poll() {
                Ok(None) => return,
                Ok(Some(Ok(m))) => m,
                Ok(Some(Err(n))) => return fail(&mut commands, &mut next, n),
                Err(()) => return fail(&mut commands, &mut next, Notice::error("Map didn't load", "The map file couldn't be read.")),
            };
            let Some(client) = pending.as_mut().and_then(|p| p.0.as_mut()) else { return };
            if let Some(world) = map.world.take() {
                client.world = world;
            }
            if let Some(m) = client.map.clone() {
                commands.insert_resource(GameConfig { map: m, ..config.clone() });
            }
            commands.insert_resource(map);
            loading.step = Step::Joining;
            loading.progress = 0.95;
            next.set(AppState::InGame);
        }
    }
}

/// Keep the connection alive while loading (the server drops a silent player after 5 s).
pub(super) fn tick_connection(mut pending: Option<ResMut<PendingClient>>) {
    if let Some(client) = pending.as_mut().and_then(|p| p.0.as_mut()) {
        client.tick(sim::pmove::Input::default());
    }
}

/// Esc or the screen's Cancel: back to the menu (the match is left).
pub(super) fn cancel(keys: Res<ButtonInput<KeyCode>>, mut actions: ResMut<UiActions>, mut next: ResMut<NextState<AppState>>) {
    let clicked = actions.0.iter().any(|a| *a == UiAction::CancelLoading);
    actions.0.retain(|a| *a != UiAction::CancelLoading);
    if clicked || keys.just_pressed(KeyCode::Escape) {
        next.set(AppState::Menu);
    }
}

/// Leaving the loading screen: drop the steps; a cancelled join says goodbye.
pub(super) fn stop_loading(mut commands: Commands, mut pending: Option<ResMut<PendingClient>>, server: Option<Res<LocalServer>>, state: Res<State<AppState>>) {
    commands.remove_resource::<Work>();
    commands.remove_resource::<Loading>();
    // (the state is already the new one here)
    if *state.get() == AppState::InGame {
        return;
    }
    if let Some(c) = pending.as_mut().and_then(|p| p.0.as_mut()) {
        c.disconnect();
    }
    commands.remove_resource::<PendingClient>();
    if let Some(s) = server {
        s.0.store(true, Ordering::Relaxed);
    }
    commands.remove_resource::<LocalServer>();
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
    commands.remove_resource::<MapData>();
}
