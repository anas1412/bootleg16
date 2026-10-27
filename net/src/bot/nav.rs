//! Navigation for test bots, worked out from the map itself: standing spots on a grid (every
//! floor, bridges and tunnels too), joined where a player can walk, step or jump from one to
//! the next, and A* over that graph. Built once per map and shared by all bots.
use sim::collision::{Hull, World};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Grid spacing (units).
const STEP: f32 = 48.0;
/// Where the grid sits: off the round numbers our maps' blocks are placed on, so a standing
/// box never just touches a wall (a trace from there counts as stuck, and the spot or the
/// way past it would be lost).
const OFFSET: f32 = 7.0;
/// The most a link may climb (a jump is 45 units).
const CLIMB: f32 = 40.0;
/// The furthest a link may drop.
const DROP: f32 = 200.0;

pub struct Nav {
    /// Standing origins (hull centre, 36 above the floor).
    pub points: Vec<[f32; 3]>,
    links: Vec<Vec<u32>>,
    cells: HashMap<(i32, i32), Vec<u32>>,
}

fn cell(p: [f32; 3]) -> (i32, i32) {
    (((p[0] - OFFSET) / STEP).round() as i32, ((p[1] - OFFSET) / STEP).round() as i32)
}

impl Nav {
    /// `mins` / `maxs`: the map's bounds (the world model's box).
    pub fn build(world: &World, mins: [f32; 3], maxs: [f32; 3]) -> Nav {
        let hull = Hull::Standing;
        let mut points = Vec::new();
        let mut cells: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
        let (x0, x1) = ((mins[0] / STEP).ceil() as i32, (maxs[0] / STEP).floor() as i32);
        let (y0, y1) = ((mins[1] / STEP).ceil() as i32, (maxs[1] / STEP).floor() as i32);
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                let (x, y) = (cx as f32 * STEP + OFFSET, cy as f32 * STEP + OFFSET);
                // From the top down: each stretch of open space ends on a floor.
                let mut z = maxs[2] - 40.0;
                while z > mins[2] + 40.0 {
                    if world.is_solid(hull, [x, y, z]) {
                        z -= 16.0;
                        continue;
                    }
                    let t = world.trace(hull, [x, y, z], [x, y, mins[2]]);
                    if t.start_solid || t.fraction >= 1.0 {
                        break;
                    }
                    if t.normal[2] > 0.7 {
                        cells.entry((cx, cy)).or_default().push(points.len() as u32);
                        points.push(t.end);
                    }
                    z = t.end[2] - 80.0; // through the floor to whatever is under it
                }
            }
        }
        // Links to the 8 neighbouring cells.
        let mut links = vec![Vec::new(); points.len()];
        for (i, p) in points.iter().enumerate() {
            let (cx, cy) = cell(*p);
            for (dx, dy) in [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)] {
                for &j in cells.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    let q = points[j as usize];
                    let dz = q[2] - p[2];
                    if dz > CLIMB || dz < -DROP {
                        continue;
                    }
                    // Cross at step height, or jump height to get onto something higher,
                    // then drop onto the next spot: both ends above the floors.
                    let h = if dz > 18.0 { 45.0 } else { 18.0 };
                    let a = [p[0], p[1], p[2] + h];
                    let b = [q[0], q[1], p[2] + h];
                    if world.trace(hull, *p, a).fraction < 1.0 || world.trace(hull, b, q).start_solid {
                        continue;
                    }
                    let t = world.trace(hull, a, b);
                    if !t.start_solid && t.fraction >= 1.0 {
                        links[i].push(j);
                    }
                }
            }
        }
        Nav { points, links, cells }
    }

    /// The closest point standing at about this height (None if nothing is near).
    pub fn nearest(&self, p: [f32; 3]) -> Option<u32> {
        let (cx, cy) = cell(p);
        let mut best: Option<(u32, f32)> = None;
        for dx in -2..=2 {
            for dy in -2..=2 {
                for &i in self.cells.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    let q = self.points[i as usize];
                    if (q[2] - p[2]).abs() > 72.0 {
                        continue;
                    }
                    let d = dist(p, q);
                    if best.is_none_or(|(_, b)| d < b) {
                        best = Some((i, d));
                    }
                }
            }
        }
        best.map(|(i, _)| i)
    }

    /// A* from one point to another, around `blocked` links: the points to walk through
    /// (empty if unreachable).
    pub fn path(&self, from: u32, to: u32, blocked: &std::collections::HashSet<(u32, u32)>) -> Vec<u32> {
        #[derive(PartialEq)]
        struct Open(f32, u32);
        impl Eq for Open {}
        impl PartialOrd for Open {
            fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
                Some(self.cmp(o))
            }
        }
        impl Ord for Open {
            fn cmp(&self, o: &Self) -> Ordering {
                o.0.total_cmp(&self.0) // min-heap on cost
            }
        }
        let goal = self.points[to as usize];
        let mut cost: HashMap<u32, f32> = HashMap::from([(from, 0.0)]);
        let mut came: HashMap<u32, u32> = HashMap::new();
        let mut open = BinaryHeap::from([Open(dist(self.points[from as usize], goal), from)]);
        while let Some(Open(_, n)) = open.pop() {
            if n == to {
                let mut path = vec![n];
                let mut at = n;
                while let Some(&p) = came.get(&at) {
                    path.push(p);
                    at = p;
                }
                path.reverse();
                return path;
            }
            let here = cost[&n];
            for &m in &self.links[n as usize] {
                if blocked.contains(&(n, m)) {
                    continue;
                }
                let c = here + dist(self.points[n as usize], self.points[m as usize]);
                if cost.get(&m).is_none_or(|old| c < *old) {
                    cost.insert(m, c);
                    came.insert(m, n);
                    open.push(Open(c + dist(self.points[m as usize], goal), m));
                }
            }
        }
        Vec::new()
    }
}

pub fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}
