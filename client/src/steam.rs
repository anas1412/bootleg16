//! Steam (built with the `steam` feature, on by default): your Steam name and id, your
//! friends with their avatars, invites, and rich presence so friends see your party and can
//! join it from Steam. App 480 (Valve's Spacewar test app) until we have our own App ID.
//! Without Steam running (or without the feature) the game plays anonymously; nothing here
//! is required.
use bevy::prelude::*;
use std::collections::HashMap;

pub struct SteamPlugin;

/// Our Steam app. ponytail: 480 is Valve's shared test app; swap for ours when we have it.
#[cfg(feature = "steam")]
const APP_ID: u32 = 480;

impl Plugin for SteamPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(feature = "steam")]
        match steamworks::Client::init_app(APP_ID) {
            Ok(client) => {
                let (tx, rx) = std::sync::mpsc::channel();
                let join = client.register_callback(move |r: steamworks::GameRichPresenceJoinRequested| _ = tx.send(r.connect));
                let user = client.user();
                let me = user.steam_id().raw();
                let name = client.friends().name();
                info!("Steam: signed in as {name} ({me})");
                app.insert_resource(Steam { me, name, friends: Vec::new(), avatars: HashMap::new(), presence: String::new(), sdk: Some(Sdk { client, joins: std::sync::Mutex::new(rx), _join: join }), refresh: 0.0 })
                    .add_systems(Update, update);
            }
            Err(e) => warn!("Steam isn't running or refused us ({e}): playing without Steam"),
        }
    }
}

/// Present only when signed in to Steam.
#[derive(Resource)]
pub struct Steam {
    pub me: u64,
    pub name: String,
    /// Friends, in game first, then online, then offline.
    pub friends: Vec<Friend>,
    /// Steam id -> 64x64 avatar (missing until Steam has sent it).
    pub avatars: HashMap<u64, Handle<Image>>,
    /// The rich presence we last set (the party code and what we're doing).
    presence: String,
    #[cfg(feature = "steam")]
    sdk: Option<Sdk>,
    #[cfg(not(feature = "steam"))]
    #[allow(dead_code)]
    sdk: Option<()>,
    refresh: f32,
}

/// Fields drop in order: the callback must go before the client (dropping the client shuts
/// Steam down; unregistering a callback after that trips Steam's asserts and a crash dump).
#[cfg(feature = "steam")]
struct Sdk {
    _join: steamworks::CallbackHandle,
    /// "Join game" / accepted Steam invites: their connect strings ("+party CODE").
    joins: std::sync::Mutex<std::sync::mpsc::Receiver<String>>,
    client: steamworks::Client,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Friend {
    pub id: u64,
    pub name: String,
    pub presence: Presence,
    /// Online but away / snoozing (a dimmer dot).
    pub away: bool,
    /// What they're doing: our rich presence status when in game, else Online/Away/Offline.
    pub status: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Presence {
    InGame,
    Online,
    Offline,
}

/// What a Steam "Join game" or invite carries: our party code.
pub fn connect_string(code: &str) -> String {
    format!("+party {code}")
}

/// The party code in a connect string or the game's command line (`+party CODE`).
pub fn party_code<S: AsRef<str>>(args: &[S]) -> Option<String> {
    let i = args.iter().position(|a| a.as_ref() == "+party")?;
    args.get(i + 1).map(|c| c.as_ref().to_string())
}

impl Steam {
    /// A fresh Steam login for the lobby (a ticket is used once).
    pub fn login(&self) -> Option<Box<dyn FnMut() -> Option<net::lobby::SteamLogin> + Send + Sync>> {
        #[cfg(feature = "steam")]
        if let Some(sdk) = &self.sdk {
            let client = sdk.client.clone();
            let steam_id = self.me;
            return Some(Box::new(move || {
                // ponytail: tickets are never cancelled; Steam drops them when the game exits.
                let (_ticket, ticket) = client.user().authentication_session_ticket(steamworks::networking_types::NetworkingIdentity::new());
                Some(net::lobby::SteamLogin { steam_id, ticket })
            }));
        }
        None
    }

    /// Invite a friend who isn't in the game: Steam shows them the invite and starts the game.
    pub fn invite(&self, friend: u64, code: &str) {
        #[cfg(feature = "steam")]
        if let Some(sdk) = &self.sdk {
            sdk.client.friends().get_friend(steamworks::SteamId::from_raw(friend)).invite_user_to_game(&connect_string(code));
        }
        let _ = (friend, code);
    }

    /// Tell friends what we're doing (and let them join our party from Steam).
    pub fn set_presence(&mut self, code: &str, status: &str) {
        let key = format!("{code}|{status}");
        if self.presence == key {
            return;
        }
        self.presence = key;
        #[cfg(feature = "steam")]
        if let Some(sdk) = &self.sdk {
            let f = sdk.client.friends();
            f.set_rich_presence("status", Some(status));
            f.set_rich_presence("connect", Some(&connect_string(code)));
        }
    }

    /// Party codes from Steam "Join game" / accepted invites since last asked.
    pub fn take_joins(&self) -> Vec<String> {
        #[cfg(feature = "steam")]
        if let Some(sdk) = &self.sdk {
            let rx = sdk.joins.lock().unwrap();
            return rx.try_iter().filter_map(|c| party_code(&c.split_whitespace().collect::<Vec<_>>())).collect();
        }
        Vec::new()
    }
}

/// Run Steam's callbacks; every couple of seconds refresh friends and fetch missing avatars.
#[cfg(feature = "steam")]
fn update(mut steam: ResMut<Steam>, mut images: ResMut<Assets<Image>>, time: Res<Time>, mut dirty: ResMut<crate::menu::MenuDirty>) {
    let steam = &mut *steam;
    let Some(sdk) = &steam.sdk else { return };
    sdk.client.run_callbacks();
    steam.refresh -= time.delta_secs();
    if steam.refresh > 0.0 {
        return;
    }
    steam.refresh = 2.0;
    let friends = sdk.client.friends();
    let ours = steamworks::AppId(APP_ID);
    let mut list: Vec<Friend> = friends
        .get_friends(steamworks::FriendFlags::IMMEDIATE)
        .into_iter()
        .map(|f| {
            let in_game = f.game_played().is_some_and(|g| g.game.app_id() == ours);
            let state = f.state();
            let (presence, status) = if in_game {
                (Presence::InGame, f.rich_presence("status").unwrap_or_else(|| "In game".into()))
            } else {
                match state {
                    steamworks::FriendState::Offline | steamworks::FriendState::Invisible => (Presence::Offline, "Offline".into()),
                    steamworks::FriendState::Away | steamworks::FriendState::Snooze => (Presence::Online, "Away".into()),
                    _ => (Presence::Online, "Online".into()),
                }
            };
            let away = matches!(state, steamworks::FriendState::Away | steamworks::FriendState::Snooze) && presence == Presence::Online;
            Friend { id: f.id().raw(), name: f.nick_name().unwrap_or_else(|| f.name()), presence, away, status }
        })
        .collect();
    list.sort_by(|a, b| (a.presence, a.name.to_lowercase()).cmp(&(b.presence, b.name.to_lowercase())));
    // Avatars: ours and every friend's (offline too); Steam sends them over time, so ask
    // again until they come.
    for id in std::iter::once(steam.me).chain(list.iter().map(|f| f.id)) {
        if steam.avatars.contains_key(&id) {
            continue;
        }
        let f = friends.get_friend(steamworks::SteamId::from_raw(id));
        match f.medium_avatar() {
            Some(rgba) => {
                let img = crate::level::rgba_image(64, 64, rgba, false, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb);
                steam.avatars.insert(id, images.add(img));
                dirty.0 = true;
            }
            None => _ = friends.request_user_information(f.id(), false),
        }
    }
    if list != steam.friends {
        steam.friends = list;
        dirty.0 = true;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn party_code_from_connect_string_and_command_line() {
        assert_eq!(super::party_code(&["+party", "KX7-42Q"]).as_deref(), Some("KX7-42Q"));
        assert_eq!(super::party_code(&["client", "+party", "ABC-DEF"]).as_deref(), Some("ABC-DEF"));
        assert_eq!(super::party_code(&["client"]), None);
        assert_eq!(super::party_code(&super::connect_string("ABC-DEF").split(' ').collect::<Vec<_>>()).as_deref(), Some("ABC-DEF"));
    }
}
