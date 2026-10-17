//! Print a model's bones and animations. Usage: mdlinfo <model.mdl>
use goldsrc::mdl::Model;

fn main() {
    let path = std::env::args().nth(1).expect("usage: mdlinfo <model.mdl>");
    let m = Model::load(&path).unwrap_or_else(|e| panic!("{path}: {e:?}"));
    println!("{} bones, {} animations", m.bones.len(), m.sequences.len());
    for (i, s) in m.sequences.iter().enumerate() {
        println!("  {i:2} {:20} {:5.1} fps, {:3} frames, {} blends", s.name, s.fps, s.frames, s.blends);
    }
}
