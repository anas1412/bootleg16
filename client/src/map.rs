//! Turns a GoldSrc BSP into renderer-neutral draw batches: one batch per texture,
//! with texture UVs, lightmap-atlas UVs and Y-up positions.
use goldsrc::{bsp::Bsp, texture::Texture, wad::Wad};
use std::path::Path;

pub const ATLAS_W: usize = 2048;
const PAD: usize = 1;

/// Brush entities that are never drawn (volumes, not geometry).
const INVISIBLE_CLASSES: &[&str] = &[
    "func_buyzone",
    "func_bomb_target",
    "func_hostage_rescue",
    "func_vip_safetyzone",
    "func_escapezone",
    "func_ladder",
];

/// Textures that are tool brushes, not visible surfaces.
fn is_tool_texture(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    matches!(n.as_str(), "sky" | "clip" | "origin" | "aaatrigger" | "null" | "bevel" | "hint" | "skip")
}

#[derive(Default)]
pub struct Batch {
    pub texture: usize,
    pub positions: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub lightmap_uv: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

pub struct MapGeometry {
    pub batches: Vec<Batch>,
    /// Decoded textures, indexed by `Batch::texture`. Missing ones are a checkerboard.
    pub textures: Vec<Texture>,
    pub lightmap_size: (usize, usize),
    pub lightmap_rgba: Vec<u8>,
    /// Spawn points in Y-up space: (origin, yaw in degrees).
    pub spawns: Vec<([f32; 3], f32)>,
}

/// GoldSrc is Z-up (x forward, y left); Bevy is Y-up. A proper rotation, so
/// handedness is kept: (x, y, z) -> (x, z, -y).
pub fn to_y_up(v: [f32; 3]) -> [f32; 3] {
    [v[0], v[2], -v[1]]
}

fn dot(a: [f32; 3], b: &[f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + b[3]
}

/// Shelf packer for lightmap rectangles.
struct Atlas {
    rgba: Vec<u8>,
    x: usize,
    y: usize,
    shelf_h: usize,
}

impl Atlas {
    fn height(&self) -> usize {
        self.rgba.len() / (ATLAS_W * 4)
    }

    /// Place a w*h RGB block (with a 1px border copied from its edges); returns its top-left.
    fn insert(&mut self, w: usize, h: usize, rgb: &[u8]) -> (usize, usize) {
        let (pw, ph) = (w + 2 * PAD, h + 2 * PAD);
        if self.x + pw > ATLAS_W {
            self.x = 0;
            self.y += self.shelf_h;
            self.shelf_h = 0;
        }
        let need = (self.y + ph) * ATLAS_W * 4;
        if self.rgba.len() < need {
            self.rgba.resize(need, 255);
        }
        for py in 0..ph {
            for px in 0..pw {
                let sx = px.saturating_sub(PAD).min(w - 1);
                let sy = py.saturating_sub(PAD).min(h - 1);
                let s = (sy * w + sx) * 3;
                let d = ((self.y + py) * ATLAS_W + self.x + px) * 4;
                self.rgba[d..d + 4].copy_from_slice(&[rgb[s], rgb[s + 1], rgb[s + 2], 255]);
            }
        }
        let at = (self.x + PAD, self.y + PAD);
        self.x += pw;
        self.shelf_h = self.shelf_h.max(ph);
        at
    }
}

fn checkerboard(name: &str) -> Texture {
    let rgba = (0..64)
        .flat_map(|i| if (i % 8 < 4) ^ (i / 8 < 4) { [255, 0, 255, 255] } else { [0, 0, 0, 255] })
        .collect();
    Texture { name: name.to_string(), width: 8, height: 8, rgba }
}

/// Load a map and its WADs. `search` lists game directories (e.g. cstrike, valve)
/// to look in for the WAD files the map names.
pub fn load(map: &Path, search: &[&Path]) -> Result<MapGeometry, Box<dyn std::error::Error>> {
    let bsp = Bsp::load(map)?;
    let wads: Vec<Wad> = bsp
        .wad_names()
        .iter()
        .filter_map(|n| search.iter().find_map(|d| Wad::load(d.join(n)).ok()))
        .collect();
    let textures: Vec<Texture> = bsp
        .textures
        .iter()
        .map(|t| {
            t.pixels
                .clone()
                .or_else(|| wads.iter().find_map(|w| w.texture(&t.name)))
                .unwrap_or_else(|| checkerboard(&t.name))
        })
        .collect();

    // Which brush models to draw: the world, plus visible brush entities.
    let mut models: Vec<(usize, [f32; 3])> = vec![(0, [0.0; 3])];
    let mut spawns = Vec::new();
    for e in bsp.entity_list() {
        let class = e.get("classname").map(String::as_str).unwrap_or("");
        let origin = e
            .get("origin")
            .map(|o| {
                let v: Vec<f32> = o.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)]
            })
            .unwrap_or([0.0; 3]);
        if matches!(class, "info_player_start" | "info_player_deathmatch") {
            let yaw = e.get("angles").and_then(|a| a.split_whitespace().nth(1)?.parse().ok()).unwrap_or(0.0);
            spawns.push((to_y_up(origin), yaw));
        }
        if class.starts_with("trigger_") || INVISIBLE_CLASSES.contains(&class) {
            continue;
        }
        if let Some(i) = e.get("model").and_then(|m| m.strip_prefix('*')?.parse::<usize>().ok()) {
            if i < bsp.models.len() {
                models.push((i, origin));
            }
        }
    }

    let mut atlas = Atlas { rgba: Vec::new(), x: 0, y: 0, shelf_h: 0 };
    // Fully-lit 2x2 block for faces without lightmap data.
    let fullbright = atlas.insert(2, 2, &[255; 12]);
    let mut batches: Vec<Batch> = (0..textures.len()).map(|t| Batch { texture: t, ..Default::default() }).collect();
    let mut lm_rects = Vec::new(); // (batch, first vertex, atlas pos, smin, tmin)

    for (model, offset) in models {
        let m = bsp.models[model];
        for face in &bsp.faces[m.first_face as usize..(m.first_face + m.num_faces) as usize] {
            let ti = bsp.texinfo[face.texinfo as usize];
            let tex = &textures[ti.miptex as usize];
            if is_tool_texture(&tex.name) {
                continue;
            }
            let verts: Vec<[f32; 3]> = bsp
                .face_vertices(face)
                .into_iter()
                .map(|v| [v[0] + offset[0], v[1] + offset[1], v[2] + offset[2]])
                .collect();
            // Lightmap extents in 16-unit luxels, computed from the un-offset vertices.
            let st: Vec<(f32, f32)> = bsp.face_vertices(face).iter().map(|&v| (dot(v, &ti.s), dot(v, &ti.t))).collect();
            let (mut smin, mut smax, mut tmin, mut tmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for &(s, t) in &st {
                smin = smin.min(s);
                smax = smax.max(s);
                tmin = tmin.min(t);
                tmax = tmax.max(t);
            }
            let (bs, bt) = ((smin / 16.0).floor(), (tmin / 16.0).floor());
            let lw = ((smax / 16.0).ceil() - bs) as usize + 1;
            let lh = ((tmax / 16.0).ceil() - bt) as usize + 1;
            let lit = face.light_offset >= 0 && face.styles[0] != 255;
            let data = lit
                .then(|| bsp.lighting.get(face.light_offset as usize..face.light_offset as usize + lw * lh * 3))
                .flatten();

            let b = &mut batches[ti.miptex as usize];
            let first = b.positions.len() as u32;
            for (i, v) in verts.iter().enumerate() {
                b.positions.push(to_y_up(*v));
                b.uv.push([st[i].0 / tex.width as f32, st[i].1 / tex.height as f32]);
            }
            // GoldSrc winds front faces clockwise; Bevy wants counter-clockwise.
            for i in 1..verts.len() as u32 - 1 {
                b.indices.extend([first, first + i + 1, first + i]);
            }
            match data {
                Some(rgb) => {
                    let at = atlas.insert(lw, lh, rgb);
                    b.lightmap_uv.extend(std::iter::repeat_n([0.0; 2], verts.len())); // filled in below
                    lm_rects.push((ti.miptex as usize, first, st, at, bs, bt));
                }
                None => {
                    let c = [(fullbright.0 as f32 + 1.0), (fullbright.1 as f32 + 1.0)];
                    b.lightmap_uv.extend(std::iter::repeat_n(c, verts.len()));
                }
            }
        }
    }

    // Atlas height is only known now, so lightmap UVs are filled in last.
    let ah = atlas.height().next_power_of_two();
    atlas.rgba.resize(ah * ATLAS_W * 4, 255);
    for b in &mut batches {
        for uv in &mut b.lightmap_uv {
            *uv = [uv[0] / ATLAS_W as f32, uv[1] / ah as f32];
        }
    }
    for (bi, first, st, (ax, ay), bs, bt) in lm_rects {
        let b = &mut batches[bi];
        let at = first as usize;
        for (i, (s, t)) in st.into_iter().enumerate() {
            let u = (ax as f32 + s / 16.0 - bs + 0.5) / ATLAS_W as f32;
            let v = (ay as f32 + t / 16.0 - bt + 0.5) / ah as f32;
            b.lightmap_uv[at + i] = [u, v];
        }
    }
    batches.retain(|b| !b.indices.is_empty());

    Ok(MapGeometry { batches, textures, lightmap_size: (ATLAS_W, ah), lightmap_rgba: atlas.rgba, spawns })
}
