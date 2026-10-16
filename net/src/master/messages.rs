//! What players, the lobby and game servers say to each other (the lobby's wire format).
use super::*;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LobbyMsg {
    /// Game server -> lobby: "I'm here", and whether it's running a match.
    /// `address`: where players connect (empty = where this came from).
    /// Sealed with the official token's key (see `crypto::ServerKey`).
    Heartbeat { name: String, map: String, players: u8, max: u8, region: String, busy: bool, address: String, deathmatch: bool },
    /// Player -> lobby: sign in. `public`: a fresh key for this session; `sealed`: a `SignIn`
    /// only the lobby can read (see `crypto::client_hello`). `cookie`: 0 at first, then the
    /// one from `Cookie` (proves the address is real before the lobby does any crypto).
    Hello { cookie: u64, public: [u8; 32], sealed: Vec<u8> },
    /// Lobby -> player: say hello again with this.
    Cookie { cookie: u64 },
    /// Lobby -> player: signed in. `public`: the lobby's fresh key (finishes the handshake);
    /// `sealed`: the session number, over the new channel.
    Welcome { public: [u8; 32], sealed: Vec<u8> },
    /// Player -> lobby, in the session channel: do something (or just `Poll`).
    Command { cmd: Command, pad: Vec<u8> },
    /// Player -> lobby: what's your public key? Only for builds without one built in (tests,
    /// development): trusting the answer is only as good as the network it came over.
    KeyPlease { pad: Vec<u8> },
    Key { public: [u8; 32] },
    /// Lobby -> player: everything about you now.
    State(LobbyState),
    /// Lobby -> player: unknown session (the lobby restarted, or you timed out): sign in again.
    Expired,
    /// Lobby -> player: not let in (this lobby needs a Steam login).
    Refused { reason: String },
    /// Game server -> lobby, at the end of a match (sealed): who won and lost, by ticket, and
    /// who abandoned (gone for good before the end). Empty `winners`: a draw.
    Result { winners: Vec<u64>, losers: Vec<u64>, abandoned: Vec<u64> },
    /// Game server -> lobby: a player reported another (by their match tickets). Sealed.
    /// `demo`, `tick`: the match recording and the moment of the report (empty: not recorded).
    Report { reporter: u64, target: u64, reason: crate::protocol::ReportReason, map: String, round: u32, demo: String, tick: u32 },
}

/// What a player signs in with (inside the sealed hello).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SignIn {
    pub name: String,
    /// A Steam login (the lobby checks it with Steam); None = anonymous (tests, no Steam).
    pub steam: Option<SteamLogin>,
    pub pad: Vec<u8>,
}

/// Steam sign-in: who you say you are and Steam's ticket proving it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SteamLogin {
    pub steam_id: u64,
    pub ticket: Vec<u8>,
}

/// How the lobby treats Steam logins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteamMode {
    /// Take the claimed Steam id as is (tests and lobbies built without Steam; never official).
    Trust,
    /// Ask Steam: logins wait in `take_steam_checks` until `steam_checked` answers.
    Verify,
}

/// A party member as everyone in the party sees them.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Member {
    pub id: u32,
    pub name: String,
    /// Team in a custom game.
    pub group: Group,
    /// Their Steam id (for the avatar), 0 = not signed in with Steam.
    pub steam: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Command {
    Poll,
    /// Join the party with this invite code (leaving yours).
    JoinParty { code: String },
    /// Leave your party (you get a new one of your own).
    LeaveParty,
    /// Leader: casual queue or custom game.
    SetMode(Mode),
    /// Leader, custom games: put a member on team A or B.
    Move { member: u32, group: Group },
    /// Leader: queue for casual / start the custom game.
    Play,
    /// Leader: leave the queue.
    Cancel,
    /// Invite a friend who is in the game (by Steam id) to your party.
    Invite { steam: u64 },
    /// Your ping (ms) to each region (from `LobbyState::regions`); the queue uses it.
    Pings(Vec<(String, u16)>),
    /// A match was found (`LobbyState::ready`): in, or out (out: a short queue cooldown).
    Accept,
    Decline,
    /// A seat on a deathmatch server (comes back as `found`).
    Deathmatch,
}

/// Lobby -> game server, sealed with its token's key.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LobbyOrder {
    /// Run this match.
    Book(crate::server::MatchSetup),
    /// Deathmatch: let this ticket in, encrypted with this key.
    Seat { ticket: u64, key: [u8; 32] },
}

/// A match found, waiting for everyone to accept.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct ReadyNet {
    pub accepted: u8,
    pub total: u8,
    /// Seconds left to accept.
    pub left: f32,
    /// You've accepted.
    pub you: bool,
}

/// How long an invite stays up.
pub const INVITE_SECONDS: f32 = 30.0;

/// An invite to someone's party.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Invite {
    pub from: String,
    /// The inviter's Steam id (for the avatar).
    pub steam: u64,
    /// Their party's code: accepting is joining it.
    pub code: String,
    /// Seconds before it goes away.
    pub left: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Matchmaking with strangers: up to 5 friends, always on one team.
    Casual,
    /// Private game: up to 10, you pick the teams; knife round for sides.
    Custom,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LobbyState {
    /// Your public id (party members see it).
    pub you: u32,
    pub code: String,
    pub leader: u32,
    pub mode: Mode,
    pub members: Vec<Member>,
    /// Seconds in the casual queue, if queued.
    pub searching: Option<f32>,
    /// A match found: accept it (`Command::Accept`) before the time runs out.
    pub ready: Option<ReadyNet>,
    /// Can't queue for this many more seconds (didn't accept matches, abandoned one).
    pub cooldown: Option<f32>,
    /// Match ready: the server, your ticket and your key for it.
    pub found: Option<(SocketAddr, u64, [u8; 32])>,
    /// Your match is still running (you left it, or your game restarted): go back with this.
    pub rejoin: Option<(SocketAddr, u64, [u8; 32])>,
    /// Something to tell the player ("no server free", "party full"...).
    pub notice: String,
    /// Invites to other parties, newest last.
    pub invites: Vec<Invite>,
    /// One server per region to measure your ping to (answer with `Command::Pings`).
    pub regions: Vec<(String, SocketAddr)>,
}
