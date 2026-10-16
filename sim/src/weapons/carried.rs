use super::*;

impl Loadout {
    pub(super) fn knife(&mut self, c: &Context, out: &mut Fired) {
        // Timings are refined by the hit result in `knife_result`.
        if c.attack2 && c.now >= self.knife_next_secondary {
            out.swing = Some(Swing { stab: true, angles: [c.pitch, c.yaw] });
            self.knife_next_primary = c.now + 1.0;
            self.knife_next_secondary = c.now + 1.0;
        } else if c.attack && c.now >= self.knife_next_primary {
            out.swing = Some(Swing { stab: false, angles: [c.pitch, c.yaw] });
            self.knife_next_primary = c.now + 0.35;
            self.knife_next_secondary = c.now + 0.5;
        }
    }

    /// Hold attack in a bomb site, standing on the ground, for `C4_PLANT_TIME`; letting go
    /// or leaving the site starts over. Once planted, the best other weapon comes out.
    pub(super) fn plant(&mut self, c: &Context, out: &mut Fired) {
        if !(c.attack && c.can_shoot && c.in_bomb_site && c.on_ground) {
            self.planting = None;
            return;
        }
        let start = *self.planting.get_or_insert(c.now);
        if c.now - start >= C4_PLANT_TIME {
            self.planting = None;
            self.c4 = false;
            out.planted = true;
            let next = [Slot::Primary, Slot::Secondary, Slot::Knife].into_iter().find(|s| self.has(*s)).unwrap_or(Slot::Knife);
            self.switch_to(next, c.now);
        }
    }

    /// Hold fire to pull the pin; the throw happens when you let go (at least 0.5 s
    /// after pulling it). After the last of a type, the next type or the last weapon.
    pub(super) fn grenade_update(&mut self, c: &Context, out: &mut Fired) {
        let Some(id) = self.grenade_id() else {
            self.switch_to(self.last_slot, c.now);
            return;
        };
        self.grenade = match id {
            Id::HeGrenade => 0,
            Id::Flashbang => 1,
            _ => 2,
        };
        match self.pin_until {
            None if c.attack && c.now >= self.grenade_next && c.can_shoot => self.pin_until = Some(c.now + 0.5),
            Some(t) if !c.attack && c.now >= t => {
                let kind = [crate::grenades::Kind::He, crate::grenades::Kind::Flash, crate::grenades::Kind::Smoke][self.grenade as usize];
                out.throw = Some((kind, [c.pitch + self.punch[0], c.yaw + self.punch[1]]));
                self.grenades[self.grenade as usize] -= 1;
                self.pin_until = None;
                self.grenade_next = c.now + 0.5;
                self.next_attack = c.now + 0.5;
                if self.grenade_id().is_none() {
                    let back = if self.last_slot == Slot::Grenade { Slot::Knife } else { self.last_slot };
                    self.switch_to(back, c.now + 0.5);
                }
            }
            _ => {}
        }
    }

    /// After tracing a swing: hits recover a bit slower than misses.
    pub fn knife_result(&mut self, stab: bool, hit: bool, now: f64) {
        if hit {
            if stab {
                self.knife_next_primary = now + 1.1;
                self.knife_next_secondary = now + 1.1;
            } else {
                self.knife_next_primary = now + 0.4;
            }
        }
    }
}
