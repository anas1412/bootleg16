//! The lobby service (the "master"): everyone online is in a party (a party of one to start,
//! like CS2) with an invite code. A party queues for a casual match, or plays a custom game:
//! up to 10 people split into team A and team B. When a match is ready it reserves a free
//! official server and gives each player a ticket for it. Game servers announce themselves
//! here; it never carries game traffic.
//!
//! Over UDP, kept simple: players send commands and poll their state every second; every
//! answer is the full state, so a lost packet never leaves anyone out of date. Requests are
//! padded so answers are never bigger than requests (nobody can use this to flood others).
use crate::protocol::{decode, encode};
use crate::server::Group;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

mod client;
mod persist;
mod queue;
mod standby;
pub use standby::Role;
pub use client::LobbyClient;

pub const MASTER_PORT: u16 = 27010;
/// Game servers announce this often and are forgotten after missing a few.
pub const HEARTBEAT_SECONDS: u64 = 5;
const SERVER_TIMEOUT: Duration = Duration::from_secs(HEARTBEAT_SECONDS * 3);
/// Players poll every second; ten seconds of silence and they're gone.
const PLAYER_TIMEOUT: Duration = Duration::from_secs(10);
/// Every player request is padded to this: the biggest state answer fits in it.
pub const PAD: usize = 1200;
/// Packets per second one IP may send the lobby (a player polls once a second; a household
/// of players and their clicks fit easily).
const IP_RATE: f32 = 40.0;
/// Signed-in players from one IP (loopback exempt: tests run many from it), and in all.
const SESSIONS_PER_IP: usize = 10;
const MAX_PLAYERS: usize = 50_000;
/// Game servers in the pool (official ones only).
const MAX_SERVERS: usize = 2_000;
/// What a state answer may hold, so it never outgrows the padded request (see the test).
pub const MAX_REGIONS: usize = 8;
pub const MAX_INVITES: usize = 5;
pub const MAX_PARTY: usize = 5;
pub const MAX_CUSTOM: usize = 10;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LobbyMsg {
    /// Game server -> lobby: "I'm here", and whether it's running a match.
    /// `address`: where players connect (empty = where this came from).
    /// Sealed with the official token's key (see `crypto::ServerKey`).
    Heartbeat { name: String, map: String, players: u8, max: u8, region: String, busy: bool, address: String },
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
    /// Game server -> lobby: a player reported another (by their match tickets). Sealed.
    Report { reporter: u64, target: u64, reason: crate::protocol::ReportReason, map: String, round: u32 },
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
    /// Match ready: the server and your ticket.
    /// Match ready: the server, your ticket and your key for it.
    pub found: Option<(SocketAddr, u64, [u8; 32])>,
    /// Something to tell the player ("no server free", "party full"...).
    pub notice: String,
    /// Invites to other parties, newest last.
    pub invites: Vec<Invite>,
    /// One server per region to measure your ping to (answer with `Command::Pings`).
    pub regions: Vec<(String, SocketAddr)>,
}

struct Player {
    /// The encrypted session (commands in, states out).
    channel: crate::crypto::Channel,
    id: u32,
    name: String,
    /// Steam id once signed in with Steam (0 = anonymous).
    steam: u64,
    addr: SocketAddr,
    party: u32,
    seen: Instant,
    found: Option<(SocketAddr, u64, [u8; 32])>,
    notice: String,
    /// Who invited us (player id) and when.
    invites: Vec<(u32, Instant)>,
    /// Ping per region, as the player measured it.
    pings: HashMap<String, u16>,
}

struct Party {
    code: String,
    leader: u32,
    mode: Mode,
    /// Members in order joined, with their custom-game team.
    members: Vec<(u32, Group)>,
    queued: Option<Instant>,
}

struct GameServer {
    /// Where players connect (the heartbeat's source unless the server says otherwise).
    public: SocketAddr,
    region: String,
    /// Which official token (its key seals our messages to it).
    key: usize,
    official: bool,
    busy: bool,
    seen: Instant,
    /// When its newest heartbeat was sealed (older ones are replays).
    sent: u64,
}

pub struct Master {
    udp: UdpSocket,
    servers: HashMap<SocketAddr, GameServer>,
    /// session secret -> player.
    players: HashMap<u64, Player>,
    parties: HashMap<u32, Party>,
    next_id: u32,
    /// Our long-term key (its public half is built into the game) and the secret behind the
    /// sign-in cookies.
    key: crate::crypto::LobbyKey,
    cookie_secret: [u8; 32],
    /// The official tokens' keys (game servers seal everything with theirs).
    pub(super) keys: Vec<crate::crypto::ServerKey>,
    /// Packet budgets per IP (players and servers alike).
    limits: crate::secure::Buckets<std::net::IpAddr>,
    started: Instant,
    /// Players per casual match (10; tests use 2).
    pub match_size: usize,
    /// When the last status line was printed.
    reported: Instant,
    pub steam_mode: SteamMode,
    /// Only players signed in with Steam (official lobbies: keeps test bots and anonymous
    /// clients out of real matches).
    pub require_steam: bool,
    /// Steam logins waiting for Steam's answer: (session, Steam id, ticket).
    steam_checks: Vec<(u64, u64, Vec<u8>)>,
    /// Who holds each match ticket (Steam id, name), for reports; kept a few hours.
    pub(super) seats: HashMap<u64, (u64, String, Instant)>,
    /// Where reports are written (one JSON object per line); None = only printed.
    pub reports: Option<std::path::PathBuf>,
    /// Bookings the server hasn't confirmed yet (a busy heartbeat): the message, when it was
    /// first and last sent. Resent until confirmed, so a lost packet can't lose a match.
    pub(super) booking: HashMap<SocketAddr, (Vec<u8>, Instant, Instant)>,
    /// Active (serving players) or standby (a paired lobby is; see `standby`).
    pub role: Role,
    pub(super) peer: Option<standby::Peer>,
    /// Saved here every couple of seconds, and loaded at start (see `persist`).
    pub state_file: Option<std::path::PathBuf>,
    saved: Instant,
}

impl Master {
    pub fn bind(addr: SocketAddr, tokens: Vec<String>) -> std::io::Result<Master> {
        let udp = UdpSocket::bind(addr)?;
        udp.set_read_timeout(Some(Duration::from_millis(100)))?;
        let keys = tokens.iter().map(|t| crate::crypto::ServerKey::from_token(t)).collect();
        Ok(Master { udp, key: crate::crypto::LobbyKey::generate(), cookie_secret: crate::crypto::new_key(), keys, servers: HashMap::new(), players: HashMap::new(), parties: HashMap::new(), next_id: 1, limits: crate::secure::Buckets::new(100_000), started: Instant::now(), match_size: 10, reported: Instant::now(), steam_mode: SteamMode::Trust, require_steam: false, steam_checks: Vec::new(), seats: HashMap::new(), reports: None, booking: HashMap::new(), role: Role::Active, peer: None, state_file: None, saved: Instant::now() })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.udp.local_addr().expect("bound")
    }

    /// Tickets, sessions and codes come from the OS random source: seeing your own must not
    /// tell you anyone else's.
    fn rand(&mut self) -> u64 {
        crate::secure::random()
    }

    /// A party code nobody else has.
    fn new_code(&mut self) -> String {
        loop {
            let code = self.random_code();
            if !self.parties.values().any(|p| p.code == code) {
                return code;
            }
        }
    }

    fn random_code(&mut self) -> String {
        // No 0/O/1/I: easy to read out to a friend.
        const ABC: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let r = self.rand();
        let c: String = (0..6).map(|i| ABC[((r >> (i * 5)) & 31) as usize] as char).collect();
        format!("{}-{}", &c[..3], &c[3..])
    }

    fn new_party(&mut self, player: u32) -> u32 {
        let code = self.new_code();
        let id = self.next_id;
        self.next_id += 1;
        self.parties.insert(id, Party { code, leader: player, mode: Mode::Casual, members: vec![(player, Group::A)], queued: None });
        id
    }

    /// Take a player out of their party (it closes if they were the last).
    fn leave(&mut self, player: u32, party: u32) {
        if let Some(p) = self.parties.get_mut(&party) {
            p.members.retain(|(m, _)| *m != player);
            p.queued = None;
            if p.members.is_empty() {
                self.parties.remove(&party);
            } else if p.leader == player {
                p.leader = p.members[0].0;
            }
        }
    }

    /// Handle what has arrived (waits up to 100 ms), then run matchmaking.
    pub fn step(&mut self) {
        let mut buf = [0u8; 2048];
        let _ = self.udp.set_nonblocking(false);
        // At most this many packets a step: under a flood the lobby still gets round to
        // matchmaking, saving and its standby (the rest wait in, and overflow, the OS queue).
        let mut room = 20_000;
        while let Ok((n, from)) = self.udp.recv_from(&mut buf) {
            room -= 1;
            if room == 0 {
                break;
            }
            let _ = self.udp.set_nonblocking(true);
            // Every IP has a budget: floods cost nothing past here.
            let now = self.started.elapsed().as_secs_f64();
            // (Loopback can't be faked from outside, and tests run everyone from it.)
            if !from.ip().is_loopback() && !self.limits.allow(from.ip(), IP_RATE, now) {
                continue;
            }
            if buf[0] == crate::crypto::SERVER {
                self.from_server(from, &buf[..n]);
                continue;
            }
            // Standing by: the active lobby answers players.
            if self.role == Role::Standby {
                continue;
            }
            if buf[0] == crate::crypto::CHANNEL {
                self.from_player(from, &buf[..n]);
                continue;
            }
            match decode::<LobbyMsg>(&buf[..n]) {
                Some(LobbyMsg::KeyPlease { pad }) if pad.len() >= 64 => {
                    let _ = self.udp.send_to(&encode(&LobbyMsg::Key { public: self.key.public() }), from);
                }
                Some(LobbyMsg::Hello { cookie, public, sealed }) if sealed.len() >= PAD => self.hello(from, cookie, public, &sealed),
                _ => {}
            }
        }
        let now = Instant::now();
        self.seats.retain(|_, (_, _, at)| at.elapsed() < Duration::from_secs(6 * 3600));
        let secs = self.started.elapsed().as_secs_f64();
        self.limits.tidy(secs, 10.0);
        for (addr, (msg, first, last)) in &mut self.booking {
            if last.elapsed() > Duration::from_millis(500) {
                let _ = self.udp.send_to(msg, addr);
                *last = Instant::now();
            }
            if first.elapsed() > Duration::from_secs(5) {
                println!("server {addr} didn't take its match");
            }
        }
        self.booking.retain(|_, (_, first, _)| first.elapsed() <= Duration::from_secs(5));
        self.servers.retain(|_, s| {
            let alive = now - s.seen < SERVER_TIMEOUT;
            if !alive {
                println!("server {} stopped answering, out of the pool", s.public);
            }
            alive
        });
        self.peer_step();
        if self.role == Role::Standby {
            return;
        }
        let gone: Vec<(u32, u32)> = self.players.values().filter(|p| now - p.seen > PLAYER_TIMEOUT).map(|p| (p.id, p.party)).collect();
        for (id, party) in gone {
            self.leave(id, party);
            self.players.retain(|_, p| p.id != id);
        }
        self.matchmake();
        if let Some(path) = &self.state_file {
            if self.saved.elapsed() > Duration::from_secs(2) {
                self.saved = now;
                if let Err(e) = self.save(path) {
                    println!("can't save the lobby state to {}: {e}", path.display());
                }
            }
        }
        if now - self.reported > Duration::from_secs(60) {
            self.reported = now;
            let queued: usize = self.parties.values().filter(|p| p.queued.is_some()).map(|p| p.members.len()).sum();
            let free = self.servers.values().filter(|s| s.official && !s.busy).count();
            let busy = self.servers.values().filter(|s| s.busy).count();
            println!("{} online, {queued} in queue, servers: {free} free, {busy} in a match", self.players.len());
        }
    }

    pub fn run(&mut self) {
        loop {
            self.step();
        }
    }

    /// Our public key (build it into the game: `BOOTLEG16_LOBBY_KEY`).
    pub fn public_key(&self) -> [u8; 32] {
        self.key.public()
    }

    /// Use this long-term key (kept in a file, so the built-in public key stays valid).
    pub fn set_key(&mut self, key: crate::crypto::LobbyKey) {
        self.key = key;
    }

    /// The sign-in cookie for an address in a 10-second window (stateless).
    fn cookie(&self, from: SocketAddr, previous: bool) -> u64 {
        let window = self.started.elapsed().as_secs() / 10 - previous as u64;
        let mut h = blake3::Hasher::new_keyed(&self.cookie_secret);
        h.update(from.to_string().as_bytes());
        h.update(&window.to_le_bytes());
        u64::from_le_bytes(h.finalize().as_bytes()[..8].try_into().unwrap()) | 1
    }

    /// A sign-in: check the cookie (cheap) before the key exchange (not cheap), then read
    /// who it is and start their session.
    fn hello(&mut self, from: SocketAddr, cookie: u64, public: [u8; 32], sealed: &[u8]) {
        if cookie != self.cookie(from, false) && (self.started.elapsed().as_secs() < 10 || cookie != self.cookie(from, true)) {
            let _ = self.udp.send_to(&encode(&LobbyMsg::Cookie { cookie: self.cookie(from, false) }), from);
            return;
        }
        let Some((payload, fresh, channel)) = crate::crypto::lobby_hello(&self.key, &public, sealed) else { return };
        let Some(SignIn { name, steam, pad }) = decode::<SignIn>(&payload) else { return };
        if pad.len() < PAD {
            return;
        }
        let refuse = |m: &Master, reason: &str| _ = m.udp.send_to(&encode(&LobbyMsg::Refused { reason: reason.into() }), from);
        if steam.is_none() && self.require_steam {
            return refuse(self, "Start Steam to play online.");
        }
        if self.players.len() >= MAX_PLAYERS || (!from.ip().is_loopback() && self.players.values().filter(|p| p.addr.ip() == from.ip()).count() >= SESSIONS_PER_IP) {
            return refuse(self, "Too many players from your address, or the lobby is full. Try again later.");
        }
        let session = self.rand();
        let id = self.next_id;
        self.next_id += 1;
        let name: String = name.chars().filter(|c| !c.is_control()).take(24).collect();
        let party = self.new_party(id);
        self.players.insert(session, Player { channel, id, name, steam: 0, addr: from, party, seen: Instant::now(), found: None, notice: String::new(), invites: Vec::new(), pings: HashMap::new() });
        if let Some(login) = steam {
            match self.steam_mode {
                // Unchecked: never let a claim push anyone else out.
                SteamMode::Trust if self.players.values().any(|p| p.steam == login.steam_id) => {}
                SteamMode::Trust => self.steam_checked(login.steam_id, Some(session)),
                SteamMode::Verify => self.steam_checks.push((session, login.steam_id, login.ticket)),
            }
        }
        let p = self.players.get_mut(&session).unwrap();
        let sealed = p.channel.seal(session, &session.to_le_bytes());
        let _ = self.udp.send_to(&encode(&LobbyMsg::Welcome { public: fresh, sealed }), from);
    }

    /// A packet in a player's session: their command, answered with their state. The key
    /// proves who it is, so a player whose address changed (a phone between networks) carries on.
    fn from_player(&mut self, from: SocketAddr, packet: &[u8]) {
        let Some(session) = crate::crypto::channel_id(packet) else { return };
        let Some(p) = self.players.get_mut(&session) else {
            let _ = self.udp.send_to(&encode(&LobbyMsg::Expired), from);
            return;
        };
        let Some(plain) = p.channel.open(packet) else { return };
        let Some(LobbyMsg::Command { cmd, pad }) = decode::<LobbyMsg>(&plain) else { return };
        if pad.len() < PAD {
            return;
        }
        p.addr = from;
        self.command(session, cmd);
        if let Some(state) = self.state_of(session) {
            let p = self.players.get_mut(&session).unwrap();
            let sealed = p.channel.seal(session, &encode(&LobbyMsg::State(state)));
            let _ = self.udp.send_to(&sealed, from);
        }
    }

    /// Steam logins to check (`SteamMode::Verify`): (session, Steam id, ticket).
    pub fn take_steam_checks(&mut self) -> Vec<(u64, u64, Vec<u8>)> {
        std::mem::take(&mut self.steam_checks)
    }

    /// Steam's answer for a login: `session` is who passed (None = the ticket was bad).
    /// Signing in somewhere else ends your older session.
    pub fn steam_checked(&mut self, steam_id: u64, session: Option<u64>) {
        let Some(session) = session.filter(|s| self.players.contains_key(s)) else { return };
        let older: Vec<(u32, u32)> = self.players.iter().filter(|(s, p)| p.steam == steam_id && **s != session).map(|(_, p)| (p.id, p.party)).collect();
        for (id, party) in older {
            self.leave(id, party);
            self.players.retain(|_, p| p.id != id);
        }
        self.players.get_mut(&session).unwrap().steam = steam_id;
    }

    /// A Steam login was refused: the player stays signed in without Steam, and is told.
    pub fn steam_refused(&mut self, session: u64) {
        if self.require_steam {
            let Some(p) = self.players.remove(&session) else { return };
            self.leave(p.id, p.party);
            let _ = self.udp.send_to(&encode(&LobbyMsg::Refused { reason: "Steam couldn't confirm your sign-in. Restart Steam and try again.".into() }), p.addr);
        } else if let Some(p) = self.players.get_mut(&session) {
            p.notice = "Steam sign-in failed: playing without Steam.".into();
        }
    }

    fn player_party(&self, session: u64) -> Option<(u32, u32)> {
        self.players.get(&session).map(|p| (p.id, p.party))
    }

    fn command(&mut self, session: u64, cmd: Command) {
        let Some((me, party)) = self.player_party(session) else { return };
        if let Some(p) = self.players.get_mut(&session) {
            p.seen = Instant::now();
        }
        let leader = self.parties.get(&party).is_some_and(|p| p.leader == me);
        let mut notice = String::new();
        // Official lobbies: nothing but waiting until Steam has confirmed who you are.
        if self.require_steam && self.players.get(&session).is_some_and(|p| p.steam == 0) && cmd != Command::Poll {
            if let Some(p) = self.players.get_mut(&session) {
                p.notice = "Waiting for Steam to confirm your sign-in...".into();
            }
            return;
        }
        match cmd {
            Command::Poll => {}
            Command::JoinParty { code } => {
                let code = code.trim().to_ascii_uppercase();
                match self.parties.iter().find(|(_, p)| p.code == code).map(|(id, p)| (*id, p.members.len(), p.mode)) {
                    Some((target, _, _)) if target == party => {}
                    Some((target, n, mode)) => {
                        let max = if mode == Mode::Custom { MAX_CUSTOM } else { MAX_PARTY };
                        if n >= max {
                            notice = "That party is full.".into();
                        } else {
                            self.leave(me, party);
                            let t = self.parties.get_mut(&target).unwrap();
                            // New members go to the smaller custom team.
                            let a = t.members.iter().filter(|(_, g)| *g == Group::A).count();
                            t.members.push((me, if a * 2 <= n { Group::A } else { Group::B }));
                            t.queued = None;
                            self.players.get_mut(&session).unwrap().party = target;
                        }
                    }
                    None => notice = "No party with that code.".into(),
                }
            }
            Command::LeaveParty => {
                self.leave(me, party);
                let p = self.new_party(me);
                self.players.get_mut(&session).unwrap().party = p;
            }
            Command::SetMode(mode) if leader => {
                let p = self.parties.get_mut(&party).unwrap();
                if mode == Mode::Casual && p.members.len() > MAX_PARTY {
                    notice = "Casual parties are up to 5. Use a custom game for more.".into();
                } else {
                    p.mode = mode;
                    p.queued = None;
                }
            }
            Command::Move { member, group } if leader => {
                if let Some(m) = self.parties.get_mut(&party).unwrap().members.iter_mut().find(|(m, _)| *m == member) {
                    m.1 = group;
                }
            }
            Command::Play if leader => {
                let mode = self.parties[&party].mode;
                match mode {
                    Mode::Casual => self.parties.get_mut(&party).unwrap().queued = Some(Instant::now()),
                    Mode::Custom => {
                        let members = self.parties[&party].members.clone();
                        if members.len() < 2 {
                            notice = "Invite at least one friend first.".into();
                        } else if !self.start_custom(members) {
                            notice = "No server free right now. Try again in a moment.".into();
                        }
                    }
                }
            }
            Command::Cancel if leader => self.parties.get_mut(&party).unwrap().queued = None,
            Command::Pings(pings) => {
                if let Some(p) = self.players.get_mut(&session) {
                    p.pings = pings.into_iter().take(32).map(|(r, ms)| (r.chars().take(16).collect(), ms)).collect();
                }
            }
            Command::Invite { steam } => match self.players.values_mut().find(|p| p.steam == steam && steam != 0) {
                Some(friend) => {
                    friend.invites.retain(|(from, _)| *from != me);
                    friend.invites.push((me, Instant::now()));
                    // The newest few (spamming invites can't push the answer past its size).
                    let extra = friend.invites.len().saturating_sub(MAX_INVITES);
                    friend.invites.drain(..extra);
                }
                None => notice = "They're not in the game right now.".into(),
            },
            _ => notice = "Only the party leader can do that.".into(),
        }
        if !notice.is_empty() {
            if let Some(p) = self.players.get_mut(&session) {
                p.notice = notice;
            }
        }
    }

    fn state_of(&mut self, session: u64) -> Option<LobbyState> {
        let p = self.players.get_mut(&session)?;
        let notice = std::mem::take(&mut p.notice);
        p.invites.retain(|(_, at)| at.elapsed().as_secs_f32() < INVITE_SECONDS);
        let invited = p.invites.clone();
        let (you, party_id, found) = (p.id, p.party, p.found.take());
        let party = self.parties.get(&party_id)?;
        let who = |id: u32| self.players.values().find(|q| q.id == id).map(|q| (q.name.clone(), q.steam)).unwrap_or_default();
        Some(LobbyState {
            you,
            code: party.code.clone(),
            leader: party.leader,
            mode: party.mode,
            members: party.members.iter().map(|(m, g)| (*m, who(*m), *g)).map(|(id, (name, steam), group)| Member { id, name, group, steam }).collect(),
            searching: party.queued.map(|t| t.elapsed().as_secs_f32()),
            found,
            notice,
            regions: self.ping_targets().into_iter().take(MAX_REGIONS).collect(),
            invites: invited
                .iter()
                .filter_map(|(from, at)| {
                    let q = self.players.values().find(|q| q.id == *from)?;
                    let code = self.parties.get(&q.party)?.code.clone();
                    Some(Invite { from: q.name.clone(), steam: q.steam, code, left: INVITE_SECONDS - at.elapsed().as_secs_f32() })
                })
                .collect(),
        })
    }
}

impl Master {
    /// Write a report down for review: who, whom (Steam ids and names), why, where.
    fn file_report(&mut self, reporter: u64, target: u64, reason: crate::protocol::ReportReason, map: &str, round: u32) {
        let (Some(by), Some(of)) = (self.seats.get(&reporter), self.seats.get(&target)) else { return };
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        // Names have no control characters (see Hello), so Debug quoting is valid JSON here.
        let line = format!(
            "{{\"time\":{time},\"reason\":\"{reason:?}\",\"reporter\":{{\"steam\":{},\"name\":{:?}}},\"reported\":{{\"steam\":{},\"name\":{:?}}},\"map\":{map:?},\"round\":{round}}}",
            by.0, by.1, of.0, of.1
        );
        println!("report: {line}");
        if let Some(path) = &self.reports {
            use std::io::Write;
            match std::fs::OpenOptions::new().create(true).append(true).open(path) {
                Ok(mut f) => _ = writeln!(f, "{line}"),
                Err(e) => println!("can't write reports to {}: {e}", path.display()),
            }
        }
    }
}

impl Master {
    /// A sealed message from a game server: only official ones (holding a token) can make
    /// one, and an old one replayed is ignored.
    fn from_server(&mut self, from: SocketAddr, packet: &[u8]) {
        let Some(id) = crate::crypto::server_key_id(packet) else { return };
        let Some(key) = self.keys.iter().position(|k| k.id == id) else { return };
        let Some((sent, body)) = self.keys[key].open(packet) else { return };
        match decode::<LobbyMsg>(&body) {
            Some(LobbyMsg::Heartbeat { busy, address, region, .. }) => {
                if self.servers.get(&from).is_some_and(|s| sent < s.sent) || (!self.servers.contains_key(&from) && self.servers.len() >= MAX_SERVERS) {
                    return;
                }
                let region: String = region.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(16).collect();
                let public = address.parse().unwrap_or(from);
                if !self.servers.contains_key(&from) {
                    println!("server {public} ({region}) joined the pool");
                }
                if busy {
                    self.booking.remove(&from); // took the match
                }
                // Booked, not confirmed yet: still busy as far as we're concerned.
                let busy = busy || self.booking.contains_key(&from);
                self.servers.insert(from, GameServer { public, region, key, official: true, busy, seen: Instant::now(), sent });
            }
            Some(LobbyMsg::Report { reporter, target, reason, map, round }) if self.role == Role::Active => self.file_report(reporter, target, reason, &map, round),
            _ => {}
        }
    }
}
