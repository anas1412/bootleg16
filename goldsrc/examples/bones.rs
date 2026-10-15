//! Compare bone names of two models (e.g. p_ weapon vs player). Usage: bones <a.mdl> <b.mdl>
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let x = goldsrc::mdl::Model::load(&a[1]).unwrap();
    let y = goldsrc::mdl::Model::load(&a[2]).unwrap();
    for b in &x.bones {
        let hit = y.bones.iter().position(|o| o.name == b.name);
        println!("{:?} parent {} -> {:?}", b.name, b.parent, hit);
    }
}
