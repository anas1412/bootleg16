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

/// GoldSrc `rendermode` / `renderamt` of a brush entity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Render {
    /// 0 normal, 1 color, 2 texture (see-through by `amt`), 3 glow, 4 solid (masked), 5 additive.
    pub mode: u8,
    /// 0..1 opacity for modes 1, 2 and 5.
    pub amt: f32,
}

pub struct MapGeometry {
    /// The world and everything that never moves.
    pub batches: Vec<Batch>,
    /// Doors: (brush model index, batches in the model's own space). The sim says where they are.
    pub movers: Vec<(usize, Vec<Batch>)>,
    /// Brush entities drawn on their own: breakables (hidden once broken) and see-through ones.
    pub pieces: Vec<(usize, Render, Vec<Batch>)>,
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

/// A loaded map's geometry and its WADs. `search` lists the directories to look in for the
/// WAD files the map names.
pub fn load(bsp: &Bsp, search: &[&Path]) -> MapGeometry {
    let wads: Vec<Wad> = bsp
        .wad_names()
        .iter()
        .filter_map(|n| search.iter().find_map(|d| Wad::load(d.join(n.to_lowercase())).ok()))
        .collect();
    let textures: Vec<Texture> = bsp
        .textures
        .iter()
        .map(|t| {
            // Our texture files first: a map's own copy (many carry one) is only the fallback.
            wads.iter()
                .find_map(|w| w.texture(&t.name))
                .or_else(|| t.pixels.clone())
                .unwrap_or_else(|| checkerboard(&t.name))
        })
        .collect();

    // Which brush models to draw: the world, plus visible brush entities.
    // (model, offset, group): group 0 is static; each door gets its own group.
    let mut models: Vec<(usize, [f32; 3], usize)> = vec![(0, [0.0; 3], 0)];
    let mut mover_models = Vec::new();
    let mut piece_models: Vec<(usize, Render)> = Vec::new();
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
                let render = Render {
                    mode: e.get("rendermode").and_then(|v| v.parse().ok()).unwrap_or(0),
                    amt: e.get("renderamt").and_then(|v| v.parse::<f32>().ok()).unwrap_or(255.0) / 255.0,
                };
                if matches!(class, "func_door" | "func_door_rotating" | "func_button" | "func_rot_button") {
                    mover_models.push(i);
                    models.push((i, [0.0; 3], mover_models.len()));
                } else if class == "func_breakable" || matches!(render.mode, 1 | 2 | 3 | 5) {
                    // Groups after the doors' (renumbered below, once all doors are known).
                    piece_models.push((i, render));
                    models.push((i, origin, usize::MAX - (piece_models.len() - 1)));
                } else {
                    models.push((i, origin, 0));
                }
            }
        }
    }

    let mut atlas = Atlas { rgba: Vec::new(), x: 0, y: 0, shelf_h: 0 };
    // Fully-lit 2x2 block for faces without lightmap data.
    let fullbright = atlas.insert(2, 2, &[255; 12]);
    let new_group = || -> Vec<Batch> { (0..textures.len()).map(|t| Batch { texture: t, ..Default::default() }).collect() };
    // Pieces' groups come after the doors'.
    let doors = mover_models.len();
    for m in &mut models {
        if m.2 > usize::MAX / 2 {
            m.2 = doors + 1 + (usize::MAX - m.2);
        }
    }
    let mut groups: Vec<Vec<Batch>> = (0..=doors + piece_models.len()).map(|_| new_group()).collect();
    let mut lm_rects = Vec::new(); // (group, batch, first vertex, atlas pos, smin, tmin)

    for (model, offset, group) in models {
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

            let b = &mut groups[group][ti.miptex as usize];
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
                    lm_rects.push((group, ti.miptex as usize, first, st, at, bs, bt));
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
    for b in groups.iter_mut().flatten() {
        for uv in &mut b.lightmap_uv {
            *uv = [uv[0] / ATLAS_W as f32, uv[1] / ah as f32];
        }
    }
    for (g, bi, first, st, (ax, ay), bs, bt) in lm_rects {
        let b = &mut groups[g][bi];
        let at = first as usize;
        for (i, (s, t)) in st.into_iter().enumerate() {
            let u = (ax as f32 + s / 16.0 - bs + 0.5) / ATLAS_W as f32;
            let v = (ay as f32 + t / 16.0 - bt + 0.5) / ah as f32;
            b.lightmap_uv[at + i] = [u, v];
        }
    }
    for g in &mut groups {
        g.retain(|b| !b.indices.is_empty());
    }
    let mut groups = groups.into_iter();
    let batches = groups.next().unwrap_or_default();
    let movers = mover_models.into_iter().zip(groups.by_ref().take(doors)).collect();
    let pieces = piece_models.into_iter().zip(groups).map(|((m, r), g)| (m, r, g)).collect();

    MapGeometry { batches, movers, pieces, textures, lightmap_size: (ATLAS_W, ah), lightmap_rgba: atlas.rgba, spawns }
}
