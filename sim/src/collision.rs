//! Swept player-hull traces against the map's precomputed collision hulls.
//!
//! GoldSrc compiles each map with "expanded" hulls: the world grown by the player
//! box. Tracing the player's centre point through that hull equals sweeping the
//! box through the real world.
use crate::doors::{angle_vectors, local_to_world, world_to_local, Actor, Doors};
use crate::brush::{self, Brush, Clip, Contents as BrushContents};
use crate::{add, dot, length, lerp, sub, Vec3};
use goldsrc::bsp::Bsp;

const CONTENTS_EMPTY: i32 = -1;
const CONTENTS_SOLID: i32 = -2;
/// Traces stop this far short of a surface so the next one doesn't start inside it.
const DIST_EPSILON: f32 = 0.03125;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Hull {
    Standing = 1,
    Crouched = 3,
}

impl Hull {
    /// Box size relative to the player's origin (the box centre).
    pub fn mins(self) -> Vec3 {
        match self {
            Hull::Standing => [-16.0, -16.0, -36.0],
            Hull::Crouched => [-16.0, -16.0, -18.0],
        }
    }
    pub fn maxs(self) -> Vec3 {
        match self {
            Hull::Standing => [16.0, 16.0, 36.0],
            Hull::Crouched => [16.0, 16.0, 18.0],
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Trace {
    /// Started inside solid.
    pub start_solid: bool,
    /// The whole move was inside solid.
    pub all_solid: bool,
    /// 0..1: how far along the move it got.
    pub fraction: f32,
    pub end: Vec3,
    /// Normal of the surface hit (valid when fraction < 1).
    pub normal: Vec3,
    hit_contents: i32,
}

#[derive(Clone, Copy)]
struct Node {
    normal: Vec3,
    dist: f32,
    children: [i32; 2],
}

/// A solid brush model: the world (offset zero) or a brush entity at an offset.
struct Model {
    heads: [i32; 4],
    offset: Vec3,
    /// Index into `World::doors` when this model moves.
    door: Option<usize>,
    bsp_model: usize,
    /// False once broken (a `func_breakable` that was shot or blown up): no longer solid.
    enabled: bool,
    /// Drawn normally (`rendermode` 0): you can't see through it. Glass, fences and other
    /// see-through brushes aren't.
    opaque: bool,
    /// World-space bounds (where it sits closed, for doors), for skipping far-off models.
    bounds: (Vec3, Vec3),
}

/// Which BSP tree a trace walks: the player-sized clip hulls, or the exact
/// geometry (hull 0, the render tree) that bullets use.
#[derive(Clone, Copy)]
enum Tree<'a> {
    Clip(&'a [Node]),
    Point(&'a [Node], &'a [i32]),
}

const CONTENTS_SKY: i32 = -6;

impl Tree<'_> {
    fn nodes(&self) -> &[Node] {
        match self {
            Tree::Clip(n) | Tree::Point(n, _) => n,
        }
    }
    /// Contents of a leaf reached as child `num` (< 0).
    fn leaf(&self, num: i32) -> i32 {
        match self {
            Tree::Clip(_) => num,
            Tree::Point(_, leaves) => leaves.get((-(num + 1)) as usize).copied().unwrap_or(CONTENTS_EMPTY),
        }
    }
    fn blocks(&self, contents: i32) -> bool {
        contents == CONTENTS_SOLID || (matches!(self, Tree::Point(..)) && contents == CONTENTS_SKY)
    }
    fn contents(&self, mut num: i32, p: Vec3) -> i32 {
        let nodes = self.nodes();
        while num >= 0 {
            let n = &nodes[num as usize];
            num = n.children[(dot(n.normal, p) - n.dist < 0.0) as usize];
        }
        self.leaf(num)
    }
}

/// A bullet trace through the exact map geometry.
#[derive(Clone, Copy, Debug)]
pub struct LineTrace {
    pub fraction: f32,
    pub end: Vec3,
    pub normal: Vec3,
    /// Started inside solid (e.g. the muzzle is in a wall).
    pub start_solid: bool,
    /// Stopped by the sky (no decal, no penetration).
    pub hit_sky: bool,
    /// The BSP model hit (0 = the world, else a brush entity such as a breakable).
    pub model: usize,
}

/// Point `p` in a model's own space, given where its door (if any) currently is.
fn to_local(m: &Model, door: Option<&crate::doors::Door>, p: Vec3) -> Vec3 {
    let Some(d) = door else { return sub(p, m.offset) };
    let (off, ang) = d.transform();
    let rel = sub(p, add(m.offset, off));
    if ang == [0.0; 3] {
        return rel;
    }
    let (f, r, u) = angle_vectors(ang);
    world_to_local(rel, f, r, u)
}

fn contents(nodes: &[Node], mut num: i32, p: Vec3) -> i32 {
    while num >= 0 {
        let n = &nodes[num as usize];
        num = n.children[(dot(n.normal, p) - n.dist < 0.0) as usize];
    }
    num
}

/// Point contents a player can be in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Contents {
    Empty,
    Solid,
    Water,
}

/// Axis-aligned box of a ladder brush, in world space.
#[derive(Clone, Copy, Debug)]
pub struct Ladder {
    pub mins: Vec3,
    pub maxs: Vec3,
}

/// Everything the movement code collides with.
pub struct World {
    nodes: Vec<Node>,
    models: Vec<Model>,
    /// Hull-0 BSP (render nodes): the only tree that knows about water.
    point_nodes: Vec<Node>,
    leaf_contents: Vec<i32>,
    /// Hull-0 roots to test for water: the world plus `func_water` brushes.
    water_roots: Vec<(i32, Vec3)>,
    pub ladders: Vec<Ladder>,
    pub doors: Doors,
    /// Buy zones, bomb sites.
    pub zones: crate::zones::Zones,
    /// Bodies lying on the floor (`bodies::rest_box`): boxes players bump into and step over
    /// (not bullets). Set each tick from the bodies, the same on the server and the client.
    pub boxes: Vec<(Vec3, Vec3)>,
    /// The convex brushes of a map that isn't a BSP (`from_brushes`): walls, glass, sky, water.
    brushes: Vec<Brush>,
    /// Faces on each point node, for finding the texture a bullet hit.
    node_faces: Vec<(u32, u32)>,
    surfaces: Vec<Surface>,
    textures: Vec<String>,
}

/// Enough of a face to tell whether a point on its plane is inside it.
#[derive(Clone, Copy)]
struct Surface {
    s: [f32; 4],
    t: [f32; 4],
    /// Texture-space extents of the face.
    min: [f32; 2],
    max: [f32; 2],
    texture: u32,
}

/// Brush entities players walk through.
fn non_solid(class: &str) -> bool {
    class.starts_with("trigger_")
        || class.starts_with("func_buyzone")
        || matches!(
            class,
            "func_bomb_target" | "func_hostage_rescue" | "func_vip_safetyzone" | "func_escapezone" | "func_illusionary" | "func_ladder"
        )
}

impl World {
    pub fn from_bsp(bsp: &Bsp) -> World {
        let nodes = bsp
            .clipnodes
            .iter()
            .map(|c| {
                let p = bsp.planes[c.plane as usize];
                Node { normal: p.normal, dist: p.dist, children: [c.children[0] as i32, c.children[1] as i32] }
            })
            .collect();
        // Point nodes: map leaf children to the clip-node convention (negative = contents).
        let point_nodes = bsp
            .nodes
            .iter()
            .map(|n| {
                let p = bsp.planes[n.plane as usize];
                Node { normal: p.normal, dist: p.dist, children: [n.children[0] as i32, n.children[1] as i32] }
            })
            .collect();
        let doors = Doors::from_bsp(bsp);
        let mut models = vec![Model { heads: bsp.models[0].head_nodes, offset: [0.0; 3], door: None, bsp_model: 0, enabled: true, opaque: true, bounds: ([f32::MIN; 3], [f32::MAX; 3]) }];
        let mut water_roots = vec![(bsp.models[0].head_nodes[0], [0.0; 3])];
        let mut ladders = Vec::new();
        for e in bsp.entity_list() {
            let class = e.get("classname").map(String::as_str).unwrap_or("");
            let Some(i) = e.get("model").and_then(|m| m.strip_prefix('*')?.parse::<usize>().ok()) else { continue };
            if i == 0 || i >= bsp.models.len() {
                continue;
            }
            let o: Vec<f32> = e.get("origin").map(|o| o.split_whitespace().filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
            let offset = [o.first().copied().unwrap_or(0.0), o.get(1).copied().unwrap_or(0.0), o.get(2).copied().unwrap_or(0.0)];
            let m = bsp.models[i];
            match class {
                "func_ladder" => ladders.push(Ladder { mins: crate::add(m.mins, offset), maxs: crate::add(m.maxs, offset) }),
                "func_water" => water_roots.push((m.head_nodes[0], offset)),
                c if non_solid(c) => {}
                _ => {
                    let opaque = e.get("rendermode").and_then(|v| v.parse::<i32>().ok()).unwrap_or(0) == 0;
                    models.push(Model { heads: m.head_nodes, offset, door: doors.door_for_model(i), bsp_model: i, enabled: true, opaque, bounds: (crate::add(m.mins, offset), crate::add(m.maxs, offset)) })
                }
            }
        }
        let surfaces = bsp
            .faces
            .iter()
            .map(|f| {
                let ti = bsp.texinfo[f.texinfo as usize];
                let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
                for v in bsp.face_vertices(f) {
                    for (k, axis) in [ti.s, ti.t].iter().enumerate() {
                        let x = dot(v, [axis[0], axis[1], axis[2]]) + axis[3];
                        min[k] = min[k].min(x);
                        max[k] = max[k].max(x);
                    }
                }
                Surface { s: ti.s, t: ti.t, min, max, texture: ti.miptex }
            })
            .collect();
        World {
            nodes,
            models,
            point_nodes,
            leaf_contents: bsp.leaf_contents.clone(),
            water_roots,
            ladders,
            doors,
            zones: crate::zones::Zones::from_bsp(bsp),
            boxes: Vec::new(),
            brushes: Vec::new(),
            node_faces: bsp.nodes.iter().map(|n| (n.first_face as u32, n.num_faces as u32)).collect(),
            surfaces,
            textures: bsp.textures.iter().map(|t| t.name.clone()).collect(),
        }
    }

    /// Name of the world texture hit by the line `start`..`end` (first surface crossed).
    /// The world face (BSP face index) a line meets first, if any.
    pub fn face_at(&self, start: Vec3, end: Vec3) -> Option<usize> {
        self.surface_at(self.water_roots.first()?.0, start, end)
    }

    pub fn texture_at(&self, start: Vec3, end: Vec3) -> Option<&str> {
        let root = self.water_roots.first()?.0;
        let face = self.surface_at(root, start, end)?;
        self.textures.get(self.surfaces[face].texture as usize).map(String::as_str)
    }

    /// The texture a line meets on brush model `bsp_model` (0: the world), where it is now
    /// (GoldSrc `TRACE_TEXTURE` on the entity a bullet hit: glass, crates, doors).
    pub fn texture_of(&self, bsp_model: usize, start: Vec3, end: Vec3) -> Option<&str> {
        if bsp_model == 0 {
            return self.texture_at(start, end);
        }
        let m = self.models.iter().find(|m| m.bsp_model == bsp_model)?;
        let door = self.door_of(m);
        let face = self.surface_at(m.heads[0], to_local(m, door, start), to_local(m, door, end))?;
        self.textures.get(self.surfaces[face].texture as usize).map(String::as_str)
    }

    fn surface_at(&self, num: i32, start: Vec3, end: Vec3) -> Option<usize> {
        if num < 0 {
            return None;
        }
        let n = self.point_nodes[num as usize];
        let (d1, d2) = (dot(n.normal, start) - n.dist, dot(n.normal, end) - n.dist);
        let side = (d1 < 0.0) as usize;
        if (d2 < 0.0) as usize == side {
            return self.surface_at(n.children[side], start, end);
        }
        let mid = lerp(start, end, d1 / (d1 - d2));
        if let Some(f) = self.surface_at(n.children[side], start, mid) {
            return Some(f);
        }
        let (first, count) = self.node_faces[num as usize];
        for i in first..first + count {
            let sf = &self.surfaces[i as usize];
            let s = dot(mid, [sf.s[0], sf.s[1], sf.s[2]]) + sf.s[3];
            let t = dot(mid, [sf.t[0], sf.t[1], sf.t[2]]) + sf.t[3];
            if s >= sf.min[0] - 0.5 && s <= sf.max[0] + 0.5 && t >= sf.min[1] - 0.5 && t <= sf.max[1] + 0.5 {
                return Some(i as usize);
            }
        }
        self.surface_at(n.children[side ^ 1], mid, end)
    }

    /// Advance doors and triggers one tick.
    pub fn update(&mut self, dt: f32, actors: &[Actor]) {
        let (nodes, models) = (&self.nodes, &self.models);
        self.doors.update(dt, actors, |door, a| {
            let origin = crate::scale(add(a.mins, a.maxs), 0.5);
            let hull = if a.maxs[2] - a.mins[2] > 40.0 { Hull::Standing } else { Hull::Crouched };
            models
                .iter()
                .filter(|m| m.bsp_model == door.model)
                .any(|m| contents(nodes, m.heads[hull as usize], to_local(m, Some(door), origin)) == CONTENTS_SOLID)
        });
    }

    fn contents(&self, num: i32, p: Vec3) -> i32 {
        contents(&self.nodes, num, p)
    }

    fn door_of(&self, m: &Model) -> Option<&crate::doors::Door> {
        m.door.map(|i| &self.doors.doors[i])
    }

    /// Whether a move inside the box `lo..hi` can touch model `m` at all (its bounds where it
    /// is now; a rotating door is always checked): far-off models are skipped without walking
    /// their tree.
    fn reaches(&self, m: &Model, lo: Vec3, hi: Vec3) -> bool {
        let (mut bmin, mut bmax) = m.bounds;
        if let Some(d) = self.door_of(m) {
            let (off, ang) = d.transform();
            if ang != [0.0; 3] {
                return true;
            }
            (bmin, bmax) = (add(bmin, off), add(bmax, off));
        }
        (0..3).all(|i| lo[i] <= bmax[i] + 1.0 && hi[i] >= bmin[i] - 1.0)
    }

    /// What a single point is in (water comes from the hull-0 tree and `func_water`).
    pub fn point_contents(&self, p: Vec3) -> Contents {
        if self.is_solid_point(p) {
            return Contents::Solid;
        }
        if self.brushes.iter().any(|b| b.contents == BrushContents::Water && b.overlaps(p, [0.0; 3])) {
            return Contents::Water;
        }
        for &(root, offset) in &self.water_roots {
            let mut num = root;
            let q = sub(p, offset);
            while num >= 0 {
                let n = &self.point_nodes[num as usize];
                num = n.children[(dot(n.normal, q) - n.dist < 0.0) as usize];
            }
            // Leaf index is -(num + 1); its contents say what it is.
            let c = self.leaf_contents.get((-(num + 1)) as usize).copied().unwrap_or(CONTENTS_EMPTY);
            // func_water brushes are marked solid/water in their own tree.
            if matches!(c, -5..=-3) || (root != self.water_roots[0].0 && c == CONTENTS_SOLID) {
                return Contents::Water;
            }
        }
        Contents::Empty
    }

    fn is_solid_point(&self, p: Vec3) -> bool {
        if self.brushes.iter().any(|b| b.contents != BrushContents::Water && b.overlaps(p, [0.0; 3])) {
            return true;
        }
        let Some(&(mut num, _)) = self.water_roots.first() else { return false };
        while num >= 0 {
            let n = &self.point_nodes[num as usize];
            num = n.children[(dot(n.normal, p) - n.dist < 0.0) as usize];
        }
        self.leaf_contents.get((-(num + 1)) as usize) == Some(&CONTENTS_SOLID)
    }

    /// The ladder the player box at `origin` is touching, if any.
    pub fn ladder_at(&self, hull: Hull, origin: Vec3) -> Option<&Ladder> {
        let (mins, maxs) = (crate::add(origin, hull.mins()), crate::add(origin, hull.maxs()));
        self.ladders.iter().find(|l| (0..3).all(|i| mins[i] <= l.maxs[i] + 1.0 && maxs[i] >= l.mins[i] - 1.0))
    }

    /// Is the player box at `origin` stuck in something?
    pub fn is_solid(&self, hull: Hull, origin: Vec3) -> bool {
        let (lo, hi) = (add(origin, hull.mins()), add(origin, hull.maxs()));
        self.models.iter().filter(|m| m.enabled && self.reaches(m, lo, hi)).any(|m| self.contents(m.heads[hull as usize], to_local(m, self.door_of(m), origin)) == CONTENTS_SOLID)
            || self.boxes.iter().any(|(blo, bhi)| (0..3).all(|i| lo[i] < bhi[i] && hi[i] > blo[i]))
            || self.brushes.iter().any(|b| b.contents != BrushContents::Water && b.near(lo, hi, [0.0; 3]) && b.overlaps(origin, brush::half(hull)))
    }

    /// Sweep the player box from `start` to `end`; returns where it stops.
    pub fn trace(&self, hull: Hull, start: Vec3, end: Vec3) -> Trace {
        let mut best = Trace { start_solid: false, all_solid: false, fraction: 1.0, end, normal: [0.0; 3], hit_contents: CONTENTS_EMPTY };
        let lo: Vec3 = std::array::from_fn(|i| start[i].min(end[i]) + hull.mins()[i]);
        let hi: Vec3 = std::array::from_fn(|i| start[i].max(end[i]) + hull.maxs()[i]);
        for m in self.models.iter().filter(|m| m.enabled && self.reaches(m, lo, hi)) {
            let head = m.heads[hull as usize];
            let door = self.door_of(m);
            let (s, e) = (to_local(m, door, start), to_local(m, door, end));
            let mut t = Trace { start_solid: false, all_solid: true, fraction: 1.0, end: e, normal: [0.0; 3], hit_contents: CONTENTS_EMPTY };
            self.recursive(Tree::Clip(&self.nodes), head, head, 0.0, 1.0, s, e, &mut t);
            // Rotated door: bring the hit normal back to world space.
            if let Some(ang) = door.map(|d| d.transform().1).filter(|a| *a != [0.0; 3]) {
                let (f, r, u) = angle_vectors(ang);
                t.normal = local_to_world(t.normal, f, r, u);
            }
            if t.all_solid {
                t.start_solid = true;
            }
            if t.start_solid {
                best.start_solid = true;
                best.all_solid |= t.all_solid;
            }
            if t.fraction < best.fraction {
                best.fraction = t.fraction;
                best.normal = t.normal;
                best.end = lerp(start, end, t.fraction);
            }
        }
        for (blo, bhi) in &self.boxes {
            if let Some((f, normal)) = sweep_box(hull, start, end, *blo, *bhi) {
                if f < best.fraction {
                    best.fraction = f;
                    best.normal = normal;
                    best.end = lerp(start, end, f);
                }
            }
        }
        for b in self.brushes.iter().filter(|b| b.contents != BrushContents::Water && b.near(lo, hi, [0.0; 3])) {
            match b.clip(start, end, brush::half(hull)) {
                Clip::Inside { all } => {
                    best.start_solid = true;
                    best.all_solid |= all;
                }
                Clip::Hit(h) if h.fraction < best.fraction => {
                    best.fraction = h.fraction;
                    best.normal = h.normal;
                    best.end = lerp(start, end, h.fraction);
                }
                _ => {}
            }
        }
        if best.start_solid {
            // GoldSrc's player trace: starting inside solid counts as blocked at once.
            best.fraction = 0.0;
            best.end = start;
        } else if best.fraction == 1.0 {
            best.end = end;
        }
        best
    }

    /// A bullet-style line trace through the exact geometry (world + doors).
    /// A breakable broke (false) or was put back at round start (true).
    pub fn set_broken(&mut self, bsp_model: usize, broken: bool) {
        for m in self.models.iter_mut().filter(|m| m.bsp_model == bsp_model) {
            m.enabled = !broken;
        }
    }

    /// Whether anything you can't see through blocks a line of sight: the world, closed doors,
    /// crates (as they are now: open doors and broken pieces don't). For hiding players you
    /// can't see: anything see-through (glass, fences) must not count.
    pub fn sight_blocked(&self, start: Vec3, end: Vec3) -> bool {
        // (see-through brushes don't hide anyone; water doesn't either)
        let (lo, hi): (Vec3, Vec3) = (std::array::from_fn(|i| start[i].min(end[i])), std::array::from_fn(|i| start[i].max(end[i])));
        if self.brushes.iter().filter(|b| matches!(b.contents, BrushContents::Solid | BrushContents::Sky) && b.near(lo, hi, [0.0; 3])).any(|b| !matches!(b.clip(start, end, [0.0; 3]), Clip::Miss)) {
            return true;
        }
        let tree = Tree::Point(&self.point_nodes, &self.leaf_contents);
        let near = |m: &Model| (0..3).all(|i| start[i].min(end[i]) <= m.bounds.1[i] && start[i].max(end[i]) >= m.bounds.0[i]);
        self.models.iter().filter(|m| m.enabled && m.opaque && m.heads[0] >= 0).any(|m| {
            let door = self.door_of(m);
            if door.is_none() && !near(m) {
                return false;
            }
            let (s, e) = (to_local(m, door, start), to_local(m, door, end));
            let head = m.heads[0];
            let mut t = Trace { start_solid: false, all_solid: true, fraction: 1.0, end: e, normal: [0.0; 3], hit_contents: CONTENTS_EMPTY };
            self.recursive(tree, head, head, 0.0, 1.0, s, e, &mut t);
            t.fraction < 1.0
        })
    }

    pub fn trace_line(&self, start: Vec3, end: Vec3) -> LineTrace {
        let tree = Tree::Point(&self.point_nodes, &self.leaf_contents);
        let mut best = LineTrace { fraction: 1.0, end, normal: [0.0; 3], start_solid: false, hit_sky: false, model: 0 };
        let lo: Vec3 = std::array::from_fn(|i| start[i].min(end[i]));
        let hi: Vec3 = std::array::from_fn(|i| start[i].max(end[i]));
        for m in self.models.iter().filter(|m| m.enabled && self.reaches(m, lo, hi)) {
            let head = m.heads[0];
            if head < 0 {
                continue;
            }
            let door = self.door_of(m);
            let (s, e) = (to_local(m, door, start), to_local(m, door, end));
            let mut t = Trace { start_solid: false, all_solid: true, fraction: 1.0, end: e, normal: [0.0; 3], hit_contents: CONTENTS_EMPTY };
            self.recursive(tree, head, head, 0.0, 1.0, s, e, &mut t);
            if let Some(ang) = door.map(|d| d.transform().1).filter(|a| *a != [0.0; 3]) {
                let (f, r, u) = angle_vectors(ang);
                t.normal = local_to_world(t.normal, f, r, u);
            }
            best.start_solid |= t.start_solid || t.all_solid;
            if t.fraction < best.fraction {
                best.fraction = t.fraction;
                best.normal = t.normal;
                best.end = lerp(start, end, t.fraction);
                best.hit_sky = t.hit_contents == CONTENTS_SKY;
                best.model = m.bsp_model;
            }
        }
        for b in self.brushes.iter().filter(|b| b.contents != BrushContents::Water && b.near(lo, hi, [0.0; 3])) {
            match b.clip(start, end, [0.0; 3]) {
                Clip::Inside { .. } => best.start_solid = true,
                Clip::Hit(h) if h.fraction < best.fraction => {
                    (best.fraction, best.normal, best.end) = (h.fraction, h.normal, lerp(start, end, h.fraction));
                    (best.hit_sky, best.model) = (b.contents == BrushContents::Sky, b.model);
                }
                _ => {}
            }
        }
        best
    }

    /// Returns false once the move is blocked (the hit is recorded in `t`).
    #[allow(clippy::too_many_arguments)]
    fn recursive(&self, tree: Tree, head: i32, num: i32, f1: f32, f2: f32, p1: Vec3, p2: Vec3, t: &mut Trace) -> bool {
        if num < 0 {
            if !tree.blocks(tree.leaf(num)) {
                t.all_solid = false;
            } else {
                t.start_solid = true;
            }
            return true;
        }
        let n = tree.nodes()[num as usize];
        let (d1, d2) = (dot(n.normal, p1) - n.dist, dot(n.normal, p2) - n.dist);
        if d1 >= 0.0 && d2 >= 0.0 {
            return self.recursive(tree, head, n.children[0], f1, f2, p1, p2, t);
        }
        if d1 < 0.0 && d2 < 0.0 {
            return self.recursive(tree, head, n.children[1], f1, f2, p1, p2, t);
        }
        // The move crosses this plane: split it, stopping just short of the plane.
        let mut frac = if d1 < 0.0 { (d1 + DIST_EPSILON) / (d1 - d2) } else { (d1 - DIST_EPSILON) / (d1 - d2) }.clamp(0.0, 1.0);
        let mut midf = f1 + (f2 - f1) * frac;
        let mut mid = lerp(p1, p2, frac);
        let side = (d1 < 0.0) as usize;

        if !self.recursive(tree, head, n.children[side], f1, midf, p1, mid, t) {
            return false;
        }
        let beyond = tree.contents(n.children[side ^ 1], mid);
        if !tree.blocks(beyond) {
            return self.recursive(tree, head, n.children[side ^ 1], midf, f2, mid, p2, t);
        }
        t.hit_contents = beyond;
        if t.all_solid {
            return false; // never left solid
        }
        // Hit the plane from `side`.
        t.normal = if side == 0 { n.normal } else { [-n.normal[0], -n.normal[1], -n.normal[2]] };
        // Back off until outside solid (float error can land the midpoint inside).
        while tree.blocks(tree.contents(head, mid)) {
            frac -= 0.1;
            if frac < 0.0 {
                t.fraction = midf;
                t.end = mid;
                return false;
            }
            midf = f1 + (f2 - f1) * frac;
            mid = lerp(p1, p2, frac);
        }
        t.fraction = midf;
        t.end = mid;
        false
    }

    /// A map made of solid boxes (mins, maxs): concrete walls and floors, nothing else.
    pub fn from_boxes(solids: Vec<(Vec3, Vec3)>) -> World {
        World::from_brushes(solids.into_iter().map(|(lo, hi)| Brush::from_box(lo, hi, BrushContents::Solid, b'C', 0)).collect())
    }

    /// A map made of convex brushes, not a BSP (maps built in Godot).
    pub fn from_brushes(brushes: Vec<Brush>) -> World {
        World {
            nodes: Vec::new(),
            models: Vec::new(),
            point_nodes: Vec::new(),
            leaf_contents: Vec::new(),
            water_roots: Vec::new(),
            ladders: Vec::new(),
            doors: Doors::default(),
            zones: Default::default(),
            boxes: Vec::new(),
            brushes,
            node_faces: Vec::new(),
            surfaces: Vec::new(),
            textures: Vec::new(),
        }
    }

    /// Test helper: an infinite floor at height `z` (solid below).
    pub fn flat_floor(z: f32) -> World {
        // One node per hull: plane z = floor + half the hull height; below is solid.
        let nodes = vec![
            Node { normal: [0.0, 0.0, 1.0], dist: z + 36.0, children: [CONTENTS_EMPTY, CONTENTS_SOLID] },
            Node { normal: [0.0, 0.0, 1.0], dist: z + 18.0, children: [CONTENTS_EMPTY, CONTENTS_SOLID] },
        ];
        World {
            nodes,
            // Hull 0 (exact geometry) root is point node 0; hulls 1 and 3 are clip nodes 0 and 1.
            models: vec![Model { heads: [0, 0, -1, 1], offset: [0.0; 3], door: None, bsp_model: 0, enabled: true, opaque: true, bounds: ([f32::MIN; 3], [f32::MAX; 3]) }],
            point_nodes: vec![Node { normal: [0.0, 0.0, 1.0], dist: z, children: [-1, -2] }],
            leaf_contents: vec![CONTENTS_EMPTY, CONTENTS_SOLID],
            water_roots: vec![(0, [0.0; 3])],
            ladders: Vec::new(),
            doors: Doors::default(),
            zones: Default::default(),
            boxes: Vec::new(),
            brushes: Vec::new(),
            node_faces: vec![(0, 0)],
            surfaces: Vec::new(),
            textures: Vec::new(),
        }
    }
}

/// The player box swept from `start` to `end` against a box: where it first touches and the
/// face's normal. Starting inside it doesn't count (a body that lands on you never traps you).
fn sweep_box(hull: Hull, start: Vec3, end: Vec3, lo: Vec3, hi: Vec3) -> Option<(f32, Vec3)> {
    let (lo, hi) = expand(hull, lo, hi);
    if inside(start, lo, hi) {
        return None;
    }
    sweep_aabb(start, end, lo, hi)
}

/// A box grown by the player box (Minkowski sum), so the player is a point against it.
fn expand(hull: Hull, lo: Vec3, hi: Vec3) -> (Vec3, Vec3) {
    (std::array::from_fn(|i| lo[i] - hull.maxs()[i]), std::array::from_fn(|i| hi[i] - hull.mins()[i]))
}

/// Strictly inside the box (touching a face is outside: standing on a floor isn't stuck).
fn inside(p: Vec3, lo: Vec3, hi: Vec3) -> bool {
    (0..3).all(|i| p[i] > lo[i] && p[i] < hi[i])
}

/// A point moving from `start` to `end` against a box it starts outside of: where it first
/// touches (stopping DIST_EPSILON short, as GoldSrc does) and that face's normal.
fn sweep_aabb(start: Vec3, end: Vec3, lo: Vec3, hi: Vec3) -> Option<(f32, Vec3)> {
    // (entering from a face it starts exactly on counts: t0 may be 0)
    let (mut t0, mut t1, mut normal) = (f32::NEG_INFINITY, f32::INFINITY, [0.0; 3]);
    for i in 0..3 {
        let d = end[i] - start[i];
        if d.abs() < 1e-6 {
            if start[i] <= lo[i] || start[i] >= hi[i] {
                return None;
            }
            continue;
        }
        let (near, far, side) = if d > 0.0 { ((lo[i] - start[i]) / d, (hi[i] - start[i]) / d, -1.0) } else { ((hi[i] - start[i]) / d, (lo[i] - start[i]) / d, 1.0) };
        if near > t0 {
            t0 = near;
            normal = [0.0; 3];
            normal[i] = side;
        }
        t1 = t1.min(far);
    }
    if t0 >= t1 || t0 < 0.0 || t0 > 1.0 || normal == [0.0; 3] {
        return None;
    }
    // stop just short, as GoldSrc's traces do (DIST_EPSILON)
    let len = length(sub(end, start)).max(1e-3);
    Some(((t0 - 0.03125 / len).max(0.0), normal))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falling_onto_floor_stops_just_above_it() {
        let w = World::flat_floor(0.0);
        let t = w.trace(Hull::Standing, [0.0, 0.0, 100.0], [0.0, 0.0, 0.0]);
        assert!(t.fraction < 1.0 && !t.start_solid);
        assert!((t.end[2] - 36.0).abs() < 0.1, "stopped at {:?}", t.end);
        assert_eq!(t.normal, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn open_move_is_unblocked_and_solid_start_detected() {
        let w = World::flat_floor(0.0);
        let t = w.trace(Hull::Standing, [0.0, 0.0, 100.0], [50.0, 0.0, 100.0]);
        assert_eq!(t.fraction, 1.0);
        assert!(w.trace(Hull::Standing, [0.0, 0.0, 10.0], [0.0, 0.0, 20.0]).all_solid);
        assert!(!w.is_solid(Hull::Crouched, [0.0, 0.0, 20.0]));
        assert!(w.is_solid(Hull::Standing, [0.0, 0.0, 20.0]));
    }
}
