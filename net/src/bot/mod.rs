//! Test bots: real network players for filling test matches and load tests. A bot is a normal
//! `Client` (it signs in, gets a ticket, connects, predicts) with a simple brain on top: buy a
//! rifle, head for the enemy spawn, turn away from walls, and shoot enemies it can see, with a
//! human-ish reaction time and aim error, finding its way with `nav`. It only knows what the server shows it (the same
//! anti-wallhack view as a person). Never in real matches: the official lobby requires Steam.
use crate::client::Client;
use crate::protocol::{BuyItem, Team};
use crate::rounds::Phase;
use sim::pmove::Input;
use sim::weapons::Id;
use std::sync::Arc;

pub mod nav;
pub use nav::Nav;
use nav::dist;

/// Reaction time before the first shot at a newly seen enemy (ticks at 128 Hz: ~230 ms).
const REACTION: u32 = 30;
/// Re-aim this often while firing in bursts.
const BURST: u32 = 24;

pub struct Bot {
    pub client: Client,
    nav: Arc<Nav>,
    /// The two spawns: (terrorist, counter-terrorist). Each side heads for the other's.
    spawns: ([f32; 3], [f32; 3]),
    /// The way there (nav points) and how far along it we are; plan again at `replan`.
    path: Vec<u32>,
    path_at: usize,
    replan: u32,
    /// Reached the enemy spawn this life: roam the map from now on.
    roaming: bool,
    /// Links we got stuck on (a door, a crate the map scan missed): planned around.
    blocked: std::collections::HashSet<(u32, u32)>,
    /// Stuck checks in a row on the same link.
    stuck_on: u32,
    seen: Option<(u8, u32)>,
    bought_round: Option<u32>,
    t: u32,
    rng: u64,
}

impl Bot {
    /// `nav`: the map's (`Nav::of_map`, shared by all bots); `spawns`: from `spawns_of`.
    pub fn new(client: Client, nav: Arc<Nav>, spawns: ([f32; 3], [f32; 3]), seed: u64) -> Bot {
        Bot { client, nav, spawns, path: Vec::new(), path_at: 0, replan: 0, roaming: false, blocked: Default::default(), stuck_on: 0, seen: None, bought_round: None, t: 0, rng: seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1 }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Decide and send this tick's input.
    pub fn tick(&mut self) {
        self.t += 1;
        let input = self.think();
        self.client.tick(input);
    }

    fn think(&mut self) -> Input {
        let team = self.client.team();
        if self.client.id().is_none() || team == Team::Spectator || !self.client.vitals.alive() {
            self.seen = None;
            self.path.clear();
            self.roaming = false;
            return Input::default();
        }
        // T heads for the CT spawn and the other way round (after halftime `team` follows).
        let goal = if team == Team::T { self.spawns.1 } else { self.spawns.0 };
        let round = self.client.round.round;
        if self.client.me_info.can_buy && self.bought_round != Some(round) {
            self.bought_round = Some(round);
            self.buy(team);
        }
        if self.client.round.phase == Phase::Freeze {
            return Input::default();
        }
        let me = *self.client.me();
        let eye = me.eye();
        // The nearest enemy we can actually see.
        let enemy = self
            .client
            .others()
            .into_iter()
            .filter(|o| o.alive && o.team != team && o.team != Team::Spectator)
            .map(|o| {
                let head = [o.origin[0], o.origin[1], o.origin[2] + if o.crouched { 12.0 } else { 26.0 }];
                (o.id, head, dist(eye, head))
            })
            .filter(|(_, head, _)| self.client.world.trace_line(eye, *head).fraction > 0.98)
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let mut input = Input::default();
        if let Some((id, head, range)) = enemy {
            let since = match self.seen {
                Some((who, at)) if who == id => at,
                _ => {
                    self.seen = Some((id, self.t));
                    self.t
                }
            };
            // Aim at the head with an error that shrinks the longer we track them.
            let tracked = (self.t - since) as f32;
            let err = 6.0 / (1.0 + tracked / 32.0);
            let d = [head[0] - eye[0], head[1] - eye[1], head[2] - eye[2]];
            input.yaw = d[1].atan2(d[0]).to_degrees() + (self.rand() - 0.5) * err;
            input.pitch = -(d[2].atan2((d[0] * d[0] + d[1] * d[1]).sqrt())).to_degrees() + (self.rand() - 0.5) * err;
            // Stand still (crouched when far, for accuracy) and fire short bursts once we've
            // reacted; single taps at long range.
            let tap = if range > 800.0 { 1 } else { 4 };
            input.attack = self.t - since > REACTION && (self.t % BURST) < tap;
            input.crouch = range > 800.0;
            input.reload = self.client.state.loadout.active_gun().is_some_and(|g| g.clip == 0);
            return input;
        }
        self.seen = None;
        // Walk the path; plan a new one when it's done, every few seconds, or when stuck.
        let stuck = self.t % 64 == 0 && (me.velocity[0].powi(2) + me.velocity[1].powi(2)).sqrt() < 30.0;
        // Stuck: press use (doors), and if that doesn't help, stop using this link.
        let mut give_up = false;
        if stuck {
            self.stuck_on += 1;
            if self.stuck_on >= 3 {
                if let (Some(&a), Some(&b)) = (self.path.get(self.path_at.saturating_sub(1)), self.path.get(self.path_at)) {
                    self.blocked.insert((a, b));
                }
                self.stuck_on = 0;
                give_up = true;
            }
        } else if self.t % 64 == 0 {
            self.stuck_on = 0;
        }
        if self.path_at >= self.path.len() || self.t >= self.replan || give_up {
            if dist(me.origin, goal) < 400.0 {
                self.roaming = true;
            }
            let to = if self.roaming { (self.rand() * self.nav.points.len() as f32) as u32 } else { self.nav.nearest(goal).unwrap_or(0) };
            self.path = self.nav.nearest(me.origin).map(|from| self.nav.path(from, to, &self.blocked)).unwrap_or_default();
            self.path_at = 1;
            self.replan = self.t + 512;
        }
        let Some(&next) = self.path.get(self.path_at) else {
            // Nowhere to go from here: turn and walk a little.
            return Input { yaw: (self.t / 128) as f32 * 77.0, forward: 400.0, ..Input::default() };
        };
        let p = self.nav.points[next as usize];
        let (dx, dy) = (p[0] - me.origin[0], p[1] - me.origin[1]);
        if (dx * dx + dy * dy).sqrt() < 24.0 {
            self.path_at += 1;
        }
        input.yaw = dy.atan2(dx).to_degrees();
        input.forward = 400.0;
        // Tap jump (holding it only jumps once, as in CS) to get onto something higher;
        // crouch in the air for the extra height.
        let climb = p[2] - me.origin[2] > 18.0 && (dx * dx + dy * dy).sqrt() < 64.0;
        input.jump = (climb || self.stuck_on > 0) && self.t % 16 == 0;
        input.use_key = self.stuck_on > 0 && self.t % 32 < 2;
        input.crouch = climb && !me.on_ground;
        input
    }

    /// A rifle and armor when there's money, else armor or nothing.
    fn buy(&mut self, team: Team) {
        let money = self.client.me_info.money;
        let rifle = if team == Team::T { Id::Ak47 } else { Id::M4a1 };
        let price = if team == Team::T { 2500 } else { 3100 };
        if money >= price + 1000 {
            self.client.buy(BuyItem::Weapon(rifle));
            self.client.buy(BuyItem::KevlarHelmet);
        } else if money >= 650 {
            self.client.buy(BuyItem::Kevlar);
        }
    }
}

impl Nav {
    /// The navigation for a map (takes a moment: build it once and share it).
    pub fn of_map(bsp: &goldsrc::bsp::Bsp, world: &sim::collision::World) -> Nav {
        let m = &bsp.models[0];
        Nav::build(world, m.mins, m.maxs)
    }
}

/// The first terrorist and counter-terrorist spawn of a map.
pub fn spawns_of(bsp: &goldsrc::bsp::Bsp) -> ([f32; 3], [f32; 3]) {
    let ents = bsp.entity_list();
    let first = |class: &str| -> [f32; 3] {
        ents.iter()
            .filter(|e| e.get("classname").map(String::as_str) == Some(class))
            .find_map(|e| {
                let v: Vec<f32> = e.get("origin")?.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                Some([*v.first()?, *v.get(1)?, *v.get(2)?])
            })
            .unwrap_or_default()
    };
    (first("info_player_deathmatch"), first("info_player_start"))
}
