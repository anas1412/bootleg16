//! Thrown grenades: GoldSrc toss/bounce physics, fuses, and what each one does
//! when it goes off (HE damage, flash blindness, smoke). docs/WEAPONS.md 2.8.
use crate::collision::World;
use crate::{add, dot, length, scale, sub, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    He,
    Flash,
    Smoke,
}

impl Kind {
    /// Gravity scale and friction (`ShootTimed`, `ShootSmokeGrenade`).
    fn physics(self) -> (f32, f32) {
        match self {
            Kind::He => (0.55, 0.7),
            Kind::Flash | Kind::Smoke => (0.5, 0.8),
        }
    }
}

pub const FUSE: f64 = 1.5;
const MAX_THROW_SPEED: f32 = 750.0;
pub const HE_DAMAGE: f32 = 100.0;
pub const HE_RADIUS: f32 = HE_DAMAGE * 3.5;
pub const FLASH_RADIUS: f32 = 1500.0;
/// How long a smoke cloud lasts (21 puffs, one per second).
pub const SMOKE_SECONDS: f64 = 21.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grenade {
    pub kind: Kind,
    pub owner: u8,
    pub origin: Vec3,
    pub velocity: Vec3,
    pub on_ground: bool,
    bounces: u32,
    pub detonate_at: f64,
}

/// Start a throw: GoldSrc's pitch remap and speed `(90 - pitch) * 6`, capped at 750,
/// launched from 16 units in front of the eye, plus the thrower's own velocity.
pub fn throw(kind: Kind, owner: u8, eye: Vec3, pitch: f32, yaw: f32, player_velocity: Vec3, now: f64) -> Grenade {
    let p = if pitch < 0.0 { -10.0 + pitch * (80.0 / 90.0) } else { -10.0 + pitch * (100.0 / 90.0) };
    let speed = ((90.0 - p) * 6.0).min(MAX_THROW_SPEED);
    let (sp, cp) = p.to_radians().sin_cos();
    let (sy, cy) = yaw.to_radians().sin_cos();
    let forward = [cp * cy, cp * sy, -sp];
    Grenade {
        kind,
        owner,
        origin: add(eye, scale(forward, 16.0)),
        velocity: add(scale(forward, speed), player_velocity),
        on_ground: false,
        bounces: 0,
        detonate_at: now + FUSE,
    }
}

fn clip(v: Vec3, n: Vec3, backoff: f32) -> Vec3 {
    let mut out = sub(v, scale(n, dot(v, n) * backoff));
    for c in &mut out {
        if c.abs() < 0.1 {
            *c = 0.0;
        }
    }
    out
}

impl Grenade {
    /// One tick of `MOVETYPE_BOUNCE` physics plus the grenade's own touch rule.
    /// Returns true when it hit something hard enough to make a bounce sound.
    pub fn physics(&mut self, dt: f32, world: &World, gravity: f32) -> bool {
        if self.velocity[2] > 0.0 {
            self.on_ground = false;
        }
        if self.on_ground && self.velocity == [0.0; 3] {
            return false;
        }
        let (grav, friction) = self.kind.physics();
        self.velocity[2] -= grav * gravity * dt;
        let t = world.trace_line(self.origin, add(self.origin, scale(self.velocity, dt)));
        self.origin = t.end;
        if t.fraction >= 1.0 || t.start_solid {
            return false;
        }
        // Touch (`BounceTouch`): rolling slows it; after 10 bounces it settles.
        // The bounce sound plays for the first five bounces in the air.
        let sound = !self.on_ground && self.bounces < 5;
        if self.on_ground {
            self.velocity = scale(self.velocity, 0.8);
        } else {
            self.bounces += 1;
            if self.bounces >= 10 {
                self.on_ground = true;
                self.velocity = [0.0; 3];
                return sound;
            }
        }
        // Bounce off the surface: friction 0.7 keeps 30% of the speed into it, 0.8 keeps 20%.
        self.velocity = clip(self.velocity, t.normal, 2.0 - friction);
        if t.normal[2] > 0.7 {
            if dot(self.velocity, self.velocity) < 900.0 {
                self.on_ground = true;
                self.velocity = [0.0; 3];
            } else {
                // Use up the rest of the move along the new direction.
                let rest = scale(self.velocity, (1.0 - t.fraction) * dt * 0.9);
                self.origin = world.trace_line(self.origin, add(self.origin, rest)).end;
            }
        }
        sound
    }

    /// Ready to go off: HE and flash on the fuse; smoke on the fuse once on the ground.
    pub fn due(&self, now: f64) -> bool {
        now >= self.detonate_at && (self.kind != Kind::Smoke || self.on_ground)
    }

    /// HE blast point: pulled 45.6 units off the surface below, if it's close (`Explode3`).
    pub fn blast_point(&self, world: &World) -> Vec3 {
        let t = world.trace_line(add(self.origin, [0.0, 0.0, 8.0]), sub(self.origin, [0.0, 0.0, 40.0]));
        if t.fraction < 1.0 {
            add(t.end, scale(t.normal, (HE_DAMAGE - 24.0) * 0.6))
        } else {
            self.origin
        }
    }
}

/// How much of a player at `origin` a blast at `blast` can see, 0..1 (Condition Zero's
/// `RadiusDamage`, later CS:GO's: the middle 0.4, the head and the feet 0.2 each, either side
/// 0.1). CS 1.6 had no such check, so an HE hurt through walls; CS2 doesn't.
pub fn blast_share(world: &World, blast: Vec3, origin: Vec3, crouched: bool) -> f32 {
    let from = add(blast, [0.0, 0.0, 1.0]);
    let clear = |to: Vec3| world.trace_line(from, to).fraction >= 1.0;
    let (dx, dy) = (origin[0] - from[0], origin[1] - from[1]);
    let l = (dx * dx + dy * dy).sqrt();
    let side = if l > 0.0 { [-dy / l * 13.0, dx / l * 13.0, 0.0] } else { [0.0, 13.0, 0.0] };
    let mut share = 0.0;
    for (to, w) in [
        (origin, 0.4),
        (add(origin, [0.0, 0.0, 25.0]), 0.2),
        (add(origin, [0.0, 0.0, if crouched { -14.0 } else { -34.0 }]), 0.2),
        (add(origin, side), 0.1),
        (sub(origin, side), 0.1),
    ] {
        if clear(to) {
            share += w;
        }
    }
    share
}

/// HE damage to someone standing at `victim_origin`, in the open (CS 1.6's falloff; times
/// `blast_share` for cover).
pub fn he_damage(blast: Vec3, victim_origin: Vec3) -> f32 {
    let d = length(sub(add(blast, [0.0, 0.0, 1.0]), victim_origin));
    (HE_DAMAGE - d * (HE_DAMAGE / HE_RADIUS)).max(0.0)
}

/// How hard a flash blinds someone (`RadiusFlash`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Blind {
    /// Seconds fully white, then seconds fading out.
    pub hold: f32,
    pub fade: f32,
    /// 255 when looking at it, 200 when facing away.
    pub alpha: u8,
}

/// `None` if the flash can't see the victim's eyes (wall in between) or is too far.
pub fn flash(world: &World, blast: Vec3, eye: Vec3, forward: Vec3) -> Option<Blind> {
    let from = add(blast, [0.0, 0.0, 1.0]);
    let blocked = |a: Vec3, b: Vec3| {
        let t = world.trace_line(a, b);
        t.fraction < 1.0 || t.start_solid
    };
    if blocked(from, eye) || blocked(eye, from) {
        return None;
    }
    let dist = length(sub(eye, from));
    if dist > FLASH_RADIUS {
        return None;
    }
    let adj = (4.0 - dist * (4.0 / FLASH_RADIUS)).max(0.0);
    if adj <= 0.0 {
        return None;
    }
    // How much they face it: CS2's (CS:GO's) bands, not CS 1.6's (which whites out anyone
    // even slightly facing it, and still 200 white for seconds behind). Strength as CS 1.6
    // (`RadiusFlash`: held adj / 1.5, fading 3 x adj, looking at it), shares as CS:GO's.
    let facing = dot(sub(from, eye), forward) / dist.max(1.0);
    let (share, alpha) = match facing {
        f if f >= 0.6 => (1.0, 255),
        f if f >= 0.3 => (0.65, 255),
        f if f >= -0.2 => (0.4, 255),
        _ => (0.2, 200),
    };
    Some(Blind { hold: adj / 1.5 * share, fade: 3.0 * adj * share, alpha })
}

/// A flash on someone still blinded by an earlier one (`RadiusFlash`): looking at it adds
/// the white hold that's left; while the old one lasts, the longer fade and the stronger white
/// win. `old`: when the last one started and how it was.
pub fn combine(new: Blind, old: Option<(f64, Blind)>, now: f64) -> Blind {
    let Some((start, old)) = old else { return new };
    let mut b = new;
    let hold_left = (start + old.hold as f64 - now) as f32;
    if hold_left > 0.0 && b.alpha == 255 {
        b.hold += hold_left;
    }
    if start + (old.fade + old.hold) as f64 > now {
        b.fade = b.fade.max(old.fade);
        b.alpha = b.alpha.max(old.alpha);
    }
    b
}

/// Flashes and HE blasts don't cross the water's surface (`RadiusFlash`, `RadiusDamage`): one
/// in water only reaches people in it (feet at least), one out of it nobody whose head is under.
pub fn blast_reaches(blast_in_water: bool, water_level: u8) -> bool {
    if blast_in_water {
        water_level > 0
    } else {
        water_level < 3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 128.0;

    #[test]
    fn level_throw_speed_and_landing() {
        // Looking level: pitch remaps to -10, speed (90 + 10) * 6 = 600.
        let g = throw(Kind::He, 0, [0.0, 0.0, 100.0], 0.0, 0.0, [0.0; 3], 0.0);
        assert!((length(g.velocity) - 600.0).abs() < 0.01);
        assert!(g.velocity[2] > 0.0, "a level throw goes slightly upwards");
        // Looking down at 45 degrees throws slower and lower; straight up caps at 750.
        let down = throw(Kind::He, 0, [0.0; 3], 45.0, 0.0, [0.0; 3], 0.0);
        assert!(length(down.velocity) < 600.0);
        assert_eq!(length(throw(Kind::He, 0, [0.0; 3], -89.0, 0.0, [0.0; 3], 0.0).velocity), 750.0);

        let w = World::flat_floor(0.0);
        let mut g = throw(Kind::Flash, 0, [0.0, 0.0, 64.0], 0.0, 0.0, [0.0; 3], 0.0);
        let mut t = 0.0;
        while !g.on_ground && t < 10.0 {
            g.physics(DT, &w, 800.0);
            t += DT;
            if g.origin[2] < -0.01 {
                panic!("fell through the floor at t={t}: {:?} v={:?}", g.origin, g.velocity);
            }
        }
        assert!(g.on_ground && g.velocity == [0.0; 3], "came to rest");
        assert!(g.origin[0] > 200.0, "travelled {:?}", g.origin);
    }

    #[test]
    fn a_second_flash_adds_to_the_first() {
        let full = Blind { hold: 2.0, fade: 6.0, alpha: 255 };
        let away = Blind { hold: 0.5, fade: 2.0, alpha: 200 };
        assert_eq!(combine(full, None, 10.0), full);
        // 1 s into a 2 s hold: 1 s left is added; fade and white stay the stronger.
        assert_eq!(combine(full, Some((9.0, full)), 10.0), Blind { hold: 3.0, fade: 6.0, alpha: 255 });
        // Facing away from the second: no hold added, but the first's white and fade carry on.
        assert_eq!(combine(away, Some((9.0, full)), 10.0), Blind { hold: 0.5, fade: 6.0, alpha: 255 });
        // Long over: the new one alone.
        assert_eq!(combine(away, Some((0.0, full)), 10.0), away);
        assert!(blast_reaches(false, 2) && !blast_reaches(false, 3));
        assert!(blast_reaches(true, 1) && !blast_reaches(true, 0));
    }

    #[test]
    fn he_falloff_and_flash_rules() {
        assert_eq!(he_damage([0.0; 3], [0.0, 0.0, 1.0]), 100.0);
        assert!((he_damage([0.0; 3], [175.0, 0.0, 1.0]) - 50.0).abs() < 0.01);
        assert_eq!(he_damage([0.0; 3], [400.0, 0.0, 1.0]), 0.0);
        // In the open the blast sees all of you; under the floor none.
        let ground = World::flat_floor(-100.0);
        assert!((blast_share(&ground, [0.0, 0.0, -50.0], [100.0, 0.0, -50.0], false) - 1.0).abs() < 1e-5);
        assert_eq!(blast_share(&ground, [0.0, 0.0, -50.0], [100.0, 0.0, -150.0], false), 0.0);
        let w = World::flat_floor(-100.0);
        let eye = [100.0, 0.0, 50.0];
        let facing = flash(&w, [0.0, 0.0, 49.0], eye, [-1.0, 0.0, 0.0]).unwrap();
        let away = flash(&w, [0.0, 0.0, 49.0], eye, [1.0, 0.0, 0.0]).unwrap();
        let side = flash(&w, [0.0, 0.0, 49.0], eye, [0.0, 1.0, 0.0]).unwrap();
        assert_eq!(facing.alpha, 255);
        assert!(facing.hold > side.hold && side.hold > away.hold);
        // CS2: behind you it's only a short flash, even close
        assert!(away.hold < 0.6 && away.fade < 2.5 && away.alpha == 200, "{away:?}");
        assert!(flash(&w, [0.0, 0.0, 49.0], [2000.0, 0.0, 50.0], [-1.0, 0.0, 0.0]).is_none(), "too far");
        // Behind the floor (no line of sight).
        assert!(flash(&w, [0.0, 0.0, -150.0], eye, [-1.0, 0.0, 0.0]).is_none());
    }

    #[test]
    fn smoke_waits_for_the_ground() {
        let mut g = throw(Kind::Smoke, 0, [0.0, 0.0, 2000.0], 0.0, 0.0, [0.0; 3], 0.0);
        assert!(!g.due(2.0), "still in the air after the fuse");
        g.on_ground = true;
        assert!(g.due(2.0));
    }
}
