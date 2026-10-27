//! Convex brushes: the solid shapes of a map that isn't a BSP (built in Godot). A brush is
//! the space behind all of its planes. A player box is swept against it the way Quake and
//! GoldSrc do (`CM_ClipBoxToBrush`): each plane moves out by the box's reach along its
//! normal, so the box becomes a point; the brush's bounding box planes are always added
//! ("bevels"), so sliding along edges and corners doesn't catch.
use crate::collision::Hull;
use crate::{dot, Vec3};

/// GoldSrc's DIST_EPSILON: traces stop this far short of a surface.
const EPSILON: f32 = 0.03125;

/// What a brush is made of, for the rules (players, bullets, sight) rather than its looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contents {
    /// Blocks everything.
    Solid,
    /// Blocks players and bullets, but you see through it (glass, fences).
    Clear,
    /// The sky: stops bullets with no hole, blocks players.
    Sky,
    /// You swim in it; nothing is blocked.
    Water,
}

#[derive(Clone, Debug)]
pub struct Brush {
    /// Outward normals (unit length) and distances: inside is where dot(n, p) < d for all.
    planes: Vec<(Vec3, f32)>,
    pub lo: Vec3,
    pub hi: Vec3,
    pub contents: Contents,
    /// Surface material for footsteps and bullet penetration (CS's `materials.txt` letters:
    /// C concrete, M metal, W wood, G grate, V vent, T tile, P computer, Y glass, D dirt...).
    pub material: u8,
    /// Which map piece it belongs to (0 = the world; else a breakable or door).
    pub model: usize,
}

/// Where a sweep first touched a brush.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub fraction: f32,
    pub normal: Vec3,
}

/// A sweep against one brush.
pub enum Clip {
    Miss,
    Hit(Hit),
    /// It started inside (`all`: and ended inside).
    Inside { all: bool },
}

impl Brush {
    /// A brush from its planes (outward normals; they are normalized here) and corners'
    /// bounds. Its bounding box planes are added as bevels.
    pub fn new(planes: Vec<(Vec3, f32)>, lo: Vec3, hi: Vec3, contents: Contents, material: u8, model: usize) -> Brush {
        let mut planes: Vec<(Vec3, f32)> = planes
            .into_iter()
            .filter_map(|(n, d)| {
                let len = dot(n, n).sqrt();
                (len > 1e-6).then(|| (n.map(|v| v / len), d / len))
            })
            .collect();
        for i in 0..3 {
            let mut n = [0.0; 3];
            n[i] = 1.0;
            planes.push((n, hi[i]));
            n[i] = -1.0;
            planes.push((n, -lo[i]));
        }
        Brush { planes, lo, hi, contents, material, model }
    }

    /// An axis-aligned box.
    pub fn from_box(lo: Vec3, hi: Vec3, contents: Contents, material: u8, model: usize) -> Brush {
        Brush::new(Vec::new(), lo, hi, contents, material, model)
    }

    /// Whether its bounds (grown by `grow`) overlap the box lo..hi.
    pub fn near(&self, lo: Vec3, hi: Vec3, grow: Vec3) -> bool {
        (0..3).all(|i| lo[i] <= self.hi[i] + grow[i] && hi[i] >= self.lo[i] - grow[i])
    }

    /// Sweep a box of half-size `half` (zero for a line) from `start` to `end`.
    pub fn clip(&self, start: Vec3, end: Vec3, half: Vec3) -> Clip {
        let (mut enter, mut leave) = (-1.0f32, 1.0f32);
        let mut normal = [0.0; 3];
        let (mut starts_out, mut ends_out) = (false, false);
        for &(n, d) in &self.planes {
            // the plane pushed out by the box's reach along it
            let d = d + n[0].abs() * half[0] + n[1].abs() * half[1] + n[2].abs() * half[2];
            let (d1, d2) = (dot(n, start) - d, dot(n, end) - d);
            starts_out |= d1 > 0.0;
            ends_out |= d2 > 0.0;
            // outside this plane the whole way (or moving away from it): no hit
            if d1 > 0.0 && (d2 >= EPSILON || d2 >= d1) {
                return Clip::Miss;
            }
            if d1 <= 0.0 && d2 <= 0.0 {
                continue;
            }
            if d1 > d2 {
                // going in: stop EPSILON short
                let f = ((d1 - EPSILON) / (d1 - d2)).max(0.0);
                if f > enter {
                    enter = f;
                    normal = n;
                }
            } else {
                let f = ((d1 + EPSILON) / (d1 - d2)).min(1.0);
                leave = leave.min(f);
            }
        }
        if !starts_out {
            return Clip::Inside { all: !ends_out };
        }
        if enter < leave && enter > -1.0 {
            return Clip::Hit(Hit { fraction: enter.max(0.0), normal });
        }
        Clip::Miss
    }

    /// Whether a box of half-size `half` centred at `p` overlaps it.
    pub fn overlaps(&self, p: Vec3, half: Vec3) -> bool {
        self.planes.iter().all(|&(n, d)| dot(n, p) - (d + n[0].abs() * half[0] + n[1].abs() * half[1] + n[2].abs() * half[2]) < 0.0)
    }
}

/// Half the size of a player box (both hulls are centred on the origin).
pub fn half(hull: Hull) -> Vec3 {
    hull.maxs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wedge() -> Brush {
        // a ramp: floor 0..256 in x, rising 45 degrees (top surface z = x), 0..64 wide
        let s = std::f32::consts::FRAC_1_SQRT_2;
        Brush::new(vec![([-s, 0.0, s], 0.0)], [0.0, 0.0, 0.0], [256.0, 64.0, 256.0], Contents::Solid, b'C', 0)
    }

    #[test]
    fn a_line_stops_on_a_slope_and_misses_beside_it() {
        let b = wedge();
        // straight down onto the slope at x = 100: it's at z 100
        match b.clip([100.0, 32.0, 300.0], [100.0, 32.0, 0.0], [0.0; 3]) {
            Clip::Hit(h) => {
                let z = 300.0 - 300.0 * h.fraction;
                assert!((z - 100.0).abs() < 0.1 && h.normal[2] > 0.7 && h.normal[0] < -0.7, "{z} {:?}", h.normal);
            }
            _ => panic!("missed the slope"),
        }
        assert!(matches!(b.clip([100.0, 100.0, 300.0], [100.0, 100.0, 0.0], [0.0; 3]), Clip::Miss));
        assert!(matches!(b.clip([200.0, 32.0, 10.0], [210.0, 32.0, 10.0], [0.0; 3]), Clip::Inside { all: true }));
    }

    #[test]
    fn a_player_box_stands_on_a_slope_with_its_corner() {
        let b = wedge();
        // a standing player dropping at x = 100: the uphill corner of its box (x 116) meets
        // the slope at z 116, so the origin stops 36 over that
        let half = half(Hull::Standing);
        match b.clip([100.0, 32.0, 300.0], [100.0, 32.0, 0.0], half) {
            Clip::Hit(h) => {
                let z = 300.0 - 300.0 * h.fraction;
                assert!((z - (116.0 + 36.0)).abs() < 0.2, "{z}");
            }
            _ => panic!("fell through"),
        }
        assert!(b.overlaps([100.0, 32.0, 150.0], half) && !b.overlaps([100.0, 32.0, 153.0], half));
    }
}
