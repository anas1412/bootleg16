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
/// Inputs left waiting after a tick that count as a backlog, and how many ticks in a row a
/// backlog may last before the oldest are dropped (else a burst after a client hitch, e.g.
/// loading the map, turns into permanent input lag: the budget only allows real time).
const BACKLOG_INPUTS: usize = 4;
const BACKLOG_TICKS: u32 = TICK_RATE / 4;

/// How a server runs the game until the round system exists.
#[derive(Clone, Debug)]
pub struct Rules {
    /// Practice room: everything below is set for practising.
    pub practice: bool,
    /// Ticks before a dead player comes back.
    pub respawn_ticks: u32,
    /// Reserve ammo is topped up all the time (you still reload).
    pub infinite_ammo: bool,
    /// Practice dummies on the other team.
    pub dummies: u8,
    /// Dummies strafe left and right instead of standing still.
    pub dummy_move: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { practice: false, respawn_ticks: 3 * TICK_RATE, infinite_ammo: false, dummies: 0, dummy_move: false }
    }
}

impl Rules {
    pub fn practice(dummies: u8, dummy_move: bool) -> Rules {
        Rules { practice: true, respawn_ticks: TICK_RATE / 2, infinite_ammo: true, dummies, dummy_move }
    }
}

/// Dummies stay down a little longer so you see them fall.
const DUMMY_RESPAWN_TICKS: u32 = 2 * TICK_RATE;

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

/// A gun on the ground (dropped, or left by a dead player).
struct GroundGun {
    gun: sim::weapons::GunState,
    origin: [f32; 3],
    velocity: [f32; 3],
    /// Who dropped it, and until when they can't pick it straight back up.
    dropper: u8,
    dropper_wait_until: u32,
}

/// Seconds before the dropper can pick their own gun back up.
const REPICK_TICKS: u32 = TICK_RATE / 2;

struct Remote {
    id: u8,
    name: String,
    team: Team,
    /// A practice dummy (no network connection; the server makes its inputs).
    bot: bool,
    kills: i32,
    deaths: u32,
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
    /// Ticks in a row that ended with a backlog of inputs.
    backlog: u32,
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
    /// Spawn points: T (`info_player_deathmatch`), CT (`info_player_start`).
    spawns_t: Vec<[f32; 3]>,
    spawns_ct: Vec<[f32; 3]>,
    rules: Rules,
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
    ground: Vec<GroundGun>,
    /// Kill events in `events` already counted (events are cleared at each snapshot).
    kills_settled: usize,
}

impl Server {
    pub fn new(bind: SocketAddr, map: &str, bsp: &Bsp, assets: Assets) -> std::io::Result<Server> {
        let ents = bsp.entity_list();
        let spawns = |class: &str| -> Vec<[f32; 3]> {
            ents.iter()
                .filter(|e| e.get("classname").map(String::as_str) == Some(class))
                .filter_map(|e| {
                    let v: Vec<f32> = e.get("origin")?.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                    Some([*v.first()?, *v.get(1)?, *v.get(2)?])
                })
                .collect()
        };
        let (spawns_t, spawns_ct) = (spawns("info_player_deathmatch"), spawns("info_player_start"));
        Ok(Server {
            socket: Socket::bind(bind)?,
            map: map.to_string(),
            world: World::from_bsp(bsp),
            assets,
            vars: MoveVars::default(),
            clients: HashMap::new(),
            spawns_t,
            spawns_ct,
            rules: Rules::default(),
            tick: 0,
            history: VecDeque::new(),
            events: Vec::new(),
            give: Vec::new(),
            grenades: Vec::new(),
            smokes: Vec::new(),
            ground: Vec::new(),
            kills_settled: 0,
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

    /// Practice room, dummies, respawn time and so on.
    pub fn set_rules(&mut self, rules: Rules) {
        self.rules = rules;
        self.clients.retain(|_, c| !c.bot);
        for i in 0..self.rules.dummies {
            let id = (0..=u8::MAX).find(|i| self.clients.values().all(|c| c.id != *i)).unwrap_or(0);
            let addr = SocketAddr::from(([0, 0, 0, 0], 60000 + i as u16)); // never sent to
            let mut r = Remote::new(id, format!("Dummy {}", i + 1), self.tick);
            r.bot = true;
            self.clients.insert(addr, r);
        }
        self.place_dummies();
    }

    /// Dummies go on the other side from the earliest-joined real player (T if nobody has picked).
    fn place_dummies(&mut self) {
        let human = self.clients.values().filter(|c| !c.bot && c.team != Team::Spectator).min_by_key(|c| c.id).map(|c| c.team);
        let side = if human == Some(Team::T) { Team::Ct } else { Team::T };
        let tick = self.tick;
        for c in self.clients.values_mut().filter(|c| c.bot && c.team != side) {
            c.team = side;
            c.vitals.health = 0.0;
            c.respawn_at = Some(tick);
        }
    }

    /// A free spawn point for the team (not on top of someone else).
    fn spawn_point(&self, team: Team) -> [f32; 3] {
        let list = if team == Team::Ct { &self.spawns_ct } else { &self.spawns_t };
        let taken = |p: &[f32; 3]| {
            self.clients.values().filter(|c| c.vitals.alive()).any(|c| {
                let o = c.state.movement.origin;
                (o[0] - p[0]).abs() < 40.0 && (o[1] - p[1]).abs() < 40.0 && (o[2] - p[2]).abs() < 80.0
            })
        };
        list.iter().find(|p| !taken(p)).or(list.first()).copied().unwrap_or([0.0; 3])
    }

    /// CS starting weapons: T Glock + 40 rounds, CT USP + 24; dummies hold a rifle.
    fn spawn_state(&self, team: Team, bot: bool) -> PlayerState {
        let origin = self.spawn_point(team);
        let mut loadout = if team == Team::Ct { Loadout::spawn(Id::Usp, 24) } else { Loadout::spawn(Id::Glock, 40) };
        if bot {
            loadout.give(if team == Team::Ct { Id::M4a1 } else { Id::Ak47 }, 0.0);
        }
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
        self.run_until(&std::sync::atomic::AtomicBool::new(false));
    }

    /// Run at the tick rate until `stop` is set (the game's built-in server stops when you leave).
    pub fn run_until(&mut self, stop: &std::sync::atomic::AtomicBool) {
        let tick = Duration::from_secs_f64(1.0 / TICK_RATE as f64);
        let mut next = Instant::now();
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
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

        // Respawn the dead whose wait is over (spectators never do).
        let tick = self.tick;
        let due: Vec<(SocketAddr, Team, bool)> =
            self.clients.iter().filter(|(_, c)| c.team != Team::Spectator && c.respawn_at.is_some_and(|t| t <= tick)).map(|(a, c)| (*a, c.team, c.bot)).collect();
        for (addr, team, bot) in due {
            let state = self.spawn_state(team, bot);
            let c = self.clients.get_mut(&addr).unwrap();
            c.state = state;
            c.vitals = Vitals::default();
            c.respawn_at = None;
        }
        // Dummies "send" an input every tick: stand still, or strafe left and right.
        let strafe = if self.rules.dummy_move { if (tick / TICK_RATE) % 2 == 0 { 250.0 } else { -250.0 } } else { 0.0 };
        for c in self.clients.values_mut().filter(|c| c.bot) {
            let seq = c.last_queued + 1;
            c.queue.push_back((seq, TimedInput { input: sim::pmove::Input { side: strafe, ..Default::default() }, view_tick: tick as f32 }));
            c.last_queued = seq;
            c.last_heard = tick;
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
                if ev.footstep {
                    self.events.push(GameEvent::Footstep { player: c.id, at: c.state.movement.origin });
                }
                if ev.fall_damage > 0.0 && c.vitals.hurt(ev.fall_damage, 0.0) {
                    c.respawn_at = Some(tick + if c.bot { DUMMY_RESPAWN_TICKS } else { self.rules.respawn_ticks });
                    self.events.push(GameEvent::Kill { killer: c.id, victim: c.id, weapon: None, headshot: false });
                }
                if let Some(gun) = fired.dropped {
                    let (f, _, _) = bullets::view_vectors(input.pitch, input.yaw);
                    let eye = c.state.movement.eye();
                    let v = c.state.movement.velocity;
                    self.ground.push(GroundGun {
                        gun,
                        origin: eye,
                        velocity: std::array::from_fn(|i| f[i] * 400.0 + v[i]),
                        dropper: c.id,
                        dropper_wait_until: tick + REPICK_TICKS,
                    });
                }
                if !fired.shots.is_empty() || fired.swing.is_some() || fired.throw.is_some() {
                    pending.push(Pending { shooter: c.id, eye: c.state.movement.eye(), yaw: input.yaw, velocity: c.state.movement.velocity, fired, view_tick });
                }
            }
            c.backlog = if c.queue.len() > BACKLOG_INPUTS { c.backlog + 1 } else { 0 };
            if c.backlog > BACKLOG_TICKS {
                // Stale movement: skip it (the client's prediction corrects once).
                while c.queue.len() > 1 {
                    c.queue.pop_front();
                }
                c.backlog = 0;
            }
            if c.vitals.alive() {
                actors.push(actor(&c.state.movement, c.view, c.used));
            }
        }
        for p in pending {
            self.resolve(p);
        }
        if self.rules.infinite_ammo {
            for c in self.clients.values_mut() {
                c.state.loadout.refill_ammo(true);
                c.state.loadout.refill_ammo(false);
            }
        }
        self.world.update(TICK, &actors);
        self.grenades_step();
        self.settle_deaths();
        self.ground_step();

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
        self.clients.retain(|_, c| c.bot || tick - c.last_heard < TIMEOUT_TICKS);

        if self.tick % (TICK_RATE / 4) == 0 {
            let mut scores: Vec<Score> = self
                .clients
                .values()
                .map(|c| Score { id: c.id, name: c.name.clone(), kills: c.kills, deaths: c.deaths, alive: c.vitals.alive(), team: c.team, bot: c.bot })
                .collect();
            scores.sort_by_key(|s| (-s.kills, s.deaths));
            let msg = encode(&ServerMsg::Scores(scores));
            let addrs: Vec<_> = self.clients.iter().filter(|(_, c)| !c.bot).map(|(a, _)| *a).collect();
            for a in addrs {
                self.socket.send(a, msg.clone());
            }
        }
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
                    team: c.team,
                })
                .collect();
            let doors = self.world.doors.net_state();
            let events = std::mem::take(&mut self.events);
            self.kills_settled = 0;
            let now = self.tick as f64 * TICK as f64;
            let grenades: Vec<_> = self.grenades.iter().map(|g| (g.kind, g.origin)).collect();
            let smokes: Vec<SmokeNet> = self.smokes.iter().map(|s| SmokeNet { at: s.0, left: (s.1 - now) as f32 }).collect();
            let ground: Vec<(Id, [f32; 3])> = self.ground.iter().map(|g| (g.gun.id, g.origin)).collect();
            let sends: Vec<_> = self.clients.iter().filter(|(_, c)| !c.bot).map(|(a, c)| (*a, c.last_run, c.state.loadout.clone())).collect();
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
                    ground: ground.clone(),
                };
                self.socket.send(addr, encode(&msg));
            }
        }
    }

    /// Dropped guns fall and slide to a stop; walking over one picks it up if that slot is free.
    fn ground_step(&mut self) {
        for g in &mut self.ground {
            if g.velocity == [0.0; 3] {
                continue;
            }
            g.velocity[2] -= self.vars.gravity * TICK;
            let end = std::array::from_fn(|i| g.origin[i] + g.velocity[i] * TICK);
            let t = self.world.trace_line(g.origin, end);
            g.origin = t.end;
            if t.fraction < 1.0 {
                if t.normal[2] > 0.7 {
                    g.velocity = [0.0; 3];
                } else {
                    let d = g.velocity[0] * t.normal[0] + g.velocity[1] * t.normal[1] + g.velocity[2] * t.normal[2];
                    g.velocity = std::array::from_fn(|i| g.velocity[i] - t.normal[i] * d);
                }
            }
        }
        let tick = self.tick;
        let mut i = 0;
        while i < self.ground.len() {
            let g = &self.ground[i];
            let taker = self.clients.values_mut().find(|c| {
                let o = c.state.movement.origin;
                let h = c.state.movement.hull();
                let near = (0..3).all(|k| g.origin[k] >= o[k] + h.mins()[k] - 8.0 && g.origin[k] <= o[k] + h.maxs()[k] + 8.0);
                near && c.vitals.alive() && !(c.id == g.dropper && tick < g.dropper_wait_until)
            });
            let gun = g.gun;
            if taker.is_some_and(|c| c.state.loadout.pick_up(gun)) {
                self.ground.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// A player died: their primary (or pistol) falls where they were.
    fn drop_on_death(&mut self, victim: u8) {
        let Some(c) = self.clients.values_mut().find(|c| c.id == victim) else { return };
        let l = &mut c.state.loadout;
        let gun = l.primary.take().or_else(|| l.secondary.take());
        if let Some(gun) = gun {
            self.ground.push(GroundGun { gun, origin: c.state.movement.origin, velocity: [0.0, 0.0, -1.0], dropper: victim, dropper_wait_until: 0 });
        }
    }

    /// Fly grenades and set off the ones that are due.
    fn grenades_step(&mut self) {
        use sim::grenades::{self, Kind};
        let now = self.tick as f64 * TICK as f64;
        for g in &mut self.grenades {
            if g.physics(TICK, &self.world, self.vars.gravity) {
                self.events.push(GameEvent::GrenadeBounce { kind: g.kind, at: g.origin });
            }
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
                        self.events.push(GameEvent::Damage { attacker: g.owner, victim: c.id, amount: health, group: HitGroup::Generic, sound: HitSound::Explosion, at: None });
                        if killed {
                            c.respawn_at = Some(tick + if c.bot { DUMMY_RESPAWN_TICKS } else { self.rules.respawn_ticks });
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
        let mut hits: Vec<(u8, f32, HitGroup, Id, Option<[f32; 3]>)> = Vec::new();
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
            let holes = r.impacts.iter().filter(|i| !i.sky).map(|i| (i.point, i.normal)).collect();
            self.events.push(GameEvent::Shot { shooter: p.shooter, weapon: shot.weapon, silenced: shot.silenced, from: p.eye, to: end, holes });
            hits.extend(r.hits.into_iter().map(|h| (h.victim, h.damage, h.group, shot.weapon, Some(h.point))));
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
                hits.push((victim, dmg * combat::hitgroup_multiplier(group), group, Id::Knife, None));
            }
            if let Some(c) = self.clients.values_mut().find(|c| c.id == p.shooter) {
                c.state.loadout.knife_result(swing.stab, hit.is_some(), c.last_run as f64 * TICK as f64);
            }
        }
        let tick = self.tick;
        for (victim, damage, group, weapon, at) in hits {
            let Some(c) = self.clients.values_mut().find(|c| c.id == victim) else { continue };
            if !c.vitals.alive() {
                continue;
            }
            let (health, armor) = combat::bullet_damage(&c.vitals, damage, group, data::armor_factor(weapon), false);
            let sound = match group {
                HitGroup::Head if c.vitals.helmet && c.vitals.armor > 0.0 => HitSound::Helmet,
                HitGroup::Head => HitSound::Headshot,
                HitGroup::LeftLeg | HitGroup::RightLeg => HitSound::Flesh,
                _ if c.vitals.armor > 0.0 => HitSound::Kevlar,
                _ => HitSound::Flesh,
            };
            let killed = c.vitals.hurt(health, armor);
            self.events.push(GameEvent::Damage { attacker: p.shooter, victim, amount: health, group, sound, at });
            if killed {
                c.respawn_at = Some(tick + if c.bot { DUMMY_RESPAWN_TICKS } else { self.rules.respawn_ticks });
                self.events.push(GameEvent::Kill { killer: p.shooter, victim, weapon: Some(weapon), headshot: group == HitGroup::Head });
            }
        }
        self.settle_deaths();
    }

    /// Bookkeeping for kill events raised this tick: scores and dropped guns.
    fn settle_deaths(&mut self) {
        let kills: Vec<(u8, u8)> = self.events.iter().filter_map(|e| match e {
            GameEvent::Kill { killer, victim, .. } => Some((*killer, *victim)),
            _ => None,
        }).collect();
        for (killer, victim) in kills.into_iter().skip(self.kills_settled) {
            if let Some(v) = self.clients.values_mut().find(|c| c.id == victim) {
                v.deaths += 1;
            }
            if let Some(k) = self.clients.values_mut().find(|c| c.id == killer) {
                k.kills += if killer == victim { -1 } else { 1 };
            }
            self.drop_on_death(victim);
            self.kills_settled += 1;
        }
    }

    fn handle(&mut self, from: SocketAddr, msg: ClientMsg) {
        match msg {
            ClientMsg::Hello { version, name } => {
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
                    // Everyone starts as a spectator and picks a side (JoinTeam), like CS.
                    let name: String = name.chars().filter(|c| !c.is_control()).take(24).collect();
                    self.clients.insert(from, Remote::new(id, name, self.tick));
                }
                let id = self.clients[&from].id;
                self.socket.send(from, encode(&ServerMsg::Welcome { id, map: self.map.clone(), tick_rate: TICK_RATE, practice: self.rules.practice }));
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
                if !c.vitals.alive() {
                    return;
                }
                let now = c.last_run as f64 * TICK as f64;
                let l = &mut c.state.loadout;
                match item {
                    BuyItem::Weapon(id) => {
                        // Buying a gun for a taken slot drops the old one, like CS.
                        let taken = match data::def(id).slot {
                            data::Slot::Primary => l.primary.is_some(),
                            data::Slot::Secondary => l.secondary.is_some(),
                            _ => false,
                        };
                        if taken && sim::weapons::is_gun(id) {
                            l.switch_to(data::def(id).slot, now);
                            if let Some(old) = l.drop_active(now) {
                                let o = c.state.movement.origin;
                                self.ground.push(GroundGun { gun: old, origin: o, velocity: [0.0, 0.0, -1.0], dropper: c.id, dropper_wait_until: self.tick + REPICK_TICKS });
                            }
                        }
                        c.state.loadout.give(id, now);
                    }
                    BuyItem::PrimaryAmmo => l.refill_ammo(true),
                    BuyItem::SecondaryAmmo => l.refill_ammo(false),
                    BuyItem::Kevlar => c.vitals.armor = combat::MAX_ARMOR,
                    BuyItem::KevlarHelmet => {
                        c.vitals.armor = combat::MAX_ARMOR;
                        c.vitals.helmet = true;
                    }
                    BuyItem::DefuseKit | BuyItem::NightVision => {} // with the bomb / later
                }
            }
            ClientMsg::JoinTeam { team } => {
                let (t, ct) = self.clients.values().filter(|c| !c.bot).fold((0, 0), |(t, ct), c| match c.team {
                    Team::T => (t + 1, ct),
                    Team::Ct => (t, ct + 1),
                    Team::Spectator => (t, ct),
                });
                let tick = self.tick;
                let Some(c) = self.clients.get_mut(&from) else { return };
                let team = team.unwrap_or(if t <= ct { Team::T } else { Team::Ct });
                if c.team == team {
                    return;
                }
                c.team = team;
                c.vitals.health = 0.0; // switching sides respawns you (or makes you watch)
                c.respawn_at = if team == Team::Spectator { None } else { Some(tick) };
                self.place_dummies();
            }
            ClientMsg::Bye => {
                self.clients.remove(&from);
            }
        }
    }
}

impl Remote {
    /// A newly connected player: spectating until they pick a side.
    fn new(id: u8, name: String, tick: u32) -> Remote {
        Remote {
            id,
            name,
            team: Team::Spectator,
            bot: false,
            kills: 0,
            deaths: 0,
            state: PlayerState { movement: Player::new([0.0; 3]), loadout: Loadout::knife_only() },
            vitals: Vitals { health: 0.0, ..Vitals::default() },
            respawn_at: None,
            queue: VecDeque::new(),
            last_queued: 0,
            last_run: 0,
            budget: 0.0,
            backlog: 0,
            last_heard: tick,
            use_held: false,
            used: false,
            view: (0.0, 0.0),
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
