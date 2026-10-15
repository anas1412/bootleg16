//! The authoritative server: owns the world, runs every player's movement from
//! their inputs, and sends snapshots.
use crate::protocol::*;
use crate::socket::Socket;
use goldsrc::bsp::Bsp;
use sim::collision::World;
use sim::combat::Vitals;
use sim::doors::Actor;
use sim::pmove::{step, MoveVars, Player};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Inputs a client may bank while its packets are delayed. Anything sent faster
/// than real time beyond this is thrown away, which stops speed hacks.
const MAX_INPUT_BUDGET: f32 = 16.0;

/// Seconds a dead player waits before respawning (until rounds exist).
const RESPAWN_TICKS: u32 = 3 * TICK_RATE;

struct Remote {
    id: u8,
    player: Player,
    vitals: Vitals,
    /// Tick to respawn at while dead.
    respawn_at: Option<u32>,
    queue: VecDeque<(u32, TimedInput)>,
    /// Highest input number accepted into the queue.
    last_queued: u32,
    /// Highest input number actually run.
    last_run: u32,
    budget: f32,
    last_heard: u32,
    use_held: bool,
    /// +use pressed during this tick (for doors).
    used: bool,
    view: (f32, f32),
    /// Server time this client was seeing others at, as of its latest input.
    view_tick: f32,
}

/// Where everyone was at one tick (for lag compensation).
#[derive(Clone, Debug)]
pub struct PastPositions {
    pub tick: u32,
    pub players: Vec<(u8, [f32; 3], bool)>,
}

pub struct Server {
    socket: Socket,
    map: String,
    world: World,
    vars: MoveVars,
    clients: HashMap<SocketAddr, Remote>,
    spawns: Vec<[f32; 3]>,
    next_spawn: usize,
    tick: u32,
    /// The last second of positions, newest last.
    history: VecDeque<PastPositions>,
}

impl Server {
    pub fn new(bind: SocketAddr, map: &str, bsp: &Bsp) -> std::io::Result<Server> {
        let spawns = bsp
            .entity_list()
            .iter()
            .filter(|e| matches!(e.get("classname").map(String::as_str), Some("info_player_start" | "info_player_deathmatch")))
            .filter_map(|e| {
                let v: Vec<f32> = e.get("origin")?.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                Some([*v.first()?, *v.get(1)?, *v.get(2)?])
            })
            .collect();
        Ok(Server {
            socket: Socket::bind(bind)?,
            map: map.to_string(),
            world: World::from_bsp(bsp),
            vars: MoveVars::default(),
            clients: HashMap::new(),
            spawns,
            next_spawn: 0,
            tick: 0,
            history: VecDeque::new(),
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.socket.local_addr()
    }

    pub fn player_count(&self) -> usize {
        self.clients.len()
    }

    fn next_spawn_point(&mut self) -> [f32; 3] {
        let spawn = self.spawns.get(self.next_spawn % self.spawns.len().max(1)).copied().unwrap_or([0.0; 3]);
        self.next_spawn += 1;
        spawn
    }

    /// Server-side health of a player.
    pub fn vitals(&self, id: u8) -> Option<&Vitals> {
        self.clients.values().find(|c| c.id == id).map(|c| &c.vitals)
    }

    /// Server-side state of a player (for tests and admin tools).
    pub fn player(&self, id: u8) -> Option<&Player> {
        self.clients.values().find(|c| c.id == id).map(|c| &c.player)
    }

    /// Everyone's positions as a client saw them at `view_tick` (interpolated, like the
    /// client draws them), capped at `MAX_UNLAG_TICKS` into the past.
    pub fn rewind(&self, view_tick: f32) -> Vec<(u8, [f32; 3], bool)> {
        let oldest = self.tick.saturating_sub(MAX_UNLAG_TICKS) as f32;
        let t = view_tick.clamp(oldest, self.tick as f32);
        // Only the ticks that went out as snapshots: that's exactly what clients blend between.
        let sent: Vec<&PastPositions> = self.history.iter().filter(|h| h.tick % SNAPSHOT_EVERY == 0).collect();
        let pair = sent.iter().zip(sent.iter().skip(1)).find(|(a, b)| a.tick as f32 <= t && t <= b.tick as f32);
        let Some((a, b)) = pair else {
            return self.history.back().map(|h| h.players.clone()).unwrap_or_default();
        };
        let f = (t - a.tick as f32) / (b.tick - a.tick) as f32;
        b.players
            .iter()
            .map(|&(id, pb, crouched)| match a.players.iter().find(|p| p.0 == id) {
                Some(&(_, pa, _)) => (id, std::array::from_fn(|i| pa[i] + (pb[i] - pa[i]) * f), crouched),
                None => (id, pb, crouched),
            })
            .collect()
    }

    pub fn tick(&self) -> u32 {
        self.tick
    }

    /// Run forever at the tick rate.
    pub fn run(&mut self) {
        let tick = Duration::from_secs_f64(1.0 / TICK_RATE as f64);
        let mut next = Instant::now();
        loop {
            self.step();
            next += tick;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else if now - next > Duration::from_millis(250) {
                next = now; // fell far behind (machine was suspended): don't fast-forward
            }
        }
    }

    /// One server tick: read packets, run inputs, move doors, send snapshots.
    pub fn step(&mut self) {
        self.tick += 1;
        self.socket.tick();
        for (from, bytes) in self.socket.recv_all() {
            if let Some(msg) = decode::<ClientMsg>(&bytes) {
                self.handle(from, msg);
            }
        }

        // Respawn the dead whose wait is over.
        let tick = self.tick;
        let mut respawns = Vec::new();
        for (addr, c) in &self.clients {
            if c.respawn_at.is_some_and(|t| t <= tick) {
                respawns.push(*addr);
            }
        }
        for addr in respawns {
            let spawn = self.next_spawn_point();
            let c = self.clients.get_mut(&addr).unwrap();
            c.player = Player::new(spawn);
            c.vitals = Vitals::default();
            c.respawn_at = None;
        }

        let mut actors = Vec::new();
        for c in self.clients.values_mut() {
            c.budget = (c.budget + 1.0).min(MAX_INPUT_BUDGET);
            c.used = false;
            while c.budget >= 1.0 {
                let Some((seq, TimedInput { input, view_tick })) = c.queue.pop_front() else { break };
                c.view_tick = view_tick;
                c.budget -= 1.0;
                c.used |= input.use_key && !c.use_held;
                c.use_held = input.use_key;
                c.view = (input.yaw, input.pitch);
                c.last_run = seq;
                if !c.vitals.alive() {
                    continue; // the dead don't move (their inputs are still acknowledged)
                }
                let ev = step(&mut c.player, &input, TICK, &self.world, &self.vars);
                if ev.fall_damage > 0.0 && c.vitals.hurt(ev.fall_damage, 0.0) {
                    c.respawn_at = Some(tick + RESPAWN_TICKS);
                }
            }
            if c.vitals.alive() {
                actors.push(actor(&c.player, c.view, c.used));
            }
        }
        self.world.update(TICK, &actors);

        self.history.push_back(PastPositions {
            tick: self.tick,
            players: self.clients.values().map(|c| (c.id, c.player.origin, c.player.crouched)).collect(),
        });
        while self.history.len() > TICK_RATE as usize {
            self.history.pop_front();
        }

        let tick = self.tick;
        self.clients.retain(|_, c| tick - c.last_heard < TIMEOUT_TICKS);

        if self.tick % SNAPSHOT_EVERY == 0 {
            let players: Vec<PlayerNet> = self.clients.values().map(|c| PlayerNet { id: c.id, player: c.player, vitals: c.vitals, yaw: c.view.0, pitch: c.view.1 }).collect();
            let doors = self.world.doors.net_state();
            let sends: Vec<_> = self.clients.iter().map(|(a, c)| (*a, c.last_run)).collect();
            for (addr, ack) in sends {
                let msg = ServerMsg::Snapshot { tick, ack, players: players.clone(), doors: doors.clone() };
                self.socket.send(addr, encode(&msg));
            }
        }
    }

    fn handle(&mut self, from: SocketAddr, msg: ClientMsg) {
        match msg {
            ClientMsg::Hello { version, .. } => {
                if version != PROTOCOL_VERSION {
                    let reason = format!("server runs protocol {PROTOCOL_VERSION}, you have {version}");
                    self.socket.send(from, encode(&ServerMsg::Reject { reason }));
                    return;
                }
                if !self.clients.contains_key(&from) {
                    if self.clients.len() >= MAX_PLAYERS {
                        self.socket.send(from, encode(&ServerMsg::Reject { reason: "server full".into() }));
                        return;
                    }
                    let id = (0..=u8::MAX).find(|i| self.clients.values().all(|c| c.id != *i)).unwrap_or(0);
                    let spawn = self.next_spawn_point();
                    self.clients.insert(
                        from,
                        Remote {
                            id,
                            player: Player::new(spawn),
                            vitals: Vitals::default(),
                            respawn_at: None,
                            queue: VecDeque::new(),
                            last_queued: 0,
                            last_run: 0,
                            budget: 0.0,
                            last_heard: self.tick,
                            use_held: false,
                            used: false,
                            view: (0.0, 0.0),
                            view_tick: 0.0,
                        },
                    );
                }
                let id = self.clients[&from].id;
                self.socket.send(from, encode(&ServerMsg::Welcome { id, map: self.map.clone(), tick_rate: TICK_RATE }));
            }
            ClientMsg::Inputs { first_seq, inputs } => {
                let Some(c) = self.clients.get_mut(&from) else { return };
                c.last_heard = self.tick;
                for (i, input) in inputs.into_iter().enumerate() {
                    let seq = first_seq + i as u32;
                    if seq > c.last_queued {
                        c.queue.push_back((seq, input));
                        c.last_queued = seq;
                    }
                }
                // Never let a client queue up more than a second of movement.
                while c.queue.len() > TICK_RATE as usize {
                    c.queue.pop_front();
                }
            }
            ClientMsg::Bye => {
                self.clients.remove(&from);
            }
        }
    }
}

pub(crate) fn actor(p: &Player, (yaw, pitch): (f32, f32), using: bool) -> Actor {
    let h = p.hull();
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    Actor {
        mins: [p.origin[0] + h.mins()[0], p.origin[1] + h.mins()[1], p.origin[2] + h.mins()[2]],
        maxs: [p.origin[0] + h.maxs()[0], p.origin[1] + h.maxs()[1], p.origin[2] + h.maxs()[2]],
        eye: p.eye(),
        forward: [cp * cy, cp * sy, -sp],
        using,
    }
}
