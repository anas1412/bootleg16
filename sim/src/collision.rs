//! Swept player-hull traces against the map's precomputed collision hulls.
//!
//! GoldSrc compiles each map with "expanded" hulls: the world grown by the player
//! box. Tracing the player's centre point through that hull equals sweeping the
//! box through the real world.
use crate::doors::{angle_vectors, local_to_world, world_to_local, Actor, Doors};
use crate::{add, dot, lerp, sub, Vec3};
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
        let mut models = vec![Model { heads: bsp.models[0].head_nodes, offset: [0.0; 3], door: None, bsp_model: 0 }];
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
                _ => models.push(Model { heads: m.head_nodes, offset, door: doors.door_for_model(i), bsp_model: i }),
            }
        }
        World { nodes, models, point_nodes, leaf_contents: bsp.leaf_contents.clone(), water_roots, ladders, doors }
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

    /// What a single point is in (water comes from the hull-0 tree and `func_water`).
    pub fn point_contents(&self, p: Vec3) -> Contents {
        if self.is_solid_point(p) {
            return Contents::Solid;
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
        let mut num = self.water_roots[0].0;
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
        self.models.iter().any(|m| self.contents(m.heads[hull as usize], to_local(m, self.door_of(m), origin)) == CONTENTS_SOLID)
    }

    /// Sweep the player box from `start` to `end`; returns where it stops.
    pub fn trace(&self, hull: Hull, start: Vec3, end: Vec3) -> Trace {
        let mut best = Trace { start_solid: false, all_solid: false, fraction: 1.0, end, normal: [0.0; 3], hit_contents: CONTENTS_EMPTY };
        for m in &self.models {
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
    pub fn trace_line(&self, start: Vec3, end: Vec3) -> LineTrace {
        let tree = Tree::Point(&self.point_nodes, &self.leaf_contents);
        let mut best = LineTrace { fraction: 1.0, end, normal: [0.0; 3], start_solid: false, hit_sky: false };
        for m in &self.models {
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

    /// Test helper: an infinite floor at height `z` (solid below).
    pub fn flat_floor(z: f32) -> World {
        // One node per hull: plane z = floor + half the hull height; below is solid.
        let nodes = vec![
            Node { normal: [0.0, 0.0, 1.0], dist: z + 36.0, children: [CONTENTS_EMPTY, CONTENTS_SOLID] },
            Node { normal: [0.0, 0.0, 1.0], dist: z + 18.0, children: [CONTENTS_EMPTY, CONTENTS_SOLID] },
        ];
        World {
            nodes,
            models: vec![Model { heads: [-1, 0, -1, 1], offset: [0.0; 3], door: None, bsp_model: 0 }],
            point_nodes: vec![Node { normal: [0.0, 0.0, 1.0], dist: z, children: [-1, -2] }],
            leaf_contents: vec![CONTENTS_EMPTY, CONTENTS_SOLID],
            water_roots: vec![(0, [0.0; 3])],
            ladders: Vec::new(),
            doors: Doors::default(),
        }
    }
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
