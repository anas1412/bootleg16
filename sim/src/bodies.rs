//! Dead bodies (our addition: CS only lets them lie). They stay the whole match, lie where
//! they fell as a low bump you step over, and anyone alive can pick one up with E (use) and
//! carry it in front of them as a shield: it takes most of a bullet that would go through it,
//! but you move slower. E again (or dying) drops it. How they die (the animation, and how the
//! shot throws them) is CS 1.6's `CBasePlayer::SetAnimation(PLAYER_DIE)` and `Killed`.
use crate::collision::{Hull, World};
use crate::hitbox::HitGroup;
use crate::{add, dot, length, scale, sub, Vec3};

/// Carrying a body: this share of your normal speed.
pub const CARRY_SPEED: f32 = 0.6;
/// A bullet through the body you carry keeps this share of its damage.
pub const SHIELD_PASS: f32 = 0.25;
/// How far you reach for a body (your centre to the body's), and how straight at it you
/// must look (cosine), like CS's use radius.
pub const GRAB_REACH: f32 = 80.0;
pub const GRAB_LOOK: f32 = 0.7;
/// A carried body hangs this far in front of your centre, and this far lower (its head
/// under your eyes: you see over it, and your own head stays open).
const CARRY_AHEAD: f32 = 28.0;
const CARRY_DROP: f32 = 16.0;
/// A body on the floor: half its width, and how high it stands (under the 18 step height,
/// so you walk over it).
const REST_HALF: f32 = 24.0;
const REST_HEIGHT: f32 = 12.0;
/// The shield: depth, half width, half height around the carried body's centre.
const SHIELD: Vec3 = [8.0, 16.0, 34.0];

/// The death animations (player model sequence names), CS's activities: ACT_DIE_HEADSHOT
/// ("head"), _CHESTSHOT ("death1"), _GUTSHOT ("gutshot"), _BACKSHOT ("death3"), ACT_DIESIMPLE
/// ("death2" twice as likely as "left" or "right"), crouched, and the throws.
pub const DEATHS: [&str; 10] = ["crouch_die", "left", "head", "death1", "gutshot", "right", "death3", "back", "forward", "death2"];
const CROUCH: u8 = 0;
const LEFT: u8 = 1;
const HEAD: u8 = 2;
const CHEST: u8 = 3;
const GUT: u8 = 4;
const RIGHT: u8 = 5;
const BACKSHOT: u8 = 6;
const BACK: u8 = 7;
const FORWARD: u8 = 8;
const SIMPLE: u8 = 9;

fn die_simple(roll: u32) -> u8 {
    [SIMPLE, SIMPLE, LEFT, RIGHT][roll as usize % 4]
}

/// What a death knows: how they stood, what hit them last and from where.
#[derive(Clone, Copy, Debug, Default)]
pub struct Death {
    pub at: Vec3,
    /// Which way they faced (degrees).
    pub yaw: f32,
    pub crouched: bool,
    pub last_hit: Option<HitGroup>,
    pub last_damage: f32,
    /// The killer's eyes.
    pub killer: Option<Vec3>,
    /// An explosion did it: where, and whether it was the bomb.
    pub blast: Option<(Vec3, bool)>,
}

/// How a body is thrown when they die (`CBasePlayer::Killed`'s `m_iThrowDirection`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Throw {
    None,
    /// Their own forward or back: 100-200 a second, and up 50-100.
    Forward,
    Back,
    /// Along the killer's aim, 200-300 (and up 200-300; flat for some arm shots).
    Shooter,
    ShooterFlat,
    /// Away from the grenade (500 less its distance, up 1.5 x 350 less it) or the bomb
    /// (2300 less its distance, a quarter of it, up a 2.75th).
    Blast,
}

/// The death animation (index in `DEATHS`) and the body's starting velocity. `roll` is any
/// random number.
pub fn death(d: &Death, roll: u32) -> (u8, Vec3) {
    // the blast behind them throws them forward ("left"), in front of them back
    let blast = d.blast.map(|(b, _)| {
        let (s, c) = d.yaw.to_radians().sin_cos();
        c * (d.at[0] - b[0]) + s * (d.at[1] - b[1]) > 0.0
    });
    let big = d.last_damage > 20.0;
    let (anim, throw) = match (d.crouched, blast, d.last_hit) {
        (true, _, _) => (CROUCH, Throw::Back),
        (_, Some(true), _) => (LEFT, Throw::Blast),
        (_, Some(false), _) => (if roll % 2 == 0 { HEAD } else { CROUCH }, Throw::Blast),
        (_, _, Some(HitGroup::Head)) => (HEAD, match roll % 9 + big as u32 {
            1 | 2 => Throw::Back,
            3 | 4 => Throw::Forward,
            5 | 6 => Throw::Shooter,
            _ => Throw::None,
        }),
        (_, _, Some(HitGroup::Chest)) => (CHEST, Throw::None),
        (_, _, Some(HitGroup::Stomach)) => (GUT, Throw::None),
        (_, _, Some(HitGroup::LeftArm)) => (LEFT, Throw::None),
        (_, _, Some(HitGroup::RightArm)) => (RIGHT, if roll % 2 == 0 { Throw::Shooter } else { Throw::ShooterFlat }),
        (_, _, Some(HitGroup::Generic) | None) => match roll % 9 {
            0 => (HEAD, Throw::Back),
            8 => (HEAD, Throw::None),
            1 | 7 => (GUT, Throw::None),
            2 => (BACKSHOT, Throw::Shooter),
            4 => (BACK, Throw::Shooter),
            5 => (FORWARD, Throw::Forward),
            6 => (CHEST, Throw::None),
            _ => (die_simple(roll / 9), Throw::None),
        },
        _ => (die_simple(roll / 9), Throw::None),
    };
    let r = |k: u32, lo: f32, hi: f32| lo + (hi - lo) * ((roll.wrapping_mul(k) >> 8) % 1000) as f32 / 1000.0;
    (anim, throw_velocity(throw, d, r))
}

fn throw_velocity(throw: Throw, d: &Death, r: impl Fn(u32, f32, f32) -> f32) -> Vec3 {
    let (s, c) = d.yaw.to_radians().sin_cos();
    let aim = d.killer.map(|k| flat_dir(sub(d.at, k))).unwrap_or([0.0; 3]);
    match throw {
        Throw::None => [0.0; 3],
        Throw::Forward => [c * r(3, 100.0, 200.0), s * r(3, 100.0, 200.0), r(5, 50.0, 100.0)],
        Throw::Back => [-c * r(3, 100.0, 200.0), -s * r(3, 100.0, 200.0), r(5, 50.0, 100.0)],
        Throw::Shooter => add(scale(aim, r(3, 200.0, 300.0)), [0.0, 0.0, r(5, 200.0, 300.0)]),
        Throw::ShooterFlat => scale(aim, r(3, 200.0, 300.0)),
        Throw::Blast => {
            let Some((b, bomb)) = d.blast else { return [0.0; 3] };
            let v = sub(d.at, b);
            let dist = length(v);
            let dir = if dist > 1e-3 { scale(v, 1.0 / dist) } else { [0.0; 3] };
            if bomb {
                let k = 2300.0 - dist;
                [dir[0] * k * 0.25, dir[1] * k * 0.25, k / 2.75]
            } else {
                let k = 500.0 - dist;
                [dir[0] * k, dir[1] * k, (350.0 - dist) * 1.5]
            }
        }
    }
}

fn flat_dir(v: Vec3) -> Vec3 {
    let l = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if l > 1e-3 {
        [v[0] / l, v[1] / l, 0.0]
    } else {
        [0.0; 3]
    }
}

/// A body falling and sliding (gravity, then PlayerDeathThink's friction: 2000 a second off
/// its speed on the ground). `origin` is its centre as a player box of `hull`.
pub fn fall(origin: &mut Vec3, velocity: &mut Vec3, hull: Hull, world: &World, dt: f32, gravity: f32) {
    if *velocity == [0.0; 3] {
        return;
    }
    velocity[2] -= gravity * dt;
    let to = add(*origin, scale(*velocity, dt));
    let t = world.trace(hull, *origin, to);
    if t.fraction < 1.0 {
        let into = dot(*velocity, t.normal);
        *velocity = sub(*velocity, scale(t.normal, into));
    }
    *origin = t.end;
    let ground = world.trace(hull, t.end, [t.end[0], t.end[1], t.end[2] - 2.0]).fraction < 1.0;
    if ground {
        velocity[2] = velocity[2].max(0.0);
        let speed = length(*velocity);
        *velocity = if speed > 2000.0 * dt { scale(*velocity, 1.0 - 2000.0 * dt / speed) } else { [0.0; 3] };
    }
}

/// A body on the floor as something you bump into: a low box around where it lies.
pub fn rest_box(origin: Vec3, hull: Hull) -> (Vec3, Vec3) {
    let floor = origin[2] + hull.mins()[2];
    ([origin[0] - REST_HALF, origin[1] - REST_HALF, floor], [origin[0] + REST_HALF, origin[1] + REST_HALF, floor + REST_HEIGHT])
}

/// Where a carried body is (its centre): in front of its carrier.
pub fn carried_at(carrier: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    [carrier[0] + c * CARRY_AHEAD, carrier[1] + s * CARRY_AHEAD, carrier[2] - CARRY_DROP]
}

/// Whether the line `from`..`to` (a bullet) goes through the body carried by a player at
/// `carrier` facing `yaw` (a box turned with them).
pub fn shield_blocks(carrier: Vec3, yaw: f32, from: Vec3, to: Vec3) -> bool {
    let centre = carried_at(carrier, yaw);
    let (s, c) = yaw.to_radians().sin_cos();
    // into the shield's own frame: x forward, y left, z up
    let local = |p: Vec3| {
        let d = sub(p, centre);
        [c * d[0] + s * d[1], -s * d[0] + c * d[1], d[2]]
    };
    segment_hits_box(local(from), local(to), scale(SHIELD, -1.0), SHIELD)
}

/// Whether a line segment crosses an axis-aligned box (slab test).
pub fn segment_hits_box(a: Vec3, b: Vec3, lo: Vec3, hi: Vec3) -> bool {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for i in 0..3 {
        let d = b[i] - a[i];
        if d.abs() < 1e-6 {
            if a[i] < lo[i] || a[i] > hi[i] {
                return false;
            }
            continue;
        }
        let (mut near, mut far) = ((lo[i] - a[i]) / d, (hi[i] - a[i]) / d);
        if near > far {
            std::mem::swap(&mut near, &mut far);
        }
        t0 = t0.max(near);
        t1 = t1.min(far);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// Whether a player could pick this body up: close, and looking at it.
pub fn can_grab(player: Vec3, eye: Vec3, forward: Vec3, body: Vec3, hull: Hull) -> bool {
    let (lo, hi) = rest_box(body, hull);
    let centre = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
    let to = sub(centre, eye);
    let d = length(to);
    length(sub(centre, player)) <= GRAB_REACH && (d < 1.0 || dot(to, forward) / d > GRAB_LOOK)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies_are_thrown_the_way_cs_throws_them() {
        let mid = |_: u32, lo: f32, hi: f32| (lo + hi) / 2.0;
        let d = Death { killer: Some([0.0; 3]), at: [100.0, 0.0, 0.0], ..Default::default() };
        // facing +x: back is -x and a little up
        let v = throw_velocity(Throw::Back, &d, mid);
        assert!(v[0] < -100.0 && v[2] > 50.0, "{v:?}");
        // along the killer's aim, away from them, and up
        let v = throw_velocity(Throw::Shooter, &d, mid);
        assert!(v[0] > 200.0 && v[2] > 200.0, "{v:?}");
        // a grenade 100 units west throws them east (400) and up (375)
        let d = Death { blast: Some(([-100.0, 0.0, 0.0], false)), ..Default::default() };
        let v = throw_velocity(Throw::Blast, &d, mid);
        assert!((v[0] - 400.0).abs() < 1e-3 && (v[2] - 375.0).abs() < 1e-3, "{v:?}");
    }

    #[test]
    fn the_carried_body_stops_shots_from_the_front_only() {
        // carrier at the origin facing +x; the body hangs 28 ahead
        let front = shield_blocks([0.0; 3], 0.0, [500.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        let back = shield_blocks([0.0; 3], 0.0, [-500.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        // your head (28 over your centre) shows over it
        let over = shield_blocks([0.0; 3], 0.0, [500.0, 0.0, 26.0], [0.0, 0.0, 26.0]);
        assert!(front && !back && !over);
        // turned to face +y, a shot from +y hits it
        assert!(shield_blocks([0.0; 3], 90.0, [0.0, 500.0, 0.0], [0.0, 0.0, 0.0]));
    }

    #[test]
    fn you_grab_a_body_you_look_at_within_reach() {
        let body = [50.0, 0.0, 0.0];
        let eye = [0.0, 0.0, 28.0];
        assert!(can_grab([0.0; 3], eye, [0.8, 0.0, -0.6], body, Hull::Standing));
        assert!(!can_grab([0.0; 3], eye, [-1.0, 0.0, 0.0], body, Hull::Standing));
        assert!(!can_grab([-200.0, 0.0, 0.0], [-200.0, 0.0, 28.0], [1.0, 0.0, 0.0], body, Hull::Standing));
    }
}
