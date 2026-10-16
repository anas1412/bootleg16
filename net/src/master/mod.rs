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
mod messages;
mod party;
mod servers;
mod signin;
pub use messages::*;
mod persist;
mod matchmaker;
mod queue;
pub mod rating;
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
    /// A deathmatch server (drop-in seats, never booked for matches), its players and slots,
    /// and the seats we gave out lately (not in its player count yet).
    deathmatch: bool,
    players: u8,
    max: u8,
    seated: Vec<Instant>,
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
    cookies: crate::secure::Cookies,
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
    /// Hidden skill per Steam id (see `rating`), kept in `ratings_file` for good.
    pub(super) ratings: HashMap<u64, rating::Rating>,
    pub ratings_file: Option<std::path::PathBuf>,
    /// Queue cooldowns: (who: Steam id, or a session's player id for players without Steam)
    /// -> (how many in the last day, until when: unix seconds).
    pub(super) penalties: HashMap<u64, (u32, u64)>,
    /// Matches found, waiting for everyone to accept.
    pub(super) checks: Vec<matchmaker::ReadyCheck>,
    /// Running matches, per player (Steam id, or a session's player key): where, and their
    /// seat. Offered as `rejoin` until the match ends.
    pub(super) active: HashMap<u64, (SocketAddr, u64, [u8; 32])>,
    saved: Instant,
}

impl Master {
    pub fn bind(addr: SocketAddr, tokens: Vec<String>) -> std::io::Result<Master> {
        let udp = UdpSocket::bind(addr)?;
        udp.set_read_timeout(Some(Duration::from_millis(100)))?;
        let keys = tokens.iter().map(|t| crate::crypto::ServerKey::from_token(t)).collect();
        Ok(Master { udp, key: crate::crypto::LobbyKey::generate(), cookies: crate::secure::Cookies::new(), keys, servers: HashMap::new(), players: HashMap::new(), parties: HashMap::new(), next_id: 1, limits: crate::secure::Buckets::new(100_000), started: Instant::now(), match_size: 10, reported: Instant::now(), steam_mode: SteamMode::Trust, require_steam: false, steam_checks: Vec::new(), seats: HashMap::new(), reports: None, booking: HashMap::new(), role: Role::Active, peer: None, state_file: None, saved: Instant::now(), ratings: HashMap::new(), ratings_file: None, penalties: HashMap::new(), checks: Vec::new(), active: HashMap::new() })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.udp.local_addr().expect("bound")
    }

    /// Tickets, sessions and codes come from the OS random source: seeing your own must not
    /// tell you anyone else's.
    fn rand(&mut self) -> u64 {
        crate::secure::random()
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
            let free = self.servers.values().filter(|s| s.official && !s.busy && !s.deathmatch).count();
            let busy = self.servers.values().filter(|s| s.busy).count();
            println!("{} online, {queued} in queue, servers: {free} free, {busy} in a match", self.players.len());
        }
    }

    pub fn run(&mut self) {
        loop {
            self.step();
        }
    }

}
