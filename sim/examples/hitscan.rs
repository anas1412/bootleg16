//! Print which hit group a horizontal shot hits at each height. Usage: hitscan <model.mdl> [crouch]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let m = goldsrc::mdl::Model::load(&a[1]).unwrap();
    let hb = sim::hitbox::Hitboxes::from_model(&m, "carbine").unwrap();
    let crouched = a.get(2).is_some();
    for z in (-40..=40).step_by(2) {
        let front = hb.trace([0.0; 3], 0.0, 0.0, crouched, [200.0, 0.0, z as f32], [-1.0, 0.0, 0.0], 400.0);
        let side = hb.trace([0.0; 3], 0.0, 0.0, crouched, [0.0, 200.0, z as f32], [0.0, -1.0, 0.0], 400.0);
        println!("z {z:4}: front {:<22} side {:?}", format!("{:?}", front.map(|h| h.group)), side.map(|h| h.group));
    }
}
