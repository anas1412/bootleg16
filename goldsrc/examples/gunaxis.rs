//! Long axis of a p_ weapon merged onto a player pose. Usage: gunaxis <p_.mdl> <player.mdl> <seq>...
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let w = goldsrc::mdl::Model::load(&a[1]).unwrap();
    let p = goldsrc::mdl::Model::load(&a[2]).unwrap();
    println!("player sequences with 'aim': {:?}", p.sequences.iter().filter(|s| s.name.contains("aim")).map(|s| s.name.as_str()).collect::<Vec<_>>());
    for name in &a[3..] {
        let Some(seq) = p.sequence(name) else { println!("{name}: missing"); continue };
        let merged = w.borrow_pose(&p, &p.pose(seq, 0, 0.5));
        let verts: Vec<[f32; 3]> = w.posed_mesh(&merged).into_iter().flat_map(|m| m.positions).collect();
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &verts {
            for i in 0..3 {
                lo[i] = lo[i].min(v[i]);
                hi[i] = hi[i].max(v[i]);
            }
        }
        let size: Vec<f32> = (0..3).map(|i| (hi[i] - lo[i]).round()).collect();
        println!("{name}: extent x(fwd) {} y(left) {} z(up) {}  centre {:?}", size[0], size[1], size[2], (0..3).map(|i| ((lo[i] + hi[i]) / 2.0).round()).collect::<Vec<_>>());
    }
}
