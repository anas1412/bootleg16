//! Print animation events of a model. Usage: mdlevents <model.mdl>
fn main() {
    let m = goldsrc::mdl::Model::load(std::env::args().nth(1).unwrap()).unwrap();
    for s in &m.sequences {
        if !s.events.is_empty() {
            println!("{} ({} frames @ {} fps): {:?}", s.name, s.frames, s.fps, s.events);
        }
    }
}
