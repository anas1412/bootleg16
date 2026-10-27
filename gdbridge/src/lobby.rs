//! The lobby connection for the Godot client: your party, the casual queue, custom games and
//! invites, as the Bevy client's `play.rs` does it. Godot ticks it every frame from an autoload
//! (so the party survives matches), reads the state as a dictionary and sends commands.
use godot::prelude::*;
use net::lobby::{Command, LobbyClient, LobbyState, Mode};
use net::server::Group;

/// The official lobby, built into release builds (`BOOTLEG16_LOBBY=host:port cargo build`);
/// setting `BOOTLEG16_LOBBY` when starting the game overrides it (testing another lobby).
fn lobby_address() -> String {
    let built_in = option_env!("BOOTLEG16_LOBBY").map(String::from).unwrap_or_else(|| format!("127.0.0.1:{}", net::lobby::LOBBY_PORT));
    std::env::var("BOOTLEG16_LOBBY").unwrap_or(built_in)
}

/// The official lobby's public key (built in, or `BOOTLEG16_LOBBY_KEY=<hex>` at run time for
/// testing another lobby). None: learned from the lobby (development only).
fn lobby_key() -> Option<[u8; 32]> {
    std::env::var("BOOTLEG16_LOBBY_KEY").ok().or(option_env!("BOOTLEG16_LOBBY_KEY").map(String::from)).and_then(|k| net::crypto::from_hex(&k))
}

#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct Lobby {
    client: Option<LobbyClient>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for Lobby {
    fn init(base: Base<RefCounted>) -> Self {
        Lobby { client: None, base }
    }
}

#[godot_api]
impl Lobby {
    /// Start talking to the lobby as `name`, telling it the pool `maps` this game has (a match
    /// only gets a map everyone has). False (and an error in the log) if there's no address
    /// or no socket. Never blocks: the lobby is reached in `tick`.
    #[func]
    fn start(&mut self, name: GString, maps: PackedStringArray) -> bool {
        // literal addresses only: a name lookup would block the game
        let lobbies = net::config::addresses(&lobby_address(), false);
        match LobbyClient::with_lobbies(lobbies, &name.to_string()) {
            Ok(mut c) => {
                c.lobby_key = lobby_key();
                c.builtin_key = c.lobby_key.is_some();
                c.maps = maps.as_slice().iter().map(|m| m.to_string()).collect();
                self.client = Some(c);
                true
            }
            Err(e) => {
                godot_error!("bootleg16: no lobby connection ({e})");
                false
            }
        }
    }

    /// Sign in, resend, poll and measure pings, as needed. Call every frame.
    #[func]
    fn tick(&mut self) {
        if let Some(c) = &mut self.client {
            c.tick();
        }
    }

    /// Signed in and heard from the lobby.
    #[func]
    fn is_online(&self) -> bool {
        self.client.as_ref().is_some_and(|c| c.online())
    }

    /// The party and queue as the lobby last told us (empty until signed in):
    /// you, code, leader, mode (0 casual, 1 custom), members [{id, name, group (0 A, 1 B),
    /// steam}], searching (seconds in the queue, -1 when not), ready ({accepted, total, left,
    /// you} or empty), cooldown (seconds, -1 for none), can_rejoin, invites [{from, code,
    /// left}], map ("" for random).
    #[func]
    fn state(&self) -> VarDictionary {
        let Some(s) = self.client.as_ref().and_then(|c| c.state.as_ref()) else { return VarDictionary::new() };
        state_dict(s)
    }

    /// The lobby's last message for the player ("" if none); taken, so it shows once.
    #[func]
    fn take_notice(&mut self) -> GString {
        GString::from(self.client.as_mut().map(|c| std::mem::take(&mut c.notice)).unwrap_or_default().as_str())
    }

    /// A match to go to ({address, ticket, key}, or empty); taken, so it's joined once.
    #[func]
    fn take_found(&mut self) -> VarDictionary {
        let Some((addr, ticket, key)) = self.client.as_mut().and_then(|c| c.found.take()) else { return VarDictionary::new() };
        vdict! { "address" => addr.to_string(), "ticket" => ticket as i64, "key" => &PackedByteArray::from(&key[..]) }
    }

    /// Go back to the match you're still in (the state's can_rejoin): it comes back as found.
    #[func]
    fn rejoin(&mut self) {
        if let Some(c) = &mut self.client {
            c.found = c.state.as_ref().and_then(|s| s.rejoin);
        }
    }

    #[func]
    fn join_party(&mut self, code: GString) {
        self.send(Command::JoinParty { code: code.to_string().trim().to_uppercase() });
    }

    #[func]
    fn leave_party(&mut self) {
        self.send(Command::LeaveParty);
    }

    /// Leader: 0 casual queue, 1 custom game.
    #[func]
    fn set_mode(&mut self, mode: i64) {
        self.send(Command::SetMode(if mode == 1 { Mode::Custom } else { Mode::Casual }));
    }

    /// Leader: queue for casual, or start the custom game.
    #[func]
    fn play(&mut self) {
        self.send(Command::Play);
    }

    /// Leader: leave the queue.
    #[func]
    fn cancel(&mut self) {
        self.send(Command::Cancel);
    }

    /// A match was found: take it, or pass (passing costs a short cooldown).
    #[func]
    fn accept_match(&mut self) {
        self.send(Command::Accept);
    }

    #[func]
    fn decline_match(&mut self) {
        self.send(Command::Decline);
    }

    /// Leader, custom games: put a member on team A (0) or B (1).
    #[func]
    fn move_member(&mut self, member: i64, group: i64) {
        self.send(Command::Move { member: member as u32, group: if group == 1 { Group::B } else { Group::A } });
    }

    /// Leader, custom games: play this map ("" for a random one).
    #[func]
    fn set_map(&mut self, map: GString) {
        let m = map.to_string();
        self.send(Command::SetMap((!m.is_empty()).then_some(m)));
    }

    fn send(&mut self, cmd: Command) {
        if let Some(c) = &mut self.client {
            c.send(cmd);
        }
    }
}

fn state_dict(s: &LobbyState) -> VarDictionary {
    let mut members = VarArray::new();
    for m in &s.members {
        members.push(&vdict! { "id" => m.id as i64, "name" => m.name.as_str(), "group" => if m.group == Group::B { 1i64 } else { 0 }, "steam" => m.steam as i64 }.to_variant());
    }
    let mut invites = VarArray::new();
    for i in &s.invites {
        invites.push(&vdict! { "from" => i.from.as_str(), "code" => i.code.as_str(), "left" => i.left }.to_variant());
    }
    let ready = s.ready.map(|r| vdict! { "accepted" => r.accepted as i64, "total" => r.total as i64, "left" => r.left, "you" => r.you }).unwrap_or_default();
    vdict! {
        "you" => s.you as i64,
        "code" => s.code.as_str(),
        "leader" => s.leader as i64,
        "mode" => if s.mode == Mode::Custom { 1i64 } else { 0 },
        "members" => &members,
        "searching" => s.searching.unwrap_or(-1.0),
        "ready" => &ready,
        "cooldown" => s.cooldown.unwrap_or(-1.0),
        "can_rejoin" => s.rejoin.is_some(),
        "invites" => &invites,
        "map" => s.map.clone().unwrap_or_default().as_str(),
    }
}
