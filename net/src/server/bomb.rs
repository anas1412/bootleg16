//! The bomb: carried by a terrorist, dropped when they die (or press G), planted in a bomb
//! site (the plant itself is weapon code in `sim`), defused by a counter-terrorist holding
//! use on it, or it explodes. In practice the bomb is yours when you play the Strays (T): handed
//! to you, and again 3 s after it went off or was defused. Only a CT defuses, as in CS 1.6
//! (`CGrenade::Use`), practice too.
use super::*;
use crate::rounds::{EndReason, Phase};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Bomb {
    None,
    Carried(u8),
    Dropped([f32; 3]),
    Planted {
        at: [f32; 3],
        explode: u32,
        /// Who is defusing and the tick they finish.
        defuse: Option<(u8, u32)>,
    },
    /// Exploded or defused this round.
    Gone,
}

/// CS 1.6 `PlayerUse`: things within `MAX_PLAYER_USE_RADIUS` (player centre to the bomb's
/// box centre, 4 units over where it lies) and in front (`VIEW_FIELD_NARROW`, the cosine from
/// the eyes; no wall check) can be used.
const DEFUSE_REACH: f32 = 64.0;
const DEFUSE_LOOK: f32 = 0.7;
const BOMB_CENTRE: f32 = 4.0;

impl Server {
    pub(super) fn defuser(&self) -> Option<u8> {
        match self.bomb {
            Bomb::Planted { defuse: Some((id, _)), .. } => Some(id),
            _ => None,
        }
    }

    /// Round start: the bomb goes to one terrorist (taking turns by round number).
    pub(super) fn give_bomb(&mut self) {
        let mut ts: Vec<&mut Remote> = self.clients.values_mut().filter(|c| c.team == Team::T && c.vitals.alive()).collect();
        ts.sort_by_key(|c| c.id);
        let n = ts.len();
        self.bomb = Bomb::None;
        if n == 0 || self.world.zones.bomb_sites.is_empty() {
            return;
        }
        let c = &mut ts[self.round.round as usize % n];
        c.state.loadout.give(Id::C4, 0.0);
        self.bomb = Bomb::Carried(c.id);
    }

    /// Practice: no bomb in play (or 3 s after the last one ended): hand it to the player, if
    /// they play the Strays.
    fn practice_bomb_step(&mut self) {
        if !self.rules.practice || self.world.zones.bomb_sites.is_empty() {
            return;
        }
        match self.bomb {
            Bomb::Gone if self.practice_bomb == 0 => self.practice_bomb = self.tick + self.rate.secs(3.0),
            Bomb::None | Bomb::Gone if self.tick >= self.practice_bomb => {
                let Some(c) = self.clients.values_mut().filter(|c| !c.bot && c.vitals.alive() && c.team == Team::T).min_by_key(|c| c.id) else { return };
                c.state.loadout.give(Id::C4, 0.0);
                self.bomb = Bomb::Carried(c.id);
                self.practice_bomb = 0;
            }
            _ => {}
        }
    }

    /// Where the bomb lands from someone's hands (on the floor under them).
    fn floor_under(&self, p: [f32; 3]) -> [f32; 3] {
        self.world.trace_line(p, [p[0], p[1], p[2] - 4096.0]).end
    }

    /// Carrier dropped it (G) or died: it falls where they stand.
    pub(super) fn drop_bomb(&mut self, id: u8) {
        if self.bomb != Bomb::Carried(id) {
            return;
        }
        let Some(c) = self.clients.values_mut().find(|c| c.id == id) else { return };
        let l = &mut c.state.loadout;
        l.c4 = false;
        l.planting = None;
        if l.active == data::Slot::Bomb {
            let best = [data::Slot::Primary, data::Slot::Secondary, data::Slot::Knife].into_iter().find(|s| match s {
                data::Slot::Primary => l.primary.is_some(),
                data::Slot::Secondary => l.secondary.is_some(),
                _ => true,
            });
            l.switch_to(best.unwrap_or(data::Slot::Knife), c.last_run as f64 * self.rate.tick() as f64);
        }
        let at = c.state.movement.origin;
        self.bomb = Bomb::Dropped(self.floor_under(at));
    }

    pub(super) fn bomb_planted(&mut self, by: u8, at: [f32; 3]) {
        let at = self.floor_under(at);
        let explode = self.tick + (self.round.cfg.c4_timer * self.rate.hz() as f32) as u32;
        self.bomb = Bomb::Planted { at, explode, defuse: None };
        self.planter = Some(by);
        self.events.push(GameEvent::BombPlanted { by, at });
    }

    /// Pick-ups, defusing and the explosion; once a tick.
    pub(super) fn bomb_step(&mut self) {
        let tick = self.tick;
        self.practice_bomb_step();
        match self.bomb {
            Bomb::Carried(id) => {
                if !self.clients.values().any(|c| c.id == id && c.vitals.alive() && c.state.loadout.c4) {
                    // Carrier gone or dead (death drops it in `settle_deaths`): don't lose it.
                    if let Some(c) = self.clients.values().find(|c| c.id == id) {
                        let at = c.state.movement.origin;
                        self.bomb = Bomb::Dropped(self.floor_under(at));
                    } else {
                        self.bomb = Bomb::None;
                    }
                }
            }
            Bomb::Dropped(at) => {
                let taker = self.clients.values_mut().find(|c| {
                    let o = c.state.movement.origin;
                    let h = c.state.movement.hull();
                    c.team == Team::T && c.vitals.alive() && (0..3).all(|k| at[k] >= o[k] + h.mins()[k] - 16.0 && at[k] <= o[k] + h.maxs()[k] + 16.0)
                });
                if let Some(c) = taker {
                    c.state.loadout.give(Id::C4, 0.0);
                    self.bomb = Bomb::Carried(c.id);
                }
            }
            Bomb::Planted { at, explode, defuse } => {
                if tick >= explode {
                    self.explode(at);
                    return;
                }
                // The defuser must keep holding use, alive, close and looking at it.
                let still = |c: &Remote| {
                    let (o, eye) = (c.state.movement.origin, c.state.movement.eye());
                    let centre = [at[0], at[1], at[2] + BOMB_CENTRE];
                    let near = (0..3).map(|i| (centre[i] - o[i]).powi(2)).sum::<f32>().sqrt() <= DEFUSE_REACH;
                    let d = [centre[0] - eye[0], centre[1] - eye[1], centre[2] - eye[2]];
                    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                    let (f, _, _) = bullets::view_vectors(c.view.1, c.view.0);
                    let look = if len > 1.0 { (d[0] * f[0] + d[1] * f[1] + d[2] * f[2]) / len } else { 1.0 };
                    c.team == Team::Ct && c.vitals.alive() && c.use_held && near && look > DEFUSE_LOOK
                };
                match defuse {
                    Some((id, done)) => {
                        if !self.clients.values().any(|c| c.id == id && still(c)) {
                            self.bomb = Bomb::Planted { at, explode, defuse: None };
                        } else if tick >= done {
                            self.bomb = Bomb::Gone;
                            self.events.push(GameEvent::BombDefused { by: id });
                            if self.round.phase == Phase::Live {
                                self.end_round(Team::Ct, EndReason::BombDefused);
                            }
                        }
                    }
                    None => {
                        if let Some(c) = self.clients.values().filter(|c| still(c)).min_by_key(|c| c.id) {
                            let secs = if c.defuse_kit { self.round.cfg.defuse_kit_time } else { self.round.cfg.defuse_time };
                            let done = tick + (secs * self.rate.hz() as f32) as u32;
                            self.events.push(GameEvent::Defusing { by: c.id, kit: c.defuse_kit });
                            self.bomb = Bomb::Planted { at, explode, defuse: Some((c.id, done)) };
                        }
                    }
                }
            }
            Bomb::None | Bomb::Gone => {}
        }
    }

    /// Boom: hurts everyone out to 3.5 × the map's bomb radius, through walls (CS 1.6 C4).
    fn explode(&mut self, at: [f32; 3]) {
        self.bomb = Bomb::Gone;
        self.events.push(GameEvent::BombExploded { at });
        let radius = self.world.zones.bomb_radius;
        self.bomb_triggers(at);
        self.blast_pieces(at, radius * 3.5, radius);
        for c in self.clients.values_mut().filter(|c| c.vitals.alive()) {
            let o = c.state.movement.origin;
            let d = ((o[0] - at[0]).powi(2) + (o[1] - at[1]).powi(2) + (o[2] - at[2]).powi(2)).sqrt();
            let dmg = radius * (1.0 - d / (radius * 3.5));
            if dmg <= 0.0 {
                continue;
            }
            let (health, armor) = combat::explosion_damage(&c.vitals, dmg);
            let killed = c.vitals.hurt(health, armor);
            self.events.push(GameEvent::Damage { attacker: WORLD, victim: c.id, amount: health, group: HitGroup::Generic, sound: HitSound::Explosion, at: None });
            if killed {
                self.events.push(GameEvent::Kill { killer: WORLD, victim: c.id, weapon: Some(Id::C4), headshot: false });
            }
        }
        if self.round.phase == Phase::Live {
            self.end_round(Team::T, EndReason::BombExploded);
        }
    }

    /// The bomb as one client may see it (only terrorists are told who carries it).
    pub(super) fn bomb_net(&self, viewer: Team) -> BombNet {
        let now = self.tick;
        match self.bomb {
            Bomb::Carried(id) if viewer == Team::T => BombNet::Carried(id),
            Bomb::Dropped(at) => BombNet::Dropped(at),
            Bomb::Planted { at, explode, defuse } => BombNet::Planted {
                at,
                left: explode.saturating_sub(now) as f32 * self.rate.tick(),
                defuse: defuse.map(|(id, done)| (id, done.saturating_sub(now) as f32 * self.rate.tick())),
            },
            _ => BombNet::None,
        }
    }
}
