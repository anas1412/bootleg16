//! The lobby service (the "master"): everyone online is in a party (a party of one to start,
//! like CS2) with an invite code. A party queues for a casual match, or plays a custom game:
//! up to 10 people split into team A and team B. When a match is ready it reserves a free
//! official server and gives each player a ticket for it. Game servers announce themselves
//! here; it never carries game traffic.
//!
//! Over UDP, kept simple: players send commands and poll their state every second; every
//! answer is the full state, so a lost packet never leaves anyone out of date. Requests are
//! padded so answers are never bigger than requests (nobody can use this to flood others).
use crate::protocol::{decode, encode, ClientMsg};
use crate::server::{Group, MatchSetup};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

pub const MASTER_PORT: u16 = 27010;
/// Game servers announce this often and are forgotten after missing a few.
pub const HEARTBEAT_SECONDS: u64 = 5;
const SERVER_TIMEOUT: Duration = Duration::from_secs(HEARTBEAT_SECONDS * 3);
/// Players poll every second; ten seconds of silence and they're gone.
const PLAYER_TIMEOUT: Duration = Duration::from_secs(10);
/// Every player request is padded to this: the biggest state answer fits in it.
pub const PAD: usize = 900;
pub const MAX_PARTY: usize = 5;
pub const MAX_CUSTOM: usize = 10;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum LobbyMsg {
    /// Game server -> lobby: "I'm here", and whether it's running a match.
    /// `address`: where players connect (empty = where this came from).
    Heartbeat { name: String, map: String, players: u8, max: u8, region: String, token: String, busy: bool, address: String },
    /// Player -> lobby: sign in (no accounts yet: a fresh session each time).
    Hello { name: String, pad: Vec<u8> },
    /// Player -> lobby: do something (or just `Poll`), proving who you are with your session.
    Command { session: u64, cmd: Command, pad: Vec<u8> },
    /// Lobby -> player: your session secret.
    Welcome { session: u64 },
    /// Lobby -> player: everything about you now.
    State(LobbyState),
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
    /// (id, name, team in a custom game).
    pub members: Vec<(u32, String, Group)>,
    /// Seconds in the casual queue, if queued.
    pub searching: Option<f32>,
    /// Match ready: the server and your ticket.
    pub found: Option<(SocketAddr, u64)>,
    /// Something to tell the player ("no server free", "party full"...).
    pub notice: String,
}

struct Player {
    id: u32,
    name: String,
    addr: SocketAddr,
    party: u32,
    seen: Instant,
    found: Option<(SocketAddr, u64)>,
    notice: String,
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
    token: String,
    official: bool,
    busy: bool,
    seen: Instant,
}

pub struct Master {
    udp: UdpSocket,
    tokens: Vec<String>,
    servers: HashMap<SocketAddr, GameServer>,
    /// session secret -> player.
    players: HashMap<u64, Player>,
    parties: HashMap<u32, Party>,
    next_id: u32,
    rng: u64,
    /// Players per casual match (10; tests use 2).
    pub match_size: usize,
    /// When the last status line was printed.
    reported: Instant,
}

impl Master {
    pub fn bind(addr: SocketAddr, tokens: Vec<String>) -> std::io::Result<Master> {
        let udp = UdpSocket::bind(addr)?;
        udp.set_read_timeout(Some(Duration::from_millis(100)))?;
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7) ^ ((std::process::id() as u64) << 32);
        Ok(Master { udp, tokens, servers: HashMap::new(), players: HashMap::new(), parties: HashMap::new(), next_id: 1, rng: seed | 1, match_size: 10, reported: Instant::now() })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.udp.local_addr().expect("bound")
    }

    /// xorshift64*: tickets, sessions and codes (unguessable enough without a crypto crate;
    /// ponytail: use OS randomness once there are accounts and money involved).
    fn rand(&mut self) -> u64 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        self.rng.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn new_code(&mut self) -> String {
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
        while let Ok((n, from)) = self.udp.recv_from(&mut buf) {
            let _ = self.udp.set_nonblocking(true);
            match decode::<LobbyMsg>(&buf[..n]) {
                Some(LobbyMsg::Heartbeat { token, busy, address, .. }) => {
                    let public = address.parse().unwrap_or(from);
                    let official = !token.is_empty() && self.tokens.contains(&token);
                    if !self.servers.contains_key(&from) {
                        println!("server {public} joined the pool{}", if official { "" } else { " (wrong token: never gets matches)" });
                    }
                    self.servers.insert(from, GameServer { public, token, official, busy, seen: Instant::now() });
                }
                Some(LobbyMsg::Hello { name, pad }) if pad.len() >= PAD => {
                    let session = self.rand();
                    let id = self.next_id;
                    self.next_id += 1;
                    let name: String = name.chars().filter(|c| !c.is_control()).take(24).collect();
                    let party = self.new_party(id);
                    self.players.insert(session, Player { id, name, addr: from, party, seen: Instant::now(), found: None, notice: String::new() });
                    let _ = self.udp.send_to(&encode(&LobbyMsg::Welcome { session }), from);
                }
                Some(LobbyMsg::Command { session, cmd, pad }) if pad.len() >= PAD => {
                    if self.players.get(&session).is_some_and(|p| p.addr == from) {
                        self.command(session, cmd);
                        if let Some(state) = self.state_of(session) {
                            let _ = self.udp.send_to(&encode(&LobbyMsg::State(state)), from);
                        }
                    }
                }
                _ => {}
            }
        }
        let now = Instant::now();
        self.servers.retain(|_, s| {
            let alive = now - s.seen < SERVER_TIMEOUT;
            if !alive {
                println!("server {} stopped answering, out of the pool", s.public);
            }
            alive
        });
        let gone: Vec<(u32, u32)> = self.players.values().filter(|p| now - p.seen > PLAYER_TIMEOUT).map(|p| (p.id, p.party)).collect();
        for (id, party) in gone {
            self.leave(id, party);
            self.players.retain(|_, p| p.id != id);
        }
        self.matchmake();
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
                        } else if !self.start_match(members, true) {
                            notice = "No server free right now. Try again in a moment.".into();
                        }
                    }
                }
            }
            Command::Cancel if leader => self.parties.get_mut(&party).unwrap().queued = None,
            _ => notice = "Only the party leader can do that.".into(),
        }
        if !notice.is_empty() {
            if let Some(p) = self.players.get_mut(&session) {
                p.notice = notice;
            }
        }
    }

    /// Reserve a free official server for these players and hand out tickets. False if none.
    fn start_match(&mut self, players: Vec<(u32, Group)>, knife_round: bool) -> bool {
        let Some((addr, public, token)) = self.servers.iter().find(|(_, s)| s.official && !s.busy).map(|(a, s)| (*a, s.public, s.token.clone())) else { return false };
        let tickets: Vec<(u32, u64, Group)> = players.iter().map(|(id, g)| (*id, self.rand() | 1, *g)).collect();
        let setup = MatchSetup { players: tickets.iter().map(|(_, t, g)| (*t, *g)).collect(), knife_round };
        // The server checks the token: only we can reserve official servers.
        let _ = self.udp.send_to(&encode(&ClientMsg::Reserve { token, setup }), addr);
        self.servers.get_mut(&addr).unwrap().busy = true;
        println!("match booked on {public}: {} players{}", players.len(), if knife_round { ", custom" } else { "" });
        for (id, ticket, _) in tickets {
            if let Some(p) = self.players.values_mut().find(|p| p.id == id) {
                p.found = Some((public, ticket));
            }
            if let Some(party) = self.parties.values_mut().find(|q| q.members.iter().any(|(m, _)| *m == id)) {
                party.queued = None;
            }
        }
        true
    }

    /// Casual: fill two teams from queued parties (each party stays on one team), oldest first.
    fn matchmake(&mut self) {
        let side = self.match_size / 2;
        let mut queued: Vec<(Instant, Vec<u32>)> = self.parties.values().filter_map(|p| Some((p.queued?, p.members.iter().map(|(m, _)| *m).collect()))).collect();
        queued.sort_by_key(|q| q.0);
        let (mut a, mut b): (Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new());
        for (_, members) in &queued {
            if a.len() + members.len() <= side {
                a.extend(members);
            } else if b.len() + members.len() <= side {
                b.extend(members);
            }
            if a.len() == side && b.len() == side {
                let players: Vec<(u32, Group)> = a.iter().map(|m| (*m, Group::A)).chain(b.iter().map(|m| (*m, Group::B))).collect();
                if !self.start_match(players, false) {
                    for p in self.players.values_mut().filter(|p| a.contains(&p.id) || b.contains(&p.id)) {
                        p.notice = "Match found, waiting for a free server...".into();
                    }
                }
                return; // ponytail: one match per step; plenty at our scale
            }
        }
    }

    fn state_of(&mut self, session: u64) -> Option<LobbyState> {
        let p = self.players.get_mut(&session)?;
        let notice = std::mem::take(&mut p.notice);
        let (you, party_id, found) = (p.id, p.party, p.found.take());
        let party = self.parties.get(&party_id)?;
        let name_of = |id: u32| self.players.values().find(|q| q.id == id).map(|q| q.name.clone()).unwrap_or_default();
        Some(LobbyState {
            you,
            code: party.code.clone(),
            leader: party.leader,
            mode: party.mode,
            members: party.members.iter().map(|(m, g)| (*m, name_of(*m), *g)).collect(),
            searching: party.queued.map(|t| t.elapsed().as_secs_f32()),
            found,
            notice,
        })
    }
}

/// The game's side of the lobby: call `tick` every frame; it signs in, sends commands, polls
/// every second, and keeps the latest state. Never blocks.
pub struct LobbyClient {
    udp: UdpSocket,
    lobby: SocketAddr,
    name: String,
    session: Option<u64>,
    last_sent: Option<Instant>,
    pub state: Option<LobbyState>,
    /// Kept until taken: the match to join (server, ticket).
    pub found: Option<(SocketAddr, u64)>,
    /// Kept until taken: the lobby's last message for the player.
    pub notice: String,
}

impl LobbyClient {
    pub fn new(lobby: SocketAddr, name: &str) -> std::io::Result<LobbyClient> {
        let udp = UdpSocket::bind(if lobby.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })?;
        udp.set_nonblocking(true)?;
        Ok(LobbyClient { udp, lobby, name: name.into(), session: None, last_sent: None, state: None, found: None, notice: String::new() })
    }

    /// Signed in and heard from the lobby.
    pub fn online(&self) -> bool {
        self.state.is_some()
    }

    /// Send a command now (the answer is the new state).
    pub fn send(&mut self, cmd: Command) {
        if let Some(session) = self.session {
            let _ = self.udp.send_to(&encode(&LobbyMsg::Command { session, cmd, pad: vec![0; PAD] }), self.lobby);
            self.last_sent = Some(Instant::now());
        }
    }

    pub fn tick(&mut self) {
        let mut buf = [0u8; 2048];
        while let Ok((n, from)) = self.udp.recv_from(&mut buf) {
            if from != self.lobby {
                continue;
            }
            match decode::<LobbyMsg>(&buf[..n]) {
                Some(LobbyMsg::Welcome { session }) if self.session.is_none() => {
                    self.session = Some(session);
                    self.send(Command::Poll);
                }
                Some(LobbyMsg::State(s)) => {
                    if s.found.is_some() {
                        self.found = s.found;
                    }
                    if !s.notice.is_empty() {
                        self.notice = s.notice.clone();
                    }
                    self.state = Some(s);
                }
                _ => {}
            }
        }
        if self.last_sent.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
            match self.session {
                Some(_) => self.send(Command::Poll),
                None => {
                    let _ = self.udp.send_to(&encode(&LobbyMsg::Hello { name: self.name.clone(), pad: vec![0; PAD] }), self.lobby);
                    self.last_sent = Some(Instant::now());
                }
            }
        }
    }
}
