//! Summarise a model's posed mesh. Usage: mdlinfo <model.mdl> [sequence]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = goldsrc::mdl::Model::load(&a[1]).unwrap();
    let seq = a.get(2).and_then(|s| m.sequence(s)).unwrap_or(0);
    let parts = m.posed_mesh(&m.pose(seq, 0, 0.5));
    let tris: usize = parts.iter().map(|p| p.indices.len() / 3).sum();
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &parts {
        for v in &p.positions {
            for i in 0..3 {
                lo[i] = lo[i].min(v[i]);
                hi[i] = hi[i].max(v[i]);
            }
        }
    }
    println!("{} textures, {} parts, {} triangles, bounds {:?} .. {:?}", m.textures().len(), parts.len(), tris, lo, hi);
}
