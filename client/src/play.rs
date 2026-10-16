//! Playing with others: the lobby connection (your party, the casual queue, custom games),
//! invites (from the lobby for friends in the game, from Steam for the rest), Steam rich
//! presence, and going into a match when one is found. Talks to the lobby all the time, so
//! the party survives matches. Drawing is in `play_page`.
use crate::menu::{AppState, GameConfig, MenuDirty, MenuState, Page, ServerChoice, Settings};
use crate::steam::Steam;
use bevy::prelude::*;
use net::lobby::{Command, LobbyClient, LobbyState, Mode};
use net::server::Group;
use std::collections::HashMap;

pub use crate::play_page::{identity, screen};

pub struct PlayPlugin;

impl Plugin for PlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((crate::steam::SteamPlugin, crate::match_flow::MatchFlowPlugin)).add_systems(Startup, start).add_systems(Update, (poll, steam_glue, back));
    }
}

/// The official lobby, built into release builds (`BOOTLEG16_LOBBY=host:port cargo build`,
/// see README "Shipping the game"), so players just open the game. Setting `BOOTLEG16_LOBBY`
/// when starting the game overrides it (testing another lobby).
fn lobby_address() -> String {
    let built_in = option_env!("BOOTLEG16_LOBBY").map(String::from).unwrap_or_else(|| format!("127.0.0.1:{}", net::lobby::LOBBY_PORT));
    std::env::var("BOOTLEG16_LOBBY").unwrap_or(built_in)
}

/// The official lobby's public key (`BOOTLEG16_LOBBY_KEY=<hex>` when building, printed by
/// `./host.sh`), or at run time for testing another lobby. None: learned from the lobby.
fn lobby_key() -> Option<[u8; 32]> {
    std::env::var("BOOTLEG16_LOBBY_KEY").ok().or(option_env!("BOOTLEG16_LOBBY_KEY").map(String::from)).and_then(|k| net::crypto::from_hex(&k))
}

#[derive(Resource)]
pub struct Lobby {
    client: Option<LobbyClient>,
    pub(crate) address: String,
    /// What the page was last drawn with (redraw when it changes).
    pub(crate) shown: Option<LobbyState>,
    pub(crate) notice: String,
    /// Friends we invited (Steam id -> seconds ago), shown as "Invited" for a while.
    pub(crate) invited: HashMap<u64, f32>,
    /// Invites we waved away (by party code).
    pub(crate) ignored: Vec<String>,
    /// A party to join once signed in (from `+party CODE` on the command line: Steam's "Join
    /// game" or an accepted invite when the game wasn't running).
    join_on_start: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LobbyAction {
    /// Join the party whose code is typed in the field.
    JoinParty,
    LeaveParty,
    SetMode(Mode),
    Move(u32, Group),
    Play,
    Cancel,
    /// Invite a Steam friend; `in_game`: they're in the game (lobby invite), else Steam's.
    Invite(u64, bool),
    /// Accept an invite (join that party) / wave it away.
    Accept(String),
    Ignore(String),
    /// A match was found (`LobbyState::ready`): take it / pass (passing costs a short cooldown).
    AcceptMatch,
    DeclineMatch,
    /// Custom games: the leader picks a map (None: random), or starts a veto; a captain bans.
    SetMap(Option<String>),
    StartVeto,
    Ban(String),
    /// Drop into a deathmatch server (the lobby picks one with room; it comes back as `found`).
    Deathmatch,
    /// Go back to your running match (`LobbyState::rejoin`: you left it, or the game restarted).
    Reconnect,
}

fn start(mut commands: Commands, settings: Res<Settings>, steam: Option<Res<Steam>>) {
    let address = lobby_address();
    // Signed in to Steam: your Steam name, and a Steam login for the lobby.
    let name = steam.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| settings.name.clone());
    // One address, or the main and standby lobbies ("a:27010,b:27010"): whichever answers.
    let lobbies = net::config::addresses(&address, true);
    let client = LobbyClient::with_lobbies(lobbies, &name).ok().map(|mut c| {
        c.steam = steam.as_ref().and_then(|s| s.login());
        // The lobby's public key, built into release builds: only the real lobby can answer.
        c.lobby_key = lobby_key();
        c.builtin_key = c.lobby_key.is_some();
        // The pool maps this game has (a match only gets a map everyone has).
        c.maps = net::config::pool_maps_in(&crate::game_dir());
        c
    });
    let args: Vec<String> = std::env::args().collect();
    commands.insert_resource(Lobby { client, address, shown: None, notice: String::new(), invited: HashMap::new(), ignored: Vec::new(), join_on_start: crate::steam::party_code(&args) });
}

pub fn apply(lobby: &mut Lobby, st: &mut MenuState, steam: Option<&Steam>, a: LobbyAction) {
    let code = lobby.shown.as_ref().map(|s| s.code.clone()).unwrap_or_default();
    let Some(c) = &mut lobby.client else { return };
    let cmd = match a {
        LobbyAction::JoinParty => Command::JoinParty { code: std::mem::take(&mut st.address) },
        LobbyAction::LeaveParty => Command::LeaveParty,
        LobbyAction::SetMode(m) => Command::SetMode(m),
        LobbyAction::Move(member, group) => Command::Move { member, group },
        LobbyAction::Play => Command::Play,
        LobbyAction::Cancel => Command::Cancel,
        LobbyAction::AcceptMatch => Command::Accept,
        LobbyAction::Deathmatch => Command::Deathmatch,
        LobbyAction::SetMap(m) => Command::SetMap(m),
        LobbyAction::StartVeto => Command::StartVeto,
        LobbyAction::Ban(m) => Command::Ban(m),
        LobbyAction::DeclineMatch => Command::Decline,
        LobbyAction::Reconnect => {
            // Same as a match found: `poll` connects with the seat.
            c.found = lobby.shown.as_ref().and_then(|s| s.rejoin);
            return;
        }
        LobbyAction::Accept(code) => Command::JoinParty { code },
        LobbyAction::Ignore(code) => {
            lobby.ignored.push(code);
            return;
        }
        LobbyAction::Invite(friend, in_game) => {
            lobby.invited.insert(friend, 0.0);
            if in_game {
                Command::Invite { steam: friend }
            } else {
                // Not in the game: Steam shows them the invite and starts the game for them.
                if let Some(s) = steam {
                    s.invite(friend, &code);
                }
                return;
            }
        }
    };
    c.send(cmd);
}

/// Keep the lobby session alive, redraw on news, and go play when a match is found.
fn poll(mut commands: Commands, mut lobby: ResMut<Lobby>, mut dirty: ResMut<MenuDirty>, state: Res<State<AppState>>, mut next: ResMut<NextState<AppState>>, time: Res<Time>) {
    let lobby = &mut *lobby;
    let Some(c) = &mut lobby.client else { return };
    c.tick();
    if c.state != lobby.shown {
        lobby.shown = c.state.clone();
        dirty.0 = true;
    }
    if !c.notice.is_empty() {
        lobby.notice = std::mem::take(&mut c.notice);
        dirty.0 = true;
    }
    if c.online() {
        if let Some(code) = lobby.join_on_start.take() {
            c.send(Command::JoinParty { code });
        }
    }
    let before = lobby.invited.len();
    lobby.invited.retain(|_, t| {
        *t += time.delta_secs();
        *t < net::lobby::INVITE_SECONDS
    });
    dirty.0 |= lobby.invited.len() != before;
    if *state.get() == AppState::Menu {
        if let Some((addr, ticket, key)) = c.found.take() {
            commands.insert_resource(GameConfig { map: String::new(), server: ServerChoice::Remote(addr.to_string()), give: Vec::new(), ticket, key: Some(key) });
            next.set(AppState::Connecting);
        }
    }
}

/// Steam friends see what we're doing and can join our party; their "Join game" lands here.
fn steam_glue(steam: Option<ResMut<Steam>>, mut lobby: ResMut<Lobby>, state: Res<State<AppState>>) {
    let Some(mut steam) = steam else { return };
    for code in steam.take_joins() {
        if let Some(c) = &mut lobby.client {
            c.send(Command::JoinParty { code });
        }
    }
    let Some(s) = &lobby.shown else { return };
    let status = match (*state.get(), s.searching) {
        (AppState::InGame | AppState::Connecting, _) => "In a match".to_string(),
        (_, Some(_)) => "Looking for a match".to_string(),
        _ if s.members.len() > 1 => format!("In a party ({} of {})", s.members.len(), if s.mode == Mode::Custom { net::lobby::MAX_CUSTOM } else { net::lobby::MAX_PARTY }),
        _ => "In menus".to_string(),
    };
    steam.set_presence(&s.code, &status);
}

/// Esc on the Play page goes back to the main menu.
fn back(keys: Res<ButtonInput<KeyCode>>, st: Option<ResMut<MenuState>>) {
    if let Some(mut st) = st {
        if st.page == Page::Join && keys.just_pressed(KeyCode::Escape) {
            st.page = Page::Main;
            st.rebuild = true;
        }
    }
}
