//! The authoritative server: owns the world, runs every player's movement and
//! weapons from their inputs, resolves shots with lag compensation, and sends snapshots.
use crate::protocol::*;
use crate::socket::Socket;
use goldsrc::bsp::Bsp;
use goldsrc::materials::Materials;
use sim::collision::World;
use sim::combat::{self, Vitals};
use sim::doors::Actor;
use sim::hitbox::{HitGroup, Hitboxes};
use sim::player::{self, PlayerState};
use sim::pmove::{MoveVars, Player};
use sim::weapons::bullets::{self, Target};
use sim::weapons::{data, Fired, Id, Loadout};
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Inputs a client may bank while its packets are delayed. Anything sent faster
/// than real time beyond this is thrown away, which stops speed hacks.
const MAX_INPUT_BUDGET: f32 = 16.0;

/// Seconds a dead player waits before respawning (until rounds exist).
const RESPAWN_TICKS: u32 = 3 * TICK_RATE;

/// Knife reach (slash / stab) and damage.
const SLASH_RANGE: f32 = 48.0;
const STAB_RANGE: f32 = 32.0;
const SLASH_DAMAGE: f32 = 15.0;
const STAB_DAMAGE: f32 = 65.0;

/// What the server needs from the game install besides the map.
pub struct Assets {
    pub hitboxes: Hitboxes,
    pub materials: Materials,
}

impl Assets {
    /// From a Half-Life install (the directory containing `cstrike/`).
    pub fn load(game_dir: &std::path::Path) -> Result<Assets, String> {
        let mdl = game_dir.join("cstrike/models/player/arctic/arctic.mdl");
        let model = goldsrc::mdl::Model::load(&mdl).map_err(|e| format!("{}: {e}", mdl.display()))?;
        let hitboxes = Hitboxes::from_model(&model, "carbine").ok_or("player model has no aim animations")?;
        // materials.txt is optional: without it every surface counts as concrete.
        let materials = std::fs::read_to_string(game_dir.join("cstrike/sound/materials.txt")).map(|t| Materials::parse(&t)).unwrap_or_default();
        Ok(Assets { hitboxes, materials })
    }
}

struct Remote {
    id: u8,
    state: PlayerState,
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
}

/// Where everyone was at one tick (for lag compensation).
#[derive(Clone, Debug)]
pub struct PastPositions {
    pub tick: u32,
    pub players: Vec<Target>,
}

/// A shot or swing waiting to be traced at the end of the input loop.
struct Pending {
    shooter: u8,
    eye: [f32; 3],
    yaw: f32,
    velocity: [f32; 3],
    fired: Fired,
    view_tick: f32,
}

pub struct Server {
    socket: Socket,
    map: String,
    world: World,
    assets: Assets,
    vars: MoveVars,
    clients: HashMap<SocketAddr, Remote>,
    spawns: Vec<[f32; 3]>,
    next_spawn: usize,
    tick: u32,
    /// The last second of positions, newest last.
    history: VecDeque<PastPositions>,
    /// Happened since the last snapshot.
    events: Vec<GameEvent>,
    /// Extra weapons everyone spawns with (server option, for testing).
    give: Vec<Id>,
    grenades: Vec<sim::grenades::Grenade>,
    /// Smoke clouds: where and until when (server seconds).
    smokes: Vec<([f32; 3], f64)>,
}

impl Server {
    pub fn new(bind: SocketAddr, map: &str, bsp: &Bsp, assets: Assets) -> std::io::Result<Server> {
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
            assets,
            vars: MoveVars::default(),
            clients: HashMap::new(),
            spawns,
            next_spawn: 0,
            tick: 0,
            history: VecDeque::new(),
            events: Vec::new(),
            give: Vec::new(),
            grenades: Vec::new(),
            smokes: Vec::new(),
        })
    }

    /// Weapons every player gets on (re)spawn, on top of the starting pistol.
    pub fn set_give(&mut self, weapons: Vec<Id>) {
        self.give = weapons;
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

    /// Starting weapons: T (even ids) Glock + 40, CT USP + 24, plus the server's extras.
    fn spawn_state(&mut self, id: u8) -> PlayerState {
        let origin = self.next_spawn_point();
        let mut loadout = if id % 2 == 0 { Loadout::spawn(Id::Glock, 40) } else { Loadout::spawn(Id::Usp, 24) };
        for &w in &self.give {
            loadout.give(w, 0.0);
        }
        PlayerState { movement: Player::new(origin), loadout }
    }

    /// Server-side health of a player.
    pub fn vitals(&self, id: u8) -> Option<&Vitals> {
        self.clients.values().find(|c| c.id == id).map(|c| &c.vitals)
    }

    /// Server-side state of a player (for tests and admin tools).
    pub fn player(&self, id: u8) -> Option<&Player> {
        self.clients.values().find(|c| c.id == id).map(|c| &c.state.movement)
    }

    pub fn player_state(&self, id: u8) -> Option<&PlayerState> {
        self.clients.values().find(|c| c.id == id).map(|c| &c.state)
    }

    /// Everyone alive as a client saw them at `view_tick` (interpolated, like the
    /// client draws them), capped at `MAX_UNLAG_TICKS` into the past.
    pub fn rewind(&self, view_tick: f32) -> Vec<Target> {
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
            .map(|pb| match a.players.iter().find(|p| p.id == pb.id) {
                Some(pa) => Target { origin: std::array::from_fn(|i| pa.origin[i] + (pb.origin[i] - pa.origin[i]) * f), ..*pb },
                None => *pb,
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

    /// One server tick: read packets, run inputs, resolve shots, move doors, send snapshots.
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
        let due: Vec<(SocketAddr, u8)> = self.clients.iter().filter(|(_, c)| c.respawn_at.is_some_and(|t| t <= tick)).map(|(a, c)| (*a, c.id)).collect();
        for (addr, id) in due {
            let state = self.spawn_state(id);
            let c = self.clients.get_mut(&addr).unwrap();
            c.state = state;
            c.vitals = Vitals::default();
            c.respawn_at = None;
        }

        let mut actors = Vec::new();
        let mut pending = Vec::new();
        for c in self.clients.values_mut() {
            c.budget = (c.budget + 1.0).min(MAX_INPUT_BUDGET);
            c.used = false;
            while c.budget >= 1.0 {
                let Some((seq, TimedInput { input, view_tick })) = c.queue.pop_front() else { break };
                c.budget -= 1.0;
                c.used |= input.use_key && !c.use_held;
                c.use_held = input.use_key;
                c.view = (input.yaw, input.pitch);
                c.last_run = seq;
                if !c.vitals.alive() {
                    continue; // the dead don't move (their inputs are still acknowledged)
                }
                let cmd_time = seq as f64 * TICK as f64;
                let (ev, fired) = player::step(&mut c.state, &input, TICK, cmd_time, player::seed(c.id, seq), true, &self.world, &self.vars);
                if ev.fall_damage > 0.0 && c.vitals.hurt(ev.fall_damage, 0.0) {
                    c.respawn_at = Some(tick + RESPAWN_TICKS);
                    self.events.push(GameEvent::Kill { killer: c.id, victim: c.id, weapon: None, headshot: false });
                }
                if !fired.shots.is_empty() || fired.swing.is_some() || fired.throw.is_some() {
                    pending.push(Pending { shooter: c.id, eye: c.state.movement.eye(), yaw: input.yaw, velocity: c.state.movement.velocity, fired, view_tick });
                }
            }
            if c.vitals.alive() {
                actors.push(actor(&c.state.movement, c.view, c.used));
            }
        }
        for p in pending {
            self.resolve(p);
        }
        self.world.update(TICK, &actors);
        self.grenades_step();

        self.history.push_back(PastPositions {
            tick: self.tick,
            players: self
                .clients
                .values()
                .filter(|c| c.vitals.alive())
                .map(|c| Target { id: c.id, origin: c.state.movement.origin, yaw: c.view.0, pitch: c.view.1, crouched: c.state.movement.crouched })
                .collect(),
        });
        while self.history.len() > TICK_RATE as usize {
            self.history.pop_front();
        }

        let tick = self.tick;
        self.clients.retain(|_, c| tick - c.last_heard < TIMEOUT_TICKS);

        if self.tick % SNAPSHOT_EVERY == 0 {
            let players: Vec<PlayerNet> = self
                .clients
                .values()
                .map(|c| PlayerNet {
                    id: c.id,
                    player: c.state.movement,
                    vitals: c.vitals,
                    yaw: c.view.0,
                    pitch: c.view.1,
                    weapon: c.state.loadout.active_id(),
                })
                .collect();
            let doors = self.world.doors.net_state();
            let events = std::mem::take(&mut self.events);
            let now = self.tick as f64 * TICK as f64;
            let grenades: Vec<_> = self.grenades.iter().map(|g| (g.kind, g.origin)).collect();
            let smokes: Vec<SmokeNet> = self.smokes.iter().map(|s| SmokeNet { at: s.0, left: (s.1 - now) as f32 }).collect();
            let sends: Vec<_> = self.clients.iter().map(|(a, c)| (*a, c.last_run, c.state.loadout.clone())).collect();
            for (addr, ack, loadout) in sends {
                let msg = ServerMsg::Snapshot {
                    tick,
                    ack,
                    players: players.clone(),
                    doors: doors.clone(),
                    loadout,
                    events: events.clone(),
                    grenades: grenades.clone(),
                    smokes: smokes.clone(),
                };
                self.socket.send(addr, encode(&msg));
            }
        }
    }

    /// Fly grenades and set off the ones that are due.
    fn grenades_step(&mut self) {
        use sim::grenades::{self, Kind};
        let now = self.tick as f64 * TICK as f64;
        for g in &mut self.grenades {
            g.physics(TICK, &self.world, self.vars.gravity);
        }
        let (due, flying): (Vec<_>, Vec<_>) = self.grenades.drain(..).partition(|g| g.due(now));
        self.grenades = flying;
        self.smokes.retain(|s| s.1 > now);
        let tick = self.tick;
        for g in due {
            match g.kind {
                Kind::He => {
                    let blast = g.blast_point(&self.world);
                    self.events.push(GameEvent::Explosion { at: blast });
                    for c in self.clients.values_mut().filter(|c| c.vitals.alive()) {
                        let dmg = grenades::he_damage(blast, c.state.movement.origin);
                        if dmg <= 0.0 {
                            continue;
                        }
                        let (health, armor) = combat::explosion_damage(&c.vitals, dmg);
                        let killed = c.vitals.hurt(health, armor);
                        self.events.push(GameEvent::Damage { attacker: g.owner, victim: c.id, amount: health, group: HitGroup::Generic });
                        if killed {
                            c.respawn_at = Some(tick + RESPAWN_TICKS);
                            self.events.push(GameEvent::Kill { killer: g.owner, victim: c.id, weapon: Some(Id::HeGrenade), headshot: false });
                        }
                    }
                }
                Kind::Flash => {
                    let blinded = self
                        .clients
                        .values()
                        .filter(|c| c.vitals.alive())
                        .filter_map(|c| {
                            let (yaw, pitch) = c.view;
                            let (f, _, _) = bullets::view_vectors(pitch, yaw);
                            grenades::flash(&self.world, g.origin, c.state.movement.eye(), f).map(|b| (c.id, b))
                        })
                        .collect();
                    self.events.push(GameEvent::Flash { at: g.origin, blinded });
                }
                Kind::Smoke => self.smokes.push((g.origin, now + grenades::SMOKE_SECONDS)),
            }
        }
    }

    /// Trace a player's shots/swing against everyone else as the shooter saw them.
    fn resolve(&mut self, p: Pending) {
        if let Some((kind, angles)) = p.fired.throw {
            let now = self.tick as f64 * TICK as f64;
            self.grenades.push(sim::grenades::throw(kind, p.shooter, p.eye, angles[0], angles[1], p.velocity, now));
        }
        let targets: Vec<Target> = self.rewind(p.view_tick).into_iter().filter(|t| t.id != p.shooter).collect();
        let mut hits: Vec<(u8, f32, HitGroup, Id)> = Vec::new();
        for shot in &p.fired.shots {
            let r = if shot.pellets.is_some() {
                bullets::fire_pellets(&self.world, &self.assets.hitboxes, p.eye, shot, p.shooter, &targets)
            } else {
                bullets::fire_bullet(&self.world, &self.assets.materials, &self.assets.hitboxes, p.eye, shot, p.shooter, &targets)
            };
            let end = r.impacts.last().map(|i| i.point).unwrap_or_else(|| {
                let (f, _, _) = bullets::view_vectors(shot.angles[0], shot.angles[1]);
                std::array::from_fn(|i| p.eye[i] + f[i] * 2048.0)
            });
            self.events.push(GameEvent::Shot { shooter: p.shooter, weapon: shot.weapon, from: p.eye, to: end });
            hits.extend(r.hits.into_iter().map(|h| (h.victim, h.damage, h.group, shot.weapon)));
        }
        if let Some(swing) = p.fired.swing {
            let range = if swing.stab { STAB_RANGE } else { SLASH_RANGE };
            let (f, _, _) = bullets::view_vectors(swing.angles[0], swing.angles[1]);
            let end = std::array::from_fn(|i| p.eye[i] + f[i] * range);
            let hit = bullets::trace_melee(&self.world, &self.assets.hitboxes, p.eye, end, p.shooter, &targets);
            if let Some((victim, group)) = hit {
                let mut dmg = if swing.stab { STAB_DAMAGE } else { SLASH_DAMAGE };
                // Backstab: both facing the same way (within ~37 degrees).
                if swing.stab {
                    if let Some(t) = targets.iter().find(|t| t.id == victim) {
                        if (p.yaw - t.yaw).to_radians().cos() > 0.8 {
                            dmg *= 3.0;
                        }
                    }
                }
                hits.push((victim, dmg * combat::hitgroup_multiplier(group), group, Id::Knife));
            }
            if let Some(c) = self.clients.values_mut().find(|c| c.id == p.shooter) {
                c.state.loadout.knife_result(swing.stab, hit.is_some(), c.last_run as f64 * TICK as f64);
            }
        }
        let tick = self.tick;
        for (victim, damage, group, weapon) in hits {
            let Some(c) = self.clients.values_mut().find(|c| c.id == victim) else { continue };
            if !c.vitals.alive() {
                continue;
            }
            let (health, armor) = combat::bullet_damage(&c.vitals, damage, group, data::armor_factor(weapon), false);
            let killed = c.vitals.hurt(health, armor);
            self.events.push(GameEvent::Damage { attacker: p.shooter, victim, amount: health, group });
            if killed {
                c.respawn_at = Some(tick + RESPAWN_TICKS);
                self.events.push(GameEvent::Kill { killer: p.shooter, victim, weapon: Some(weapon), headshot: group == HitGroup::Head });
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
                    let state = self.spawn_state(id);
                    self.clients.insert(
                        from,
                        Remote {
                            id,
                            state,
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
            ClientMsg::Buy { item } => {
                // Free for everyone until rounds and money exist.
                let Some(c) = self.clients.get_mut(&from) else { return };
                if c.vitals.alive() {
                    let now = c.last_run as f64 * TICK as f64;
                    c.state.loadout.give(item, now);
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
