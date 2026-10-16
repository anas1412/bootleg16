//! Guns on the ground: dropped ones fall and slide; walking over one picks it up.
use super::*;

impl Server {
    /// Dropped guns fall and slide to a stop; walking over one picks it up if that slot is free.
    pub(super) fn ground_step(&mut self) {
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
    pub(super) fn drop_on_death(&mut self, victim: u8) {
        let Some(c) = self.clients.values_mut().find(|c| c.id == victim) else { return };
        let l = &mut c.state.loadout;
        let gun = l.primary.take().or_else(|| l.secondary.take());
        if let Some(gun) = gun {
            self.ground.push(GroundGun { gun, origin: c.state.movement.origin, velocity: [0.0, 0.0, -1.0], dropper: victim, dropper_wait_until: 0 });
        }
    }
}
