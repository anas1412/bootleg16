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
    latest_tick: u32,
    /// Server time we are drawing other players at.
    render_tick: f32,
    ticks: u32,
    /// How far the last correction moved us (for stats and tests).
    pub last_correction: f32,
    /// Server events not yet taken by the game (see `take_events`).
    events: Vec<GameEvent>,
    /// Grenades and smoke clouds as of the last snapshot.
    pub grenades: Vec<(sim::grenades::Kind, [f32; 3])>,
    pub smokes: Vec<SmokeNet>,
    /// Guns on the ground.
    pub ground: Vec<(sim::weapons::Id, [f32; 3])>,
    /// Scoreboard as last sent.
    pub scores: Vec<Score>,
    /// The server runs practice rules.
    pub practice: bool,
    /// The server's map and its file hash, once welcomed.
    pub map: Option<String>,
    pub map_hash: u64,
    /// Round trip to the server in ms (smoothed): how long until it acknowledges an input.
    pub ping_ms: f32,
    /// Voice frames received and not yet taken: (from, channel, gain 0..255, Opus data).
    voice: Vec<(u8, VoiceChannel, u8, Vec<u8>)>,
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
            latest_tick: 0,
            render_tick: 0.0,
            ticks: 0,
            last_correction: 0.0,
            events: Vec::new(),
            grenades: Vec::new(),
            smokes: Vec::new(),
            ground: Vec::new(),
            scores: Vec::new(),
            practice: false,
            map: None,
            map_hash: 0,
            ping_ms: 0.0,
            voice: Vec::new(),
            round: RoundNet { phase: crate::rounds::Phase::Warmup, round: 0, score: [0; 2], left: 0.0 },
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
        for (from, bytes) in self.socket.recv_all() {
            if from != self.server {
                continue;
            }
            if let Some(msg) = decode::<ServerMsg>(&bytes) {
                self.handle(msg);
            }
        }

        let Status::Connected { .. } = self.status else {
            if self.status == Status::Connecting && self.ticks % HELLO_EVERY == 1 {
                let hello = ClientMsg::Hello { version: PROTOCOL_VERSION, name: self.name.clone() };
                self.socket.send(self.server, encode(&hello));
            }
            return Default::default();
        };

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
        let msg = ClientMsg::Inputs { first_seq, inputs: recent.iter().map(|(_, i)| *i).collect() };
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
            ServerMsg::Voice { from, channel, gain, data } => self.voice.push((from, channel, gain, data)),
            ServerMsg::Reject { reason } => {
                self.status = Status::Rejected;
                self.reject_reason = Some(reason);
            }
            ServerMsg::Snapshot { tick, ack, players, doors, loadout, events, grenades, smokes, ground, round, bomb, me } => {
                let Status::Connected { id } = self.status else { return };
                if tick <= self.latest_tick {
                    return; // old or duplicate
                }
                self.latest_tick = tick;
                self.world.doors.apply_net_state(&doors);
                self.events.extend(events);
                self.grenades = grenades;
                self.smokes = smokes;
                self.ground = ground;
                self.round = round;
                self.bomb = bomb;
                self.me_info = me;

                // Correct our prediction: take the server's state, replay what it hasn't run yet.
                if let Some(mine) = players.iter().find(|p| p.id == id) {
                    // Our last input went out at `seq`; the server had run up to `ack`.
                    let sample = self.seq.saturating_sub(ack) as f32 * TICK * 1000.0;
                    self.ping_ms = if self.ping_ms == 0.0 { sample } else { self.ping_ms * 0.9 + sample * 0.1 };
                    while self.history.front().is_some_and(|(s, _)| *s <= ack) {
                        self.history.pop_front();
                    }
                    let before = self.state.movement.origin;
                    self.vitals = mine.vitals;
                    self.state = PlayerState { movement: mine.player, loadout };
                    if self.vitals.alive() {
                        for (seq, timed) in &self.history {
                            let cmd_time = *seq as f64 * TICK as f64;
                            player::step(&mut self.state, &timed.input, TICK, cmd_time, player::seed(id, *seq), self.me_info.limits, &self.world, &self.vars);
                        }
                    }
                    let now = self.state.movement.origin;
                    let d = [now[0] - before[0], now[1] - before[1], now[2] - before[2]];
                    self.last_correction = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                }
                self.snaps.push_back(Snap { tick, players });
                while self.snaps.len() > 32 {
                    self.snaps.pop_front();
                }
            }
        }
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
                        Some(pa) => std::array::from_fn(|i| pa.player.origin[i] + (pb.player.origin[i] - pa.player.origin[i]) * t),
                        None => pb.player.origin,
                    };
                    // Turn the short way round when blending yaw.
                    let yaw = match a.players.iter().find(|p| p.id == pb.id) {
                        Some(pa) => pa.yaw + ((pb.yaw - pa.yaw + 540.0).rem_euclid(360.0) - 180.0) * t,
                        None => pb.yaw,
                    };
                    out.push(Remote { id: pb.id, origin, crouched: pb.player.crouched, alive: pb.vitals.alive(), yaw, pitch: pb.pitch, weapon: pb.weapon, team: pb.team });
                }
            }
            None => {
                for p in newest.players.iter().filter(|p| Some(p.id) != me) {
                    out.push(Remote { id: p.id, origin: p.player.origin, crouched: p.player.crouched, alive: p.vitals.alive(), yaw: p.yaw, pitch: p.pitch, weapon: p.weapon, team: p.team });
                }
            }
        }
        out
    }

    /// Pick a side (None = auto).
    pub fn join_team(&mut self, team: Option<Team>) {
        self.socket.send(self.server, encode(&ClientMsg::JoinTeam { team }));
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
    pub fn take_voice(&mut self) -> Vec<(u8, VoiceChannel, u8, Vec<u8>)> {
        std::mem::take(&mut self.voice)
    }

    pub fn disconnect(&mut self) {
        self.socket.send(self.server, encode(&ClientMsg::Bye));
    }
}
