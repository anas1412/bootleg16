//! Weapon behaviour: firing timing, ammo and reloads, accuracy, spread, recoil,
//! fire modes, scopes, switching. Follows docs/WEAPONS.md. Bullets are traced in
//! `bullets`; this module decides when and how each shot happens.
pub mod bullets;
pub mod data;

pub use data::{def, gun, Id, Slot};
use data::{Accuracy, Kind, Recoil, Secondary, SniperRule, Spread};
use serde::{Deserialize, Serialize};

/// Deterministic random numbers: the same (seed, k) always gives the same value,
/// so a shot replays identically (tests, replays, client effects).
pub fn rand01(seed: u64, k: u64) -> f32 {
    let mut z = seed.wrapping_add(k.wrapping_mul(0x9E37_79B9_7F4A_7C15)).wrapping_add(0x6A09_E667_F3BC_C909);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}

fn rand_range(seed: u64, k: u64, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * rand01(seed, k)
}

fn rand_int(seed: u64, k: u64, lo: i32, hi: i32) -> i32 {
    lo + ((rand01(seed, k) * (hi - lo + 1) as f32) as i32).min(hi - lo)
}

/// One owned gun and everything about its state.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GunState {
    pub id: Id,
    pub clip: u32,
    pub reserve: u32,
    /// Index into the gun's modes (silencer on, burst on).
    pub mode: u8,
    /// Zoom step: 0 = none, n = `Secondary::Zoom` entry n-1.
    pub zoom: u8,
    /// AWP/Scout: zoom to restore once the bolt is back.
    resume_zoom: u8,
    pub accuracy: f32,
    pub shots_fired: i32,
    delay_fire: bool,
    decrease_at: f64,
    /// Time of the last shot; 0 = never fired.
    last_fire: f64,
    /// KickBack lateral direction (0/1); never reset.
    direction: u8,
    next_primary: f64,
    next_secondary: f64,
    pub reloading: bool,
    /// Shotguns: 0 idle, 1 between shells, 2 inserting.
    shell_state: u8,
    shell_at: f64,
    /// Pending burst follow-ups: (bullets left, time of next, spread).
    burst: Option<(u8, f64, f32)>,
}

impl GunState {
    pub fn new(id: Id) -> GunState {
        let g = gun(id).expect("not a gun");
        GunState {
            id,
            clip: g.clip,
            reserve: g.reserve,
            mode: 0,
            zoom: 0,
            resume_zoom: 0,
            accuracy: initial_accuracy(&g.accuracy),
            shots_fired: 0,
            delay_fire: false,
            decrease_at: 0.0,
            last_fire: 0.0,
            direction: 0,
            next_primary: 0.0,
            next_secondary: 0.0,
            reloading: false,
            shell_state: 0,
            shell_at: 0.0,
            burst: None,
        }
    }

    pub fn fov(&self) -> u8 {
        match gun(self.id).map(|g| g.secondary) {
            Some(Secondary::Zoom(steps)) if self.zoom > 0 => steps[self.zoom as usize - 1],
            _ => 90,
        }
    }
}

fn initial_accuracy(a: &Accuracy) -> f32 {
    match *a {
        Accuracy::Auto { initial, .. } | Accuracy::PistolTime { initial, .. } | Accuracy::SniperTime { initial, .. } => initial,
        Accuracy::None => 0.0,
    }
}

/// What the player is carrying.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Loadout {
    pub primary: Option<GunState>,
    pub secondary: Option<GunState>,
    pub knife: bool,
    /// HE, flash, smoke counts.
    pub grenades: [u8; 3],
    pub c4: bool,
    pub active: Slot,
    last_slot: Slot,
    /// Blocks all weapon actions until this time (deploy, reload).
    next_attack: f64,
    /// Recoil view punch (pitch, yaw, roll) in degrees.
    pub punch: [f32; 3],
    knife_next_primary: f64,
    knife_next_secondary: f64,
}

/// A bullet (or pellet volley) to trace, decided by the weapon update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub weapon: Id,
    pub damage: i32,
    pub range_mod: f32,
    pub penetration: u32,
    pub distance: f32,
    pub bullet: data::Bullet,
    /// Spread number (0 = perfectly accurate); x/y offsets come from `seed`.
    pub spread: f32,
    /// View angles + punch at the moment of the shot (pitch, yaw), degrees.
    pub angles: [f32; 2],
    /// Sideways offset of the muzzle (Elite alternates guns), units.
    pub side_offset: f32,
    pub seed: u64,
    pub pellets: Option<data::Pellets>,
}

/// A knife swing to trace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swing {
    pub stab: bool,
    pub angles: [f32; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fired {
    pub shots: Vec<Shot>,
    pub swing: Option<Swing>,
    /// The empty-clip click.
    pub dry_fire: bool,
}

/// What the weapon code needs to know about the player this tick.
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub now: f64,
    pub dt: f32,
    pub on_ground: bool,
    pub crouched: bool,
    pub speed_2d: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub attack: bool,
    pub attack2: bool,
    pub reload: bool,
    /// Slot key pressed this tick (0 = none).
    pub slot: u8,
    /// Freeze time etc.: no shooting.
    pub can_shoot: bool,
    /// Per-tick random seed (player and input number).
    pub seed: u64,
}

impl Loadout {
    /// Knife only.
    pub fn knife_only() -> Loadout {
        Loadout {
            primary: None,
            secondary: None,
            knife: true,
            grenades: [0; 3],
            c4: false,
            active: Slot::Knife,
            last_slot: Slot::Knife,
            next_attack: 0.0,
            punch: [0.0; 3],
            knife_next_primary: 0.0,
            knife_next_secondary: 0.0,
        }
    }

    /// Knife + a pistol with spawn ammo (T: Glock 20+40, CT: USP 12+24), pistol out.
    pub fn spawn(pistol: Id, reserve: u32) -> Loadout {
        let mut l = Loadout::knife_only();
        let mut p = GunState::new(pistol);
        p.reserve = reserve;
        l.secondary = Some(p);
        l.active = Slot::Secondary;
        l
    }

    /// Put a gun in its slot (replacing what was there) and draw it.
    pub fn give(&mut self, id: Id, now: f64) {
        let g = GunState::new(id);
        match def(id).slot {
            Slot::Primary => self.primary = Some(g),
            Slot::Secondary => self.secondary = Some(g),
            _ => return,
        }
        self.switch_to(def(id).slot, now);
    }

    pub fn active_gun(&self) -> Option<&GunState> {
        match self.active {
            Slot::Primary => self.primary.as_ref(),
            Slot::Secondary => self.secondary.as_ref(),
            _ => None,
        }
    }

    fn active_gun_mut(&mut self) -> Option<&mut GunState> {
        match self.active {
            Slot::Primary => self.primary.as_mut(),
            Slot::Secondary => self.secondary.as_mut(),
            _ => None,
        }
    }

    pub fn active_id(&self) -> Id {
        self.active_gun().map(|g| g.id).unwrap_or(Id::Knife)
    }

    /// Movement speed with the current weapon.
    pub fn max_speed(&self) -> f32 {
        self.active_gun().map(|g| data::max_speed(g.id, g.zoom > 0)).unwrap_or(250.0)
    }

    fn has(&self, slot: Slot) -> bool {
        match slot {
            Slot::Primary => self.primary.is_some(),
            Slot::Secondary => self.secondary.is_some(),
            Slot::Knife => self.knife,
            Slot::Grenade => self.grenades.iter().any(|&n| n > 0),
            Slot::Bomb => self.c4,
        }
    }

    /// Holster the current weapon (cancels a reload or burst) and deploy another.
    pub fn switch_to(&mut self, slot: Slot, now: f64) {
        if !self.has(slot) {
            return;
        }
        if let Some(g) = self.active_gun_mut() {
            g.reloading = false;
            g.shell_state = 0;
            g.burst = None;
            g.zoom = 0;
            g.resume_zoom = 0;
        }
        self.last_slot = self.active;
        self.active = slot;
        let deploy = match self.active_gun_mut() {
            Some(g) => {
                let gd = gun(g.id).unwrap();
                g.accuracy = initial_accuracy(&gd.accuracy);
                g.shots_fired = 0;
                g.delay_fire = false;
                g.decrease_at = now;
                g.zoom = 0;
                g.next_primary = now + gd.deploy as f64;
                gd.deploy
            }
            None => 0.75,
        };
        self.next_attack = now + deploy as f64;
    }

    /// One tick of weapon logic. Call after the movement step.
    pub fn update(&mut self, c: &Context) -> Fired {
        let mut out = Fired::default();
        // Recoil punch fades: at least 10 deg/s, faster when large (PM_DropPunchAngle).
        let len = (self.punch[0].powi(2) + self.punch[1].powi(2) + self.punch[2].powi(2)).sqrt();
        if len > 0.0 {
            let new = (len - (10.0 + 0.5 * len) * c.dt).max(0.0);
            self.punch = self.punch.map(|p| p * new / len);
        }

        if c.slot != 0 {
            let slot = match c.slot {
                1 => Slot::Primary,
                2 => Slot::Secondary,
                3 => Slot::Knife,
                4 => Slot::Grenade,
                _ => Slot::Bomb,
            };
            if slot != self.active {
                self.switch_to(slot, c.now);
            }
        }

        if c.now < self.next_attack {
            return out;
        }
        if self.active == Slot::Knife {
            self.knife(c, &mut out);
            return out;
        }
        let mut punch = self.punch;
        let Some(g) = self.active_gun_mut().copied() else { return out };
        let mut g = g;
        let gd = gun(g.id).unwrap();
        let mut next_attack = None;

        // 1. Burst follow-up bullets.
        if let Some((left, at, spread)) = g.burst {
            let burst = gd.modes[g.mode as usize].burst.unwrap();
            if c.now >= at && g.clip > 0 {
                g.clip -= 1;
                out.shots.push(Shot {
                    weapon: g.id,
                    damage: burst.damage,
                    range_mod: burst.range_mod,
                    penetration: gd.penetration,
                    distance: 8192.0,
                    bullet: gd.bullet,
                    spread: burst.spread.unwrap_or(spread),
                    angles: [c.pitch + punch[0], c.yaw + punch[1]],
                    side_offset: 0.0,
                    seed: c.seed,
                    pellets: None,
                });
                let idx = 3 - left as usize; // which delay comes next
                g.burst = if left > 1 { Some((left - 1, c.now + burst.delays.get(idx).copied().unwrap_or(0.0) as f64, spread)) } else { None };
            } else if g.clip == 0 {
                g.burst = None;
            }
        }
        // 2. AWP/Scout zoom comes back once the bolt is done.
        if g.resume_zoom > 0 && c.now >= g.next_primary {
            g.zoom = g.resume_zoom;
            g.resume_zoom = 0;
        }
        // 3. Reload finishes: ammo moves only now, so cancelling loses nothing.
        if g.reloading && gd.pellets.is_none() {
            let moved = (gd.clip - g.clip).min(g.reserve);
            g.clip += moved;
            g.reserve -= moved;
            g.reloading = false;
        }

        // 4. Exactly one of: secondary, primary, reload, idle.
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
        } else if c.attack && c.now >= g.next_primary && c.can_shoot && g.burst.is_none() {
            if gd.pellets.is_some() && g.shell_state != 0 {
                g.shell_state = 0; // firing interrupts a shotgun reload
            }
            fire(&mut g, gd, c, &mut out, &mut punch);
            if gd.pellets.is_some() && g.clip == 0 && out.dry_fire {
                g.next_primary = c.now + 0.15;
            }
        } else if c.reload && !c.attack && !g.reloading && g.burst.is_none() {
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

fn fire(g: &mut GunState, gd: &data::Gun, c: &Context, out: &mut Fired, punch: &mut [f32; 3]) {
    {
        let mode = gd.modes[g.mode as usize];
        if g.clip == 0 {
            out.dry_fire = true;
            g.next_primary = c.now + 0.2;
            return;
        }
        // Semi-auto: one shot per click (shots_fired resets when the button is released).
        let semi = gd.semi_auto && mode.burst.is_none();
        if semi {
            g.shots_fired += 1;
            if g.shots_fired > 1 {
                return;
            }
        }
        let since_last = if g.last_fire > 0.0 { Some((c.now - g.last_fire) as f32) } else { None };

        // Spread from the accuracy left by the previous shot (G3SG1 updates first).
        let mut acc_before = g.accuracy;
        if let (Accuracy::SniperTime { base, k, cap, first_shot, .. }, Spread::Sniper { rule: SniperRule::G3, .. }) = (gd.accuracy, mode.spread) {
            g.accuracy = match since_last {
                Some(dt) => (dt * k + base).min(cap),
                None => first_shot.unwrap_or(g.accuracy),
            };
            acc_before = g.accuracy;
        }
        let spread = spread(&mode.spread, acc_before, c, g.zoom > 0);

        // Accuracy for the next shot.
        if !semi {
            g.delay_fire = true;
            g.shots_fired += 1;
        }
        match gd.accuracy {
            Accuracy::Auto { power, div, fdiv, add, cap, .. } => {
                let n = g.shots_fired;
                let p = if power == 3 { n * n * n } else { n * n };
                let step = if div > 0 { (p / div) as f32 } else { p as f32 / fdiv };
                g.accuracy = (step + add).min(cap);
            }
            Accuracy::PistolTime { k, s, lo, hi, .. } => {
                if let Some(dt) = since_last {
                    g.accuracy = (g.accuracy - (k - dt) * s).clamp(lo, hi);
                }
            }
            Accuracy::SniperTime { base, k, cap, .. } if !matches!(mode.spread, Spread::Sniper { rule: SniperRule::G3, .. }) => {
                if let Some(dt) = since_last {
                    g.accuracy = (dt * k + base).min(cap);
                }
            }
            _ => {}
        }
        g.last_fire = c.now;

        g.clip -= 1;
        let side_offset = if g.id == Id::Elite { if g.clip % 2 == 0 { -5.0 } else { 5.0 } } else { 0.0 };
        out.shots.push(Shot {
            weapon: g.id,
            damage: mode.damage,
            range_mod: mode.range_mod,
            penetration: gd.penetration,
            distance: gd.distance,
            bullet: gd.bullet,
            spread,
            angles: [c.pitch + punch[0], c.yaw + punch[1]],
            side_offset,
            seed: c.seed,
            pellets: gd.pellets,
        });
        let cycle = if g.zoom > 0 { gd.zoomed_cycle.unwrap_or(mode.cycle) } else { mode.cycle };
        g.next_primary = c.now + cycle as f64;
        if let Some(b) = mode.burst {
            g.burst = Some((2, c.now + b.delays[0] as f64, spread));
        }
        if gd.unscope_on_fire && g.zoom > 0 {
            g.resume_zoom = g.zoom;
            g.zoom = 0;
        }
        kick(punch, g, gd, c);
    }
}

impl Loadout {
    fn knife(&mut self, c: &Context, out: &mut Fired) {
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

fn start_reload(g: &mut GunState, gd: &data::Gun, now: f64) -> Option<f64> {
    if g.reserve == 0 || g.clip >= gd.clip || now <= g.next_primary {
        return None;
    }
    g.zoom = 0;
    g.resume_zoom = 0;
    if gd.pellets.is_some() {
        if g.shell_state == 0 {
            g.shell_state = 1;
            g.shell_at = now + 0.55;
            g.next_primary = now + 0.55;
        }
        return None;
    }
    g.reloading = true;
    g.shots_fired = 0;
    g.delay_fire = false;
    if let Accuracy::Auto { on_reload, .. } = gd.accuracy {
        g.accuracy = on_reload;
    } else if let Accuracy::PistolTime { initial, .. } = gd.accuracy {
        g.accuracy = initial;
    } else if let Accuracy::SniperTime { on_reload: Some(a), .. } = gd.accuracy {
        g.accuracy = a;
    }
    Some(now + gd.reload as f64)
}

fn shotgun_reload_step(g: &mut GunState, gd: &data::Gun, p: &data::Pellets, now: f64) {
    if g.shell_state == 0 || now < g.shell_at {
        return;
    }
    match g.shell_state {
        1 => {
            g.shell_state = 2;
            g.shell_at = now + p.shell_time as f64;
        }
        _ => {
            if g.reserve > 0 && g.clip < gd.clip {
                g.clip += 1;
                g.reserve -= 1;
            }
            g.shell_state = if g.clip < gd.clip && g.reserve > 0 { 1 } else { 0 };
            g.shell_at = now;
        }
    }
}

/// The spread number for a shot in the current movement state.
fn spread(s: &Spread, acc: f32, c: &Context, zoomed: bool) -> f32 {
    let air = !c.on_ground;
    match *s {
        Spread::Rifle { air: a, fast_speed, fast, still } => {
            let (b, k) = if air {
                a
            } else if c.speed_2d > fast_speed {
                fast
            } else {
                still
            };
            b + k * acc
        }
        Spread::Smg { air: a, fast, still } => {
            if air {
                a * acc
            } else if let Some((speed, k)) = fast.filter(|(s, _)| c.speed_2d > *s) {
                let _ = speed;
                k * acc
            } else {
                still * acc
            }
        }
        Spread::Pistol { air: a, moving, duck, stand } => {
            let k = if air {
                a
            } else if c.speed_2d > 0.0 {
                moving
            } else if c.crouched {
                duck
            } else {
                stand
            };
            k * (1.0 - acc)
        }
        Spread::Sniper { air: a, moving, duck, stand, unscoped, rule } => {
            let moving_value = moving.iter().find(|(speed, _)| c.speed_2d > *speed).map(|(_, v)| *v);
            let (base, uses_acc) = if air {
                (a, true)
            } else if let Some(v) = moving_value {
                (v, false)
            } else if c.crouched {
                (duck, true)
            } else {
                (stand, true)
            };
            let extra = if zoomed { 0.0 } else { unscoped };
            match rule {
                SniperRule::Flat => base + extra,
                SniperRule::G3 => (base + extra) * (1.0 - acc),
                SniperRule::Sg550 => (if uses_acc { base * (1.0 - acc) } else { base }) + extra,
            }
        }
    }
}

/// Recoil of one main shot (burst follow-ups don't kick).
fn kick(punch: &mut [f32; 3], g: &mut GunState, gd: &data::Gun, c: &Context) {
    match gd.recoil {
        Recoil::Kick { smg_order, sets } => {
            let moving = c.speed_2d > 0.0;
            let air = !c.on_ground;
            let set = if smg_order {
                if air {
                    sets[1]
                } else if moving {
                    sets[0]
                } else if c.crouched {
                    sets[2]
                } else {
                    sets[3]
                }
            } else if moving {
                sets[0]
            } else if air {
                sets[1]
            } else if c.crouched {
                sets[2]
            } else {
                sets[3]
            };
            let [up_base, lat_base, up_mod, lat_mod, up_max, lat_max, dir_change] = set;
            let n = g.shots_fired as f32;
            let (up, lat) = if g.shots_fired <= 1 { (up_base, lat_base) } else { (n * up_mod + up_base, n * lat_mod + lat_base) };
            punch[0] = (punch[0] - up).max(-up_max);
            if g.direction == 1 {
                punch[1] = (punch[1] + lat).min(lat_max);
            } else {
                punch[1] = (punch[1] - lat).max(-lat_max);
            }
            if rand_int(c.seed, 10, 0, dir_change as i32) == 0 {
                g.direction = 1 - g.direction;
            }
        }
        Recoil::Flat(p) => punch[0] -= p,
        Recoil::AutoSniper { lo, hi } => {
            punch[0] -= rand_range(c.seed, 4, lo, hi) + punch[0] * 0.25;
            punch[1] += rand_range(c.seed, 5, -0.75, 0.75);
        }
        Recoil::Shotgun { ground, air } => {
            let (lo, hi) = if c.on_ground { ground } else { air };
            punch[0] -= rand_int(c.seed, 1, lo, hi) as f32;
        }
        Recoil::None => {}
    }
}

/// Weapon state machine tests (no map needed).
#[cfg(test)]
mod tests;

/// Kind check helper used by the server.
pub fn is_gun(id: Id) -> bool {
    matches!(def(id).kind, Kind::Gun(_))
}
