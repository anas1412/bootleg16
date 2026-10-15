//! Print a model's hitbox centres (model space) for a sequence. Usage: hitboxes <model.mdl> <sequence> [blend]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = goldsrc::mdl::Model::load(&a[1]).unwrap();
    let seq = m.sequence(&a[2]).expect("no such sequence");
    let blend: f32 = a.get(3).and_then(|b| b.parse().ok()).unwrap_or(0.5);
    let pose = m.pose(seq, 0, blend);
    println!("{} blends={} frames={}", m.sequences[seq].name, m.sequences[seq].blends, m.sequences[seq].frames);
    for h in &m.hitboxes {
        let c = [(h.mins[0] + h.maxs[0]) / 2.0, (h.mins[1] + h.maxs[1]) / 2.0, (h.mins[2] + h.maxs[2]) / 2.0];
        let w = pose[h.bone].apply(c);
        println!("group {} {:<18} centre {:6.1} {:6.1} {:6.1}", h.group, m.bones[h.bone].name, w[0], w[1], w[2]);
    }
}
