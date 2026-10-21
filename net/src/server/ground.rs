//! Guns on the ground: dropped ones fall and slide; walking over one picks it up.
use super::*;

impl Server {
    /// Dropped guns fall and slide to a stop; walking over one picks it up if that slot is free.
    pub(super) fn ground_step(&mut self) {
        let dt = self.rate.tick();
        for g in &mut self.ground {
            if g.velocity == [0.0; 3] {
                continue;
            }
            g.velocity[2] -= self.vars.gravity * dt;
            let end = std::array::from_fn(|i| g.origin[i] + g.velocity[i] * dt);
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
        // A CT without a kit walking over one takes it.
        let mut k = 0;
        while k < self.kits.len() {
            let at = self.kits[k];
            let taker = self.clients.values_mut().find(|c| {
                let (o, h) = (c.state.movement.origin, c.state.movement.hull());
                c.team == Team::Ct && c.vitals.alive() && !c.defuse_kit && (0..3).all(|i| at[i] >= o[i] + h.mins()[i] - 16.0 && at[i] <= o[i] + h.maxs()[i] + 16.0)
            });
            if let Some(c) = taker {
                c.defuse_kit = true;
                self.kits.swap_remove(k);
            } else {
                k += 1;
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

    /// A player died (CS 1.6 `PackDeadPlayerItems`): their best gun (primary, else the pistol;
    /// never grenades) falls where they were, and their defuse kit
    /// (CS 1.6 drops it as `item_thighpack`; a CT without one picks it up).
    pub(super) fn drop_on_death(&mut self, victim: u8) {
        let Some(c) = self.clients.values().find(|c| c.id == victim) else { return };
        if c.defuse_kit {
            let o = c.state.movement.origin;
            let floor = self.world.trace_line(o, [o[0], o[1], o[2] - 4096.0]).end;
            self.kits.push(floor);
        }
        let Some(c) = self.clients.values_mut().find(|c| c.id == victim) else { return };
        c.defuse_kit = false;
        let l = &mut c.state.loadout;
        let gun = l.primary.take().or_else(|| l.secondary.take());
        if let Some(gun) = gun {
            // (`PackPlayerItem`: it flies on with 1.2 x the player's last velocity)
            let v = c.state.movement.velocity;
            let velocity = [v[0] * 1.2, v[1] * 1.2, v[2] * 1.2 - 1.0];
            self.ground.push(GroundGun { gun, origin: c.state.movement.origin, velocity, dropper: victim, dropper_wait_until: 0 });
        }
    }
}
