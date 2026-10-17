//! Main menu, practice setup, joining a server, settings, the connecting screen,
//! the in-game Esc menu and team selection. `settings`: the player settings and their file;
//! `connect`: starting and joining a match.
mod connect;
mod settings;
mod stage3d;

pub use connect::*;
pub use settings::Settings;
pub use stage3d::Layer;
use crate::ui::{Kit, UiAction, UiActions};
use crate::game::Walker;
use crate::game_dir;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};

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
    Voice,
    Characters,
    Maps,
}

/// Set to redraw the menu from outside (e.g. after a voice key was bound).
#[derive(Resource, Default)]
pub struct MenuDirty(pub bool);

/// Menu-only state: current page, chosen map, typed party code or address, last error.
#[derive(Resource)]
pub(crate) struct MenuState {
    pub(crate) page: Page,
    pub(crate) maps: Vec<String>,
    pub(crate) map: String,
    pub(crate) dummies: i32,
    /// Practice page: deathmatch online (else solo, offline).
    pub(crate) deathmatch: bool,
    /// Settings page section (`UiAction::SettingsTab`).
    pub(crate) settings_tab: u8,
    pub(crate) address: String,
    pub(crate) error: Option<String>,
    pub(crate) rebuild: bool,
}

#[derive(Component)]
pub(crate) struct MenuRoot;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        crate::found::plugin(app);
        stage3d::plugin(app);
        app.add_plugins(crate::play::PlayPlugin)
            .init_resource::<UiActions>()
            .init_resource::<Kit>()
            .init_resource::<MenuDirty>()
            .insert_resource(Settings::load())
            .add_systems(Startup, from_command_line)
            .add_systems(OnEnter(AppState::Menu), enter_menu)
            .add_systems(Update, (menu_actions, crate::menu_pages::rebuild_menu, type_text).chain().run_if(in_state(AppState::Menu)))
            .add_systems(OnEnter(AppState::Connecting), (connect::start_connecting, connect::connecting_screen))
            .add_systems(FixedUpdate, connect::poll_connection.run_if(in_state(AppState::Connecting)))
            .add_systems(Update, (crate::overlay::pause_menu, crate::overlay::in_game_actions).chain().run_if(in_state(AppState::InGame)).run_if(resource_exists::<Walker>))
            .add_systems(OnExit(AppState::InGame), connect::leave_game);
    }
}

/// `client [map] [--connect host:port] [--name name] [--give ak47,awp] [--practice N]`:
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
        (None, Some(n)) => ServerChoice::Local(net::server::Rules::practice(n.parse().unwrap_or(5))),
        // A match on a local server (for testing the rules; nobody else is there).
        (None, None) => ServerChoice::Local(net::server::Rules::default()),
    };
    let give = flag("--give").map(|l| l.split(',').filter_map(sim::weapons::data::by_name).collect()).unwrap_or_default();
    commands.insert_resource(GameConfig { map, server, give, ticket: 0, key: None });
    next.set(AppState::Connecting);
}

/// The map pool, in order: stand-ins for our four maps (docs/MAPS.md: port is Train,
/// data_center Dust2, bank Mirage, high_rise Inferno), the ones that are installed.
const POOL: [&str; 4] = ["de_dust2", "de_inferno", "de_mirage", "de_train"];

fn maps_installed() -> Vec<String> {
    let dir = game_dir().join("cstrike/maps");
    POOL.iter().filter(|m| dir.join(format!("{m}.bsp")).exists()).map(|m| m.to_string()).collect()
}

fn enter_menu(mut commands: Commands, mut cursor: Single<&mut CursorOptions>, error: Option<Res<MenuError>>) {
    cursor.grab_mode = CursorGrabMode::None;
    cursor.visible = true;
    let maps = maps_installed();
    // Screenshot runs can open a page directly (BOOTLEG16_MENU_PAGE=practice|join|settings|
    // crosshair|assistant|keys|voice; crosshair, assistant and keys are Settings sections).
    let wanted = std::env::var("BOOTLEG16_MENU_PAGE");
    let settings_tab = match wanted.as_deref() {
        Ok("crosshair") => 1,
        Ok("assistant") => 2,
        Ok("keys") => 3,
        _ => 0,
    };
    let page = match wanted.as_deref() {
        Ok("practice") => Page::Practice,
        Ok("join") => Page::Join,
        Ok("settings" | "crosshair" | "assistant" | "keys") => Page::Settings,
        Ok("voice") => Page::Voice,
        Ok("characters") => Page::Characters,
        Ok("maps") => Page::Maps,
        _ => Page::Main,
    };
    commands.insert_resource(MenuState {
        page,
        map: maps.first().cloned().unwrap_or_else(|| "de_dust2".into()),
        maps,
        dummies: 5,
        deathmatch: false,
        settings_tab,
        address: String::new(),
        error: error.map(|e| e.0.clone()),
        rebuild: true,
    });
    commands.remove_resource::<MenuError>();
    // The page colour is the clear colour, so menu parts can tune in and out over it.
    // Flat screens (Play, a match found) over the 3D menu (stage3d), which clears to the page colour.
    // (no MSAA, like the 3D camera under it: cameras sharing a window must agree)
    commands.spawn((Camera2d, Camera { order: 1, clear_color: ClearColorConfig::None, ..default() }, Msaa::Off, IsDefaultUiCamera, DespawnOnExit(AppState::Menu)));
}

/// Why we ended up back at the menu (shown once).
#[derive(Resource)]
pub struct MenuError(pub String);

/// Typing into the party code / address field (Play page) or the name (Settings page).
/// With Steam your name is your Steam name, so the Settings field is only typed into without it.
fn type_text(mut keys: MessageReader<KeyboardInput>, mut st: ResMut<MenuState>, mut settings: ResMut<Settings>, steam: Option<Res<crate::steam::Steam>>) {
    for k in keys.read().filter(|k| k.state.is_pressed()) {
        let field = match st.page {
            Page::Join => &mut st.address,
            Page::Settings if steam.is_none() && st.settings_tab == 0 => &mut settings.name,
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
    (mut voice, devices): (ResMut<crate::voice::VoiceSettings>, Res<crate::voice::Devices>),
    (mut lobby, steam): (ResMut<crate::play::Lobby>, Option<Res<crate::steam::Steam>>),
    mut binds: ResMut<crate::binds::Binds>,
) {
    let ready = lobby.shown.as_ref().and_then(|s| s.ready);
    if keys.just_pressed(KeyCode::Enter) && ready.is_some_and(|r| !r.you) {
        // a match found takes Enter first (the accept button says so)
        actions.0.push(UiAction::Lobby(crate::play::LobbyAction::AcceptMatch));
    } else if keys.just_pressed(KeyCode::Enter) && st.page == Page::Join {
        // Nothing typed: the big button (find match / start). An address (has a dot or colon)
        // connects directly; anything else is a party code.
        actions.0.push(match st.address.trim() {
            "" => UiAction::Lobby(crate::play::LobbyAction::Play),
            a if a.contains(['.', ':']) => UiAction::Connect,
            _ => UiAction::Lobby(crate::play::LobbyAction::JoinParty),
        });
    } else if keys.just_pressed(KeyCode::Enter) && st.page == Page::Practice {
        // the page's main button, as its key hint says
        if !st.deathmatch {
            actions.0.push(UiAction::StartPractice);
        } else if lobby.shown.is_some() {
            actions.0.push(UiAction::Lobby(crate::play::LobbyAction::Deathmatch));
        }
    }
    for a in std::mem::take(&mut actions.0) {
        st.rebuild = true;
        match a {
            UiAction::Page(p) => {
                st.page = p;
                st.error = None;
            }
            UiAction::PickMap(m) => st.map = m,
            // Characters page: wear this one on that side (the other side keeps its pick).
            UiAction::SetLook(side, i) => {
                let mut looks = settings.looks.unwrap_or([0, 0]);
                looks[side.min(1) as usize] = i.min(net::protocol::CLASSES - 1);
                settings.looks = Some(looks);
                settings.save();
            }
            UiAction::PracticeMode(dm) => st.deathmatch = dm,
            UiAction::Dummies(d) => st.dummies = (st.dummies + d).clamp(0, 20),
            UiAction::StartPractice => {
                commands.insert_resource(GameConfig {
                    map: st.map.clone(),
                    server: ServerChoice::Local(net::server::Rules::practice(st.dummies as u8)),
                    give: Vec::new(),
                    ticket: 0,
                    key: None,
                });
                next.set(AppState::Connecting);
            }
            UiAction::Lobby(a) => crate::play::apply(&mut lobby, &mut st, steam.as_deref(), a),
            UiAction::Connect => {
                commands.insert_resource(GameConfig { map: String::new(), server: ServerChoice::Remote(st.address.trim().to_string()), give: Vec::new(), ticket: 0, key: None });
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
            UiAction::Voice(a) => crate::voice::apply(&mut voice, &devices, a),
            UiAction::SetTeamIds(v) => {
                settings.team_ids = v.min(2);
                settings.save();
            }
            UiAction::SetVsync(v) => {
                settings.vsync = v;
                settings.save();
            }
            UiAction::SetAssistant(i) => {
                settings.assistant = i.min(crate::assistant::OFF);
                settings.save();
            }
            UiAction::Chattiness(c) => {
                settings.chattiness = c.min(2);
                settings.save();
            }
            UiAction::SetRelay(v) => {
                settings.relay = v;
                settings.save();
            }
            UiAction::SetAssistantInRound(v) => {
                settings.assistant_in_round = v;
                settings.save();
            }
            UiAction::SettingsTab(t) => st.settings_tab = t.min(3),
            UiAction::Rebind(a) => binds.capture = if binds.capture == Some(a) { None } else { Some(a) },
            UiAction::ResetBinds => {
                *binds = crate::binds::Binds::default();
                binds.save();
            }
            UiAction::CrosshairColor(c) => {
                settings.crosshair_color = c.min(crate::hud::CROSSHAIR_COLORS.len() as u8 - 1);
                settings.save();
            }
            UiAction::CrosshairSize(v) => {
                settings.crosshair_size = v.min(2);
                settings.save();
            }
            UiAction::CrosshairDynamic(v) => {
                settings.crosshair_dynamic = v;
                settings.save();
            }
            UiAction::HudCurve(d) => {
                settings.hud_curve = (settings.hud_curve as i16 + d as i16 * 20).clamp(0, 100) as u8;
                settings.save();
            }
            UiAction::CrosshairDot(v) => {
                settings.crosshair_dot = v;
                settings.save();
            }
            UiAction::Quit => {
                exit.write(AppExit::Success);
            }
            _ => {}
        }
    }
}

pub use crate::overlay::{overlay_open, Overlay};

