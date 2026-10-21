//! Source engine studio models (MDL v44 to v49, with their VVD and VTX files) and VTF
//! textures: enough to draw a skinned character in its bind pose and pose it by its bones.
//! Clean-room from the public file layouts; std only.
//!
//! A model is three files beside each other: `name.mdl` (skeleton, meshes, materials),
//! `name.vvd` (vertices with up to three bone weights) and `name.dx90.vtx` (triangle strips
//! that index the vertices). Materials are `.vmt` text files naming a `.vtf` texture.
use std::{fs, io, path::Path};

fn bad(what: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("bad Source model: {what}"))
}

/// Little-endian reads that fail cleanly on truncated files.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn i32(&self, o: usize) -> io::Result<i32> {
        self.0.get(o..o + 4).map(|b| i32::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| bad("truncated"))
    }
    fn u32(&self, o: usize) -> io::Result<u32> {
        Ok(self.i32(o)? as u32)
    }
    fn u16(&self, o: usize) -> io::Result<u16> {
        self.0.get(o..o + 2).map(|b| u16::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| bad("truncated"))
    }
    fn u8(&self, o: usize) -> io::Result<u8> {
        self.0.get(o).copied().ok_or_else(|| bad("truncated"))
    }
    fn f32(&self, o: usize) -> io::Result<f32> {
        Ok(f32::from_bits(self.u32(o)?))
    }
    fn vec3(&self, o: usize) -> io::Result<[f32; 3]> {
        Ok([self.f32(o)?, self.f32(o + 4)?, self.f32(o + 8)?])
    }
    /// A zero-terminated string at `o`.
    fn str(&self, o: usize) -> io::Result<String> {
        let rest = self.0.get(o..).ok_or_else(|| bad("string"))?;
        let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
        Ok(String::from_utf8_lossy(&rest[..end]).into_owned())
    }
    /// The count and absolute offset of a table: (count at `o`, offset at `o + 4`, from `base`).
    fn table(&self, o: usize, base: usize) -> io::Result<(usize, usize)> {
        Ok((self.i32(o)?.max(0) as usize, (base as i64 + self.i32(o + 4)? as i64).max(0) as usize))
    }
}

/// A bone and its bind pose.
#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    /// Index of the parent bone, or -1 for a root.
    pub parent: i32,
    /// Bind pose relative to the parent: position and rotation (x, y, z, w).
    pub pos: [f32; 3],
    pub quat: [f32; 4],
    /// Model space to this bone's space in the bind pose (3 rows of a 3x4 matrix).
    pub pose_to_bone: [[f32; 4]; 3],
}

/// One vertex, as the VVD stores it (Z up, like GoldSrc).
#[derive(Debug, Clone, Copy, Default)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    /// Up to three bones (indices into `Model::bones`) and their weights (sum to 1).
    pub bones: [u8; 3],
    pub weights: [f32; 3],
}

/// The triangles that share one material.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    /// Index into `Model::materials` (skin family 0).
    pub material: usize,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// A Source studio model: skeleton, meshes (detail level 0, every body part's first model)
/// and the material names with the folders to look them up in.
#[derive(Debug, Clone)]
pub struct Model {
    pub version: i32,
    pub bones: Vec<Bone>,
    pub meshes: Vec<Mesh>,
    pub materials: Vec<String>,
    /// Folders under `materials/` where the material files are (`$cdmaterials`).
    pub material_dirs: Vec<String>,
}

const MDL_BONE: usize = 216;
const MDL_BODYPART: usize = 16;
const MDL_MESH: usize = 116;
const MDL_TEXTURE: usize = 64;
const VVD_VERTEX: usize = 48;
const VTX_VERTEX: usize = 9;
const VTX_STRIPGROUP: usize = 25;
const VTX_STRIP: usize = 27;
const VTX_MESH: usize = 9;

impl Model {
    /// Loads `path` (the .mdl) with the .vvd and .dx90.vtx beside it.
    pub fn load(path: impl AsRef<Path>) -> io::Result<Model> {
        let path = path.as_ref();
        let mdl = fs::read(path)?;
        let vvd = fs::read(path.with_extension("vvd"))?;
        // the DirectX 9 strips; older packs only have the plain .vtx
        let vtx = fs::read(path.with_extension("dx90.vtx")).or_else(|_| fs::read(path.with_extension("vtx")))?;
        Model::parse(&mdl, &vvd, &vtx)
    }

    pub fn parse(mdl: &[u8], vvd: &[u8], vtx: &[u8]) -> io::Result<Model> {
        let m = Bytes(mdl);
        if mdl.get(..4) != Some(b"IDST") {
            return Err(bad("not an IDST model"));
        }
        let version = m.i32(4)?;
        if !(44..=49).contains(&version) {
            return Err(bad(&format!("version {version} (44 to 49 supported)")));
        }
        // Header after id, version, checksum, name[64], length and six vectors: flags at 152.
        let h = 156;
        let (num_bones, bone_index) = m.table(h, 0)?;
        let (num_textures, texture_index) = m.table(h + 48, 0)?;
        let (num_cd, cd_index) = m.table(h + 56, 0)?;
        let num_skinref = m.i32(h + 64)?.max(0) as usize;
        let skin_index = m.i32(h + 72)?.max(0) as usize;
        let (num_bodyparts, bodypart_index) = m.table(h + 76, 0)?;

        let mut bones = Vec::with_capacity(num_bones);
        for i in 0..num_bones {
            let o = bone_index + i * MDL_BONE;
            let name = m.str(o + m.i32(o)? as usize)?;
            let mut pose_to_bone = [[0.0; 4]; 3];
            for (r, row) in pose_to_bone.iter_mut().enumerate() {
                for (c, v) in row.iter_mut().enumerate() {
                    *v = m.f32(o + 96 + (r * 4 + c) * 4)?;
                }
            }
            bones.push(Bone { name, parent: m.i32(o + 4)?, pos: m.vec3(o + 32)?, quat: [m.f32(o + 44)?, m.f32(o + 48)?, m.f32(o + 52)?, m.f32(o + 56)?], pose_to_bone });
        }

        let mut textures = Vec::with_capacity(num_textures);
        for i in 0..num_textures {
            let o = texture_index + i * MDL_TEXTURE;
            textures.push(m.str(o + m.i32(o)? as usize)?);
        }
        let material_dirs = (0..num_cd).map(|i| m.str(m.i32(cd_index + i * 4)? as usize)).collect::<io::Result<Vec<_>>>()?;
        // skin family 0: a mesh's material (a skin reference) to a texture
        let skin = |r: usize| -> usize { if r < num_skinref { m.u16(skin_index + r * 2).map(|t| t as usize).unwrap_or(r) } else { r } };

        let vertices = vvd_vertices(vvd)?;
        let x = Bytes(vtx);
        if x.i32(0)? != 7 {
            return Err(bad("VTX version (7 expected)"));
        }
        let (vtx_bodyparts, vtx_bodypart_index) = x.table(28, 0)?;
        if vtx_bodyparts != num_bodyparts {
            return Err(bad("VTX and MDL body parts differ"));
        }

        let mut meshes: Vec<Mesh> = Vec::new();
        for b in 0..num_bodyparts {
            let bp = bodypart_index + b * MDL_BODYPART;
            // body part: name, model count, base, model offset
            let num_models = m.i32(bp + 4)?.max(0) as usize;
            let model_index = bp + m.i32(bp + 12)?.max(0) as usize;
            let vbp = vtx_bodypart_index + b * 8;
            let (_, vtx_model_index) = x.table(vbp, vbp)?;
            // every body part's first model is the default one
            if num_models == 0 {
                continue;
            }
            let mo = model_index;
            let (num_meshes, mesh_index) = m.table(mo + 72, mo)?;
            let model_first = m.i32(mo + 84)?.max(0) as usize / VVD_VERTEX;
            // VTX model, then its detail level 0
            let (_, lod_index) = x.table(vtx_model_index, vtx_model_index)?;
            let (vtx_meshes, vtx_mesh_index) = x.table(lod_index, lod_index)?;
            if vtx_meshes != num_meshes {
                return Err(bad("VTX and MDL meshes differ"));
            }
            for k in 0..num_meshes {
                let me = mesh_index + k * MDL_MESH;
                let first = model_first + m.i32(me + 12)?.max(0) as usize;
                let mut mesh = Mesh { material: skin(m.i32(me)?.max(0) as usize), ..Default::default() };
                let vm = vtx_mesh_index + k * VTX_MESH;
                let (num_groups, group_index) = x.table(vm, vm)?;
                for g in 0..num_groups {
                    let sg = group_index + g * VTX_STRIPGROUP;
                    let (num_verts, vert_index) = x.table(sg, sg)?;
                    let (_, index_index) = x.table(sg + 8, sg)?;
                    let (num_strips, strip_index) = x.table(sg + 16, sg)?;
                    // this group's vertices, appended to the mesh
                    let base = mesh.vertices.len() as u32;
                    for v in 0..num_verts {
                        let orig = x.u16(vert_index + v * VTX_VERTEX + 4)? as usize;
                        mesh.vertices.push(*vertices.get(first + orig).ok_or_else(|| bad("vertex index"))?);
                    }
                    for s in 0..num_strips {
                        let st = strip_index + s * VTX_STRIP;
                        let count = x.i32(st)?.max(0) as usize;
                        let offset = x.i32(st + 4)?.max(0) as usize;
                        let flags = x.u8(st + 18)?;
                        let idx = |n: usize| x.u16(index_index + (offset + n) * 2).map(|i| base + i as u32);
                        if flags & 2 != 0 {
                            // a triangle strip: every other triangle flips
                            for n in 2..count {
                                let (a, b, c) = (idx(n - 2)?, idx(n - 1)?, idx(n)?);
                                if a != b && b != c && a != c {
                                    mesh.indices.extend(if n % 2 == 0 { [a, b, c] } else { [b, a, c] });
                                }
                            }
                        } else {
                            for n in 0..count {
                                mesh.indices.push(idx(n)?);
                            }
                        }
                    }
                }
                if !mesh.indices.is_empty() {
                    meshes.push(mesh);
                }
            }
        }
        Ok(Model { version, bones, meshes, materials: textures, material_dirs })
    }

    /// Each bone's bind pose in model space, parents before children (`bones` order).
    pub fn bind_pose(&self) -> Vec<([f32; 4], [f32; 3])> {
        let mut out: Vec<([f32; 4], [f32; 3])> = Vec::with_capacity(self.bones.len());
        for b in &self.bones {
            let world = match out.get(b.parent.max(0) as usize).filter(|_| b.parent >= 0) {
                Some(&(pq, pp)) => (quat_mul(pq, b.quat), add(pp, quat_rotate(pq, b.pos))),
                None => (b.quat, b.pos),
            };
            out.push(world);
        }
        out
    }
}

/// The VVD's vertices at detail level 0, with the fixup table applied.
fn vvd_vertices(vvd: &[u8]) -> io::Result<Vec<Vertex>> {
    let v = Bytes(vvd);
    if vvd.get(..4) != Some(b"IDSV") {
        return Err(bad("not an IDSV vertex file"));
    }
    let lod0 = v.i32(16)?.max(0) as usize;
    let num_fixups = v.i32(48)?.max(0) as usize;
    let fixup_start = v.i32(52)?.max(0) as usize;
    let data = v.i32(56)?.max(0) as usize;
    let read = |i: usize| -> io::Result<Vertex> {
        let o = data + i * VVD_VERTEX;
        Ok(Vertex {
            weights: [v.f32(o)?, v.f32(o + 4)?, v.f32(o + 8)?],
            bones: [v.u8(o + 12)?, v.u8(o + 13)?, v.u8(o + 14)?],
            pos: v.vec3(o + 16)?,
            normal: v.vec3(o + 28)?,
            uv: [v.f32(o + 40)?, v.f32(o + 44)?],
        })
    };
    let mut out = Vec::with_capacity(lod0);
    if num_fixups == 0 {
        for i in 0..lod0 {
            out.push(read(i)?);
        }
    } else {
        // level 0 keeps every range whose level is 0 or more
        for f in 0..num_fixups {
            let o = fixup_start + f * 12;
            if v.i32(o)? < 0 {
                continue;
            }
            let (from, count) = (v.i32(o + 4)?.max(0) as usize, v.i32(o + 8)?.max(0) as usize);
            for i in from..from + count {
                out.push(read(i)?);
            }
        }
    }
    // weights past the bone count are left over: zero them
    for x in &mut out {
        let n = x.weights.iter().filter(|w| **w > 0.0).count();
        for k in n..3 {
            x.weights[k] = 0.0;
        }
    }
    Ok(out)
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    [
        a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
        a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
        a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
        a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
    ]
}

fn quat_rotate(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    let p = quat_mul(quat_mul(q, [v[0], v[1], v[2], 0.0]), [-q[0], -q[1], -q[2], q[3]]);
    [p[0], p[1], p[2]]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// What a `.vmt` says about drawing its material.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Material {
    /// `$basetexture`: a path under `materials/`, without `.vtf`.
    pub base_texture: String,
    /// `$translucent` (blended) or `$alphatest` (cut out): hair, eyelashes.
    pub translucent: bool,
    pub alpha_test: bool,
    /// `$nocull`: drawn from both sides (hair cards, skirts).
    pub double_sided: bool,
}

impl Material {
    /// Reads a `.vmt` file (comments may be in any encoding: only the keys matter).
    pub fn load(path: impl AsRef<Path>) -> io::Result<Material> {
        Ok(Material::parse(&String::from_utf8_lossy(&fs::read(path)?)))
    }

    /// Reads the parameters we use from a `.vmt` (KeyValues text).
    pub fn parse(text: &str) -> Material {
        let mut mat = Material::default();
        for line in text.lines() {
            let mut words = line.split(|c: char| c == '"' || c.is_whitespace()).filter(|w| !w.is_empty());
            let (Some(key), Some(value)) = (words.next(), words.next()) else { continue };
            match key.to_ascii_lowercase().as_str() {
                "$basetexture" => mat.base_texture = value.replace('\\', "/"),
                "$translucent" => mat.translucent = value == "1",
                "$alphatest" => mat.alpha_test = value == "1",
                "$nocull" => mat.double_sided = value == "1",
                _ => {}
            }
        }
        mat
    }
}

/// A VTF texture's largest image as RGBA8.
#[derive(Debug, Clone)]
pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Image formats (`IMAGE_FORMAT_*`) we can read, and their size for a w x h image.
fn image_size(format: i32, w: usize, h: usize) -> Option<usize> {
    let blocks = w.div_ceil(4) * h.div_ceil(4);
    Some(match format {
        0 | 1 | 11 | 12 | 16 => w * h * 4,
        2 | 3 | 9 | 10 => w * h * 3,
        4 | 6 | 17 | 18 | 19 | 21 => w * h * 2,
        5 | 8 => w * h,
        13 | 20 => blocks * 8,
        14 | 15 => blocks * 16,
        _ => return None,
    })
}

impl Texture {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Texture> {
        Texture::parse(&fs::read(path)?)
    }

    pub fn parse(data: &[u8]) -> io::Result<Texture> {
        let t = Bytes(data);
        if data.get(..4) != Some(b"VTF\0") {
            return Err(bad("not a VTF texture"));
        }
        let minor = t.u32(8)?;
        let header = t.u32(12)? as usize;
        let (w, h) = (t.u16(16)? as usize, t.u16(18)? as usize);
        let frames = (t.u16(24)? as usize).max(1);
        let format = t.i32(52)?;
        let mips = t.u8(56)? as usize;
        let low_format = t.i32(57)?;
        let (lw, lh) = (t.u8(61)? as usize, t.u8(62)? as usize);
        let size = |m: usize| image_size(format, (w >> m).max(1), (h >> m).max(1)).ok_or_else(|| bad(&format!("VTF image format {format}")));
        // where the full-size images start: after the small preview (7.2 and older), or as
        // the 7.3 resource table says
        let mut start = if minor >= 3 {
            let count = t.u32(68)? as usize;
            (0..count).map(|i| 80 + i * 8).find(|&o| data.get(o..o + 3) == Some(&[0x30, 0, 0][..])).map(|o| t.u32(o + 4)).transpose()?.ok_or_else(|| bad("VTF without an image"))? as usize
        } else {
            header + if low_format < 0 { 0 } else { image_size(low_format, lw, lh).unwrap_or(0) }
        };
        // mips run smallest first; skip to the biggest, frame 0
        for m in (1..mips.max(1)).rev() {
            start += size(m)? * frames;
        }
        let bytes = data.get(start..start + size(0)?).ok_or_else(|| bad("VTF truncated"))?;
        Ok(Texture { width: w as u32, height: h as u32, rgba: decode(format, w, h, bytes)? })
    }
}

fn decode(format: i32, w: usize, h: usize, src: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = vec![0u8; w * h * 4];
    let px = |out: &mut Vec<u8>, i: usize, c: [u8; 4]| out[i * 4..i * 4 + 4].copy_from_slice(&c);
    match format {
        0 => out.copy_from_slice(&src[..w * h * 4]),
        12 | 16 => {
            for i in 0..w * h {
                let s = &src[i * 4..];
                px(&mut out, i, [s[2], s[1], s[0], if format == 16 { 255 } else { s[3] }]);
            }
        }
        1 => (0..w * h).for_each(|i| px(&mut out, i, [src[i * 4 + 3], src[i * 4 + 2], src[i * 4 + 1], src[i * 4]])),
        11 => (0..w * h).for_each(|i| px(&mut out, i, [src[i * 4 + 1], src[i * 4 + 2], src[i * 4 + 3], src[i * 4]])),
        2 | 9 => (0..w * h).for_each(|i| px(&mut out, i, [src[i * 3], src[i * 3 + 1], src[i * 3 + 2], 255])),
        3 | 10 => (0..w * h).for_each(|i| px(&mut out, i, [src[i * 3 + 2], src[i * 3 + 1], src[i * 3], 255])),
        5 => (0..w * h).for_each(|i| px(&mut out, i, [src[i], src[i], src[i], 255])),
        13 | 14 | 15 | 20 => {
            let block = if matches!(format, 14 | 15) { 16 } else { 8 };
            let bw = w.div_ceil(4);
            for (n, b) in src.chunks_exact(block).enumerate() {
                let (bx, by) = (n % bw * 4, n / bw * 4);
                let (alpha, color) = if block == 16 { b.split_at(8) } else { (&b[..0], b) };
                let rgb = dxt_colors(color, format == 13 || format == 20);
                let codes = u32::from_le_bytes(color[4..8].try_into().unwrap());
                for k in 0..16 {
                    let (x, y) = (bx + k % 4, by + k / 4);
                    if x >= w || y >= h {
                        continue;
                    }
                    let mut c = rgb[(codes >> (k * 2) & 3) as usize];
                    if format == 14 {
                        let bits = u64::from_le_bytes(alpha.try_into().unwrap());
                        c[3] = ((bits >> (k * 4) & 15) * 17) as u8;
                    } else if format == 15 {
                        c[3] = dxt5_alpha(alpha, k);
                    }
                    px(&mut out, y * w + x, c);
                }
            }
        }
        f => return Err(bad(&format!("VTF image format {f} not decoded"))),
    }
    Ok(out)
}

/// The four colours of a DXT colour block (DXT1 may have a transparent fourth).
fn dxt_colors(b: &[u8], dxt1: bool) -> [[u8; 4]; 4] {
    let (c0, c1) = (u16::from_le_bytes([b[0], b[1]]), u16::from_le_bytes([b[2], b[3]]));
    let rgb = |c: u16| [((c >> 11 & 31) * 255 / 31) as u8, ((c >> 5 & 63) * 255 / 63) as u8, ((c & 31) * 255 / 31) as u8, 255];
    let (a, b) = (rgb(c0), rgb(c1));
    let mix = |p: u32, q: u32, d: u32| -> [u8; 4] { std::array::from_fn(|i| if i == 3 { 255 } else { ((a[i] as u32 * p + b[i] as u32 * q) / d) as u8 }) };
    if c0 > c1 || !dxt1 {
        [a, b, mix(2, 1, 3), mix(1, 2, 3)]
    } else {
        [a, b, mix(1, 1, 2), [0, 0, 0, 0]]
    }
}

fn dxt5_alpha(b: &[u8], k: usize) -> u8 {
    let (a0, a1) = (b[0] as u32, b[1] as u32);
    let bits = u64::from_le_bytes([b[2], b[3], b[4], b[5], b[6], b[7], 0, 0]);
    let code = (bits >> (k * 3) & 7) as u32;
    (match code {
        0 => a0,
        1 => a1,
        c if a0 > a1 => ((8 - c) * a0 + (c - 1) * a1) / 7,
        6 => 0,
        7 => 255,
        c => ((6 - c) * a0 + (c - 1) * a1) / 5,
    }) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vmt_reads_the_texture_and_flags() {
        let m = Material::parse("\"VertexlitGeneric\"\n{\n\t\"$basetexture\" \"models\\player\\x\\hair\"\n\t\"$alphatest\" \"1\"\n}");
        assert_eq!(m, Material { base_texture: "models/player/x/hair".into(), translucent: false, alpha_test: true, double_sided: false });
    }

    #[test]
    fn dxt1_block_decodes_its_two_colours() {
        // pure red (c0) and pure blue (c1), codes 0 then 1 alternating along each row
        let block = [0x00, 0xF8, 0x1F, 0x00, 0x44, 0x44, 0x44, 0x44];
        let out = decode(13, 4, 4, &block).unwrap();
        assert_eq!(&out[..4], &[255, 0, 0, 255]);
        assert_eq!(&out[4..8], &[0, 0, 255, 255]);
    }

    /// The characters in the game data load whole (skipped when they're not there).
    #[test]
    fn character_models_load() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../gamedata/characters");
        let Ok(dirs) = std::fs::read_dir(&root) else { return };
        for dir in dirs.flatten() {
            let Some(mdl) = std::fs::read_dir(dir.path()).ok().and_then(|d| d.flatten().map(|e| e.path()).find(|p| p.extension().is_some_and(|e| e == "mdl"))) else { continue };
            let model = Model::load(&mdl).unwrap_or_else(|e| panic!("{}: {e}", mdl.display()));
            assert!(model.bones.iter().any(|b| b.name == "ValveBiped.Bip01_Pelvis"), "{}", mdl.display());
            let tris: usize = model.meshes.iter().map(|m| m.indices.len() / 3).sum();
            assert!(tris > 1000, "{}: {tris} triangles", mdl.display());
            for mesh in &model.meshes {
                assert!(mesh.indices.iter().all(|&i| (i as usize) < mesh.vertices.len()));
                assert!(mesh.material < model.materials.len());
            }
            // every material's texture is in the folder and decodes
            for name in &model.materials {
                let base = Material::load(dir.path().join(format!("{name}.vmt"))).unwrap_or_else(|e| panic!("{name}.vmt: {e}")).base_texture;
                let file = base.rsplit('/').next().unwrap();
                let tex = Texture::load(dir.path().join(format!("{file}.vtf"))).unwrap_or_else(|e| panic!("{file}.vtf: {e}"));
                assert_eq!(tex.rgba.len(), (tex.width * tex.height * 4) as usize);
            }
        }
    }
}
