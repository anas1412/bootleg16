//! The client's half of the netcode: sends inputs, predicts its own player,
//! corrects from snapshots, and interpolates everyone else.
use crate::protocol::*;
use crate::socket::Socket;
use sim::collision::World;
use sim::player::{self, PlayerState};
use sim::pmove::{Events, Input, MoveVars, Player};
use sim::weapons::{Fired, Loadout};
use std::collections::VecDeque;
use std::net::SocketAddr;

/// Other players are drawn this many ticks in the past, between two snapshots
/// (2 snapshot intervals = ~31 ms), so they move smoothly even with lost packets.
const INTERP_TICKS: f32 = (SNAPSHOT_EVERY * 2) as f32;
/// Re-send Hello this often while connecting.
const HELLO_EVERY: u32 = TICK_RATE / 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Connecting,
    Connected { id: u8 },
    Rejected,
}

struct Snap {
    tick: u32,
    players: Vec<PlayerNet>,
}

/// A voice frame someone sent (see `ServerMsg::Voice`).
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    pub from: u8,
    pub channel: VoiceChannel,
    /// Loudness 1..255 (proximity voice fades with distance).
    pub gain: u8,
    /// Proximity voice from behind a wall.
    pub muffled: bool,
    pub data: Vec<u8>,
}

/// Another player, as it should be drawn now.
#[derive(Clone, Copy, Debug)]
pub struct Remote {
    pub id: u8,
    pub origin: [f32; 3],
    pub crouched: bool,
    pub alive: bool,
    pub yaw: f32,
    pub pitch: f32,
    pub weapon: sim::weapons::Id,
    pub team: Team,
    /// Jumping or falling.
    pub airborne: bool,
    /// Reloading their gun.
    pub reloading: bool,
    /// Health and kit: teammates only (None for enemies).
    pub mate: Option<crate::protocol::Mate>,
}

pub struct Client {
    socket: Socket,
    server: SocketAddr,
    name: String,
    pub status: Status,
    pub reject_reason: Option<String>,
    /// Collision world (doors follow the server).
    pub world: World,
    vars: MoveVars,
    /// Our player and weapons as predicted (ahead of the server by our unacknowledged inputs).
    pub state: PlayerState,
    /// Our health and armor, as the server last said.
    pub vitals: sim::combat::Vitals,
    seq: u32,
    history: VecDeque<(u32, TimedInput)>,
    snaps: VecDeque<Snap>,
    /// Recent snapshots as received (tick, bytes): the baselines deltas apply to.
    snap_bytes: VecDeque<(u32, Vec<u8>)>,
    latest_tick: u32,
    /// Server time we are drawing other players at.
    render_tick: f32,
    ticks: u32,
    /// `ticks` when the server was last heard from.
    heard: u32,
    /// How far the last correction moved us (for stats and tests).
    pub last_correction: f32,
    /// Corrections not yet taken by the camera (GoldSrc units).
    pending_correction: [f32; 3],
    /// Server events not yet taken by the game (see `take_events`).
    events: Vec<GameEvent>,
    /// Grenades and smoke clouds as of the last snapshot.
    pub grenades: Vec<(sim::grenades::Kind, [f32; 3])>,
    pub smokes: Vec<SmokeNet>,
    /// Guns on the ground.
    pub ground: Vec<(sim::weapons::Id, [f32; 3])>,
    /// Scoreboard as last sent.
    pub scores: Vec<Score>,
    /// Every round so far this match (winner, how), from the server.
    pub round_history: Vec<(Team, crate::rounds::EndReason)>,
    /// The server runs practice rules.
    pub practice: bool,
    /// The server's map and its file hash, once welcomed.
    pub map: Option<String>,
    pub map_hash: u64,
    /// Round trip to the server in ms (smoothed): how long until it acknowledges an input.
    pub ping_ms: f32,
    /// A side we asked for, until the server confirms it.
    wanted_team: Option<Option<Team>>,
    /// Characters we asked for, until the server has them.
    wanted_looks: Option<[u8; 2]>,
    /// The server's join cookie (0 until it challenges us).
    cookie: u64,
    /// Our seat in a reserved match (0 = none), from the matchmaker.
    pub ticket: u64,
    /// The seat's key (official servers): everything to and from the server is encrypted
    /// with it. Set it with `ticket`, before the first tick.
    pub key: Option<[u8; 32]>,
    /// The reserved match ended (the game goes back to the menu).
    pub match_ended: bool,
    /// Breakables broken as of the last snapshot.
    pub broken: Vec<u16>,
    /// Highest reliable event number received (each is kept once).
    reliable_seen: u32,
    /// Voice frames received and not yet taken: (from, channel, gain 0..255, Opus data).
    voice: Vec<Heard>,
    /// The round (phase, clock, score), the bomb, and your money and limits.
    pub round: RoundNet,
    pub bomb: BombNet,
    pub me_info: Me,
}

impl Client {
    pub fn connect(server: SocketAddr, name: &str, world: World) -> std::io::Result<Client> {
        let bind: SocketAddr = if server.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }.parse().expect("valid address");
        Ok(Client {
            socket: Socket::bind(bind)?,
            server,
            name: name.to_string(),
            status: Status::Connecting,
            reject_reason: None,
            world,
            vars: MoveVars::default(),
            state: PlayerState { movement: Player::new([0.0; 3]), loadout: Loadout::knife_only() },
            vitals: sim::combat::Vitals::default(),
            seq: 0,
            history: VecDeque::new(),
            snaps: VecDeque::new(),
            snap_bytes: VecDeque::new(),
            latest_tick: 0,
            render_tick: 0.0,
            ticks: 0,
            heard: 0,
            last_correction: 0.0,
            pending_correction: [0.0; 3],
            events: Vec::new(),
            grenades: Vec::new(),
            smokes: Vec::new(),
            ground: Vec::new(),
            scores: Vec::new(),
            round_history: Vec::new(),
            practice: false,
            map: None,
            map_hash: 0,
            ping_ms: 0.0,
            voice: Vec::new(),
            reliable_seen: 0,
            broken: Vec::new(),
            cookie: 0,
            ticket: 0,
            key: None,
            match_ended: false,
            wanted_team: None,
            wanted_looks: None,
            round: RoundNet { phase: crate::rounds::Phase::Warmup, round: 0, score: [0; 2], left: 0.0, stage: Stage::Match, abandoned: [false; 2], vote: None, paused: false },
            bomb: BombNet::None,
            me_info: Me { money: 0, limits: Default::default(), can_buy: false, defuse_kit: false },
        })
    }

    /// Fake network conditions (round-trip ping in ms, loss 0..1).
    pub fn simulate_network(&mut self, ping_ms: u32, loss: f32) {
        self.socket.set_conditions(ping_ms, loss);
    }

    /// Our predicted player.
    pub fn me(&self) -> &Player {
        &self.state.movement
    }

    /// Kill feed, hits, shots from the server since the last call.
    pub fn take_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    /// One client tick (call at the tick rate): read the network, then run `input`.
    /// Returns what our own player did, predicted (footsteps, shots for effects).
    pub fn tick(&mut self, input: Input) -> (Events, Fired) {
        self.ticks += 1;
        self.socket.tick();
        if let Some(key) = self.key.filter(|_| !self.socket.is_secure(self.server)) {
            self.socket.secure(self.server, self.ticket, crate::crypto::Channel::new(&key, true));
        }
        for (from, bytes) in self.socket.recv_all() {
            if from != self.server {
                continue;
            }
            let plain = if bytes.first() == Some(&crate::crypto::CHANNEL) {
                match self.socket.open(from, &bytes) {
                    Some(p) => p,
                    None => continue,
                }
            } else if self.key.is_some() && self.status != Status::Connecting {
                continue; // an encrypted session takes nothing in the clear
            } else {
                bytes
            };
            let plain = match decode::<ServerMsg>(&plain) {
                Some(ServerMsg::Delta { base, patch }) => {
                    let Some((_, old)) = self.snap_bytes.iter().find(|(t, _)| *t == base) else { continue };
                    let Some(full) = crate::delta::apply(old, &patch) else { continue };
                    full
                }
                _ => plain,
            };
            self.heard = self.ticks;
            match decode::<ServerMsg>(&plain) {
                Some(msg @ ServerMsg::Snapshot { tick, .. }) => {
                    self.snap_bytes.push_back((tick, plain.to_vec()));
                    if self.snap_bytes.len() > 64 {
                        self.snap_bytes.pop_front();
                    }
                    self.handle(msg);
                }
                Some(msg) => self.handle(msg),
                None => {}
            }
        }

        let Status::Connected { .. } = self.status else {
            if self.status == Status::Connecting && self.ticks % HELLO_EVERY == 1 {
                let hello = ClientMsg::Hello { version: PROTOCOL_VERSION, name: self.name.clone(), cookie: self.cookie, ticket: self.ticket };
                self.socket.send(self.server, encode(&hello));
            }
            return Default::default();
        };

        self.resend_join();
        self.resend_looks();
        // Predict: run the input now, keep it until the server confirms it.
        self.seq += 1;
        self.history.push_back((self.seq, TimedInput { input, view_tick: self.render_tick }));
        let result = match self.id() {
            Some(id) if self.vitals.alive() => {
                player::step(&mut self.state, &input, TICK, self.seq as f64 * TICK as f64, player::seed(id, self.seq), self.me_info.limits, &self.world, &self.vars)
            }
            _ => Default::default(),
        };

        let recent: Vec<_> = self.history.iter().rev().take(INPUT_REDUNDANCY).rev().collect();
        let first_seq = recent.first().map(|(s, _)| *s).unwrap_or(self.seq);
        let msg = ClientMsg::Inputs { first_seq, inputs: recent.iter().map(|(_, i)| *i).collect(), events: self.reliable_seen, snap: self.latest_tick };
        self.socket.send(self.server, encode(&msg));

        // Advance the clock others are drawn at, staying INTERP_TICKS behind the newest snapshot.
        self.render_tick += 1.0;
        let target = self.latest_tick as f32 - INTERP_TICKS;
        if (self.render_tick - target).abs() > TICK_RATE as f32 / 4.0 {
            self.render_tick = target; // way off (just connected, or a long stall): jump
        } else {
            self.render_tick += (target - self.render_tick) * 0.05; // gently drift back in line
        }
        result
    }

    fn handle(&mut self, msg: ServerMsg) {
        match msg {
            ServerMsg::Welcome { id, map, map_hash, practice, .. } => {
                if self.status == Status::Connecting {
                    self.status = Status::Connected { id };
                    self.practice = practice;
                    self.map = Some(map);
                    self.map_hash = map_hash;
                }
            }
            ServerMsg::Scores(s) => self.scores = s,
            ServerMsg::History(h) => self.round_history = h,
            ServerMsg::Info { .. } => {} // for the server browser, not a connection
            ServerMsg::MatchEnded => self.match_ended = true,
            ServerMsg::Delta { .. } => {} // patched into a snapshot on arrival
            ServerMsg::Challenge { cookie } => {
                // Prove our address: say hello again with the server's cookie, right away.
                if self.status == Status::Connecting && self.cookie != cookie {
                    self.cookie = cookie;
                    let hello = ClientMsg::Hello { version: PROTOCOL_VERSION, name: self.name.clone(), cookie, ticket: self.ticket };
                    self.socket.send(self.server, encode(&hello));
                }
            }
            ServerMsg::Voice { from, channel, gain, muffled, data } => self.voice.push(Heard { from, channel, gain, muffled, data }),
            ServerMsg::Reject { reason } => {
                self.status = Status::Rejected;
                self.reject_reason = Some(reason);
            }
            ServerMsg::Snapshot { tick, ack, players, you, doors, loadout, events, reliable, broken, grenades, smokes, ground, round, bomb, me } => {
                let Status::Connected { id } = self.status else { return };
                if tick <= self.latest_tick {
                    return; // old or duplicate
                }
                self.latest_tick = tick;
                self.world.doors.apply_net_state(&doors);
                // Broken glass and crates stop blocking (and come back at round start).
                for m in self.broken.iter().filter(|m| !broken.contains(m)) {
                    self.world.set_broken(*m as usize, false);
                }
                for m in &broken {
                    self.world.set_broken(*m as usize, true);
                }
                self.broken = broken;
                self.events.extend(events);
                for (n, e) in reliable {
                    if n > self.reliable_seen {
                        self.reliable_seen = n;
                        self.events.push(e);
                    }
                }
                self.grenades = grenades;
                self.smokes = smokes;
                self.ground = ground;
                self.round = round;
                self.bomb = bomb;
                self.me_info = me;

                // Correct our prediction: take the server's state, replay what it hasn't run yet.
                if let Some((movement, vitals)) = you {
                    // Our last input went out at `seq`; the server had run up to `ack`.
                    let sample = self.seq.saturating_sub(ack) as f32 * TICK * 1000.0;
                    self.ping_ms = if self.ping_ms == 0.0 { sample } else { self.ping_ms * 0.9 + sample * 0.1 };
                    while self.history.front().is_some_and(|(s, _)| *s <= ack) {
                        self.history.pop_front();
                    }
                    let before = self.state.movement.origin;
                    self.vitals = vitals;
                    self.state = PlayerState { movement, loadout };
                    if self.vitals.alive() {
                        for (seq, timed) in &self.history {
                            let cmd_time = *seq as f64 * TICK as f64;
                            player::step(&mut self.state, &timed.input, TICK, cmd_time, player::seed(id, *seq), self.me_info.limits, &self.world, &self.vars);
                        }
                    }
                    let now = self.state.movement.origin;
                    let d = [now[0] - before[0], now[1] - before[1], now[2] - before[2]];
                    self.last_correction = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                    // How far we jumped, for the camera to glide over (`take_correction`).
                    for i in 0..3 {
                        self.pending_correction[i] += d[i];
                    }
                }
                self.snaps.push_back(Snap { tick, players });
                while self.snaps.len() > 32 {
                    self.snaps.pop_front();
                }
            }
        }
    }

    /// How far prediction corrections moved us since the last call (the camera smooths it).
    pub fn take_correction(&mut self) -> [f32; 3] {
        std::mem::take(&mut self.pending_correction)
    }

    /// Our own command clock (input number x tick), which weapon timers use.
    pub fn cmd_time(&self) -> f64 {
        self.seq as f64 * TICK as f64
    }

    /// Server time other players are currently drawn at (sent with each input).
    pub fn view_tick(&self) -> f32 {
        self.render_tick
    }

    pub fn id(&self) -> Option<u8> {
        match self.status {
            Status::Connected { id } => Some(id),
            _ => None,
        }
    }

    /// Everyone else, interpolated between the two snapshots around `render_tick`.
    pub fn others(&self) -> Vec<Remote> {
        let me = self.id();
        let rt = self.render_tick;
        let (Some(newest), Some(_)) = (self.snaps.back(), self.snaps.front()) else { return Vec::new() };
        // Snapshot pair bracketing the render time (or the newest one if we're ahead).
        let pair = self.snaps.iter().zip(self.snaps.iter().skip(1)).find(|(a, b)| a.tick as f32 <= rt && rt <= b.tick as f32);
        let mut out = Vec::new();
        match pair {
            Some((a, b)) => {
                let t = (rt - a.tick as f32) / (b.tick - a.tick) as f32;
                for pb in b.players.iter().filter(|p| Some(p.id) != me) {
                    let origin = match a.players.iter().find(|p| p.id == pb.id) {
                        Some(pa) => {
                            let (oa, ob) = (pa.origin(), pb.origin());
                            std::array::from_fn(|i| oa[i] + (ob[i] - oa[i]) * t)
                        }
                        None => pb.origin(),
                    };
                    // Turn the short way round when blending yaw.
                    let yaw = match a.players.iter().find(|p| p.id == pb.id) {
                        Some(pa) => pa.yaw() + ((pb.yaw() - pa.yaw() + 540.0).rem_euclid(360.0) - 180.0) * t,
                        None => pb.yaw(),
                    };
                    out.push(Remote { id: pb.id, origin, crouched: pb.crouched(), alive: pb.alive(), yaw, pitch: pb.pitch(), weapon: pb.weapon, team: pb.team, airborne: pb.airborne(), reloading: pb.reloading(), mate: pb.mate });
                }
            }
            None => {
                for p in newest.players.iter().filter(|p| Some(p.id) != me) {
                    out.push(Remote { id: p.id, origin: p.origin(), crouched: p.crouched(), alive: p.alive(), yaw: p.yaw(), pitch: p.pitch(), weapon: p.weapon, team: p.team, airborne: p.airborne(), reloading: p.reloading(), mate: p.mate });
                }
            }
        }
        out
    }

    /// Pick a side (None = auto).
    /// Ask for a side. Resent until the scoreboard shows it (a lost packet must not leave you
    /// spectating); None = auto, done once you're on either team.
    pub fn join_team(&mut self, team: Option<Team>) {
        self.wanted_team = Some(team);
        self.socket.send(self.server, encode(&ClientMsg::JoinTeam { team }));
    }

    /// Sell back everything bought this round (while you may still buy).
    pub fn undo_buys(&mut self) {
        self.socket.send(self.server, encode(&ClientMsg::UndoBuys));
    }

    /// Wear these characters (terrorist, counter-terrorist); sent until the scoreboard shows it.
    pub fn set_looks(&mut self, t: u8, ct: u8) {
        self.wanted_looks = Some([t.min(crate::protocol::CLASSES - 1), ct.min(crate::protocol::CLASSES - 1)]);
    }

    fn resend_looks(&mut self) {
        let Some(want) = self.wanted_looks else { return };
        if self.scores.iter().any(|s| Some(s.id) == self.id() && s.looks == want) {
            self.wanted_looks = None;
        } else if self.ticks % (TICK_RATE / 2) == 0 {
            self.socket.send(self.server, encode(&ClientMsg::Looks { t: want[0], ct: want[1] }));
        }
    }

    fn resend_join(&mut self) {
        let Some(want) = self.wanted_team else { return };
        let now = self.team();
        let done = match want {
            Some(t) => now == t,
            None => now != Team::Spectator,
        };
        if done {
            self.wanted_team = None;
        } else if self.ticks % (TICK_RATE / 4) == 0 {
            self.socket.send(self.server, encode(&ClientMsg::JoinTeam { team: want }));
        }
    }

    /// Our team according to the scoreboard (Spectator until we pick).
    pub fn team(&self) -> Team {
        self.scores.iter().find(|s| Some(s.id) == self.id()).map(|s| s.team).unwrap_or(Team::Spectator)
    }

    /// Ask the server for a weapon (the buy menu).
    pub fn buy(&mut self, item: BuyItem) {
        self.socket.send(self.server, encode(&ClientMsg::Buy { item }));
    }

    /// Send one encoded voice frame (while push-to-talk is held).
    pub fn send_voice(&mut self, channel: VoiceChannel, data: Vec<u8>) {
        if self.id().is_some() {
            self.socket.send(self.server, encode(&ClientMsg::Voice { channel, data }));
        }
    }

    /// Voice frames that arrived since the last call.
    pub fn take_voice(&mut self) -> Vec<Heard> {
        std::mem::take(&mut self.voice)
    }

    /// Knife round winners: vote to switch sides (true) or stay.
    pub fn side_vote(&mut self, switch: bool) {
        self.socket.send(self.server, encode(&ClientMsg::SideVote { switch }));
    }

    /// Ask your team to surrender (the server only starts the vote once a teammate abandoned).
    pub fn call_surrender(&mut self) {
        self.socket.send(self.server, encode(&ClientMsg::CallSurrender));
    }

    /// Report a player in this match for review (once per player; reserved matches only).
    pub fn report(&mut self, player: u8, reason: crate::protocol::ReportReason) {
        self.socket.send(self.server, encode(&ClientMsg::Report { player, reason }));
    }

    /// Ask your team to vote this teammate out (reserved matches).
    pub fn call_kick(&mut self, player: u8) {
        self.socket.send(self.server, encode(&ClientMsg::CallKick { player }));
    }

    /// Answer your team's vote (surrender or kick).
    pub fn surrender_vote(&mut self, yes: bool) {
        self.socket.send(self.server, encode(&ClientMsg::Vote { yes }));
    }

    /// The server this client talks to.
    pub fn server(&self) -> SocketAddr {
        self.server
    }

    /// Seconds since the server was last heard from.
    pub fn silence(&self) -> f32 {
        (self.ticks - self.heard) as f32 * TICK
    }

    /// Join the same server again after losing it (same seat if we have a ticket): hello
    /// until it welcomes us. Its reliable events start over, and an encrypted session gets a
    /// fresh channel (the server's starts over too once it has dropped us).
    pub fn rejoin(&mut self) {
        self.status = Status::Connecting;
        self.cookie = 0;
        self.reliable_seen = 0;
        self.history.clear();
        let server = self.server;
        self.socket.keep_channels(|a| *a != server);
    }

    pub fn disconnect(&mut self) {
        self.socket.send(self.server, encode(&ClientMsg::Bye));
    }
}
