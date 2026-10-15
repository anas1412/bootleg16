//! Compare a bone's orientation in two models. Usage: handcmp <p_.mdl> <player.mdl> <player seq> [blend]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let w = goldsrc::mdl::Model::load(&a[1]).unwrap();
    let p = goldsrc::mdl::Model::load(&a[2]).unwrap();
    let seq = p.sequence(&a[3]).unwrap();
    let blend: f32 = a.get(4).and_then(|b| b.parse().ok()).unwrap_or(0.5);
    let wp = w.pose(0, 0, 0.0);
    let pp = p.pose(seq, 0, blend);
    let name = "Bip01 R Hand";
    let wi = w.bones.iter().position(|b| b.name == name).unwrap();
    let pi = p.bones.iter().position(|b| b.name == name).unwrap();
    let show = |t: &goldsrc::mdl::BoneTransform| format!("pos {:?} x-axis {:?} y {:?} z {:?}", t.pos.map(|v| v.round()), [t.rot[0][0], t.rot[1][0], t.rot[2][0]].map(|v| (v * 100.0).round() / 100.0), [t.rot[0][1], t.rot[1][1], t.rot[2][1]].map(|v| (v * 100.0).round() / 100.0), [t.rot[0][2], t.rot[1][2], t.rot[2][2]].map(|v| (v * 100.0).round() / 100.0));
    println!("p_ own  : {}", show(&wp[wi]));
    println!("player  : {}", show(&pp[pi]));
    let gun = w.bones.len() - 1;
    println!("p_ gun bone {} own: {}", w.bones[gun].name, show(&wp[gun]));
    let merged = w.borrow_pose(&p, &pp);
    println!("p_ gun merged     : {}", show(&merged[gun]));
    let s = &p.sequences[seq];
    println!("seq {} blends {}", s.name, s.blends);
}
