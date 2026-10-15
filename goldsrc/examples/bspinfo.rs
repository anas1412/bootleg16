//! Parse every BSP in a directory and print a one-line summary each.
use goldsrc::bsp::Bsp;

fn main() {
    let dir = std::env::args().nth(1).expect("usage: bspinfo <maps dir>");
    let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
    paths.sort();
    let mut bad = 0;
    for p in paths.iter().filter(|p| p.extension().is_some_and(|e| e == "bsp")) {
        match Bsp::load(p) {
            Ok(b) => {
                let ents = b.entities.matches("\"classname\"").count();
                let ext = b.textures.iter().filter(|t| !t.embedded).count();
                // every face must resolve to a polygon
                let ok = b.faces.iter().all(|f| b.face_vertices(f).len() >= 3);
                println!(
                    "{:<22} faces={:<6} verts={:<6} tex={:<4} (external {:<4}) ents={:<4} light={}KB polys_ok={}",
                    p.file_name().unwrap().to_string_lossy(), b.faces.len(), b.vertices.len(),
                    b.textures.len(), ext, ents, b.lighting.len() / 1024, ok
                );
                if !ok { bad += 1; }
            }
            Err(e) => { bad += 1; println!("{}: ERROR {e}", p.display()); }
        }
    }
    std::process::exit(bad.min(1));
}
