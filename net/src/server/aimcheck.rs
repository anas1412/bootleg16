//! Server-side aim checks: inhuman aim is flagged for a person to review (with the match
//! recording and the moment), never punished automatically: a false positive must never ban
//! an honest player. Two signals, both only over many engagements:
//!
//! - Reaction: from an enemy coming into plain sight to the first shot aimed at them (within
//!   `AIMED_DEGREES`; misses count: spread mustn't blur the measure). People take
//!   ~200 ms and more (seeing, deciding, moving the mouse); a fight opened with a hit under
//!   `FAST_MS` is near impossible. Honest players manage it now and then (prefiring a corner
//!   someone walks into), so only a large share of fights (`FAST_SHARE`) is flagged. In tests
//!   an aimbot opens about a third of its fights that fast; honest bots never do.
//!   (Headshot rates don't tell: CS 1.6's first-shot spread spoils even perfect aim.)
//! - Snaps: a big flick (over `FLICK_DEGREES` in a few ticks) that lands a hit, again and
//!   again. People flick too, so only a large share of all hits counts.
use super::*;

/// A shot this close to an enemy (degrees off their head) is aimed at them.
const AIMED_DEGREES: f32 = 3.0;
/// A fight opened faster than this (ms) isn't a human reaction.
const FAST_MS: u32 = 80;
/// Flagged: this share of fights that fast, over at least `MIN_ENGAGEMENTS`.
const FAST_SHARE: f32 = 0.25;
const MIN_ENGAGEMENTS: usize = 20;
/// A flick: the view turning this much within `FLICK_TICKS` before a hit.
const FLICK_DEGREES: f32 = 25.0;
const FLICK_TICKS: usize = 8;
/// Out of plain sight this long before coming back counts as a new engagement (peeking back
/// and forth is one fight, not many instant "reactions").
const SAME_FIGHT_SECS: f32 = 0.5;
/// Share of hits that were flicks, over at least `MIN_HITS`, that is flagged.
const FLICK_SHARE: f32 = 0.35;
const MIN_HITS: u32 = 20;

/// One player's aim, this match.
#[derive(Default)]
pub(super) struct Aim {
    /// Recent view angles (yaw, pitch), newest last.
    history: VecDeque<(f32, f32)>,
    /// Reaction times (ticks) of engagements so far.
    reactions: Vec<u32>,
    hits: u32,
    headshots: u32,
    flick_hits: u32,
    /// Already flagged for these (once a match each).
    flagged_reaction: bool,
    flagged_flicks: bool,
}

impl Aim {
    pub(super) fn look(&mut self, yaw: f32, pitch: f32) {
        self.history.push_back((yaw, pitch));
        if self.history.len() > 16 {
            self.history.pop_front();
        }
    }

    /// How far the view turned over the last few ticks (degrees).
    fn turned(&self) -> f32 {
        let (Some(now), Some(then)) = (self.history.back(), self.history.iter().rev().nth(FLICK_TICKS)) else { return 0.0 };
        let dyaw = ((now.0 - then.0 + 540.0) % 360.0) - 180.0;
        (dyaw * dyaw + (now.1 - then.1).powi(2)).sqrt()
    }
}

/// Engagements in progress: (viewer, enemy) -> when the enemy came into sight, whether the
/// viewer has hit them yet, and when they were last in plain sight.
#[derive(Default)]
pub(super) struct Sightings {
    since: HashMap<(u8, u8), (u32, bool, u32)>,
}

/// One player's aim numbers (for reviewers and tests).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AimSummary {
    pub hits: u32,
    pub headshots: u32,
    pub flick_hits: u32,
    pub fights: usize,
    /// Fights opened with a hit faster than a human reaction.
    pub fast: usize,
}

impl Server {
    pub fn aim_summary(&self, id: u8) -> Option<AimSummary> {
        let a = &self.clients.values().find(|c| c.id == id)?.aim;
        let fast = a.reactions.iter().filter(|r| **r * 1000 / self.rate.hz() < FAST_MS).count();
        Some(AimSummary { hits: a.hits, headshots: a.headshots, flick_hits: a.flick_hits, fights: a.reactions.len(), fast })
    }

    /// After visibility is worked out (every snapshot): note enemies newly in plain sight. Not
    /// the anti-wallhack's generous "visible" (it sends players a little before they show, so
    /// they don't pop in): a clear line from eyes to eyes, no smoke in between.
    pub(super) fn note_sightings(&mut self, sights: &HashMap<u8, std::collections::HashSet<u8>>) {
        let tick = self.tick;
        let eye = |id: u8| self.clients.values().find(|c| c.id == id && c.vitals.alive()).map(|c| (c.team, c.state.movement.eye()));
        let mut plain = Vec::new();
        for (viewer, seen) in sights {
            let Some((vteam, veye)) = eye(*viewer) else { continue };
            for t in seen {
                let Some((tteam, teye)) = eye(*t) else { continue };
                if tteam != vteam && !self.world.sight_blocked(veye, teye) && !self.smoked(veye, teye) {
                    plain.push((*viewer, *t));
                }
            }
        }
        let same_fight = self.rate.secs(SAME_FIGHT_SECS);
        let s = &mut self.sightings.since;
        s.retain(|_, (_, _, last)| tick - *last < same_fight);
        for k in plain {
            s.entry(k).or_insert((tick, false, tick)).2 = tick;
        }
    }

    /// `shooter` fired along (yaw, pitch) from `eye`: the first shot aimed at an enemy in a
    /// fight is that fight's reaction.
    pub(super) fn check_shot(&mut self, shooter: u8, eye: [f32; 3], angles: [f32; 2]) {
        let tick = self.tick;
        let (f, _, _) = sim::weapons::bullets::view_vectors(angles[0], angles[1]);
        let mut reactions = Vec::new();
        for ((viewer, target), (seen, answered, _)) in self.sightings.since.iter_mut() {
            if *viewer != shooter || *answered {
                continue;
            }
            let Some(t) = self.clients.values().find(|c| c.id == *target) else { continue };
            let head = t.state.movement.eye();
            let d = [head[0] - eye[0], head[1] - eye[1], head[2] - eye[2]];
            let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1.0);
            let off = ((d[0] * f[0] + d[1] * f[1] + d[2] * f[2]) / len).clamp(-1.0, 1.0).acos().to_degrees();
            if off <= AIMED_DEGREES {
                *answered = true;
                reactions.push(tick - *seen);
            }
        }
        if let Some(c) = self.clients.values_mut().find(|c| c.id == shooter) {
            c.aim.reactions.extend(reactions);
        }
        self.judge(shooter);
    }

    /// A bullet from `shooter` hit `victim`.
    pub(super) fn check_hit(&mut self, shooter: u8, head: bool) {
        let Some(c) = self.clients.values_mut().find(|c| c.id == shooter) else { return };
        let aim = &mut c.aim;
        aim.hits += 1;
        aim.headshots += head as u32;
        if aim.turned() > FLICK_DEGREES {
            aim.flick_hits += 1;
        }
        self.judge(shooter);
    }

    /// Flag what adds up to inhuman (once a match each).
    fn judge(&mut self, shooter: u8) {
        let hz = self.rate.hz();
        let Some(c) = self.clients.values_mut().find(|c| c.id == shooter) else { return };
        let aim = &mut c.aim;
        let mut flags: Vec<String> = Vec::new();
        if !aim.flagged_reaction && aim.reactions.len() >= MIN_ENGAGEMENTS {
            let fast = aim.reactions.iter().filter(|r| **r * 1000 / hz < FAST_MS).count();
            let fast_share = fast as f32 / aim.reactions.len() as f32;
            let head_share = aim.headshots as f32 / aim.hits as f32;
            if fast_share >= FAST_SHARE {
                aim.flagged_reaction = true;
                flags.push(format!("reactions: {fast} of {} fights opened with an aimed shot under {FAST_MS} ms (people: 200+), {:.0}% of hits on the head", aim.reactions.len(), head_share * 100.0));
            }
        }
        if !aim.flagged_flicks && aim.hits >= MIN_HITS && aim.flick_hits as f32 / aim.hits as f32 >= FLICK_SHARE {
            aim.flagged_flicks = true;
            flags.push(format!("snaps: {} of {} hits came right after a flick over {FLICK_DEGREES} degrees", aim.flick_hits, aim.hits));
        }
        let (ticket, name) = (c.ticket, c.name.clone());
        for reason in flags {
            self.flag(ticket, &name, reason);
        }
    }

    /// Tell the lobbies (they file it with the reports, for a person to look at).
    fn flag(&mut self, ticket: u64, name: &str, reason: String) {
        println!("aim check flagged {name}: {reason}");
        let (demo, tick) = self.recording_now();
        let Some(key) = &self.server_key else { return };
        let sealed = key.seal(&encode(&crate::lobby::LobbyMsg::Flag { ticket, reason, map: self.map.clone(), demo, tick }));
        for lobby in self.lobbies.clone() {
            self.socket.send(lobby, sealed.clone());
        }
    }
}
