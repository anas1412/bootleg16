//! Player hit zones from the real player model's hitboxes, posed with its own
//! animations (standing / crouched aim poses at 9 pitch steps).
use crate::Vec3;
use goldsrc::mdl::{BoneTransform, Model};

/// Body part a bullet hit (GoldSrc hit groups).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HitGroup {
    Generic,
    Head,
    Chest,
    Stomach,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
    Shield,
}

impl HitGroup {
    fn from_mdl(g: i32) -> HitGroup {
        match g {
            1 => HitGroup::Head,
            2 => HitGroup::Chest,
            3 => HitGroup::Stomach,
            4 => HitGroup::LeftArm,
            5 => HitGroup::RightArm,
            6 => HitGroup::LeftLeg,
            7 => HitGroup::RightLeg,
            8 => HitGroup::Shield,
            _ => HitGroup::Generic,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PosedBox {
    group: HitGroup,
    bone: BoneTransform,
    mins: Vec3,
    maxs: Vec3,
}

/// Number of aim-pitch poses (the CS player models' blend sets).
const PITCH_STEPS: usize = 9;

pub struct Hitboxes {
    /// [stance][pitch step] -> boxes in model space (x forward, z up, origin = box centre).
    poses: [Vec<Vec<PosedBox>>; 2],
}

/// Where a ray entered a hitbox.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub group: HitGroup,
    /// Distance along the (unit) ray.
    pub distance: f32,
}

impl Hitboxes {
    /// `aim`: the aim animation to use, e.g. "carbine" (ref_aim_carbine / crouch_aim_carbine).
    pub fn from_model(model: &Model, aim: &str) -> Option<Hitboxes> {
        let stance = |prefix: &str| -> Option<Vec<Vec<PosedBox>>> {
            let seq = model.sequence(&format!("{prefix}_aim_{aim}"))?;
            Some(
                (0..PITCH_STEPS)
                    .map(|i| {
                        let pose = model.pose(seq, 0, i as f32 / (PITCH_STEPS - 1) as f32);
                        model
                            .hitboxes
                            .iter()
                            .filter(|h| h.group != 8) // the shield box only matters with a shield
                            .map(|h| PosedBox { group: HitGroup::from_mdl(h.group), bone: pose[h.bone], mins: h.mins, maxs: h.maxs })
                            .collect()
                    })
                    .collect(),
            )
        };
        Some(Hitboxes { poses: [stance("ref")?, stance("crouch")?] })
    }

    /// First hitbox the ray `start + dir * t` (dir normalised, t <= `max`) enters, for a
    /// player at `origin` facing `yaw` and aiming at `pitch` (GoldSrc degrees).
    pub fn trace(&self, origin: Vec3, yaw: f32, pitch: f32, crouched: bool, start: Vec3, dir: Vec3, max: f32) -> Option<Hit> {
        // ⚠ Which end of the 9 pitch poses is "looking up" is not yet confirmed.
        let step = (((pitch + 90.0) / 180.0) * (PITCH_STEPS - 1) as f32).round().clamp(0.0, (PITCH_STEPS - 1) as f32) as usize;
        let boxes = &self.poses[crouched as usize][step];
        // Into model space: undo the player's position and yaw.
        let (s, c) = (-yaw).to_radians().sin_cos();
        let rot = |v: Vec3| [v[0] * c - v[1] * s, v[0] * s + v[1] * c, v[2]];
        let ms = rot(crate::sub(start, origin));
        let md = rot(dir);
        let mut best: Option<Hit> = None;
        for b in boxes {
            let (o, d) = (b.bone.inverse_apply(ms), b.bone.inverse_rotate(md));
            if let Some(t) = slab(o, d, b.mins, b.maxs, max) {
                if best.is_none_or(|h| t < h.distance) {
                    best = Some(Hit { group: b.group, distance: t });
                }
            }
        }
        best
    }
}

/// Ray vs axis-aligned box in the box's own space: entry distance, if within `max`.
fn slab(o: Vec3, d: Vec3, mins: Vec3, maxs: Vec3, max: f32) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, max);
    for i in 0..3 {
        if d[i].abs() < 1e-8 {
            if o[i] < mins[i] || o[i] > maxs[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((mins[i] - o[i]) / d[i], (maxs[i] - o[i]) / d[i]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}
