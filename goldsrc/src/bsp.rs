//! GoldSrc BSP v30 reader: geometry, texture refs, lightmaps, entities, models.
use crate::texture::{decode_miptex, Texture};
use std::{collections::HashMap, fmt, fs, io, path::Path};

pub const BSP_VERSION: i32 = 30;

const L_ENTITIES: usize = 0;
const L_PLANES: usize = 1;
const L_TEXTURES: usize = 2;
const L_VERTICES: usize = 3;
const L_NODES: usize = 5;
const L_TEXINFO: usize = 6;
const L_FACES: usize = 7;
const L_LIGHTING: usize = 8;
const L_CLIPNODES: usize = 9;
const L_LEAVES: usize = 10;
const L_EDGES: usize = 12;
const L_SURFEDGES: usize = 13;
const L_MODELS: usize = 14;
const NUM_LUMPS: usize = 15;

#[derive(Debug)]
pub enum BspError {
    Io(io::Error),
    BadVersion(i32),
    Truncated(&'static str),
}

impl fmt::Display for BspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BspError::Io(e) => write!(f, "io: {e}"),
            BspError::BadVersion(v) => write!(f, "unsupported BSP version {v} (want {BSP_VERSION})"),
            BspError::Truncated(what) => write!(f, "truncated {what}"),
        }
    }
}

impl std::error::Error for BspError {}

impl From<io::Error> for BspError {
    fn from(e: io::Error) -> Self {
        BspError::Io(e)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: [f32; 3],
    pub dist: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct TexInfo {
    /// s/t axes (xyz + offset), used for texture and lightmap coordinates.
    pub s: [f32; 4],
    pub t: [f32; 4],
    pub miptex: u32,
    pub flags: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Face {
    pub plane: u16,
    pub side: u16,
    pub first_edge: u32,
    pub num_edges: u16,
    pub texinfo: u16,
    pub styles: [u8; 4],
    /// Byte offset into the lighting lump, or -1 if unlit.
    pub light_offset: i32,
}

#[derive(Debug, Clone)]
pub struct MipTex {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Decoded pixels when embedded in the map; None means look it up in a WAD.
    pub pixels: Option<Texture>,
}

#[derive(Debug, Clone, Copy)]
pub struct Model {
    /// Root node per collision hull: 0 = BSP nodes (point), 1 = standing player,
    /// 2 = large monsters, 3 = crouched player. Hulls 1-3 index `Bsp::clipnodes`.
    pub head_nodes: [i32; 4],
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub origin: [f32; 3],
    pub first_face: u32,
    pub num_faces: u32,
}

#[derive(Debug, Default)]
pub struct Bsp {
    pub entities: String,
    pub planes: Vec<Plane>,
    pub vertices: Vec<[f32; 3]>,
    pub edges: Vec<[u16; 2]>,
    pub surfedges: Vec<i32>,
    pub texinfo: Vec<TexInfo>,
    pub faces: Vec<Face>,
    pub textures: Vec<MipTex>,
    /// Raw RGB lightmap bytes, indexed by `Face::light_offset`.
    pub lighting: Vec<u8>,
    pub models: Vec<Model>,
    pub clipnodes: Vec<ClipNode>,
    pub nodes: Vec<Node>,
    /// Contents of each leaf (-1 empty, -2 solid, -3 water, -4 slime, -5 lava, -16 ladder...).
    pub leaf_contents: Vec<i32>,
}

/// Render/point BSP node (hull 0). Children >= 0 are nodes; negative child c is leaf -(c + 1).
#[derive(Debug, Clone, Copy)]
pub struct Node {
    pub plane: u32,
    pub children: [i16; 2],
    /// Faces lying on this node's plane.
    pub first_face: u16,
    pub num_faces: u16,
}

/// Collision-hull BSP node. Negative children are leaf contents (-1 empty, -2 solid).
#[derive(Debug, Clone, Copy)]
pub struct ClipNode {
    pub plane: u32,
    pub children: [i16; 2],
}

struct Reader<'a> {
    data: &'a [u8],
    what: &'static str,
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8], what: &'static str) -> Self {
        Self { data, what, pos: 0 }
    }
    fn take<const N: usize>(&mut self) -> Result<[u8; N], BspError> {
        let s = self.data.get(self.pos..self.pos + N).ok_or(BspError::Truncated(self.what))?;
        self.pos += N;
        Ok(s.try_into().unwrap())
    }
    fn i32(&mut self) -> Result<i32, BspError> {
        Ok(i32::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32, BspError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn u16(&mut self) -> Result<u16, BspError> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    fn u8(&mut self) -> Result<u8, BspError> {
        Ok(self.take::<1>()?[0])
    }
    fn f32(&mut self) -> Result<f32, BspError> {
        Ok(f32::from_le_bytes(self.take()?))
    }
    fn f32x3(&mut self) -> Result<[f32; 3], BspError> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }
    fn more(&self) -> bool {
        self.pos < self.data.len()
    }
}

impl Bsp {
    pub fn load(path: impl AsRef<Path>) -> Result<Bsp, BspError> {
        Self::parse(&fs::read(path)?)
    }

    pub fn parse(data: &[u8]) -> Result<Bsp, BspError> {
        let mut h = Reader::new(data, "header");
        let version = h.i32()?;
        if version != BSP_VERSION {
            return Err(BspError::BadVersion(version));
        }
        let mut lumps = [(0usize, 0usize); NUM_LUMPS];
        for l in &mut lumps {
            *l = (h.i32()? as usize, h.i32()? as usize);
        }
        let lump = |i: usize, what: &'static str| -> Result<Reader, BspError> {
            let (off, len) = lumps[i];
            let s = data.get(off..off + len).ok_or(BspError::Truncated(what))?;
            Ok(Reader::new(s, what))
        };

        let mut bsp = Bsp::default();

        bsp.entities = String::from_utf8_lossy(lump(L_ENTITIES, "entities")?.data).trim_end_matches('\0').to_string();

        let mut r = lump(L_PLANES, "planes")?;
        while r.more() {
            let normal = r.f32x3()?;
            let dist = r.f32()?;
            r.i32()?; // axis-type hint
            bsp.planes.push(Plane { normal, dist });
        }

        let mut r = lump(L_VERTICES, "vertices")?;
        while r.more() {
            bsp.vertices.push(r.f32x3()?);
        }

        let mut r = lump(L_EDGES, "edges")?;
        while r.more() {
            bsp.edges.push([r.u16()?, r.u16()?]);
        }

        let mut r = lump(L_SURFEDGES, "surfedges")?;
        while r.more() {
            bsp.surfedges.push(r.i32()?);
        }

        let mut r = lump(L_TEXINFO, "texinfo")?;
        while r.more() {
            let s = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
            let t = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
            bsp.texinfo.push(TexInfo { s, t, miptex: r.u32()?, flags: r.u32()? });
        }

        let mut r = lump(L_FACES, "faces")?;
        while r.more() {
            bsp.faces.push(Face {
                plane: r.u16()?,
                side: r.u16()?,
                first_edge: r.u32()?,
                num_edges: r.u16()?,
                texinfo: r.u16()?,
                styles: [r.u8()?, r.u8()?, r.u8()?, r.u8()?],
                light_offset: r.i32()?,
            });
        }

        bsp.lighting = lump(L_LIGHTING, "lighting")?.data.to_vec();

        let (tex_off, tex_len) = lumps[L_TEXTURES];
        if tex_len > 0 {
            let tex = data.get(tex_off..tex_off + tex_len).ok_or(BspError::Truncated("textures"))?;
            let mut r = Reader::new(tex, "textures");
            let count = r.i32()? as usize;
            for _ in 0..count {
                let off = r.i32()?;
                if off < 0 {
                    // Unused slot; keep indices aligned with texinfo.
                    bsp.textures.push(MipTex { name: String::new(), width: 0, height: 0, pixels: None });
                    continue;
                }
                let mut m = Reader::new(tex.get(off as usize..).ok_or(BspError::Truncated("miptex"))?, "miptex");
                let raw = m.take::<16>()?;
                let name = crate::texture::cstr(&raw);
                let (width, height) = (m.u32()?, m.u32()?);
                let pixels = decode_miptex(&tex[off as usize..]);
                bsp.textures.push(MipTex { name, width, height, pixels });
            }
        }

        let mut r = lump(L_NODES, "nodes")?;
        while r.more() {
            let plane = r.u32()?;
            let children = [r.u16()? as i16, r.u16()? as i16];
            r.take::<12>()?; // bounds
            let (first_face, num_faces) = (r.u16()?, r.u16()?);
            bsp.nodes.push(Node { plane, children, first_face, num_faces });
        }

        let mut r = lump(L_LEAVES, "leaves")?;
        while r.more() {
            bsp.leaf_contents.push(r.i32()?);
            r.take::<24>()?; // vis offset, bounds, mark surfaces, ambient sounds
        }

        let mut r = lump(L_CLIPNODES, "clipnodes")?;
        while r.more() {
            let plane = r.u32()?;
            bsp.clipnodes.push(ClipNode { plane, children: [r.u16()? as i16, r.u16()? as i16] });
        }

        let mut r = lump(L_MODELS, "models")?;
        while r.more() {
            let (mins, maxs, origin) = (r.f32x3()?, r.f32x3()?, r.f32x3()?);
            let head_nodes = [r.i32()?, r.i32()?, r.i32()?, r.i32()?];
            r.i32()?; // visleafs
            bsp.models.push(Model { head_nodes, mins, maxs, origin, first_face: r.u32()?, num_faces: r.u32()? });
        }

        Ok(bsp)
    }

    /// Parse the entity lump into key/value maps, in file order.
    pub fn entity_list(&self) -> Vec<HashMap<String, String>> {
        let mut out = Vec::new();
        let mut cur: Option<HashMap<String, String>> = None;
        let mut key: Option<String> = None;
        let mut chars = self.entities.chars();
        while let Some(c) = chars.next() {
            match c {
                '{' => cur = Some(HashMap::new()),
                '}' => out.extend(cur.take()),
                '"' => {
                    let s: String = chars.by_ref().take_while(|&c| c != '"').collect();
                    match key.take() {
                        None => key = Some(s),
                        Some(k) => {
                            if let Some(e) = cur.as_mut() {
                                e.insert(k, s);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// WAD file names (no directories) listed in worldspawn's "wad" key.
    pub fn wad_names(&self) -> Vec<String> {
        let list = self.entity_list().into_iter().find(|e| e.get("classname").map(String::as_str) == Some("worldspawn"));
        list.and_then(|e| e.get("wad").cloned())
            .unwrap_or_default()
            .split(';')
            .filter_map(|p| p.rsplit(['\\', '/']).next())
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// Vertex positions of a face, in winding order.
    pub fn face_vertices(&self, face: &Face) -> Vec<[f32; 3]> {
        (face.first_edge..face.first_edge + face.num_edges as u32)
            .map(|i| {
                let se = self.surfedges[i as usize];
                let e = self.edges[se.unsigned_abs() as usize];
                self.vertices[e[(se < 0) as usize] as usize]
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_other_versions() {
        let mut d = vec![0u8; 4 + NUM_LUMPS * 8];
        d[..4].copy_from_slice(&29i32.to_le_bytes());
        assert!(matches!(Bsp::parse(&d), Err(BspError::BadVersion(29))));
    }
}
