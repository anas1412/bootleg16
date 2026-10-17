//! Where a model sits: the box round its idle pose, in GoldSrc axes (x forward, y left, z up).
//! Usage: mdlbounds <model.mdl>...
use goldsrc::mdl::Model;

fn main() {
    for path in std::env::args().skip(1) {
        let Ok(m) = Model::load(&path) else { println!("{path}: unreadable"); continue };
        let idle = m.sequence("idle").unwrap_or(0);
        let parts = m.posed_mesh(&m.pose(idle, 0, 0.0));
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in parts.iter().flat_map(|p| &p.positions) {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        let name = std::path::Path::new(&path).file_stem().unwrap().to_string_lossy().into_owned();
        println!("{name:14} x {:6.1}..{:6.1}  y {:6.1}..{:6.1}  z {:6.1}..{:6.1}", lo[0], hi[0], lo[1], hi[1], lo[2], hi[2]);
    }
}
