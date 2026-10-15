//! The client's half of the netcode: sends inputs, predicts its own player,
//! corrects from snapshots, and interpolates everyone else.
use crate::protocol::*;
use crate::socket::Socket;
use sim::collision::World;
use sim::pmove::{step, Events, Input, MoveVars, Player};
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
    /// Our player as predicted (ahead of the server by our unacknowledged inputs).
    pub me: Player,
    seq: u32,
    history: VecDeque<(u32, Input)>,
    snaps: VecDeque<Snap>,
    latest_tick: u32,
    /// Server time we are drawing other players at.
    render_tick: f32,
    ticks: u32,
    /// How far the last correction moved us (for stats and tests).
    pub last_correction: f32,
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
            me: Player::new([0.0; 3]),
            seq: 0,
            history: VecDeque::new(),
            snaps: VecDeque::new(),
            latest_tick: 0,
            render_tick: 0.0,
            ticks: 0,
            last_correction: 0.0,
        })
    }

    /// Fake network conditions (round-trip ping in ms, loss 0..1).
    pub fn simulate_network(&mut self, ping_ms: u32, loss: f32) {
        self.socket.set_conditions(ping_ms, loss);
    }

    /// One client tick (call at the tick rate): read the network, then run `input`.
    pub fn tick(&mut self, input: Input) -> Events {
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
            return Events::default();
        };

        // Predict: run the input now, keep it until the server confirms it.
        self.seq += 1;
        self.history.push_back((self.seq, input));
        let events = step(&mut self.me, &input, TICK, &self.world, &self.vars);

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
        events
    }

    fn handle(&mut self, msg: ServerMsg) {
        match msg {
            ServerMsg::Welcome { id, .. } => {
                if self.status == Status::Connecting {
                    self.status = Status::Connected { id };
                }
            }
            ServerMsg::Reject { reason } => {
                self.status = Status::Rejected;
                self.reject_reason = Some(reason);
            }
            ServerMsg::Snapshot { tick, ack, players, doors } => {
                let Status::Connected { id } = self.status else { return };
                if tick <= self.latest_tick {
                    return; // old or duplicate
                }
                self.latest_tick = tick;
                self.world.doors.apply_net_state(&doors);

                // Correct our prediction: take the server's state, replay what it hasn't run yet.
                if let Some(mine) = players.iter().find(|p| p.id == id) {
                    while self.history.front().is_some_and(|(s, _)| *s <= ack) {
                        self.history.pop_front();
                    }
                    let before = self.me.origin;
                    self.me = mine.player;
                    for (_, input) in &self.history {
                        step(&mut self.me, input, TICK, &self.world, &self.vars);
                    }
                    let d = [self.me.origin[0] - before[0], self.me.origin[1] - before[1], self.me.origin[2] - before[2]];
                    self.last_correction = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                }
                self.snaps.push_back(Snap { tick, players });
                while self.snaps.len() > 32 {
                    self.snaps.pop_front();
                }
            }
        }
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
                    out.push(Remote { id: pb.id, origin, crouched: pb.player.crouched });
                }
            }
            None => {
                for p in newest.players.iter().filter(|p| Some(p.id) != me) {
                    out.push(Remote { id: p.id, origin: p.player.origin, crouched: p.player.crouched });
                }
            }
        }
        out
    }

    pub fn disconnect(&mut self) {
        self.socket.send(self.server, encode(&ClientMsg::Bye));
    }
}
