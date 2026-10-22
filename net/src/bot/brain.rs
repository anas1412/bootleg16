//! What a bot does each tick, after the CS 1.6 / CZ bot's states: buy, then the objective
//! (terrorists take the bomb to a site and plant it, pick it up when dropped, guard it once
//! planted; counter-terrorists guard a site, defuse a planted bomb, or run when there's no
//! time), investigating gunfire they hear, and attacking enemies once they've noticed them
//! (reaction time), with aim that improves while they track. Test orders (`Mode`) replace
//! parts of this.
use super::control::{Mode, Place};
use super::nav::dist;
use super::Bot;
use crate::protocol::{BombNet, BuyItem, GameEvent, Team};
use crate::rounds::Phase;
use sim::pmove::Input;
use sim::weapons::data::Slot;
use sim::weapons::Id;
use std::collections::HashSet;

pub(super) struct State {
    t: u32,
    rng: u64,
    path: Vec<u32>,
    path_to: Option<u32>,
    path_at: usize,
    replan: u32,
    blocked: HashSet<(u32, u32)>,
    stuck_on: u32,
    /// Who we're looking at and since when (the reaction delay runs from then).
    seen: Option<(u8, u32)>,
    round: Option<u32>,
    bought: bool,
    /// This round's site (where the bomb goes / which one to guard).
    site: usize,
    /// Pushing towards the enemy spawn this round instead of holding (aggressive bots).
    push: bool,
    /// Gunfire heard: where, and until when it's worth checking.
    noise: Option<([f32; 3], u32)>,
    /// Strafing side while fighting, and when to switch.
    strafe: (f32, u32),
    /// Where we looked last tick (an idle bot keeps looking that way).
    yaw: f32,
    /// Deathmatch: the spot we're heading for while hunting.
    hunt: Option<[f32; 3]>,
}

impl State {
    pub(super) fn new(seed: u64) -> State {
        State {
            t: 0,
            rng: seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1,
            path: Vec::new(),
            path_to: None,
            path_at: 0,
            replan: 0,
            blocked: HashSet::new(),
            stuck_on: 0,
            seen: None,
            round: None,
            bought: false,
            site: 0,
            push: false,
            noise: None,
            strafe: (1.0, 0),
            yaw: 0.0,
            hunt: None,
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Angles (yaw, pitch) to look from `eye` at `p`.
fn look(eye: [f32; 3], p: [f32; 3]) -> (f32, f32) {
    let d = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
    (d[1].atan2(d[0]).to_degrees(), -(d[2].atan2((d[0] * d[0] + d[1] * d[1]).sqrt())).to_degrees())
}

fn flat(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

impl Bot {
    /// Seconds as ticks at the server's rate (never 0: it is a divisor).
    fn secs(&self, s: f32) -> u32 {
        self.client.rate().secs(s).max(1)
    }

    /// Remember gunfire from enemies (the server only sends what we could hear).
    pub(super) fn hear(&mut self, events: &[GameEvent]) {
        let team = self.client.team();
        for e in events {
            if let GameEvent::Shot { shooter, from, .. } = e {
                let enemy = self.client.others().iter().any(|o| o.id == *shooter && o.team != team);
                if enemy {
                    self.brain.noise = Some((*from, self.brain.t + self.secs(5.0)));
                }
            }
        }
    }

    pub(super) fn think(&mut self) -> Input {
        let input = self.decide();
        self.brain.yaw = input.yaw;
        input
    }

    fn decide(&mut self) -> Input {
        self.brain.t += 1;
        let t = self.brain.t;
        let team = self.client.team();
        if self.client.id().is_none() || team == Team::Spectator || !self.client.vitals.alive() || self.mode == Mode::Afk {
            self.brain.seen = None;
            self.brain.path.clear();
            return Input::default();
        }
        let round = self.client.round.round;
        if self.brain.round != Some(round) {
            // A new round: pick this round's site and plan.
            self.brain.round = Some(round);
            self.brain.bought = false;
            self.brain.noise = None;
            self.brain.site = self.site.unwrap_or((round as usize + team as usize) % self.map.sites.len().max(1));
            let aggression = self.profile.aggression;
            self.brain.push = self.brain.rand() < aggression * 0.6;
            self.brain.path.clear();
        }
        if self.client.me_info.can_buy && !self.brain.bought {
            self.brain.bought = true;
            self.buy(team);
        }
        let me = *self.client.me();
        let eye = me.eye();
        let mut input = Input { yaw: self.brain.yaw, ..Input::default() };
        if self.mode == Mode::Knife && self.client.state.loadout.active != Slot::Knife {
            input.slot = Slot::Knife as u8;
        }
        if self.client.round.phase == Phase::Freeze {
            return input;
        }

        // Who we'd shoot: enemies (teammates for a griefer) we can actually see.
        let target = self
            .client
            .others()
            .into_iter()
            .filter(|o| o.alive && o.team != Team::Spectator && ((o.team != team) != (self.mode == Mode::Grief)))
            // Aim at the head: about eye height (17 standing, 12 crouched), a little above.
            .map(|o| (o.id, [o.origin[0], o.origin[1], o.origin[2] + if o.crouched { 14.0 } else { 20.0 }]))
            .filter(|(_, head)| self.client.world.trace_line(eye, *head).fraction > 0.98)
            .min_by(|a, b| dist(eye, a.1).total_cmp(&dist(eye, b.1)));
        let busy = self.planting() || self.defusing();
        match target {
            Some((id, head)) if self.mode != Mode::Peaceful && !busy => return self.attack(input, id, head),
            _ => self.brain.seen = None,
        }

        // Where to be, and what to do there.
        let (yaw_hold, _) = look(eye, if team == Team::T { self.map.spawns.1 } else { self.map.spawns.0 });
        match self.mode {
            Mode::Hold => return Input { yaw: yaw_hold + ((t / self.secs(2.0)) as f32 * 40.0) % 90.0 - 45.0, ..input },
            Mode::Goto(p) | Mode::Rush(p) => {
                let at = self.place(p);
                return if flat(me.origin, at) < 48.0 { input } else { self.walk(input, at) };
            }
            _ => {}
        }
        // No objective (deathmatch, practice): hunt. Gunfire first, else roam the map from one
        // random spot to the next.
        if self.client.practice {
            if let Some(at) = self.noise() {
                return self.walk(input, at);
            }
            let me = self.client.me().origin;
            let arrived = self.brain.hunt.is_none_or(|h| flat(me, h) < 100.0) || self.brain.t % self.secs(20.0) == 0;
            if arrived {
                let n = self.map.nav.points.len().max(1);
                self.brain.hunt = Some(self.map.nav.points[(self.brain.rand() * n as f32) as usize % n]);
            }
            return self.walk(input, self.brain.hunt.unwrap());
        }
        let site = self.map.sites.get(self.site.unwrap_or(self.brain.site)).copied().unwrap_or(self.map.spawns.1);
        let bomb = self.client.bomb;
        if team == Team::T {
            if self.client.state.loadout.c4 {
                // Carrying it: to the site, then plant (hold attack with the C4 out, still).
                if self.map.zones.in_bomb_site(me.origin, me.hull()) && flat(me.origin, site) < 400.0 {
                    if self.client.state.loadout.active != Slot::Bomb {
                        input.slot = Slot::Bomb as u8;
                    }
                    input.attack = true;
                    return input;
                }
                return self.walk(input, site);
            }
            match bomb {
                BombNet::Dropped(at) => return self.walk(input, at),
                BombNet::Planted { at, .. } => return self.guard(input, at, yaw_hold),
                _ => {}
            }
            if let Some(at) = self.noise() {
                return self.walk(input, at);
            }
            // Escort the bomb to this round's site (aggressive ones push the enemy spawn).
            return self.walk(input, if self.brain.push { self.map.spawns.1 } else { site });
        }
        if let BombNet::Planted { at, left, defuse } = bomb {
            let mine = defuse.is_some_and(|(by, _)| Some(by) == self.client.id());
            if defuse.is_some() && !mine {
                return self.guard(input, at, yaw_hold);
            }
            // Can we make it? Running there plus the defuse, with a little spare.
            let need = flat(me.origin, at) / 250.0 + if self.client.me_info.defuse_kit { 5.0 } else { 10.0 } + 0.5;
            if need > left && !mine {
                return self.walk(input, self.map.spawns.1); // escape from the bomb
            }
            if flat(me.origin, at) < 40.0 {
                let (yaw, pitch) = look(eye, at);
                return Input { yaw, pitch, use_key: true, crouch: true, ..input };
            }
            return self.walk(input, at);
        }
        if let Some(at) = self.noise() {
            return self.walk(input, at);
        }
        if self.brain.push {
            return self.walk(input, self.map.spawns.0);
        }
        self.guard(input, site, yaw_hold)
    }

    fn planting(&self) -> bool {
        self.client.state.loadout.c4 && self.client.state.loadout.active == Slot::Bomb && self.map.zones.in_bomb_site(self.client.me().origin, self.client.me().hull())
    }

    fn defusing(&self) -> bool {
        matches!(self.client.bomb, BombNet::Planted { defuse: Some((by, _)), .. } if Some(by) == self.client.id())
    }

    /// Gunfire worth checking (aggressive bots go more often).
    fn noise(&mut self) -> Option<[f32; 3]> {
        let (at, until) = self.brain.noise?;
        let me = self.client.me().origin;
        if self.brain.t > until || flat(me, at) < 100.0 || flat(me, at) > 1500.0 {
            self.brain.noise = None;
            return None;
        }
        (self.profile.aggression > 0.4).then_some(at)
    }

    fn place(&self, p: Place) -> [f32; 3] {
        match p {
            Place::Site(i) => self.map.sites.get(i).copied().unwrap_or(self.map.spawns.1),
            Place::TSpawn => self.map.spawns.0,
            Place::CtSpawn => self.map.spawns.1,
            Place::At(at) => at,
        }
    }

    /// Stand near a spot looking out for trouble (to it first).
    fn guard(&mut self, input: Input, at: [f32; 3], out: f32) -> Input {
        let me = self.client.me().origin;
        if flat(me, at) > 250.0 {
            return self.walk(input, at);
        }
        let sweep = ((self.brain.t / self.secs(1.5)) % 3) as f32 * 35.0 - 35.0;
        Input { yaw: out + sweep, ..input }
    }

    /// Fight: notice after the reaction time, aim (better the longer we track), shoot in
    /// bursts (taps far away), strafe close up if skilled, crouch far away.
    fn attack(&mut self, mut input: Input, id: u8, head: [f32; 3]) -> Input {
        let t = self.brain.t;
        let since = match self.brain.seen {
            Some((who, at)) if who == id => at,
            _ => {
                self.brain.seen = Some((id, t));
                t
            }
        };
        let me = *self.client.me();
        let eye = me.eye();
        let range = dist(eye, head);
        let aimbot = self.mode == Mode::Aimbot;
        let tracked = (t - since) as f32 / self.client.rate().hz() as f32;
        let err = if aimbot { 0.0 } else { self.profile.aim_error / (1.0 + tracked * 4.0) };
        let (yaw, pitch) = look(eye, head);
        input.yaw = yaw + (self.brain.rand() - 0.5) * err;
        input.pitch = pitch + (self.brain.rand() - 0.5) * err;
        let reacted = aimbot || tracked > self.profile.reaction;
        if self.mode == Mode::Knife {
            input.forward = 400.0;
            input.attack = range < 64.0;
            return input;
        }
        let tap = if range > 800.0 { 1 } else { 4 };
        // (An aimbot fires the moment it's on target and taps, keeping its shots on the head.)
        input.attack = self.mode == Mode::Spray || (reacted && if aimbot { t % self.secs(0.09375) == 0 } else { t % self.secs(0.1875) < tap });
        input.reload = self.client.state.loadout.active_gun().is_some_and(|g| g.clip == 0);
        input.crouch = range > 800.0;
        if let Mode::Rush(p) = self.mode {
            // Run and gun towards the goal.
            let at = self.place(p);
            let walk = self.walk(Input::default(), at);
            let rel = (walk.yaw - input.yaw).to_radians();
            input.forward = 400.0 * rel.cos();
            input.side = -400.0 * rel.sin();
            input.crouch = false;
        } else if self.profile.skill >= 0.75 && range < 900.0 {
            // Skilled: side-step between bursts.
            if t >= self.brain.strafe.1 {
                self.brain.strafe = (-self.brain.strafe.0, t + 40 + (self.brain.rand() * 60.0) as u32);
            }
            if !input.attack {
                input.side = 250.0 * self.brain.strafe.0;
            }
        }
        input
    }

    /// Walk the nav path to `goal` (re-planned when it changes, every few seconds, or when
    /// stuck: then press use for doors, and give up on the link after a few tries).
    fn walk(&mut self, mut input: Input, goal: [f32; 3]) -> Input {
        let me = *self.client.me();
        let nav = &self.map.nav;
        let t = self.brain.t;
        let to = nav.nearest(goal);
        let stuck = t % self.secs(0.5) == 0 && (me.velocity[0].powi(2) + me.velocity[1].powi(2)).sqrt() < 30.0;
        let mut give_up = false;
        if stuck {
            self.brain.stuck_on += 1;
            if self.brain.stuck_on >= 3 {
                if let (Some(&a), Some(&b)) = (self.brain.path.get(self.brain.path_at.saturating_sub(1)), self.brain.path.get(self.brain.path_at)) {
                    self.brain.blocked.insert((a, b));
                }
                self.brain.stuck_on = 0;
                give_up = true;
            }
        } else if t % self.secs(0.5) == 0 {
            self.brain.stuck_on = 0;
        }
        if to != self.brain.path_to || self.brain.path_at >= self.brain.path.len() || t >= self.brain.replan || give_up {
            self.brain.path_to = to;
            self.brain.path = match (nav.nearest(me.origin), to) {
                (Some(from), Some(to)) => nav.path(from, to, &self.brain.blocked),
                _ => Vec::new(),
            };
            self.brain.path_at = 1;
            self.brain.replan = t + self.secs(4.0);
        }
        let Some(&next) = self.brain.path.get(self.brain.path_at) else {
            // Nowhere to go from here: towards the goal, and turn now and then.
            let (yaw, _) = look(me.origin, goal);
            return Input { yaw: yaw + ((t / self.secs(1.0)) % 4) as f32 * 90.0, forward: 400.0, ..input };
        };
        let p = nav.points[next as usize];
        if flat(me.origin, p) < 24.0 {
            self.brain.path_at += 1;
        }
        let (yaw, _) = look(me.origin, p);
        input.yaw = yaw;
        input.pitch = 0.0;
        input.forward = 400.0;
        // Tap jump (holding it only jumps once, as in CS) onto something higher; crouch in
        // the air for the extra height. Bunny hoppers jump all the time.
        let climb = p[2] - me.origin[2] > 18.0 && flat(me.origin, p) < 64.0;
        let bhop = self.mode == Mode::Bhop && me.on_ground;
        input.jump = ((climb || self.brain.stuck_on > 0) && t % self.secs(0.125) == 0) || (bhop && t % 2 == 0);
        input.use_key = self.brain.stuck_on > 0 && t % self.secs(0.25) < 2;
        input.crouch = climb && !me.on_ground;
        input
    }

    /// A rifle and armor when there's money (a pistol round: armor), a kit for CTs who can
    /// afford it; knifers buy nothing.
    fn buy(&mut self, team: Team) {
        if self.mode == Mode::Knife {
            return;
        }
        let money = self.client.me_info.money;
        let (rifle, price) = if team == Team::T { (Id::Ak47, 2500) } else { (Id::M4a1, 3100) };
        if money >= price + 1000 {
            self.client.buy(BuyItem::Weapon(rifle));
            self.client.buy(BuyItem::KevlarHelmet);
            if team == Team::Ct && money >= price + 1000 + 400 {
                self.client.buy(BuyItem::DefuseKit);
            }
        } else if money >= 650 {
            self.client.buy(BuyItem::Kevlar);
        }
    }
}
