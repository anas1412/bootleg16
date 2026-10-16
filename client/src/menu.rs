//! Main menu, practice setup, joining a server, settings, the connecting screen,
//! the in-game Esc menu and team selection.
use crate::ui::{button, panel, text, UiAction, UiActions, ACCENT, CT_COLOR, MUTED, TEXT, T_COLOR};
use crate::game::Walker;
use crate::game_dir;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use net::protocol::Team;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(States, Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Menu,
    Connecting,
    InGame,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Main,
    Practice,
    Join,
    Settings,
}

/// Player settings, saved in ~/.config/bootleg16/settings.txt.
#[derive(Resource, Clone)]
pub struct Settings {
    pub name: String,
    /// Mouse sensitivity, as in CS (× 0.022° per mouse count).
    pub sensitivity: f32,
    pub volume: f32,
    /// Frame rate cap (0 = none), like CS's fps_max.
    pub fps_max: u32,
    pub vsync: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { name: "player".into(), sensitivity: 5.0, volume: 0.8, fps_max: 300, vsync: false }
    }
}

fn settings_path() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/bootleg16/settings.txt")
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        if let Ok(text) = std::fs::read_to_string(settings_path()) {
            for line in text.lines() {
                match line.split_once('=') {
                    Some(("name", v)) => s.name = v.trim().to_string(),
                    Some(("sensitivity", v)) => s.sensitivity = v.trim().parse().unwrap_or(s.sensitivity),
                    Some(("volume", v)) => s.volume = v.trim().parse().unwrap_or(s.volume),
                    Some(("fps_max", v)) => s.fps_max = v.trim().parse().unwrap_or(s.fps_max),
                    Some(("vsync", v)) => s.vsync = v.trim() == "true",
                    _ => {}
                }
            }
        }
        s
    }

    fn save(&self) {
        let p = settings_path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(p, format!("name={}\nsensitivity={}\nvolume={}\nfps_max={}\nvsync={}\n", self.name, self.sensitivity, self.volume, self.fps_max, self.vsync));
    }
}

/// What the next match is.
#[derive(Resource, Clone)]
pub struct GameConfig {
    /// Map to run (built-in server). For a remote server the server says which map.
    pub map: String,
    pub server: ServerChoice,
    pub give: Vec<sim::weapons::Id>,
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

/// Menu-only state: current page, chosen map, typed address, last error.
#[derive(Resource)]
struct MenuState {
    page: Page,
    maps: Vec<String>,
    map: String,
    dummies: i32,
    dummy_move: bool,
    /// Play a real match (rounds and money) instead of the practice room.
    matchup: bool,
    address: String,
    error: Option<String>,
    rebuild: bool,
}

#[derive(Component)]
struct MenuRoot;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiActions>()
            .insert_resource(Settings::load())
            .add_systems(Startup, from_command_line)
            .add_systems(OnEnter(AppState::Menu), enter_menu)
            .add_systems(Update, (menu_actions, rebuild_menu, type_text).chain().run_if(in_state(AppState::Menu)))
            .add_systems(OnEnter(AppState::Connecting), (start_connecting, connecting_screen))
            .add_systems(FixedUpdate, poll_connection.run_if(in_state(AppState::Connecting)))
            .add_systems(Update, (pause_menu, in_game_actions).chain().run_if(in_state(AppState::InGame)).run_if(resource_exists::<Walker>))
            .add_systems(OnExit(AppState::InGame), leave_game);
    }
}

/// `client [map] [--connect host:port] [--name name] [--give ak47,awp] [--practice N | --dummies N]`:
/// any argument skips the menu.
fn from_command_line(mut commands: Commands, mut settings: ResMut<Settings>, mut next: ResMut<NextState<AppState>>) {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return;
    }
    let flag = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();
    if let Some(name) = flag("--name") {
        settings.name = name; // this run only, not saved
    }
    let map = args.first().filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "de_dust2".into());
    let server = match (flag("--connect"), flag("--practice")) {
        (Some(addr), _) => ServerChoice::Remote(addr),
        (None, Some(n)) => ServerChoice::Local(net::server::Rules::practice(n.parse().unwrap_or(5), false)),
        // A match; `--dummies N` puts standing dummies on the other team.
        (None, None) => ServerChoice::Local(net::server::Rules { dummies: flag("--dummies").and_then(|n| n.parse().ok()).unwrap_or(0), ..Default::default() }),
    };
    let give = flag("--give").map(|l| l.split(',').filter_map(sim::weapons::data::by_name).collect()).unwrap_or_default();
    commands.insert_resource(GameConfig { map, server, give });
    next.set(AppState::Connecting);
}

fn maps_installed() -> Vec<String> {
    let mut maps: Vec<String> = std::fs::read_dir(game_dir().join("cstrike/maps"))
        .map(|d| d.flatten().filter_map(|e| e.file_name().to_str()?.strip_suffix(".bsp").map(str::to_string)).collect())
        .unwrap_or_default();
    // The ones we test on first, the rest alphabetically.
    let favourites = ["de_dust2", "de_inferno", "de_nuke", "de_train"];
    maps.sort_by_key(|m| (favourites.iter().position(|f| f == m).unwrap_or(99), m.clone()));
    maps
}

fn enter_menu(mut commands: Commands, mut cursor: Single<&mut CursorOptions>, error: Option<Res<MenuError>>) {
    cursor.grab_mode = CursorGrabMode::None;
    cursor.visible = true;
    let maps = maps_installed();
    commands.insert_resource(MenuState {
        page: Page::Main,
        map: maps.first().cloned().unwrap_or_else(|| "de_dust2".into()),
        maps,
        dummies: 5,
        dummy_move: false,
        matchup: false,
        address: "127.0.0.1:27015".into(),
        error: error.map(|e| e.0.clone()),
        rebuild: true,
    });
    commands.remove_resource::<MenuError>();
    commands.spawn((Camera2d, DespawnOnExit(AppState::Menu)));
}

/// Why we ended up back at the menu (shown once).
#[derive(Resource)]
pub struct MenuError(pub String);

fn rebuild_menu(mut commands: Commands, mut st: ResMut<MenuState>, settings: Res<Settings>, roots: Query<Entity, With<MenuRoot>>) {
    if !st.rebuild {
        return;
    }
    st.rebuild = false;
    for e in &roots {
        commands.entity(e).despawn();
    }
    let st = &*st;
    commands
        .spawn((
            MenuRoot,
            DespawnOnExit(AppState::Menu),
            Node { width: percent(100), height: percent(100), flex_direction: FlexDirection::Row, ..default() },
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::auto(Color::srgb(0.10, 0.11, 0.15)),
                ColorStop::auto(Color::srgb(0.03, 0.03, 0.05)),
            ])),
        ))
        .with_children(|root| {
            // Left column: title and navigation.
            root.spawn(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(px(56)), width: px(380), ..default() }).with_children(|col| {
                col.spawn(text("BOOTLEG", 54.0, TEXT));
                col.spawn((text("16", 54.0, ACCENT), Node { margin: UiRect { top: px(-18), bottom: px(36), ..default() }, ..default() }));
                for (label, page) in [("Play", Page::Practice), ("Join server", Page::Join), ("Settings", Page::Settings)] {
                    button(col, label, UiAction::Page(page), st.page == page);
                }
                button(col, "Quit", UiAction::Quit, false);
                if let Some(e) = &st.error {
                    col.spawn((text(e, 16.0, Color::srgb(1.0, 0.45, 0.4)), Node { margin: UiRect::top(px(24)), ..default() }));
                }
            });
            // Right side: the page.
            root.spawn(Node { flex_grow: 1.0, padding: UiRect::all(px(56)), align_items: AlignItems::FlexStart, ..default() }).with_children(|right| match st.page {
                Page::Main => {
                    right.spawn(panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(px(28)), max_width: px(560), ..default() })).with_children(|p| {
                        p.spawn(text("Welcome", 30.0, TEXT));
                        p.spawn((
                            text("Practice on any installed map with dummies, or join a server.\nCS 1.6 rules, guns and movement, rebuilt in Rust.", 17.0, MUTED),
                            Node { margin: UiRect::top(px(10)), ..default() },
                        ));
                    });
                }
                Page::Practice => {
                    right.spawn(panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(px(28)), ..default() })).with_children(|p| {
                        p.spawn(text("Play offline", 30.0, TEXT));
                        let about = if st.matchup {
                            "A real match: freeze time, money, the bomb, MR12. Dummies stand in for the other team until there are bots."
                        } else {
                            "Practice: no rounds, infinite ammo, instant respawn, dummies to shoot. Everything is free in the buy menu."
                        };
                        p.spawn((text(about, 16.0, MUTED), Node { margin: UiRect::vertical(px(10)), ..default() }));
                        p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(28), ..default() }).with_children(|row| {
                            row.spawn(Node { flex_direction: FlexDirection::Column, max_height: px(420), overflow: Overflow::scroll_y(), ..default() }).with_children(|list| {
                                for m in &st.maps {
                                    button(list, m, UiAction::PickMap(m.clone()), *m == st.map);
                                }
                            });
                            row.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|opts| {
                                button(opts, if st.matchup { "Mode: match" } else { "Mode: practice" }, UiAction::ToggleMatch, st.matchup);
                                opts.spawn(text(format!("Dummies: {}", st.dummies), 20.0, TEXT));
                                opts.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(8), ..default() }).with_children(|r| {
                                    button(r, "-", UiAction::Dummies(-1), false);
                                    button(r, "+", UiAction::Dummies(1), false);
                                });
                                button(opts, if st.dummy_move { "Dummies strafe: on" } else { "Dummies strafe: off" }, UiAction::ToggleDummyMove, st.dummy_move);
                                opts.spawn(Node { height: px(24), ..default() });
                                button(opts, &format!("Start  {}", st.map), UiAction::StartPractice, true);
                            });
                        });
                    });
                }
                Page::Join => {
                    right.spawn(panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(px(28)), ..default() })).with_children(|p| {
                        p.spawn(text("Join server", 30.0, TEXT));
                        p.spawn((text("Type the address (host:port) and connect. You need the server's map installed.", 16.0, MUTED), Node { margin: UiRect::vertical(px(10)), ..default() }));
                        p.spawn((
                            Node { padding: UiRect::axes(px(14), px(10)), margin: UiRect::vertical(px(8)), border: UiRect::all(px(1)), min_width: px(360), border_radius: BorderRadius::all(px(6)), ..default() },
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
                            BorderColor::all(ACCENT),
                        ))
                        .with_children(|f| {
                            f.spawn(text(format!("{}_", st.address), 20.0, TEXT));
                        });
                        button(p, "Connect", UiAction::Connect, true);
                    });
                }
                Page::Settings => {
                    right.spawn(panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(px(28)), row_gap: px(6), ..default() })).with_children(|p| {
                        p.spawn(text("Settings", 30.0, TEXT));
                        p.spawn(text(format!("Name: {}_   (type to change)", settings.name), 18.0, MUTED));
                        setting_row(p, &format!("Mouse sensitivity: {:.1}", settings.sensitivity), UiAction::Sensitivity(-0.1), UiAction::Sensitivity(0.1));
                        setting_row(p, &format!("Volume: {:.0}%", settings.volume * 100.0), UiAction::Volume(-0.05), UiAction::Volume(0.05));
                        let cap = if settings.fps_max == 0 { "unlimited".to_string() } else { settings.fps_max.to_string() };
                        setting_row(p, &format!("Max FPS: {cap}"), UiAction::FpsMax(-1), UiAction::FpsMax(1));
                        button(p, if settings.vsync { "VSync: on" } else { "VSync: off" }, UiAction::ToggleVsync, settings.vsync);
                    });
                }
            });
        });
}

fn setting_row(p: &mut ChildSpawnerCommands, label: &str, minus: UiAction, plus: UiAction) {
    p.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(10), ..default() }).with_children(|r| {
        r.spawn((text(label, 20.0, TEXT), Node { width: px(320), ..default() }));
        button(r, "-", minus, false);
        button(r, "+", plus, false);
    });
}

/// Typing into the address field (Join page) or the name (Settings page).
fn type_text(mut keys: MessageReader<KeyboardInput>, mut st: ResMut<MenuState>, mut settings: ResMut<Settings>) {
    for k in keys.read().filter(|k| k.state.is_pressed()) {
        let field = match st.page {
            Page::Join => &mut st.address,
            Page::Settings => &mut settings.name,
            _ => continue,
        };
        match &k.logical_key {
            Key::Backspace => {
                field.pop();
            }
            Key::Character(c) if field.len() < 40 && c.chars().all(|ch| !ch.is_control()) => field.push_str(c),
            _ => continue,
        }
        if st.page == Page::Settings {
            settings.save();
        }
        st.rebuild = true;
    }
}

fn menu_actions(
    mut commands: Commands,
    mut actions: ResMut<UiActions>,
    mut st: ResMut<MenuState>,
    mut settings: ResMut<Settings>,
    mut next: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if keys.just_pressed(KeyCode::Enter) && st.page == Page::Join {
        actions.0.push(UiAction::Connect);
    }
    for a in std::mem::take(&mut actions.0) {
        st.rebuild = true;
        match a {
            UiAction::Page(p) => {
                st.page = p;
                st.error = None;
            }
            UiAction::PickMap(m) => st.map = m,
            UiAction::Dummies(d) => st.dummies = (st.dummies + d).clamp(0, 20),
            UiAction::ToggleDummyMove => st.dummy_move = !st.dummy_move,
            UiAction::ToggleMatch => st.matchup = !st.matchup,
            UiAction::StartPractice => {
                commands.insert_resource(GameConfig {
                    map: st.map.clone(),
                    server: ServerChoice::Local(if st.matchup {
                        net::server::Rules { dummies: st.dummies as u8, dummy_move: st.dummy_move, ..Default::default() }
                    } else {
                        net::server::Rules::practice(st.dummies as u8, st.dummy_move)
                    }),
                    give: Vec::new(),
                });
                next.set(AppState::Connecting);
            }
            UiAction::Connect => {
                commands.insert_resource(GameConfig { map: String::new(), server: ServerChoice::Remote(st.address.trim().to_string()), give: Vec::new() });
                next.set(AppState::Connecting);
            }
            UiAction::Sensitivity(d) => {
                settings.sensitivity = (settings.sensitivity + d).clamp(0.1, 20.0);
                settings.save();
            }
            UiAction::Volume(d) => {
                settings.volume = (settings.volume + d).clamp(0.0, 1.0);
                settings.save();
            }
            UiAction::FpsMax(step) => {
                // Steps like CS players use; 0 (unlimited) last.
                const CAPS: [u32; 8] = [60, 100, 144, 200, 240, 300, 500, 0];
                let i = CAPS.iter().position(|c| *c == settings.fps_max).unwrap_or(5) as i32;
                settings.fps_max = CAPS[(i + step).clamp(0, CAPS.len() as i32 - 1) as usize];
                settings.save();
            }
            UiAction::ToggleVsync => {
                settings.vsync = !settings.vsync;
                settings.save();
            }
            UiAction::Quit => {
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}

/// Start the built-in server if needed and open the connection.
fn start_connecting(mut commands: Commands, config: Res<GameConfig>, settings: Res<Settings>, mut next: ResMut<NextState<AppState>>) {
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
    let mut client = match net::Client::connect(addr, &settings.name, sim::collision::World::flat_floor(-100_000.0)) {
        Ok(c) => c,
        Err(e) => return fail(&mut commands, &mut next, format!("Network error: {e}")),
    };
    let ping = std::env::var("BOOTLEG16_FAKE_PING").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let loss = std::env::var("BOOTLEG16_FAKE_LOSS").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    client.simulate_network(ping, loss);
    commands.insert_resource(PendingClient(Some(client)));
}

#[derive(Component)]
struct Waited(f32);

fn connecting_screen(mut commands: Commands, config: Res<GameConfig>) {
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
            BackgroundColor(Color::srgb(0.04, 0.04, 0.06)),
        ))
        .with_children(|p| {
            p.spawn(text(what, 26.0, TEXT));
        });
}

/// Wait for the welcome (or give up after 10 s).
fn poll_connection(mut commands: Commands, mut pending: ResMut<PendingClient>, mut config: ResMut<GameConfig>, mut next: ResMut<NextState<AppState>>, mut waited: Query<&mut Waited>, time: Res<Time>) {
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
fn leave_game(mut commands: Commands, walker: Option<ResMut<Walker>>, server: Option<Res<LocalServer>>) {
    if let Some(mut w) = walker {
        w.client.disconnect();
    }
    if let Some(s) = server {
        s.0.store(true, Ordering::Relaxed);
    }
    commands.remove_resource::<Walker>();
    commands.remove_resource::<LocalServer>();
}

/// In-game overlays (Esc menu, team select) as one optional panel.
#[derive(Resource, Default, PartialEq, Clone, Copy)]
pub enum Overlay {
    #[default]
    None,
    Pause,
    Teams,
}

#[derive(Component)]
struct OverlayRoot;

/// Esc toggles the pause menu; M opens team selection (also shown when you join).
fn pause_menu(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut overlay: ResMut<Overlay>,
    mut shown: Local<Option<Overlay>>,
    w: Res<Walker>,
    mut asked: Local<bool>,
    mut cursor: Single<&mut CursorOptions>,
    roots: Query<Entity, With<OverlayRoot>>,
) {
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    if !auto {
        if keys.just_pressed(KeyCode::Escape) {
            *overlay = if *overlay == Overlay::None { Overlay::Pause } else { Overlay::None };
        }
        if keys.just_pressed(KeyCode::KeyM) {
            *overlay = if *overlay == Overlay::Teams { Overlay::None } else { Overlay::Teams };
        }
        // First time in: pick a side, like CS.
        if !*asked && w.client.id().is_some() && !w.client.scores.is_empty() && w.client.team() == Team::Spectator {
            *asked = true;
            *overlay = Overlay::Teams;
        }
    } else if !*asked && w.client.id().is_some() {
        // Screenshot runs pick automatically (BOOTLEG16_TEAM=t|ct|spec, default auto).
        *asked = true;
        commands.queue(|world: &mut World| {
            let team = match std::env::var("BOOTLEG16_TEAM").as_deref() {
                Ok("t") => Some(Team::T),
                Ok("ct") => Some(Team::Ct),
                Ok("spec") => Some(Team::Spectator),
                _ => None,
            };
            if let Some(mut w) = world.get_resource_mut::<Walker>() {
                w.client.join_team(team);
            }
        });
    }
    if *shown == Some(*overlay) {
        return;
    }
    *shown = Some(*overlay);
    for e in &roots {
        commands.entity(e).despawn();
    }
    if *overlay != Overlay::None {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
    let me_team = w.client.team();
    let practice = w.client.practice;
    match *overlay {
        Overlay::None => {}
        Overlay::Pause => {
            commands
                .spawn((OverlayRoot, DespawnOnExit(AppState::InGame), full_dim()))
                .with_children(|root| {
                    root.spawn(panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(px(28)), ..default() })).with_children(|p| {
                        p.spawn((text(if practice { "Practice" } else { "Paused" }, 30.0, TEXT), Node { margin: UiRect::bottom(px(12)), ..default() }));
                        button(p, "Resume", UiAction::Resume, false);
                        button(p, "Choose team", UiAction::ShowTeams, false);
                        button(p, "Leave match", UiAction::Leave, false);
                    });
                });
        }
        Overlay::Teams => {
            commands
                .spawn((OverlayRoot, DespawnOnExit(AppState::InGame), full_dim()))
                .with_children(|root| {
                    root.spawn(panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(px(28)), ..default() })).with_children(|p| {
                        p.spawn((text("Choose your team", 30.0, TEXT), Node { margin: UiRect::bottom(px(12)), ..default() }));
                        p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(16), ..default() }).with_children(|row| {
                            team_card(row, "1", "Terrorists", T_COLOR, UiAction::ChooseTeam(Some(Team::T)), me_team == Team::T);
                            team_card(row, "2", "Counter-Terrorists", CT_COLOR, UiAction::ChooseTeam(Some(Team::Ct)), me_team == Team::Ct);
                        });
                        p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(8), margin: UiRect::top(px(12)), ..default() }).with_children(|row| {
                            button(row, "5  Auto-select", UiAction::ChooseTeam(None), false);
                            button(row, "6  Spectate", UiAction::ChooseTeam(Some(Team::Spectator)), me_team == Team::Spectator);
                        });
                    });
                });
        }
    }
}

fn full_dim() -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        GlobalZIndex(20),
    )
}

fn team_card(row: &mut ChildSpawnerCommands, key: &str, name: &str, color: Color, action: UiAction, current: bool) {
    row.spawn((
        Node { flex_direction: FlexDirection::Column, padding: UiRect::all(px(22)), width: px(260), border: UiRect::all(px(2)), border_radius: BorderRadius::all(px(10)), ..default() },
        BackgroundColor(color.with_alpha(if current { 0.30 } else { 0.14 })),
        BorderColor::all(color.with_alpha(if current { 1.0 } else { 0.4 })),
    ))
    .with_children(|c| {
        c.spawn((text(key, 18.0, MUTED), Pickable::IGNORE));
        c.spawn((text(name, 24.0, color), Pickable::IGNORE));
    })
    .observe(move |_: On<PointerClick>, mut actions: ResMut<UiActions>| actions.0.push(action.clone()));
}

fn in_game_actions(mut actions: ResMut<UiActions>, mut overlay: ResMut<Overlay>, mut w: ResMut<Walker>, mut next: ResMut<NextState<AppState>>, keys: Res<ButtonInput<KeyCode>>) {
    // CS team menu keys.
    if *overlay == Overlay::Teams {
        for (k, team) in [(KeyCode::Digit1, Some(Team::T)), (KeyCode::Digit2, Some(Team::Ct)), (KeyCode::Digit5, None), (KeyCode::Digit6, Some(Team::Spectator))] {
            if keys.just_pressed(k) {
                actions.0.push(UiAction::ChooseTeam(team));
            }
        }
    }
    let mut keep = Vec::new();
    for a in std::mem::take(&mut actions.0) {
        match a {
            UiAction::Resume => *overlay = Overlay::None,
            UiAction::ShowTeams => *overlay = Overlay::Teams,
            UiAction::Leave => next.set(AppState::Menu),
            UiAction::ChooseTeam(team) => {
                w.client.join_team(team);
                *overlay = Overlay::None;
            }
            other => keep.push(other), // buy menu actions are handled by the HUD
        }
    }
    actions.0 = keep;
}

pub fn overlay_open(overlay: Option<Res<Overlay>>) -> bool {
    overlay.is_some_and(|o| *o != Overlay::None)
}
