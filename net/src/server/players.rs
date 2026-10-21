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
            c.budget = (c.budget + 1.0).min(self.rate.secs(MAX_INPUT_BUDGET_SECS) as f32);
            c.used = false;
            while c.budget >= 1.0 {
                let Some((seq, TimedInput { input, view_tick })) = c.queue.pop_front() else { break };
                c.budget -= 1.0;
                c.used |= input.use_key && !c.use_held;
                c.use_held = input.use_key;
                c.view = (input.yaw, input.pitch);
                // Deathmatch spawn protection ends the moment they move or shoot (CS2).
                if input.attack || input.attack2 || input.jump || input.forward != 0.0 || input.side != 0.0 {
                    c.protected_until = 0;
                }
                c.aim.look(input.yaw, input.pitch);
                c.last_run = seq;
                if !c.vitals.alive() {
                    continue; // the dead don't move (their inputs are still acknowledged)
                }
                let cmd_time = seq as f64 * self.rate.tick() as f64;
                let (ev, fired) = player::step(&mut c.state, &input, self.rate.tick(), cmd_time, player::seed(c.id, seq), limits[&c.id], &self.world, &self.vars);
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
                    self.events.push(GameEvent::Damage { attacker: c.id, victim: c.id, amount: ev.fall_damage, group: sim::hitbox::HitGroup::Generic, sound: crate::protocol::HitSound::Splat, at: None });
                    c.respawn_at = Some(tick + if c.bot { self.rate.secs(DUMMY_RESPAWN_SECS) } else { self.rate.secs(self.rules.respawn_secs) });
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
                        dropper_wait_until: tick + self.rate.secs(REPICK_SECS),
                    });
                }
                if !fired.shots.is_empty() || fired.swing.is_some() {
                    c.anim.shot(tick as f32 * self.rate.tick());
                }
                if !fired.shots.is_empty() || fired.swing.is_some() || fired.throw.is_some() {
                    pending.push(Pending { shooter: c.id, eye: c.state.movement.eye(), yaw: input.yaw, velocity: c.state.movement.velocity, fired, view_tick });
                }
            }
            c.backlog = if c.queue.len() > BACKLOG_INPUTS { c.backlog + 1 } else { 0 };
            if c.backlog > self.rate.secs(BACKLOG_SECS) {
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
        // Players are solid to each other; bots hold their ground and the humans are pushed out.
        let hull = |m: &sim::pmove::Player| if m.crouched { sim::collision::Hull::Crouched } else { sim::collision::Hull::Standing };
        let boxes: Vec<(u8, [f32; 3], sim::collision::Hull)> = self.clients.values().filter(|c| c.vitals.alive() && c.team != Team::Spectator).map(|c| (c.id, c.state.movement.origin, hull(&c.state.movement))).collect();
        for c in self.clients.values_mut().filter(|c| c.vitals.alive() && c.team != Team::Spectator && !c.bot) {
            let others: Vec<_> = boxes.iter().filter(|b| b.0 != c.id).map(|b| (b.1, b.2)).collect();
            c.state.movement.origin = sim::crowd::push_out(&self.world, hull(&c.state.movement), c.state.movement.origin, &others);
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
        self.world.update(self.rate.tick(), &actors);
        // Animate everyone as clients draw them (their hitboxes follow it).
        let now = tick as f32 * self.rate.tick();
        let model = self.assets.hitboxes.model().clone();
        for c in self.clients.values_mut().filter(|c| c.vitals.alive()) {
            let d = c.drive();
            c.anim.step(&model, &d, now, self.rate.tick());
        }
        for name in self.world.doors.take_fired() {
            self.events.push(GameEvent::Fired { name });
        }
    }
}
