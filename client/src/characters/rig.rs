//! Retargeting: a character on Source's ValveBiped skeleton plays our CS animations. Each
//! frame the CS skeleton is posed as usual (`sim::anim`, which the server's hitboxes share),
//! then every character bone takes its CS twin's rotation since rest, measured from where the
//! two skeletons' limbs point at rest. Body bones also sit exactly where their twin is, so the
//! hands hold the CS weapon model and the head is in its hitbox; fingers and bones without a
//! twin (extra fingers, skirt, hair) keep the character's own lengths, scaled so its legs are
//! as long as the CS model's.
use bevy::math::{Mat3, Quat, Vec3};
use goldsrc::mdl::{BoneTransform, Model as CsModel};
use goldsrc::source::Model as Character;

/// A bone name both skeletons agree on: case, spaces and underscores dropped, Source's
/// prefix and its different numbering for the top of the spine, neck and head folded in.
fn key(name: &str) -> String {
    let k: String = name.trim_start_matches("ValveBiped.").chars().filter(|c| !matches!(c, ' ' | '_')).collect::<String>().to_ascii_lowercase();
    match k.as_str() {
        "bip01spine4" => "bip01spine3".into(),
        "bip01neck1" => "bip01neck".into(),
        "bip01head1" => "bip01head".into(),
        _ => k,
    }
}

/// GoldSrc's Euler angles (x roll, y pitch, z yaw; radians) as a rotation.
fn cs_angles(a: [f32; 3]) -> Quat {
    let (sy, cy) = (a[2] * 0.5).sin_cos();
    let (sp, cp) = (a[1] * 0.5).sin_cos();
    let (sr, cr) = (a[0] * 0.5).sin_cos();
    Quat::from_xyzw(sr * cp * cy - cr * sp * sy, cr * sp * cy + sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy)
}

fn bone_quat(b: &BoneTransform) -> Quat {
    let r = &b.rot;
    Quat::from_mat3(&Mat3::from_cols(Vec3::new(r[0][0], r[1][0], r[2][0]), Vec3::new(r[0][1], r[1][1], r[2][1]), Vec3::new(r[0][2], r[1][2], r[2][2]))).normalize()
}

/// A pose in model space (Z up, like both games): rotation and position per bone.
pub type Pose = Vec<(Quat, Vec3)>;

/// How one character follows the CS skeleton (built once per character).
pub struct Rig {
    /// Each character bone's CS twin, if it has one.
    twin: Vec<Option<usize>>,
    /// Twinned bones placed where their twin is (all but the fingers).
    pinned: Vec<bool>,
    /// Character bones' parents and bind pose relative to them.
    parent: Vec<Option<usize>>,
    local: Vec<(Quat, Vec3)>,
    /// Turns each twinned bone from its bind direction to its twin's rest direction.
    align: Vec<Quat>,
    /// The CS bones' rest rotations, inverted.
    cs_rest_inv: Vec<Quat>,
    /// The character's bind pose (model space), and its size relative to the CS model.
    bind: Pose,
    pub scale: f32,
}

impl Rig {
    pub fn new(cs: &CsModel, ch: &Character) -> Rig {
        // CS rest pose (the bones' default values), model space
        let mut cs_rest: Pose = Vec::with_capacity(cs.bones.len());
        for b in &cs.bones {
            let (q, p) = (cs_angles([b.value[3], b.value[4], b.value[5]]), Vec3::new(b.value[0], b.value[1], b.value[2]));
            let w = match usize::try_from(b.parent).ok().and_then(|i| cs_rest.get(i)) {
                Some(&(pq, pp)) => (pq * q, pp + pq * p),
                None => (q, p),
            };
            cs_rest.push(w);
        }
        let bind: Pose = ch.bind_pose().into_iter().map(|(q, p)| (Quat::from_array(q).normalize(), Vec3::from_array(p))).collect();
        let cs_keys: Vec<String> = cs.bones.iter().map(|b| key(&b.name)).collect();
        let twin: Vec<Option<usize>> = ch.bones.iter().map(|b| cs_keys.iter().position(|k| *k == key(&b.name))).collect();
        let parent: Vec<Option<usize>> = ch.bones.iter().map(|b| usize::try_from(b.parent).ok()).collect();
        let local = ch.bones.iter().map(|b| (Quat::from_array(b.quat).normalize(), Vec3::from_array(b.pos))).collect();

        // Each twinned bone points at its first twinned child: turn the character's bind
        // direction onto the CS rest direction. Ends (hands, feet, head) keep their parent's turn.
        let n = ch.bones.len();
        let mut align = vec![Quat::IDENTITY; n];
        for i in 0..n {
            let Some(t) = twin[i] else {
                align[i] = parent[i].map(|p| align[p]).unwrap_or(Quat::IDENTITY);
                continue;
            };
            let child = (0..n).find(|&c| parent[c] == Some(i) && twin[c].is_some());
            align[i] = match child {
                Some(c) => {
                    let from = (bind[c].1 - bind[i].1).normalize_or_zero();
                    let to = (cs_rest[twin[c].unwrap()].1 - cs_rest[t].1).normalize_or_zero();
                    if from == Vec3::ZERO || to == Vec3::ZERO { Quat::IDENTITY } else { Quat::from_rotation_arc(from, to) }
                }
                None => parent[i].map(|p| align[p]).unwrap_or(Quat::IDENTITY),
            };
        }

        // Legs as long as the CS model's: pelvis height above the foot, both at rest.
        let height = |pose: &Pose, pelvis: Option<usize>, foot: Option<usize>| pelvis.zip(foot).map(|(p, f)| pose[p].1.z - pose[f].1.z).filter(|h| *h > 1.0);
        let find = |keys: &[String], k: &str| keys.iter().position(|x| x == k);
        let ch_keys: Vec<String> = ch.bones.iter().map(|b| key(&b.name)).collect();
        let scale = match (height(&cs_rest, find(&cs_keys, "bip01pelvis"), find(&cs_keys, "bip01lfoot")), height(&bind, find(&ch_keys, "bip01pelvis"), find(&ch_keys, "bip01lfoot"))) {
            (Some(a), Some(b)) => a / b,
            _ => 1.0,
        };
        let pinned = ch.bones.iter().zip(&twin).map(|(b, t)| t.is_some() && !key(&b.name).contains("finger")).collect();
        Rig { twin, pinned, parent, local, align, cs_rest_inv: cs_rest.iter().map(|(q, _)| q.inverse()).collect(), bind, scale }
    }

    /// The character's pose for this frame from the CS skeleton's (both model space, Z up;
    /// positions in the CS model's units).
    pub fn pose(&self, cs: &[BoneTransform]) -> Pose {
        let mut out: Pose = Vec::with_capacity(self.local.len());
        for i in 0..self.local.len() {
            let (lq, lp) = self.local[i];
            let from_parent = self.parent[i].and_then(|p| out.get(p).copied());
            let rot = match (self.twin[i], cs.get(self.twin[i].unwrap_or(usize::MAX))) {
                // the twin's turn since rest, applied to the aligned bind rotation
                (Some(t), Some(b)) => bone_quat(b) * self.cs_rest_inv[t] * self.align[i] * self.bind[i].0,
                _ => from_parent.map(|(pq, _)| pq * lq).unwrap_or(lq),
            };
            let twin_at = self.twin[i].filter(|_| self.pinned[i]).and_then(|t| cs.get(t)).map(|b| Vec3::from_array(b.pos));
            let pos = match (twin_at, from_parent) {
                // body bones where their twin is (the pelvis carries the height and sway)
                (Some(at), _) => at,
                // fingers and extras keep the character's own bone lengths
                (None, Some((pq, pp))) => pp + pq * (lp * self.scale),
                _ => lp * self.scale,
            };
            out.push((rot.normalize(), pos));
        }
        out
    }

    /// The character's bind pose (model space, its own units): what the mesh is modelled in.
    pub fn bind(&self) -> &Pose {
        &self.bind
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_across_skeletons() {
        assert_eq!(key("ValveBiped.Bip01_L_Thigh"), key("Bip01 L Thigh"));
        assert_eq!(key("ValveBiped.Bip01_Spine4"), key("Bip01 Spine3"));
        assert_eq!(key("ValveBiped.Bip01_Head1"), key("Bip01 Head"));
        assert_ne!(key("ValveBiped.Bip01_L_Hand"), key("Bip01 R Hand"));
    }

    /// With the CS skeleton at rest, each twinned character bone points where its twin does,
    /// and the feet are on the CS model's ground.
    #[test]
    fn rest_pose_lines_up_with_the_cs_skeleton() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let (Ok(cs), Ok(ch)) = (CsModel::load(root.join("gamedata/models/player/urban/urban.mdl")), Character::load(root.join("gamedata/characters/lisa/model.mdl"))) else { return };
        let rig = Rig::new(&cs, &ch);
        assert!((0.7..1.4).contains(&rig.scale), "scale {}", rig.scale);
        // the CS rest pose as BoneTransforms
        let mut rest: Vec<(Quat, Vec3)> = Vec::new();
        for b in &cs.bones {
            let (q, p) = (cs_angles([b.value[3], b.value[4], b.value[5]]), Vec3::new(b.value[0], b.value[1], b.value[2]));
            let w = match usize::try_from(b.parent).ok().and_then(|i| rest.get(i)) {
                Some(&(pq, pp)) => (pq * q, pp + pq * p),
                None => (q, p),
            };
            rest.push(w);
        }
        let to_bt = |(q, p): &(Quat, Vec3)| {
            let m = Mat3::from_quat(*q);
            BoneTransform { rot: std::array::from_fn(|i| std::array::from_fn(|j| m.col(j)[i])), pos: p.to_array() }
        };
        let cs_pose: Vec<BoneTransform> = rest.iter().map(to_bt).collect();
        let pose = rig.pose(&cs_pose);
        let names: Vec<String> = ch.bones.iter().map(|b| key(&b.name)).collect();
        let at = |k: &str| names.iter().position(|n| n == k).unwrap();
        // the thigh points down its leg like the CS thigh does
        let (thigh, calf) = (at("bip01lthigh"), at("bip01lcalf"));
        let ch_dir = (pose[calf].1 - pose[thigh].1).normalize();
        let cs_keys: Vec<String> = cs.bones.iter().map(|b| key(&b.name)).collect();
        let cs_at = |k: &str| cs_keys.iter().position(|n| n == k).unwrap();
        let cs_dir = (rest[cs_at("bip01lcalf")].1 - rest[cs_at("bip01lthigh")].1).normalize();
        assert!(ch_dir.dot(cs_dir) > 0.98, "thigh {ch_dir} vs {cs_dir}");
        // feet, hands and head are where the CS ones are (the gun is in the hands, the head in its hitbox)
        for k in ["bip01lfoot", "bip01rhand", "bip01lhand", "bip01head"] {
            assert!(pose[at(k)].1.distance(rest[cs_at(k)].1) < 0.01, "{k} {} vs {}", pose[at(k)].1, rest[cs_at(k)].1);
        }
    }
}
