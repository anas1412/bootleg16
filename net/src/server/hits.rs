//! Grenades flying and going off, shots and knife hits resolved (with lag compensation), and deaths settled.
use super::*;

impl Server {
    /// Fly grenades and set off the ones that are due.
    pub(super) fn grenades_step(&mut self) {
        use sim::grenades::{self, Kind};
        let now = self.tick as f64 * TICK as f64;
        for g in &mut self.grenades {
            if g.physics(TICK, &self.world, self.vars.gravity) {
                self.events.push(GameEvent::GrenadeBounce { kind: g.kind, at: g.origin });
            }
        }
        let (due, flying): (Vec<_>, Vec<_>) = self.grenades.drain(..).partition(|g| g.due(now));
        self.grenades = flying;
        self.smokes.retain(|s| s.1 > now);
        let tick = self.tick;
        for g in due {
            match g.kind {
                Kind::He => {
                    let blast = g.blast_point(&self.world);
                    self.events.push(GameEvent::Explosion { at: blast });
                    self.blast_pieces(blast, 350.0, 100.0);
                    // Without friendly fire your own HE still hurts you, not your team.
                    let owner_team = self.clients.values().find(|c| c.id == g.owner).map(|c| c.team);
                    let ff = self.round.cfg.friendly_fire;
                    for c in self.clients.values_mut().filter(|c| c.vitals.alive() && (ff || c.id == g.owner || Some(c.team) != owner_team)) {
                        let dmg = grenades::he_damage(blast, c.state.movement.origin);
                        if dmg <= 0.0 {
                            continue;
                        }
                        let (health, armor) = combat::explosion_damage(&c.vitals, dmg);
                        let killed = c.vitals.hurt(health, armor);
                        self.events.push(GameEvent::Damage { attacker: g.owner, victim: c.id, amount: health, group: HitGroup::Generic, sound: HitSound::Explosion, at: None });
                        if killed {
                            c.respawn_at = Some(tick + if c.bot { DUMMY_RESPAWN_TICKS } else { self.rules.respawn_ticks });
                            self.events.push(GameEvent::Kill { killer: g.owner, victim: c.id, weapon: Some(Id::HeGrenade), headshot: false });
                        }
                    }
                }
                Kind::Flash => {
                    let blinded = self
                        .clients
                        .values()
                        .filter(|c| c.vitals.alive())
                        .filter_map(|c| {
                            let (yaw, pitch) = c.view;
                            let (f, _, _) = bullets::view_vectors(pitch, yaw);
                            grenades::flash(&self.world, g.origin, c.state.movement.eye(), f).map(|b| (c.id, b))
                        })
                        .collect();
                    self.events.push(GameEvent::Flash { at: g.origin, blinded });
                }
                Kind::Smoke => self.smokes.push((g.origin, now + grenades::SMOKE_SECONDS)),
            }
        }
    }

    /// Trace a player's shots/swing against everyone else as the shooter saw them.
    pub(super) fn resolve(&mut self, p: Pending) {
        if let Some((kind, angles)) = p.fired.throw {
            let now = self.tick as f64 * TICK as f64;
            self.grenades.push(sim::grenades::throw(kind, p.shooter, p.eye, angles[0], angles[1], p.velocity, now));
        }
        let targets: Vec<Target> = self.rewind(p.view_tick).into_iter().filter(|t| t.id != p.shooter).collect();
        let mut hits: Vec<(u8, f32, HitGroup, Id, Option<[f32; 3]>)> = Vec::new();
        for shot in &p.fired.shots {
            let r = if shot.pellets.is_some() {
                bullets::fire_pellets(&self.world, &self.assets.hitboxes, p.eye, shot, p.shooter, &targets)
            } else {
                bullets::fire_bullet(&self.world, &self.assets.materials, &self.assets.hitboxes, p.eye, shot, p.shooter, &targets)
            };
            let end = r.impacts.last().map(|i| i.point).unwrap_or_else(|| {
                let (f, _, _) = bullets::view_vectors(shot.angles[0], shot.angles[1]);
                std::array::from_fn(|i| p.eye[i] + f[i] * 2048.0)
            });
            let holes = r.impacts.iter().filter(|i| !i.sky).map(|i| (i.point, i.normal)).collect();
            for i in r.impacts.iter().filter(|i| i.model != 0) {
                self.shoot_piece(i.model, shot.damage as f32);
            }
            self.events.push(GameEvent::Shot { shooter: p.shooter, weapon: shot.weapon, silenced: shot.silenced, from: p.eye, to: end, holes });
            hits.extend(r.hits.into_iter().map(|h| (h.victim, h.damage, h.group, shot.weapon, Some(h.point))));
        }
        if let Some(swing) = p.fired.swing {
            let range = if swing.stab { STAB_RANGE } else { SLASH_RANGE };
            let (f, _, _) = bullets::view_vectors(swing.angles[0], swing.angles[1]);
            let end = std::array::from_fn(|i| p.eye[i] + f[i] * range);
            let hit = bullets::trace_melee(&self.world, &self.assets.hitboxes, p.eye, end, p.shooter, &targets);
            if let Some((victim, group)) = hit {
                let mut dmg = if swing.stab { STAB_DAMAGE } else { SLASH_DAMAGE };
                // Backstab: both facing the same way (within ~37 degrees).
                if swing.stab {
                    if let Some(t) = targets.iter().find(|t| t.id == victim) {
                        if (p.yaw - t.yaw).to_radians().cos() > 0.8 {
                            dmg *= 3.0;
                        }
                    }
                }
                hits.push((victim, dmg * combat::hitgroup_multiplier(group), group, Id::Knife, None));
            }
            if let Some(c) = self.clients.values_mut().find(|c| c.id == p.shooter) {
                c.state.loadout.knife_result(swing.stab, hit.is_some(), c.last_run as f64 * TICK as f64);
            }
        }
        let tick = self.tick;
        let shooter_team = self.clients.values().find(|c| c.id == p.shooter).map(|c| c.team);
        let ff = self.round.cfg.friendly_fire;
        for (victim, damage, group, weapon, at) in hits {
            let Some(c) = self.clients.values_mut().find(|c| c.id == victim) else { continue };
            if !c.vitals.alive() || (!ff && Some(c.team) == shooter_team) {
                continue;
            }
            let (health, armor) = combat::bullet_damage(&c.vitals, damage, group, data::armor_factor(weapon), false);
            let stab = p.fired.swing.is_some_and(|s| s.stab);
            let sound = match group {
                _ if weapon == Id::Knife => if stab { HitSound::KnifeStab } else { HitSound::KnifeSlash },
                HitGroup::Head if c.vitals.helmet && c.vitals.armor > 0.0 => HitSound::Helmet,
                HitGroup::Head => HitSound::Headshot,
                HitGroup::LeftLeg | HitGroup::RightLeg => HitSound::Flesh,
                _ if c.vitals.armor > 0.0 => HitSound::Kevlar,
                _ => HitSound::Flesh,
            };
            // Slowdown and view kick (CS 1.6); the kick only where that part had no armor.
            let unarmored = match group {
                HitGroup::Head => !(c.vitals.helmet && c.vitals.armor > 0.0),
                _ => c.vitals.armor <= 0.0,
            };
            let roll = ((tick.wrapping_mul(2_654_435_761) ^ victim as u32) % 2001) as f32 / 1000.0 - 1.0;
            player::hit_by(&mut c.state, weapon, group, damage, unarmored, roll);
            let killed = c.vitals.hurt(health, armor);
            self.events.push(GameEvent::Damage { attacker: p.shooter, victim, amount: health, group, sound, at });
            if killed {
                c.respawn_at = Some(tick + if c.bot { DUMMY_RESPAWN_TICKS } else { self.rules.respawn_ticks });
                self.events.push(GameEvent::Kill { killer: p.shooter, victim, weapon: Some(weapon), headshot: group == HitGroup::Head });
            }
        }
        self.settle_deaths();
    }

    /// Bookkeeping for kill events raised this tick: scores and dropped guns.
    pub(super) fn settle_deaths(&mut self) {
        let kills: Vec<(u8, u8)> = self.events.iter().filter_map(|e| match e {
            GameEvent::Kill { killer, victim, .. } => Some((*killer, *victim)),
            _ => None,
        }).collect();
        let respawns = self.respawns();
        for (killer, victim) in kills.into_iter().skip(self.kills_settled) {
            if let Some(v) = self.clients.values_mut().find(|c| c.id == victim) {
                v.deaths += 1;
                if !respawns {
                    v.respawn_at = None; // wait for the next round
                }
            }
            self.kill_money(killer, victim);
            self.drop_bomb(victim);
            if let Some(k) = self.clients.values_mut().find(|c| c.id == killer) {
                k.kills += if killer == victim { -1 } else { 1 };
            }
            self.kill_limit(killer);
            self.drop_on_death(victim);
            self.kills_settled += 1;
        }
    }

    /// Deathmatch: the first to the kill limit is announced, and everyone starts over.
    fn kill_limit(&mut self, killer: u8) {
        let limit = self.rules.kill_limit;
        if !self.rules.deathmatch || !self.clients.values().any(|c| c.id == killer && c.kills >= limit) {
            return;
        }
        self.events.push(GameEvent::KillLimit { player: killer });
        for c in self.clients.values_mut() {
            c.kills = 0;
            c.deaths = 0;
        }
    }
}
