//! Weapon behaviour: firing timing, ammo and reloads, accuracy, spread, recoil,
//! fire modes, scopes, switching. Follows docs/WEAPONS.md. Bullets are traced in
//! `bullets`; this module decides when and how each shot happens.
pub mod bullets;
pub mod data;
mod carried;
mod firing;
mod update;

pub use data::{def, gun, Id, Slot};
use data::{Accuracy, Kind, Recoil, Secondary, Spread};
use firing::{fire, shotgun_reload_step, spread, start_reload};
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
    /// Index into the gun's modes (silencer on).
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
        }
    }

    /// A shotgun is loading shells one by one.
    pub fn loading_shells(&self) -> bool {
        self.shell_state != 0
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

/// The most of each grenade you may carry: HE, flash, smoke (CS 1.6).
pub const GRENADE_MAX: [u8; 3] = [1, 2, 1];

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
    /// Grenade in hand: index into `grenades` (0 HE, 1 flash, 2 smoke).
    pub grenade: u8,
    /// Pin pulled; the throw happens on release once this time has passed.
    pin_until: Option<f64>,
    grenade_next: f64,
    /// Planting the bomb since this time (attack held with the C4 out, in a bomb site).
    pub planting: Option<f64>,
}

/// Seconds of holding attack to plant the bomb (CS 1.6 C4 arming time).
pub const C4_PLANT_TIME: f64 = 3.0;

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
    pub seed: u64,
    pub pellets: Option<data::Pellets>,
    /// M4A1/USP with the silencer on (quieter, different sound).
    pub silenced: bool,
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
    /// The bomb went down (planting finished this tick).
    pub planted: bool,
    /// A grenade left the hand: which, and the view angles (pitch, yaw) incl. punch.
    pub throw: Option<(crate::grenades::Kind, [f32; 2])>,
    /// A gun was dropped (G), with its ammo.
    pub dropped: Option<GunState>,
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
    /// Slot key pressed this tick (0 = none; see `pmove::SLOT_*`).
    pub slot: u8,
    /// G pressed this tick.
    pub drop: bool,
    /// Freeze time etc.: no shooting.
    pub can_shoot: bool,
    /// Standing in a bomb site (the C4 can be planted).
    pub in_bomb_site: bool,
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
            grenade: 0,
            pin_until: None,
            grenade_next: 0.0,
            planting: None,
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

    /// Add a weapon: a gun replaces what's in its slot and is drawn; grenades stack
    /// up to their carry limit (1 HE, 2 flash, 1 smoke). Returns false if it didn't fit.
    pub fn give(&mut self, id: Id, now: f64) -> bool {
        match id {
            Id::HeGrenade | Id::Flashbang | Id::Smoke => {
                let i = match id {
                    Id::HeGrenade => 0,
                    Id::Flashbang => 1,
                    _ => 2,
                };
                if self.grenades[i] >= GRENADE_MAX[i] {
                    return false;
                }
                self.grenades[i] += 1;
                return true;
            }
            Id::C4 => {
                self.c4 = true;
                return true;
            }
            Id::Knife => {
                self.knife = true;
                return true;
            }
            _ => {}
        }
        let g = GunState::new(id);
        match def(id).slot {
            Slot::Primary => self.primary = Some(g),
            Slot::Secondary => self.secondary = Some(g),
            _ => return false,
        }
        self.switch_to(def(id).slot, now);
        true
    }

    /// Take the gun in hand out of the inventory (G, or on death) and draw the best
    /// remaining weapon. Knife and grenades can't be dropped.
    pub fn drop_active(&mut self, now: f64) -> Option<GunState> {
        let gun = match self.active {
            Slot::Primary => self.primary.take(),
            Slot::Secondary => self.secondary.take(),
            _ => None,
        }?;
        let mut g = gun;
        g.reloading = false;
        g.zoom = 0;
        g.resume_zoom = 0;
        g.shell_state = 0;
        let best = [Slot::Primary, Slot::Secondary, Slot::Knife, Slot::Grenade].into_iter().find(|s| self.has(*s)).unwrap_or(Slot::Knife);
        self.active = Slot::Knife; // nothing in hand to holster
        self.switch_to(best, now);
        Some(g)
    }

    /// Pick up a dropped gun if its slot is free (keeps its ammo). Returns false if not.
    pub fn pick_up(&mut self, g: GunState) -> bool {
        let slot = match def(g.id).slot {
            Slot::Primary => &mut self.primary,
            Slot::Secondary => &mut self.secondary,
            _ => return false,
        };
        if slot.is_some() {
            return false;
        }
        *slot = Some(g);
        true
    }

    /// Fill the reserve ammo of the primary (or pistol) to the most it can carry.
    pub fn refill_ammo(&mut self, primary: bool) {
        let g = if primary { self.primary.as_mut() } else { self.secondary.as_mut() };
        if let Some(g) = g {
            g.reserve = gun(g.id).map(|d| d.reserve).unwrap_or(g.reserve);
        }
    }

    /// The pin is pulled on the grenade in hand.
    pub fn pin_pulled(&self) -> bool {
        self.pin_until.is_some()
    }

    /// The grenade type in hand (or that slot 4 would draw).
    pub fn grenade_id(&self) -> Option<Id> {
        let kinds = [Id::HeGrenade, Id::Flashbang, Id::Smoke];
        (0..3).map(|k| (self.grenade as usize + k) % 3).find(|&i| self.grenades[i] > 0).map(|i| kinds[i])
    }

    /// Spread the next shot would have in this movement state (for the crosshair).
    pub fn spread_now(&self, on_ground: bool, crouched: bool, speed_2d: f32) -> Option<f32> {
        let g = self.active_gun()?;
        let gd = gun(g.id)?;
        let c = Context { now: 0.0, dt: 0.0, on_ground, crouched, speed_2d, yaw: 0.0, pitch: 0.0, attack: false, attack2: false, reload: false, slot: 0, drop: false, can_shoot: true, in_bomb_site: false, seed: 0 };
        Some(spread(&gd.modes[g.mode as usize].spread, g.accuracy, &c, g.zoom > 0))
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
        match self.active {
            Slot::Grenade => self.grenade_id().unwrap_or(Id::Knife),
            Slot::Bomb => Id::C4,
            _ => self.active_gun().map(|g| g.id).unwrap_or(Id::Knife),
        }
    }

    /// Movement speed with the current weapon.
    pub fn max_speed(&self) -> f32 {
        // You can't move while planting.
        if self.planting.is_some() {
            return 1.0;
        }
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

    /// Holster the current weapon (cancels a reload) and deploy another.
    pub fn switch_to(&mut self, slot: Slot, now: f64) {
        if !self.has(slot) {
            return;
        }
        if let Some(g) = self.active_gun_mut() {
            g.reloading = false;
            g.shell_state = 0;
            g.zoom = 0;
            g.resume_zoom = 0;
        }
        self.pin_until = None;
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
}

/// Weapon state machine tests (no map needed).
#[cfg(test)]
mod tests;

/// Kind check helper used by the server.
pub fn is_gun(id: Id) -> bool {
    matches!(def(id).kind, Kind::Gun(_))
}
