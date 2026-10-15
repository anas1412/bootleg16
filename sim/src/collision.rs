//! Swept player-hull traces against the map's precomputed collision hulls.
//!
//! GoldSrc compiles each map with "expanded" hulls: the world grown by the player
//! box. Tracing the player's centre point through that hull equals sweeping the
//! box through the real world.
use crate::{dot, lerp, sub, Vec3};
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
}

/// Everything the movement code collides with.
pub struct World {
    nodes: Vec<Node>,
    models: Vec<Model>,
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
        let mut models = vec![Model { heads: bsp.models[0].head_nodes, offset: [0.0; 3] }];
        for e in bsp.entity_list() {
            let class = e.get("classname").map(String::as_str).unwrap_or("");
            let Some(i) = e.get("model").and_then(|m| m.strip_prefix('*')?.parse::<usize>().ok()) else { continue };
            if i == 0 || i >= bsp.models.len() || non_solid(class) {
                continue;
            }
            let o: Vec<f32> = e.get("origin").map(|o| o.split_whitespace().filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
            let offset = [o.first().copied().unwrap_or(0.0), o.get(1).copied().unwrap_or(0.0), o.get(2).copied().unwrap_or(0.0)];
            models.push(Model { heads: bsp.models[i].head_nodes, offset });
        }
        World { nodes, models }
    }

    fn contents(&self, mut num: i32, p: Vec3) -> i32 {
        while num >= 0 {
            let n = &self.nodes[num as usize];
            num = n.children[(dot(n.normal, p) - n.dist < 0.0) as usize];
        }
        num
    }

    /// Is the player box at `origin` stuck in something?
    pub fn is_solid(&self, hull: Hull, origin: Vec3) -> bool {
        self.models.iter().any(|m| self.contents(m.heads[hull as usize], sub(origin, m.offset)) == CONTENTS_SOLID)
    }

    /// Sweep the player box from `start` to `end`; returns where it stops.
    pub fn trace(&self, hull: Hull, start: Vec3, end: Vec3) -> Trace {
        let mut best = Trace { start_solid: false, all_solid: false, fraction: 1.0, end, normal: [0.0; 3] };
        for m in &self.models {
            let head = m.heads[hull as usize];
            let (s, e) = (sub(start, m.offset), sub(end, m.offset));
            let mut t = Trace { start_solid: false, all_solid: true, fraction: 1.0, end: e, normal: [0.0; 3] };
            self.recursive(head, head, 0.0, 1.0, s, e, &mut t);
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
        if best.fraction == 1.0 {
            best.end = end;
        }
        best
    }

    /// Returns false once the move is blocked (the hit is recorded in `t`).
    #[allow(clippy::too_many_arguments)]
    fn recursive(&self, head: i32, num: i32, f1: f32, f2: f32, p1: Vec3, p2: Vec3, t: &mut Trace) -> bool {
        if num < 0 {
            if num != CONTENTS_SOLID {
                t.all_solid = false;
            } else {
                t.start_solid = true;
            }
            return true;
        }
        let n = self.nodes[num as usize];
        let (d1, d2) = (dot(n.normal, p1) - n.dist, dot(n.normal, p2) - n.dist);
        if d1 >= 0.0 && d2 >= 0.0 {
            return self.recursive(head, n.children[0], f1, f2, p1, p2, t);
        }
        if d1 < 0.0 && d2 < 0.0 {
            return self.recursive(head, n.children[1], f1, f2, p1, p2, t);
        }
        // The move crosses this plane: split it, stopping just short of the plane.
        let mut frac = if d1 < 0.0 { (d1 + DIST_EPSILON) / (d1 - d2) } else { (d1 - DIST_EPSILON) / (d1 - d2) }.clamp(0.0, 1.0);
        let mut midf = f1 + (f2 - f1) * frac;
        let mut mid = lerp(p1, p2, frac);
        let side = (d1 < 0.0) as usize;

        if !self.recursive(head, n.children[side], f1, midf, p1, mid, t) {
            return false;
        }
        if self.contents(n.children[side ^ 1], mid) != CONTENTS_SOLID {
            return self.recursive(head, n.children[side ^ 1], midf, f2, mid, p2, t);
        }
        if t.all_solid {
            return false; // never left solid
        }
        // Hit the plane from `side`.
        t.normal = if side == 0 { n.normal } else { [-n.normal[0], -n.normal[1], -n.normal[2]] };
        // Back off until outside solid (float error can land the midpoint inside).
        while self.contents(head, mid) == CONTENTS_SOLID {
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
        World { nodes, models: vec![Model { heads: [-1, 0, -1, 1], offset: [0.0; 3] }] }
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
