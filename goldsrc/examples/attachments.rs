//! Print a model's attachment points. Usage: attachments <model.mdl>
fn main() {
    let m = goldsrc::mdl::Model::load(std::env::args().nth(1).unwrap()).unwrap();
    let pose = m.pose(0, 0, 0.0);
    for i in 0..m.attachments.len() {
        println!("{i}: bone {} at {:?}", m.attachments[i].0, m.attachment(i, &pose));
    }
}
