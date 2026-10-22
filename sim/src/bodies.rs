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
/// A carried body hangs this far in front of your centre (less against a wall), and this
/// far lower (its head under your eyes: you see over it, and your own head stays open).
const CARRY_AHEAD: f32 = 28.0;
const CARRY_DROP: f32 = 8.0;
/// Half a carried body's depth (kept out of walls), and its centre over its feet.
const CARRY_HALF_DEPTH: f32 = 12.0;
const CARRY_FEET: f32 = 30.0;
/// A body on the floor: how far its box reaches past the points it lies on, and how high it
/// stands (under the 18 step height, so you walk over it).
const REST_PAD: f32 = 8.0;
const REST_HEIGHT: f32 = 12.0;
/// The shield: depth, half width, half height around the carried body's centre.
const SHIELD: Vec3 = [8.0, 16.0, 30.0];

/// Where each death animation leaves the body (its last frame, from the CS player model
/// `arctic.mdl`, which every character is animated on): head, pelvis, feet and hands, x/y
/// from where they died, facing +x. A body sprawls up to 80 units from that spot.
const SPRAWL: [[[f32; 2]; 6]; 10] = [
    [[-57.0, 33.0], [-38.0, 21.0], [-5.0, 20.0], [-18.0, -4.0], [-45.0, 56.0], [-73.0, 12.0]],
    [[53.0, 29.0], [32.0, 18.0], [0.0, 23.0], [8.0, -3.0], [24.0, 34.0], [42.0, 3.0]],
    [[-68.0, 7.0], [-45.0, 5.0], [-13.0, 7.0], [-15.0, -6.0], [-78.0, 24.0], [-81.0, -4.0]],
    [[-35.0, 36.0], [-28.0, 15.0], [-20.0, -17.0], [-4.0, -4.0], [-46.0, 35.0], [-20.0, 39.0]],
    [[45.0, -5.0], [23.0, -3.0], [-6.0, 11.0], [-8.0, -5.0], [27.0, 19.0], [19.0, -19.0]],
    [[14.0, -50.0], [9.0, -28.0], [19.0, -3.0], [3.0, -2.0], [13.0, -29.0], [25.0, -30.0]],
    [[41.0, -54.0], [32.0, -34.0], [20.0, -5.0], [5.0, -19.0], [59.0, -64.0], [24.0, -55.0]],
    [[-77.0, 12.0], [-54.0, 11.0], [-23.0, 17.0], [-24.0, -1.0], [-52.0, 30.0], [-51.0, -6.0]],
    [[73.0, -5.0], [51.0, 0.0], [20.0, 4.0], [19.0, -6.0], [59.0, 22.0], [48.0, -21.0]],
    [[-55.0, -14.0], [-34.0, -6.0], [-12.0, 9.0], [-3.0, -10.0], [-37.0, 12.0], [-40.0, -7.0]],
];

/// A body lying still: where it died (its centre as a player box), facing, and how it fell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lying {
    pub origin: Vec3,
    pub hull: Hull,
    pub yaw: f32,
    pub death: u8,
}

impl Lying {
    fn floor(&self) -> f32 {
        self.origin[2] + self.hull.mins()[2]
    }

    /// Head, pelvis, feet and hands on the floor, in the world.
    fn points(&self) -> [Vec3; 6] {
        let (s, c) = self.yaw.to_radians().sin_cos();
        let z = self.floor() + 6.0;
        let sprawl = SPRAWL[(self.death as usize).min(SPRAWL.len() - 1)];
        sprawl.map(|[x, y]| [self.origin[0] + c * x - s * y, self.origin[1] + s * x + c * y, z])
    }

    /// The low box you bump into: around where the body really lies.
    pub fn rest_box(&self) -> (Vec3, Vec3) {
        let pts = self.points();
        let lo = |i: usize| pts.iter().map(|p| p[i]).fold(f32::MAX, f32::min) - REST_PAD;
        let hi = |i: usize| pts.iter().map(|p| p[i]).fold(f32::MIN, f32::max) + REST_PAD;
        let floor = self.floor();
        ([lo(0), lo(1), floor], [hi(0), hi(1), floor + REST_HEIGHT])
    }

    /// Moved out of the walls: a body that would end up with its head or arms in a wall
    /// lies that much further from it (CS lets them sink in).
    pub fn out_of_walls(mut self, world: &World) -> Lying {
        for _ in 0..3 {
            let from = [self.origin[0], self.origin[1], self.floor() + 6.0];
            // the deepest way into a wall, and how far
            let worst = self
                .points()
                .iter()
                .filter_map(|p| {
                    let t = world.trace_line(from, *p);
                    (t.fraction < 1.0).then(|| {
                        let d = sub(*p, from);
                        scale(d, -(1.0 - t.fraction) - 4.0 / length(d).max(1.0))
                    })
                })
                .max_by(|a, b| length(*a).total_cmp(&length(*b)));
            let Some(push) = worst else { break };
            let to = add(self.origin, [push[0], push[1], 0.0]);
            self.origin = world.trace(self.hull, self.origin, to).end;
        }
        self
    }
}

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
/// its speed on the ground). `origin` is its centre as a player box of `hull` (CS 1.6: the
/// dead player tossed with their box, `MOVETYPE_TOSS`). True when it just stopped.
pub fn fall(origin: &mut Vec3, velocity: &mut Vec3, hull: Hull, world: &World, dt: f32, gravity: f32) -> bool {
    if *velocity == [0.0; 3] {
        return false;
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
    *velocity == [0.0; 3]
}

/// Where a carried body is (its centre): in front of its carrier, pulled in against a wall,
/// and standing on the floor (crouch behind it and it stays up).
pub fn carried_at(world: &World, carrier: Vec3, yaw: f32) -> Vec3 {
    let (s, c) = yaw.to_radians().sin_cos();
    let mid = [carrier[0], carrier[1], carrier[2] - CARRY_DROP];
    let reach = CARRY_AHEAD + CARRY_HALF_DEPTH;
    let t = world.trace_line(mid, [mid[0] + c * reach, mid[1] + s * reach, mid[2]]);
    let ahead = (t.fraction * reach - CARRY_HALF_DEPTH).clamp(0.0, CARRY_AHEAD);
    let mut at = [mid[0] + c * ahead, mid[1] + s * ahead, mid[2]];
    let down = world.trace_line(at, [at[0], at[1], at[2] - 64.0]);
    if down.fraction < 1.0 {
        at[2] = at[2].max(down.end[2] + CARRY_FEET);
    }
    at
}

/// Whether the line `from`..`to` (a bullet) goes through the body carried by a player at
/// `carrier` facing `yaw` (a box turned with them).
pub fn shield_blocks(world: &World, carrier: Vec3, yaw: f32, from: Vec3, to: Vec3) -> bool {
    let centre = carried_at(world, carrier, yaw);
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

/// Whether a player could pick this body up: close to its middle, and looking at it.
pub fn can_grab(player: Vec3, eye: Vec3, forward: Vec3, body: &Lying) -> bool {
    let (lo, hi) = body.rest_box();
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
        // carrier standing at the origin facing +x; the body hangs 28 ahead
        let w = World::flat_floor(-36.0);
        let front = shield_blocks(&w, [0.0; 3], 0.0, [500.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        let back = shield_blocks(&w, [0.0; 3], 0.0, [-500.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        // your head (28 over your centre) shows over it
        let over = shield_blocks(&w, [0.0; 3], 0.0, [500.0, 0.0, 26.0], [0.0, 0.0, 26.0]);
        assert!(front && !back && !over);
        // turned to face +y, a shot from +y hits it
        assert!(shield_blocks(&w, [0.0; 3], 90.0, [0.0, 500.0, 0.0], [0.0, 0.0, 0.0]));
        // crouched (centre 18 off the floor), the body still stands on the floor in front
        let at = carried_at(&w, [0.0, 0.0, -18.0], 0.0);
        assert!((at[2] - (-36.0 + CARRY_FEET)).abs() < 0.1, "{at:?}");
    }

    #[test]
    fn you_grab_a_body_you_look_at_within_reach() {
        // "forward": it lies 20..73 ahead of where they died
        let body = Lying { origin: [0.0; 3], hull: Hull::Standing, yaw: 0.0, death: 8 };
        let (lo, hi) = body.rest_box();
        assert!(lo[0] < 15.0 && hi[0] > 75.0, "{lo:?} {hi:?}");
        let eye = [0.0, 0.0, 28.0];
        assert!(can_grab([0.0; 3], eye, [0.8, 0.0, -0.6], &body));
        assert!(!can_grab([0.0; 3], eye, [-1.0, 0.0, 0.0], &body));
        assert!(!can_grab([-200.0, 0.0, 0.0], [-200.0, 0.0, 28.0], [1.0, 0.0, 0.0], &body));
    }

    /// `SPRAWL` is what the CS player model's death animations really end in.
    #[test]
    fn sprawl_matches_the_player_model() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../gamedata/models/player/arctic/arctic.mdl");
        let Ok(m) = goldsrc::mdl::Model::load(path) else { return };
        let names = ["Bip01 Head", "Bip01 Pelvis", "Bip01 L Foot", "Bip01 R Foot", "Bip01 L Hand", "Bip01 R Hand"];
        for (d, name) in DEATHS.iter().enumerate() {
            let q = m.sequence(name).unwrap();
            let pose = m.pose_at(q, m.sequences[q].frames as f32 - 1.0, 0.5);
            for (k, bone) in names.iter().enumerate() {
                let p = pose[m.bones.iter().position(|b| b.name == *bone).unwrap()].pos;
                let want = SPRAWL[d][k];
                assert!((p[0] - want[0]).abs() < 1.5 && (p[1] - want[1]).abs() < 1.5, "{name} {bone}: {p:?} vs {want:?}");
            }
        }
    }
}
