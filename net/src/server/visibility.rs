//! Anti-wallhack: enemies you can't possibly see aren't sent to you at all, so a cheating
//! client has nothing to draw. Sounds are separate: footsteps and gunshots still reach you
//! within hearing range (where they're audible anyway), so you can hear people behind walls.
use super::*;
use std::collections::HashSet;

/// Keep sending someone this long after they went out of sight (no flicker at corners).
const GRACE_SECS: f32 = 0.25;
/// The target's box is grown by this much sideways and their (and your) position is pushed
/// ahead along the velocity by `LOOKAHEAD` seconds, so someone stepping round a corner is
/// already there when they come into view, even with some ping.
const MARGIN: f32 = 24.0;
const LOOKAHEAD: f32 = 0.15;
/// GoldSrc sound falloff: silent at 1000 / attenuation units (footsteps: ATTN_NORM 0.8; shots:
/// `fire_attenuation`).
const HEAR_NORMAL: f32 = 1000.0 / 0.8;
/// A smoke cloud hides what's behind it once it has bloomed (1 s) until it thins out (last 2 s):
/// a sphere this big around a point this high over where it popped.
const SMOKE_RADIUS: f32 = 140.0;
const SMOKE_LIFT: f32 = 40.0;
/// Close enough to see through the haze anyway (both in the same cloud).
const SMOKE_SEE_NEAR: f32 = 64.0;

fn ahead(p: [f32; 3], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| p[i] + v[i] * LOOKAHEAD)
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Does the segment a-b pass through the sphere (c, r)?
fn through_sphere(a: [f32; 3], b: [f32; 3], c: [f32; 3], r: f32) -> bool {
    let d: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
    let f: [f32; 3] = std::array::from_fn(|i| a[i] - c[i]);
    let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    let t = (-(f[0] * d[0] + f[1] * d[1] + f[2] * d[2]) / dd.max(1e-6)).clamp(0.0, 1.0);
    let p: [f32; 3] = std::array::from_fn(|i| f[i] + d[i] * t);
    p[0] * p[0] + p[1] * p[1] + p[2] * p[2] < r * r
}

impl Server {
    /// Is the line a-b hidden by a smoke cloud?
    pub(super) fn smoked(&self, a: [f32; 3], b: [f32; 3]) -> bool {
        if dist(a, b) < SMOKE_SEE_NEAR {
            return false;
        }
        let now = self.tick as f64 * self.rate.tick() as f64;
        self.smokes.iter().any(|(at, until)| {
            let age = sim::grenades::SMOKE_SECONDS - (until - now);
            age > 1.0 && until - now > 2.0 && through_sphere(a, b, [at[0], at[1], at[2] + SMOKE_LIFT], SMOKE_RADIUS)
        })
    }

    /// Could `viewer` see any part of `target` (now or a moment from now)?
    fn in_sight(&self, viewer: &Remote, target: &Remote) -> bool {
        let (vm, tm) = (&viewer.state.movement, &target.state.movement);
        let eyes = [vm.eye(), ahead(vm.eye(), vm.velocity)];
        let h = tm.hull();
        let (lo, hi) = (h.mins(), h.maxs());
        for o in [tm.origin, ahead(tm.origin, tm.velocity)] {
            let mut points = vec![o, [o[0], o[1], o[2] + hi[2] - 4.0]];
            for x in [lo[0] - MARGIN, hi[0] + MARGIN] {
                for y in [lo[1] - MARGIN, hi[1] + MARGIN] {
                    for z in [lo[2] + 4.0, hi[2]] {
                        points.push([o[0] + x, o[1] + y, o[2] + z]);
                    }
                }
            }
            if eyes.iter().any(|e| points.iter().any(|p| !self.world.sight_blocked(*e, *p) && !self.smoked(*e, *p))) {
                return true;
            }
        }
        false
    }

    /// The living players whose eyes and ears `viewer` gets: themselves when alive; when dead,
    /// their living teammates (CS competitive: the dead only watch their own team, so they
    /// can't call out what the enemy is doing); a whole team dead: nobody's (their screen goes
    /// black). None: sees everyone (spectators, outside official matches).
    fn eyes_of(&self, viewer: u8) -> Option<Vec<&Remote>> {
        let v = self.clients.values().find(|c| c.id == viewer)?;
        if v.team == Team::Spectator {
            return None;
        }
        if v.vitals.alive() {
            return Some(vec![v]);
        }
        Some(self.clients.values().filter(|c| c.team == v.team && c.vitals.alive()).collect())
    }

    /// Who `viewer` gets told about: themselves, teammates, and enemies in (or just out of)
    /// sight of their eyes (see `eyes_of`).
    pub(super) fn visible_to(&mut self, viewer: u8) -> HashSet<u8> {
        let Some(v) = self.clients.values().find(|c| c.id == viewer) else { return HashSet::new() };
        let eyes = self.eyes_of(viewer);
        let tick = self.tick;
        let mut out = HashSet::new();
        let mut seen = Vec::new();
        for t in self.clients.values() {
            let in_sight = match &eyes {
                None => true,
                Some(eyes) => eyes.iter().any(|e| self.in_sight(e, t)),
            };
            let visible = t.id == viewer || t.team == v.team || !t.vitals.alive() || in_sight;
            if visible {
                seen.push(t.id);
                out.insert(t.id);
            } else if self.last_seen.get(&(viewer, t.id)).is_some_and(|s| tick - s < self.rate.secs(GRACE_SECS)) {
                out.insert(t.id);
            }
        }
        for id in seen {
            self.last_seen.insert((viewer, id), tick);
        }
        out
    }

    /// The events `viewer` may get: sounds only within hearing range of someone hidden.
    pub(super) fn events_for(&self, viewer: u8, visible: &HashSet<u8>, events: &[GameEvent]) -> Vec<GameEvent> {
        if !self.clients.values().any(|c| c.id == viewer) {
            return Vec::new();
        }
        // Heard with the same ears as `eyes_of` (the dead: their living teammates').
        let ears: Option<Vec<[f32; 3]>> = self.eyes_of(viewer).map(|e| e.iter().map(|c| c.state.movement.eye()).collect());
        let hears = |at: [f32; 3], range: f32| ears.as_ref().is_none_or(|ears| ears.iter().any(|e| dist(at, *e) <= range));
        events
            .iter()
            .filter(|e| match e {
                GameEvent::Footstep { player, at, .. } => visible.contains(player) || hears(*at, HEAR_NORMAL),
                GameEvent::Shot { shooter, weapon, silenced, from, .. } => visible.contains(shooter) || hears(*from, 1000.0 / sim::weapons::data::fire_attenuation(*weapon, *silenced)),
                _ => true,
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn segment_through_sphere() {
        let c = [0.0, 0.0, 0.0];
        assert!(super::through_sphere([-200.0, 0.0, 0.0], [200.0, 0.0, 0.0], c, 50.0));
        assert!(!super::through_sphere([-200.0, 100.0, 0.0], [200.0, 100.0, 0.0], c, 50.0));
        assert!(!super::through_sphere([100.0, 0.0, 0.0], [200.0, 0.0, 0.0], c, 50.0), "sphere behind the start");
    }
}
