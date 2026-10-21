//! The authoritative server: owns the world, runs every player's movement and
//! weapons from their inputs, resolves shots with lag compensation, and sends snapshots.
mod admin;
mod aimcheck;
mod bomb;
mod breakables;
mod hits;
pub mod buy;
mod guard;
mod ground;
mod packets;
mod players;
mod handle;
mod lobby_link;
mod maps;
mod reserve;
mod report;
mod sealed;

/// Seconds between heartbeats to the lobby.
const HEARTBEAT_SECS: f32 = crate::lobby::HEARTBEAT_SECONDS as f32;
mod round;
mod recording;
mod running;
mod setup;
pub use setup::{Assets, Rules};
mod snapshot;
mod surrender;
mod visibility;
mod voice;

use crate::config::MatchConfig;
use crate::protocol::*;
use crate::rounds::Match;
use crate::socket::Socket;
use bomb::Bomb;
pub use reserve::{Group, MatchSetup};
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
const MAX_INPUT_BUDGET_SECS: f32 = 0.125;
/// Inputs left waiting after a tick that count as a backlog, and how many ticks in a row a
/// backlog may last before the oldest are dropped (else a burst after a client hitch, e.g.
/// loading the map, turns into permanent input lag: the budget only allows real time).
const BACKLOG_INPUTS: usize = 4;
const BACKLOG_SECS: f32 = 0.25;
/// Reliable events kept per client while unacknowledged.
const MAX_OUTBOX: usize = 64;


/// Dummies stay down a little longer so you see them fall.
const DUMMY_RESPAWN_SECS: f32 = 2.0;

/// Knife reach (slash / stab) and damage.
const SLASH_RANGE: f32 = 48.0;
const STAB_RANGE: f32 = 32.0;
const SLASH_DAMAGE: f32 = 15.0;
const STAB_DAMAGE: f32 = 65.0;



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
const REPICK_SECS: f32 = 0.5;

/// A player's scoreboard numbers beyond kills and deaths.
#[derive(Default)]
struct Stats {
    assists: u32,
    mvps: u32,
    headshots: u32,
    damage: u32,
    rounds: u32,
    /// Milliseconds, smoothed.
    ping: f32,
    /// This life: health each enemy took off them (for assists).
    hurt_by: Vec<(u8, f32)>,
    /// This round: kills and damage (for the MVP).
    round_kills: u32,
    round_damage: u32,
}

struct Remote {
    /// Their aim this match, for the aim checks.
    aim: aimcheck::Aim,
    id: u8,
    name: String,
    team: Team,
    /// A practice dummy (no network connection; the server makes its inputs).
    bot: bool,
    kills: i32,
    deaths: u32,
    /// Scoreboard stats (see `Score`).
    stats: Stats,
    /// What they bought this round, oldest first (`buy::undo_buys` sells it back).
    purchases: Vec<buy::Purchase>,
    /// Their character on each side (`ClientMsg::Looks`); until they say, one picked by id.
    looks: [u8; 2],
    /// How their model is animated (as clients draw it): hitboxes follow it.
    anim: sim::anim::Anim,
    money: i32,
    defuse_kit: bool,
    /// Reserved matches: this player's ticket (0 = none) and team group.
    ticket: u64,
    group: Option<Group>,
    /// Reliable events not yet acknowledged, numbered (resent in every snapshot).
    outbox: VecDeque<(u32, GameEvent)>,
    next_reliable: u32,
    /// Snapshots sent lately (tick, bytes) and the newest one the client confirmed: the next
    /// go out as changes to that one.
    sent: VecDeque<(u32, Vec<u8>)>,
    snap_ack: u32,
    /// The last flash that blinded them: when, and how (a new one combines with it).
    blind: Option<(f64, sim::grenades::Blind)>,
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
    map_hash: u64,
    world: World,
    assets: Assets,
    vars: MoveVars,
    clients: HashMap<SocketAddr, Remote>,
    /// Spawn points: T (`info_player_deathmatch`), CT (`info_player_start`).
    spawns_t: Vec<[f32; 3]>,
    spawns_ct: Vec<[f32; 3]>,
    rules: Rules,
    rate: crate::tick::TickRate,
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
    /// Defuse kits on the floor (a CT died with one).
    kits: Vec<[f32; 3]>,
    /// Kill events in `events` already counted (events are cleared at each snapshot).
    kills_settled: usize,
    round: Match,
    bomb: Bomb,
    /// Who planted this round's bomb (for the MVP).
    planter: Option<u8>,
    /// Practice: the tick the next bomb is handed out (after one went off or was defused).
    practice_bomb: u32,
    pieces: breakables::Pieces,
    guard: guard::Guard,
    /// Set while running a match for a fixed list of players.
    reserved: Option<reserve::Reserved>,
    /// Shutting down: no new matches (the lobby sees us busy), stop once the match is over.
    draining: Option<u32>,
    /// The lobbies' addresses (`master` in the config: one, or the active and standby ones,
    /// comma separated; names are looked up again every minute).
    lobbies: Vec<SocketAddr>,
    /// Name lookups running in the background (a slow DNS server must never stall the game).
    lookup: Option<std::sync::mpsc::Receiver<Vec<SocketAddr>>>,
    /// The key for talking to the lobby (from the official token, which is never sent).
    server_key: Option<crate::crypto::ServerKey>,
    /// When the last booking we took was sealed (older ones are replays).
    last_booking: u64,
    /// Enemies in sight of whom, since when (for the aim checks).
    sightings: aimcheck::Sightings,
    /// Where maps are loaded from, and the pool maps found there (see `maps`).
    game_dir: Option<std::path::PathBuf>,
    maps: Vec<String>,
    /// The match being recorded (see `recording`).
    recorder: Option<crate::demo::Recorder>,
    /// Deathmatch seats the lobby handed out: ticket -> (key, when). Unused ones expire.
    seats: HashMap<u64, ([u8; 32], u32)>,
    /// A new player's channel, opened from their ticket's key while their join is checked.
    joining: Option<(SocketAddr, u64, crate::crypto::Channel)>,
    /// Size of the last snapshot sent (bandwidth checks).
    snapshot_bytes: usize,
    /// Anti-wallhack: when each (viewer, target) pair last had line of sight.
    last_seen: HashMap<(u8, u8), u32>,
}

impl Server {
    pub fn new(bind: SocketAddr, map: &str, bsp: &Bsp, assets: Assets) -> std::io::Result<Server> {
        let rate = crate::tick::TickRate::default();
        let zones = sim::zones::Zones::from_bsp(bsp);
        let (spawns_t, spawns_ct) = (zones.spawns(sim::zones::Side::T), zones.spawns(sim::zones::Side::Ct));
        Ok(Server {
            socket: Socket::bind(bind)?,
            map: map.to_string(),
            map_hash: bsp.hash,
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
            kits: Vec::new(),
            kills_settled: 0,
            round: Match::new(MatchConfig::default(), rate.hz()),
            rate,
            bomb: Bomb::None,
            planter: None,
            practice_bomb: 0,
            last_seen: HashMap::new(),
            pieces: breakables::Pieces::from_bsp(bsp),
            snapshot_bytes: 0,
            guard: guard::Guard::new(rate),
            reserved: None,
            draining: None,
            lobbies: Vec::new(),
            lookup: None,
            server_key: None,
            last_booking: 0,
            joining: None,
            seats: HashMap::new(),
            recorder: None,
            game_dir: None,
            maps: Vec::new(),
            sightings: Default::default(),
        })
    }

    /// How many steps a second this server runs (before it starts; clients follow it).
    pub fn set_rate(&mut self, rate: crate::tick::TickRate) {
        self.rate = rate;
        self.guard = guard::Guard::new(rate);
        self.round = Match::new(self.round.cfg.clone(), rate.hz());
    }

    /// Match settings (server config file). Restarts the match.
    pub fn set_config(&mut self, cfg: MatchConfig) {
        self.lobbies = crate::config::addresses(&cfg.lobby, false);
        self.server_key = (!cfg.official_token.is_empty()).then(|| crate::crypto::ServerKey::from_token(&cfg.official_token));
        if cfg.deathmatch {
            self.rules = Rules::deathmatch();
        }
        self.set_rate(cfg.tick_rate);
        self.round = Match::new(cfg, self.rate.hz());
        self.look_up_lobby();
    }

    /// An official server (it has the lobby's token): only booked players may join.
    fn official(&self) -> bool {
        !self.round.cfg.official_token.is_empty()
    }

    /// Weapons every player gets on (re)spawn, on top of the starting pistol.
    pub fn set_give(&mut self, weapons: Vec<Id>) {
        self.give = weapons;
    }

    pub fn tick_rate(&self) -> crate::tick::TickRate {
        self.rate
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.socket.local_addr()
    }

    /// Practice room, dummies, respawn time and so on.
    pub fn set_rules(&mut self, rules: Rules) {
        self.rules = rules;
        self.clients.retain(|_, c| !c.bot);
        let dummies = if self.rules.practice { self.rules.dummies } else { 0 };
        for i in 0..dummies {
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
        if self.rules.deathmatch {
            return self.deathmatch_spawn(team);
        }
        let list = if team == Team::Ct { &self.spawns_ct } else { &self.spawns_t };
        let taken = |p: &[f32; 3]| {
            self.clients.values().filter(|c| c.vitals.alive()).any(|c| {
                let o = c.state.movement.origin;
                (o[0] - p[0]).abs() < 40.0 && (o[1] - p[1]).abs() < 40.0 && (o[2] - p[2]).abs() < 80.0
            })
        };
        list.iter().find(|p| !taken(p)).or(list.first()).copied().unwrap_or([0.0; 3])
    }

    /// Deathmatch: any spawn point on the map, picked at random among the ones furthest from
    /// living enemies (nobody spawns in someone's sights, nobody can camp a spawn).
    fn deathmatch_spawn(&self, team: Team) -> [f32; 3] {
        let enemies: Vec<[f32; 3]> = self.clients.values().filter(|c| c.vitals.alive() && c.team != team && c.team != Team::Spectator).map(|c| c.state.movement.origin).collect();
        let mut all: Vec<([f32; 3], f32)> = self
            .spawns_t
            .iter()
            .chain(&self.spawns_ct)
            .filter(|p| !self.clients.values().any(|c| c.vitals.alive() && (0..3).map(|k| (c.state.movement.origin[k] - p[k]).powi(2)).sum::<f32>() < 64.0 * 64.0))
            .map(|p| (*p, enemies.iter().map(|e| (0..3).map(|k| (e[k] - p[k]).powi(2)).sum::<f32>()).fold(f32::MAX, f32::min)))
            .collect();
        all.sort_by(|a, b| b.1.total_cmp(&a.1));
        let pick = (crate::secure::random() % all.len().clamp(1, 4) as u64) as usize;
        all.get(pick).map(|(p, _)| *p).unwrap_or([0.0; 3])
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

    /// Everyone alive as a client saw them at `view_tick` (interpolated, like the
    /// client draws them), capped at `MAX_UNLAG_SECS` into the past.
    pub fn rewind(&self, view_tick: f32) -> Vec<Target> {
        let oldest = self.tick.saturating_sub(self.rate.secs(MAX_UNLAG_SECS)) as f32;
        let t = view_tick.clamp(oldest, self.tick as f32);
        // Only the ticks that went out as snapshots: that's exactly what clients blend between.
        let every = self.rate.snapshot_every();
        let sent: Vec<&PastPositions> = self.history.iter().filter(|h| h.tick % every == 0).collect();
        let pair = sent.iter().zip(sent.iter().skip(1)).find(|(a, b)| a.tick as f32 <= t && t <= b.tick as f32);
        let Some((a, b)) = pair else {
            return self.history.back().map(|h| h.players.clone()).unwrap_or_default();
        };
        let f = (t - a.tick as f32) / (b.tick - a.tick) as f32;
        b.players
            .iter()
            .map(|pb| match a.players.iter().find(|p| p.id == pb.id) {
                Some(pa) => Target { origin: std::array::from_fn(|i| pa.origin[i] + (pb.origin[i] - pa.origin[i]) * f), ..pb.clone() },
                None => pb.clone(),
            })
            .collect()
    }

    /// One server tick: read packets, run inputs, resolve shots, move doors, send snapshots.
    pub fn step(&mut self) {
        self.tick += 1;
        self.socket.tick();
        let tick = self.tick;
        self.read_packets();
        if tick % self.rate.hz() == 0 {
            self.guard.tidy(tick);
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
        // Dummies "send" an input every tick: stand still.
        for c in self.clients.values_mut().filter(|c| c.bot) {
            let seq = c.last_queued + 1;
            c.queue.push_back((seq, TimedInput { input: sim::pmove::Input::default(), view_tick: tick as f32 }));
            c.last_queued = seq;
            c.last_heard = tick;
        }

        self.run_players();
        self.grenades_step();
        self.settle_deaths();
        self.ground_step();
        self.bomb_step();
        self.settle_deaths();
        self.round_step();

        // Positions as clients get them (1/8 unit, `PlayerNet`), so lag compensation rewinds to
        // exactly what they were shown.
        self.history.push_back(PastPositions {
            tick: self.tick,
            players: self
                .clients
                .values()
                .filter(|c| c.vitals.alive())
                .map(|c| {
                    let origin = c.state.movement.origin.map(|v| (v * 8.0).round() / 8.0);
                    let anim = Some((c.anim.clone(), c.drive(), self.tick as f32 * self.rate.tick()));
                    Target { id: c.id, origin, yaw: c.view.0, pitch: c.view.1, crouched: c.state.movement.crouched, weapon: c.state.loadout.active_id(), anim }
                })
                .collect(),
        });
        while self.history.len() > self.rate.hz() as usize {
            self.history.pop_front();
        }

        let tick = self.tick;
        let timeout = self.rate.secs(TIMEOUT_SECS);
        let gone: Vec<SocketAddr> = self.clients.iter().filter(|(_, c)| !c.bot && tick - c.last_heard >= timeout).map(|(a, _)| *a).collect();
        for a in gone {
            if let Some(c) = self.clients.remove(&a) {
                self.remember(&c);
            }
        }
        let ids: std::collections::HashSet<u8> = self.clients.values().map(|c| c.id).collect();
        self.last_seen.retain(|(v, t), _| ids.contains(v) && ids.contains(t));

        self.send_updates();
    }

}

impl Remote {
    /// What drives their animation now.
    fn drive(&self) -> sim::anim::Drive {
        let m = &self.state.movement;
        sim::anim::Drive {
            origin: m.origin,
            yaw: self.view.0,
            pitch: self.view.1,
            crouched: m.crouched,
            airborne: !m.on_ground && !m.on_ladder,
            reloading: self.state.loadout.active_gun().is_some_and(|g| g.reloading),
            weapon: self.state.loadout.active_id(),
        }
    }

    /// Their scoreboard line (money included: `send_updates` hides it from the other team).
    fn score(&self) -> Score {
        let s = &self.stats;
        Score {
            id: self.id,
            name: self.name.clone(),
            kills: self.kills,
            deaths: self.deaths,
            alive: self.vitals.alive(),
            team: self.team,
            bot: self.bot,
            assists: s.assists,
            mvps: s.mvps,
            headshots: s.headshots,
            damage: s.damage,
            rounds: s.rounds,
            ping: s.ping.round() as u16,
            money: Some(self.money),
            looks: self.looks,
        }
    }

    /// A newly connected player: spectating until they pick a side.
    fn new(id: u8, name: String, tick: u32) -> Remote {
        Remote {
            aim: Default::default(),
            id,
            name,
            team: Team::Spectator,
            bot: false,
            kills: 0,
            deaths: 0,
            stats: Stats::default(),
            anim: sim::anim::Anim::new(0.0),
            looks: [id % CLASSES; 2],
            purchases: Vec::new(),
            money: 0,
            defuse_kit: false,
            ticket: 0,
            group: None,
            outbox: VecDeque::new(),
            next_reliable: 0,
            sent: VecDeque::new(),
            snap_ack: 0,
            blind: None,
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
