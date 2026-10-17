//! A map's floor plan as a grid of bytes (1 where there is floor under open air at head height),
//! for matching against the overview images. Usage: floorplan <map.bsp> <out.bin>
//! Writes: i32 columns, i32 rows, f32 min_x, f32 min_y, f32 step, then rows*columns bytes
//! (row 0 = min y, column 0 = min x).
use goldsrc::bsp::Bsp;
use sim::collision::World;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let bsp = Bsp::load(&a[1]).unwrap();
    let world = World::from_bsp(&bsp);
    let (lo, hi) = (bsp.models[0].mins, bsp.models[0].maxs);
    let step = 16.0f32;
    let (cols, rows) = (((hi[0] - lo[0]) / step) as usize + 1, ((hi[1] - lo[1]) / step) as usize + 1);
    let mut out = Vec::new();
    out.extend((cols as i32).to_le_bytes());
    out.extend((rows as i32).to_le_bytes());
    for v in [lo[0], lo[1], step] {
        out.extend(v.to_le_bytes());
    }
    let mut grid = vec![0u8; cols * rows];
    for r in 0..rows {
        for c in 0..cols {
            let (x, y) = (lo[0] + c as f32 * step, lo[1] + r as f32 * step);
            // any floor: a downward line from above that lands on a surface facing up
            let t = world.trace_line([x, y, hi[2] - 1.0], [x, y, lo[2] + 1.0]);
            if t.fraction < 1.0 && t.normal[2] > 0.7 && !t.hit_sky {
                grid[r * cols + c] = 1;
            }
        }
    }
    out.extend(grid);
    std::fs::write(&a[2], out).unwrap();
}
