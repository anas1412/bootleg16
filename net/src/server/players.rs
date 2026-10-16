//! Each tick, every player's queued inputs: movement, weapons (shots and knife swings to
//! resolve, bombs planted or dropped), within the speed-hack budget; then the world (doors)
//! moves with them.
use super::*;

impl Server {
    pub(super) fn run_players(&mut self) {
        let tick = self.tick;
        let mut actors = Vec::new();
        let mut pending = Vec::new();
        let mut planted = Vec::new();
        let mut bomb_drops = Vec::new();
        let limits: HashMap<u8, sim::player::Limits> = self.clients.values().map(|c| (c.id, self.limits(c.id))).collect();
        for c in self.clients.values_mut() {
            c.budget = (c.budget + 1.0).min(MAX_INPUT_BUDGET);
            c.used = false;
            while c.budget >= 1.0 {
                let Some((seq, TimedInput { input, view_tick })) = c.queue.pop_front() else { break };
                c.budget -= 1.0;
                c.used |= input.use_key && !c.use_held;
                c.use_held = input.use_key;
                c.view = (input.yaw, input.pitch);
                c.aim.look(input.yaw, input.pitch);
                c.last_run = seq;
                if !c.vitals.alive() {
                    continue; // the dead don't move (their inputs are still acknowledged)
                }
                let cmd_time = seq as f64 * TICK as f64;
                let (ev, fired) = player::step(&mut c.state, &input, TICK, cmd_time, player::seed(c.id, seq), limits[&c.id], &self.world, &self.vars);
                if fired.planted {
                    planted.push((c.id, c.state.movement.origin));
                }
                if input.drop && c.state.loadout.active == data::Slot::Bomb && c.state.loadout.c4 {
                    bomb_drops.push(c.id);
                }
                if let Some(step) = ev.footstep {
                    self.events.push(GameEvent::Footstep { player: c.id, at: c.state.movement.origin, step });
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
                if !fired.shots.is_empty() || fired.swing.is_some() {
                    c.anim.shot(tick as f32 * TICK);
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
        for (id, at) in planted {
            self.bomb_planted(id, at);
        }
        for id in bomb_drops {
            self.drop_bomb(id);
        }
        if self.rules.infinite_ammo {
            for c in self.clients.values_mut() {
                c.state.loadout.refill_ammo(true);
                c.state.loadout.refill_ammo(false);
            }
        }
        self.world.update(TICK, &actors);
        // Animate everyone as clients draw them (their hitboxes follow it).
        let now = tick as f32 * TICK;
        let model = self.assets.hitboxes.model().clone();
        for c in self.clients.values_mut().filter(|c| c.vitals.alive()) {
            let d = c.drive();
            c.anim.step(&model, &d, now, TICK);
        }
        for name in self.world.doors.take_fired() {
            self.events.push(GameEvent::Fired { name });
        }
    }
}
