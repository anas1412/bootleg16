//! GoldSrc studio model (MDL v10) reader: skeleton, hitboxes, sequences and
//! the RLE-compressed animation data, enough to pose a skeleton at any frame.
use std::{fs, io, path::Path};

pub type Vec3 = [f32; 3];
/// Quaternion (x, y, z, w).
pub type Quat = [f32; 4];

#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub parent: i32,
    /// Default position (0..3) and rotation in radians (3..6).
    pub value: [f32; 6],
    /// Animation value scales for the same 6 channels.
    pub scale: [f32; 6],
}

#[derive(Debug, Clone, Copy)]
pub struct Hitbox {
    pub bone: usize,
    /// 1 head, 2 chest, 3 stomach, 4 left arm, 5 right arm, 6 left leg, 7 right leg, 8 shield.
    pub group: i32,
    pub mins: Vec3,
    pub maxs: Vec3,
}

#[derive(Debug, Clone)]
pub struct Sequence {
    pub name: String,
    pub fps: f32,
    pub frames: usize,
    pub blends: usize,
    /// Byte offset of this sequence's per-bone animation headers (one set per blend).
    anim_index: usize,
}

pub struct Model {
    data: Vec<u8>,
    pub bones: Vec<Bone>,
    pub hitboxes: Vec<Hitbox>,
    pub sequences: Vec<Sequence>,
}

/// A bone's pose in model space: rotation matrix rows and translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneTransform {
    pub rot: [[f32; 3]; 3],
    pub pos: Vec3,
}

impl BoneTransform {
    /// Bone-local point to model space.
    pub fn apply(&self, p: Vec3) -> Vec3 {
        let r = &self.rot;
        std::array::from_fn(|i| r[i][0] * p[0] + r[i][1] * p[1] + r[i][2] * p[2] + self.pos[i])
    }
    /// Model-space point to bone-local (rotation is orthonormal).
    pub fn inverse_apply(&self, p: Vec3) -> Vec3 {
        let d = [p[0] - self.pos[0], p[1] - self.pos[1], p[2] - self.pos[2]];
        let r = &self.rot;
        std::array::from_fn(|i| r[0][i] * d[0] + r[1][i] * d[1] + r[2][i] * d[2])
    }
    /// Model-space direction to bone-local.
    pub fn inverse_rotate(&self, d: Vec3) -> Vec3 {
        let r = &self.rot;
        std::array::from_fn(|i| r[0][i] * d[0] + r[1][i] * d[1] + r[2][i] * d[2])
    }
}

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("bad MDL: {what}"))
}

impl Model {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Model> {
        Self::parse(fs::read(path)?)
    }

    pub fn parse(data: Vec<u8>) -> io::Result<Model> {
        let i32_at = |o: usize| data.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| bad("truncated"));
        let f32_at = |o: usize| data.get(o..o + 4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| bad("truncated"));
        if data.get(..4) != Some(b"IDST") || i32_at(4)? != 10 {
            return Err(bad("not an IDST v10 model"));
        }
        // Header: id, version, name[64], length, eyepos, min, max, bbmin, bbmax, flags, then counts/offsets.
        let h = 8 + 64 + 4 + 12 * 5 + 4;
        let (num_bones, bone_index) = (i32_at(h)? as usize, i32_at(h + 4)? as usize);
        let (num_hitboxes, hitbox_index) = (i32_at(h + 16)? as usize, i32_at(h + 20)? as usize);
        let (num_seq, seq_index) = (i32_at(h + 24)? as usize, i32_at(h + 28)? as usize);

        let mut bones = Vec::with_capacity(num_bones);
        for i in 0..num_bones {
            let o = bone_index + i * 112;
            let name = crate::texture::cstr(data.get(o..o + 32).ok_or_else(|| bad("bones"))?);
            let parent = i32_at(o + 32)?;
            let mut value = [0.0; 6];
            let mut scale = [0.0; 6];
            for j in 0..6 {
                value[j] = f32_at(o + 64 + j * 4)?;
                scale[j] = f32_at(o + 88 + j * 4)?;
            }
            bones.push(Bone { name, parent, value, scale });
        }

        let mut hitboxes = Vec::with_capacity(num_hitboxes);
        for i in 0..num_hitboxes {
            let o = hitbox_index + i * 32;
            let v = |k: usize| f32_at(o + 8 + k * 4);
            hitboxes.push(Hitbox {
                bone: i32_at(o)? as usize,
                group: i32_at(o + 4)?,
                mins: [v(0)?, v(1)?, v(2)?],
                maxs: [v(3)?, v(4)?, v(5)?],
            });
        }

        let mut sequences = Vec::with_capacity(num_seq);
        for i in 0..num_seq {
            let o = seq_index + i * 176;
            sequences.push(Sequence {
                name: crate::texture::cstr(data.get(o..o + 32).ok_or_else(|| bad("sequences"))?),
                fps: f32_at(o + 32)?,
                frames: i32_at(o + 56)?.max(1) as usize,
                blends: i32_at(o + 120)?.max(1) as usize,
                anim_index: i32_at(o + 124)? as usize,
            });
        }
        Ok(Model { data, bones, hitboxes, sequences })
    }

    pub fn sequence(&self, name: &str) -> Option<usize> {
        self.sequences.iter().position(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// One animation channel of one bone at an integer frame (RLE decode).
    fn channel(&self, anim: usize, bone: usize, ch: usize, frame: usize) -> f32 {
        let b = &self.bones[bone];
        let u16_at = |o: usize| self.data.get(o..o + 2).map(|x| u16::from_le_bytes([x[0], x[1]])).unwrap_or(0);
        let hdr = anim + bone * 12;
        let off = u16_at(hdr + ch * 2) as usize;
        if off == 0 {
            return b.value[ch];
        }
        // Runs of (valid, total) headers, each followed by `valid` i16 values.
        let mut p = hdr + off;
        let mut k = frame;
        loop {
            let (valid, total) = (self.data.get(p).copied().unwrap_or(0) as usize, self.data.get(p + 1).copied().unwrap_or(1) as usize);
            if total == 0 || total > k {
                let idx = if valid > k { k + 1 } else { valid };
                let raw = u16_at(p + idx * 2) as i16;
                return b.value[ch] + raw as f32 * b.scale[ch];
            }
            k -= total;
            p += (valid + 1) * 2;
        }
    }

    /// Pose of every bone in model space for `seq` at `frame`, at position `blend`
    /// (0..1) across its blend sets (e.g. aim pitch; 0.5 = level).
    pub fn pose(&self, seq: usize, frame: usize, blend: f32) -> Vec<BoneTransform> {
        let s = &self.sequences[seq];
        let frame = frame.min(s.frames - 1);
        let per_blend = self.bones.len() * 12;
        let local = |set: usize| -> Vec<(Quat, Vec3)> {
            let anim = s.anim_index + set.min(s.blends - 1) * per_blend;
            (0..self.bones.len())
                .map(|b| {
                    let c = |ch| self.channel(anim, b, ch, frame);
                    (angle_quaternion([c(3), c(4), c(5)]), [c(0), c(1), c(2)])
                })
                .collect()
        };
        // `blend` spans all blend sets (CS player models use 9 for aim pitch; 0.5 = the middle one).
        let pos = blend.clamp(0.0, 1.0) * (s.blends - 1) as f32;
        let (lo, t) = (pos.floor() as usize, pos.fract());
        let a = local(lo);
        let mixed: Vec<(Quat, Vec3)> = if t > 0.0 && lo + 1 < s.blends {
            let b = local(lo + 1);
            a.iter().zip(&b).map(|(x, y)| (slerp(x.0, y.0, t), std::array::from_fn(|i| x.1[i] + (y.1[i] - x.1[i]) * t))).collect()
        } else {
            a
        };
        let mut out: Vec<BoneTransform> = Vec::with_capacity(self.bones.len());
        for (i, (q, p)) in mixed.into_iter().enumerate() {
            let t = BoneTransform { rot: quat_matrix(q), pos: p };
            let parent = self.bones[i].parent;
            out.push(if parent >= 0 && (parent as usize) < out.len() { concat(&out[parent as usize], &t) } else { t });
        }
        out
    }
}

/// GoldSrc's Euler (x roll, y pitch, z yaw; radians) to quaternion.
fn angle_quaternion(a: Vec3) -> Quat {
    let (sy, cy) = (a[2] * 0.5).sin_cos();
    let (sp, cp) = (a[1] * 0.5).sin_cos();
    let (sr, cr) = (a[0] * 0.5).sin_cos();
    [sr * cp * cy - cr * sp * sy, cr * sp * cy + sr * cp * sy, cr * cp * sy - sr * sp * cy, cr * cp * cy + sr * sp * sy]
}

fn slerp(p: Quat, mut q: Quat, t: f32) -> Quat {
    let mut cos = p[0] * q[0] + p[1] * q[1] + p[2] * q[2] + p[3] * q[3];
    if cos < 0.0 {
        q = [-q[0], -q[1], -q[2], -q[3]];
        cos = -cos;
    }
    let (a, b) = if cos > 0.999 {
        (1.0 - t, t)
    } else {
        let om = cos.acos();
        let s = om.sin();
        (((1.0 - t) * om).sin() / s, (t * om).sin() / s)
    };
    std::array::from_fn(|i| a * p[i] + b * q[i])
}

fn quat_matrix(q: Quat) -> [[f32; 3]; 3] {
    let [x, y, z, w] = q;
    [
        [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y)],
        [2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x)],
        [2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y)],
    ]
}

/// parent * child
fn concat(p: &BoneTransform, c: &BoneTransform) -> BoneTransform {
    let rot = std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| p.rot[i][k] * c.rot[k][j]).sum()));
    BoneTransform { rot, pos: p.apply(c.pos) }
}
