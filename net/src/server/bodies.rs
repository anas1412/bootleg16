//! Dead bodies (`sim::bodies`): made where a player dies (thrown as CS throws them), kept the
//! whole match (until round 1 after the warmup, or a deathmatch game ends), a bump everyone
//! walks over, and picked up with E to carry as a shield.
use super::*;
use sim::bodies;
use sim::collision::Hull;

/// Bodies kept at once (a whole match's: 18 rounds of 10 players is 180); the oldest go.
const MAX_BODIES: usize = 256;

pub(super) struct Body {
    id: u16,
    owner: u8,
    /// Its death animation (`bodies::DEATHS`).
    death: u8,
    origin: [f32; 3],
    velocity: [f32; 3],
    yaw: f32,
    hull: Hull,
    carrier: Option<u8>,
    /// The tick it died (clients play the death animation from then).
    born: u32,
}

impl Server {
    /// A player just died (`settle_deaths`): their body, thrown by what killed them.
    pub(super) fn make_body(&mut self, victim: u8) {
        // what hit them last this tick, and the blast that did it
        let (mut last_hit, mut last_damage, mut by, mut blast, mut boom) = (None, 0.0, None, None, None);
        for e in &self.events {
            match *e {
                GameEvent::Explosion { at } => boom = Some((at, false)),
                GameEvent::BombExploded { at } => boom = Some((at, true)),
                GameEvent::Damage { attacker, victim: v, amount, group, sound, .. } if v == victim => {
                    (last_hit, last_damage) = (Some(group), amount);
                    by = Some(attacker).filter(|a| *a != victim);
                    blast = boom.filter(|_| sound == HitSound::Explosion);
                }
                _ => {}
            }
        }
        let killer = by.and_then(|k| self.clients.values().find(|c| c.id == k)).map(|c| c.state.movement.eye());
        let Some(c) = self.clients.values().find(|c| c.id == victim) else { return };
        let m = &c.state.movement;
        let d = bodies::Death { at: m.origin, yaw: c.view.0, crouched: m.crouched, last_hit, last_damage, killer, blast };
        let roll = (self.tick.wrapping_mul(2_654_435_761) ^ (victim as u32).wrapping_mul(2_246_822_519)) >> 4;
        let (death, velocity) = bodies::death(&d, roll);
        let hull = m.hull();
        if self.bodies.len() >= MAX_BODIES {
            if let Some(i) = self.bodies.iter().position(|b| b.carrier.is_none()) {
                self.bodies.remove(i);
            }
        }
        self.next_body = self.next_body.wrapping_add(1);
        self.bodies.push(Body { id: self.next_body, owner: victim, death, origin: d.at, velocity, yaw: d.yaw, hull, carrier: None, born: self.tick });
    }

    /// Once a tick: E picks a body up or puts it down, carried ones follow their carrier,
    /// the rest fall and slide; then the world learns where they lie.
    pub(super) fn bodies_step(&mut self) {
        if self.events.iter().any(|e| matches!(e, GameEvent::RoundStart { round: 1 } | GameEvent::KillLimit { .. })) {
            self.bodies.clear();
        }
        // a carrier who died, left or let go drops it, with their speed
        for b in &mut self.bodies {
            let Some(id) = b.carrier else { continue };
            match self.clients.values().find(|c| c.id == id) {
                Some(c) if c.vitals.alive() && c.state.movement.carrying => {
                    b.origin = bodies::carried_at(c.state.movement.origin, c.view.0);
                    b.yaw = c.view.0;
                }
                c => {
                    b.carrier = None;
                    b.velocity = c.map(|c| c.state.movement.velocity).unwrap_or([0.0; 3]);
                    b.hull = c.map(|c| c.state.movement.hull()).unwrap_or(Hull::Standing);
                }
            }
        }
        // E: put yours down, or pick up the one you look at (a CT at the planted bomb defuses)
        let bomb = match self.bomb {
            Bomb::Planted { at, .. } => Some(at),
            _ => None,
        };
        let pressed: Vec<u8> = self.clients.values().filter(|c| c.used && c.vitals.alive()).map(|c| c.id).collect();
        for id in pressed {
            let c = self.clients.values_mut().find(|c| c.id == id).unwrap();
            let m = &mut c.state.movement;
            if m.carrying {
                m.carrying = false;
                if let Some(b) = self.bodies.iter_mut().find(|b| b.carrier == Some(id)) {
                    (b.carrier, b.velocity, b.hull) = (None, m.velocity, m.hull());
                }
                continue;
            }
            if c.team == Team::Ct && bomb.is_some_and(|at| (0..3).map(|i| (at[i] - m.origin[i]).powi(2)).sum::<f32>() < 96.0 * 96.0) {
                continue;
            }
            let (f, _, _) = bullets::view_vectors(c.view.1, c.view.0);
            let (o, eye) = (m.origin, m.eye());
            let near = |b: &&mut Body| (0..3).map(|i| (b.origin[i] - o[i]).powi(2)).sum::<f32>();
            let pick = self.bodies.iter_mut().filter(|b| b.carrier.is_none() && bodies::can_grab(o, eye, f, b.origin, b.hull)).min_by(|a, b| near(a).total_cmp(&near(b)));
            if let Some(b) = pick {
                b.carrier = Some(id);
                b.velocity = [0.0; 3];
                m.carrying = true;
            }
        }
        // where the carried ones are now; the rest fall
        let dt = self.rate.tick();
        for b in &mut self.bodies {
            match b.carrier.and_then(|id| self.clients.values().find(|c| c.id == id)) {
                Some(c) => {
                    b.origin = bodies::carried_at(c.state.movement.origin, c.view.0);
                    b.yaw = c.view.0;
                }
                None => bodies::fall(&mut b.origin, &mut b.velocity, b.hull, &self.world, dt, self.vars.gravity),
            }
        }
        self.world.boxes = self.bodies.iter().filter(|b| b.carrier.is_none()).map(|b| bodies::rest_box(b.origin, b.hull)).collect();
    }

    /// The bodies a viewer gets: all lying ones, a carried one only while they see its carrier.
    pub(super) fn bodies_for(&self, viewer: u8, seen: &std::collections::HashSet<u8>) -> Vec<BodyNet> {
        self.bodies
            .iter()
            .filter(|b| b.carrier.is_none_or(|c| c == viewer || seen.contains(&c)))
            .map(|b| BodyNet { id: b.id, owner: b.owner, death: b.death, origin: b.origin, yaw: b.yaw, crouched: b.hull == Hull::Crouched, carrier: b.carrier, born: b.born })
            .collect()
    }

    /// Whether the carried body of `victim` (at `origin` facing `yaw` as the shooter saw
    /// them) is in the way of a shot from `from` to `to`.
    pub(super) fn shielded(&self, victim: u8, origin: [f32; 3], yaw: f32, from: [f32; 3], to: [f32; 3]) -> bool {
        self.bodies.iter().any(|b| b.carrier == Some(victim)) && bodies::shield_blocks(origin, yaw, from, to)
    }
}
