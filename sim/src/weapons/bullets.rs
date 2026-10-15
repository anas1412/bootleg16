//! Tracing shots through the map and players: CS 1.6 `FireBullets3` (penetration,
//! per-segment range falloff, surface materials) and the shotgun pellet volley.
use super::{rand01, Shot};
use crate::collision::World;
use crate::combat::hitgroup_multiplier;
use crate::hitbox::{HitGroup, Hitboxes};
use crate::{add, length, scale, sub, Vec3};
use goldsrc::materials::Materials;

/// Another player a bullet can hit (positions already rewound for lag compensation).
#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub id: u8,
    pub origin: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub crouched: bool,
}

/// Damage reaching a player, after the hit group multiplier (armor comes later).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BulletHit {
    pub victim: u8,
    pub damage: f32,
    pub group: HitGroup,
}

/// Where a bullet hit the world (for decals and sparks).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ShotResult {
    pub hits: Vec<BulletHit>,
    pub impacts: Vec<Impact>,
}

/// GoldSrc view vectors (forward, right, up) for pitch/yaw in degrees.
pub fn view_vectors(pitch: f32, yaw: f32) -> (Vec3, Vec3, Vec3) {
    let (sp, cp) = pitch.to_radians().sin_cos();
    let (sy, cy) = yaw.to_radians().sin_cos();
    ([cp * cy, cp * sy, -sp], [sy, -cy, 0.0], [sp * cy, sp * sy, cp])
}

/// The nearest thing along `src -> end`: a player (id, group, fraction) or the world.
fn nearest(world: &World, hitboxes: &Hitboxes, src: Vec3, end: Vec3, targets: &[Target], skip: &[u8]) -> (f32, Option<(u8, HitGroup)>, crate::collision::LineTrace) {
    let wt = world.trace_line(src, end);
    if wt.start_solid {
        // Restarted inside a wall that's thicker than the penetration step: GoldSrc
        // reports an immediate hit, which costs another penetration.
        return (0.0, None, wt);
    }
    let seg = sub(end, src);
    let len = length(seg);
    let dir = scale(seg, 1.0 / len.max(1e-6));
    let mut best = (wt.fraction, None);
    for t in targets.iter().filter(|t| !skip.contains(&t.id)) {
        if let Some(h) = hitboxes.trace(t.origin, t.yaw, t.pitch, t.crouched, src, dir, len * best.0) {
            best = (h.distance / len, Some((t.id, h.group)));
        }
    }
    (best.0, best.1, wt)
}

/// One `FireBullets3` bullet. `eye` is the shooter's eye; `shooter` is never hit.
pub fn fire_bullet(world: &World, materials: &Materials, hitboxes: &Hitboxes, eye: Vec3, shot: &Shot, shooter: u8, targets: &[Target]) -> ShotResult {
    let (fwd, right, up) = view_vectors(shot.angles[0], shot.angles[1]);
    let mut src = add(eye, scale(right, shot.side_offset));
    // Triangular spread on each axis (sum of two uniforms), not normalised.
    let x = rand01(shot.seed, 0) - 0.5 + rand01(shot.seed, 1) - 0.5;
    let y = rand01(shot.seed, 2) - 0.5 + rand01(shot.seed, 3) - 0.5;
    let dir = add(fwd, add(scale(right, x * shot.spread), scale(up, y * shot.spread)));

    let mut out = ShotResult::default();
    let mut dmg = shot.damage;
    let mut power = shot.bullet.power;
    let mut pen_left = shot.penetration;
    let mut dist = shot.distance;
    let mut dmg_mod = 0.5f32;
    let mut end = add(src, scale(dir, dist));
    let mut hit_players: Vec<u8> = vec![shooter];

    while pen_left != 0 {
        let mut skip = hit_players.clone();
        skip.dedup();
        let (fraction, player, wt) = nearest(world, hitboxes, src, end, targets, &skip);
        if fraction >= 1.0 {
            break;
        }
        // Surface material changes how well the bullet carries on (players count as flesh).
        if player.is_none() {
            let tex = world.texture_at(src, end).map(|t| materials.kind(t)).unwrap_or('C');
            match tex {
                'M' => {
                    power = (power as f32 * 0.15) as i32;
                    dmg_mod = 0.2;
                }
                'C' => power = (power as f32 * 0.25) as i32,
                'G' => {
                    power = (power as f32 * 0.5) as i32;
                    dmg_mod = 0.4;
                }
                'V' => {
                    power = (power as f32 * 0.5) as i32;
                    dmg_mod = 0.45;
                }
                'T' => {
                    power = (power as f32 * 0.65) as i32;
                    dmg_mod = 0.3;
                }
                'P' => {
                    power = (power as f32 * 0.4) as i32;
                    dmg_mod = 0.45;
                }
                'W' => dmg_mod = 0.6,
                _ => {}
            }
        }
        pen_left -= 1;
        let seg = fraction * dist;
        dmg = (dmg as f32 * shot.range_mod.powf(seg / 500.0)) as i32;
        if seg > shot.bullet.max_pen_distance {
            pen_left = 0;
        }
        let hit_point = add(src, scale(sub(end, src), fraction));
        if player.is_none() {
            out.impacts.push(Impact { point: hit_point, normal: wt.normal });
            if wt.hit_sky {
                break; // nothing behind the sky
            }
        }
        if let Some((_, HitGroup::Shield)) = player {
            break; // shields stop bullets
        }
        let dist_mod = if player.is_some() || pen_left == 0 {
            power = 42;
            dmg_mod = 0.75;
            0.75
        } else {
            0.5
        };
        if let Some((id, group)) = player {
            out.hits.push(BulletHit { victim: id, damage: dmg as f32 * hitgroup_multiplier(group), group });
            hit_players.push(id);
        }
        src = add(hit_point, scale(dir, power as f32));
        dist = (dist - seg) * dist_mod;
        end = add(src, scale(dir, dist));
        dmg = (dmg as f32 * dmg_mod) as i32;
    }
    out
}

/// Shotgun volley (old `FireBullets`): no penetration, linear falloff, damage
/// summed per victim (armor is then applied once to the sum).
pub fn fire_pellets(world: &World, hitboxes: &Hitboxes, eye: Vec3, shot: &Shot, shooter: u8, targets: &[Target]) -> ShotResult {
    let p = shot.pellets.expect("pellet shot");
    let (fwd, right, up) = view_vectors(shot.angles[0], shot.angles[1]);
    let mut out = ShotResult::default();
    for i in 0..p.count as u64 {
        // Circular cone: redraw until inside the unit circle.
        let (mut x, mut y);
        let mut k = 100 + i * 64;
        loop {
            x = rand01(shot.seed, k) - 0.5 + rand01(shot.seed, k + 1) - 0.5;
            y = rand01(shot.seed, k + 2) - 0.5 + rand01(shot.seed, k + 3) - 0.5;
            k += 4;
            if x * x + y * y <= 1.0 {
                break;
            }
        }
        let dir = add(fwd, add(scale(right, x * p.cone), scale(up, y * p.cone)));
        let end = add(eye, scale(dir, p.range));
        let (fraction, player, wt) = nearest(world, hitboxes, eye, end, targets, &[shooter]);
        if fraction >= 1.0 {
            continue;
        }
        let dmg = ((1.0 - fraction) * p.damage) as i32 as f32;
        match player {
            Some((id, group)) => {
                let d = dmg * hitgroup_multiplier(group);
                match out.hits.iter_mut().find(|h| h.victim == id) {
                    Some(h) => {
                        h.damage += d;
                        h.group = group;
                    }
                    None => out.hits.push(BulletHit { victim: id, damage: d, group }),
                }
            }
            None => out.impacts.push(Impact { point: add(eye, scale(sub(end, eye), fraction)), normal: wt.normal }),
        }
    }
    out
}

/// Knife reach test: a line along the view, then four rays offset by 8 units to
/// approximate GoldSrc's fallback hull sweep. Returns the player hit, if any.
pub fn trace_melee(world: &World, hitboxes: &Hitboxes, src: Vec3, end: Vec3, shooter: u8, targets: &[Target]) -> Option<(u8, HitGroup)> {
    let dir = sub(end, src);
    let (_, right, up) = {
        let l = length(dir).max(1e-6);
        let d = scale(dir, 1.0 / l);
        let yaw = d[1].atan2(d[0]).to_degrees();
        let pitch = -d[2].asin().to_degrees();
        view_vectors(pitch, yaw)
    };
    let offsets = [[0.0, 0.0], [8.0, 0.0], [-8.0, 0.0], [0.0, 8.0], [0.0, -8.0]];
    offsets.iter().find_map(|o| {
        let shift = add(scale(right, o[0]), scale(up, o[1]));
        let (f, player, _) = nearest(world, hitboxes, add(src, shift), add(end, shift), targets, &[shooter]);
        if f < 1.0 {
            player
        } else {
            None
        }
    })
}
