use super::*;

impl Loadout {
    /// One tick of weapon logic. Call after the movement step.
    pub fn update(&mut self, c: &Context) -> Fired {
        let mut out = Fired::default();
        // Recoil punch fades: at least 10 deg/s, faster when large (PM_DropPunchAngle).
        let len = (self.punch[0].powi(2) + self.punch[1].powi(2) + self.punch[2].powi(2)).sqrt();
        if len > 0.0 {
            let new = (len - (10.0 + 0.5 * len) * c.dt).max(0.0);
            self.punch = self.punch.map(|p| p * new / len);
        }

        if c.drop {
            out.dropped = self.drop_active(c.now);
        }
        if c.slot != 0 {
            let order = [Slot::Primary, Slot::Secondary, Slot::Knife, Slot::Grenade, Slot::Bomb];
            let pos = order.iter().position(|s| *s == self.active).unwrap_or(0);
            let cycle = |step: usize| (1..=5).map(|k| order[(pos + k * step) % 5]).find(|s| self.has(*s)).unwrap_or(self.active);
            let slot = match c.slot {
                1 => Slot::Primary,
                2 => Slot::Secondary,
                3 => Slot::Knife,
                4 => Slot::Grenade,
                5 => Slot::Bomb,
                crate::pmove::SLOT_LAST => self.last_slot,
                crate::pmove::SLOT_NEXT => cycle(1),
                _ => cycle(4), // previous = four steps forward round the five slots
            };
            if slot != self.active {
                self.switch_to(slot, c.now);
            } else if slot == Slot::Grenade && self.pin_until.is_none() {
                // Pressing 4 again picks the next grenade type you carry.
                if let Some(i) = (1..=3).map(|k| (self.grenade as usize + k) % 3).find(|&i| self.grenades[i] > 0) {
                    self.grenade = i as u8;
                    self.next_attack = c.now + 0.75;
                }
            }
        }

        if self.active != Slot::Bomb {
            self.planting = None;
        }
        if c.now < self.next_attack {
            return out;
        }
        if self.active == Slot::Knife {
            if c.can_shoot {
                self.knife(c, &mut out);
            }
            return out;
        }
        if self.active == Slot::Bomb {
            self.plant(c, &mut out);
            return out;
        }
        if self.active == Slot::Grenade {
            self.grenade_update(c, &mut out);
            return out;
        }
        let mut punch = self.punch;
        let Some(g) = self.active_gun_mut().copied() else { return out };
        let mut g = g;
        let gd = gun(g.id).unwrap();
        let mut next_attack = None;

        // 1. AWP/Scout zoom comes back once the bolt is done.
        if g.resume_zoom > 0 && c.now >= g.next_primary {
            g.zoom = g.resume_zoom;
            g.resume_zoom = 0;
        }
        // 2. Reload finishes: ammo moves only now, so cancelling loses nothing.
        if g.reloading && gd.pellets.is_none() {
            let moved = (gd.clip - g.clip).min(g.reserve);
            g.clip += moved;
            g.reserve -= moved;
            g.reloading = false;
        }

        // 3. Exactly one of: secondary, primary, reload, idle.
        if c.attack2 && gd.secondary != Secondary::None && c.now >= g.next_secondary {
            match gd.secondary {
                Secondary::Toggle(t) => {
                    g.mode = (g.mode + 1) % gd.modes.len() as u8;
                    g.next_secondary = c.now + t as f64;
                    if t > 0.5 {
                        g.next_primary = c.now + t as f64; // attaching a silencer takes a while
                    }
                }
                Secondary::Zoom(steps) => {
                    g.zoom = (g.zoom + 1) % (steps.len() as u8 + 1);
                    g.next_secondary = c.now + 0.3;
                }
                Secondary::None => {}
            }
        } else if c.attack && c.now >= g.next_primary && c.can_shoot {
            if gd.pellets.is_some() && g.shell_state != 0 {
                g.shell_state = 0; // firing interrupts a shotgun reload
            }
            fire(&mut g, gd, c, &mut out, &mut punch);
            if gd.pellets.is_some() && g.clip == 0 && out.dry_fire {
                g.next_primary = c.now + 0.15;
            }
        } else if c.reload && !c.attack && !g.reloading {
            next_attack = start_reload(&mut g, gd, c.now);
        } else if !c.attack && !c.attack2 {
            // Idle: shots-fired decay, semi-auto release, auto reload.
            if g.delay_fire {
                g.delay_fire = false;
                g.shots_fired = g.shots_fired.min(15);
                g.decrease_at = c.now + 0.4;
            }
            if gd.semi_auto {
                g.shots_fired = 0;
            } else if g.shots_fired > 0 && g.decrease_at < c.now {
                g.decrease_at = c.now + 0.0225;
                g.shots_fired -= 1;
            }
            if g.clip == 0 && g.reserve > 0 && c.now > g.next_primary && !g.reloading && g.shell_state == 0 {
                next_attack = start_reload(&mut g, gd, c.now);
            }
        }
        // Shotguns load shell by shell in the background until full or interrupted.
        if let Some(p) = gd.pellets {
            shotgun_reload_step(&mut g, gd, &p, c.now);
        }

        if let Some(t) = next_attack {
            self.next_attack = t;
        }
        self.punch = punch;
        if let Some(slot) = self.active_gun_mut() {
            *slot = g;
        }
        out
    }
}
