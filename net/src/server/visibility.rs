//! Anti-wallhack: enemies you can't possibly see aren't sent to you at all, so a cheating
//! client has nothing to draw. Sounds are separate: footsteps and gunshots still reach you
//! within hearing range (where they're audible anyway), so you can hear people behind walls.
use super::*;
use std::collections::HashSet;

/// Keep sending someone this long after they went out of sight (no flicker at corners).
const GRACE_TICKS: u32 = TICK_RATE / 4;
/// The target's box is grown by this much sideways and their (and your) position is pushed
/// ahead along the velocity by `LOOKAHEAD` seconds, so someone stepping round a corner is
/// already there when they come into view, even with some ping.
const MARGIN: f32 = 24.0;
const LOOKAHEAD: f32 = 0.15;
/// GoldSrc sound falloff: silent at 1000 / attenuation units (ATTN_NORM 0.8, quiet 1.6).
const HEAR_NORMAL: f32 = 1000.0 / 0.8;
const HEAR_QUIET: f32 = 1000.0 / 1.6;

fn ahead(p: [f32; 3], v: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| p[i] + v[i] * LOOKAHEAD)
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

impl Server {
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
            if eyes.iter().any(|e| points.iter().any(|p| !self.world.sight_blocked(*e, *p))) {
                return true;
            }
        }
        false
    }

    /// Who `viewer` gets told about: themselves, teammates, and enemies in (or just out of) sight.
    /// The dead and spectators see everyone.
    pub(super) fn visible_to(&mut self, viewer: u8) -> HashSet<u8> {
        let Some(v) = self.clients.values().find(|c| c.id == viewer) else { return HashSet::new() };
        let all = !v.vitals.alive() || v.team == Team::Spectator;
        let tick = self.tick;
        let mut out = HashSet::new();
        let mut seen = Vec::new();
        for t in self.clients.values() {
            let visible = all || t.id == viewer || t.team == v.team || !t.vitals.alive() || self.in_sight(v, t);
            if visible {
                seen.push(t.id);
                out.insert(t.id);
            } else if self.last_seen.get(&(viewer, t.id)).is_some_and(|s| tick - s < GRACE_TICKS) {
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
        let Some(ear) = self.clients.values().find(|c| c.id == viewer).map(|c| c.state.movement.eye()) else { return Vec::new() };
        events
            .iter()
            .filter(|e| match e {
                GameEvent::Footstep { player, at } => visible.contains(player) || dist(*at, ear) <= HEAR_NORMAL,
                GameEvent::Shot { shooter, silenced, from, .. } => {
                    visible.contains(shooter) || dist(*from, ear) <= if *silenced { HEAR_QUIET } else { HEAR_NORMAL }
                }
                _ => true,
            })
            .cloned()
            .collect()
    }
}
