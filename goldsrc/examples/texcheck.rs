//! For every map, resolve each texture from the map itself or its WADs; report misses.
//! Usage: texcheck <game root containing cstrike/ and valve/>
use goldsrc::{bsp::Bsp, wad::Wad};
use std::{collections::HashMap, path::Path};

fn main() {
    let root = std::env::args().nth(1).expect("usage: texcheck <Half-Life dir>");
    let root = Path::new(&root);
    let mut wads: HashMap<String, Option<Wad>> = HashMap::new();
    let mut maps: Vec<_> = std::fs::read_dir(root.join("cstrike/maps")).unwrap().flatten().map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "bsp")).collect();
    maps.sort();
    let mut total_missing = 0;
    for m in maps {
        let bsp = Bsp::load(&m).unwrap();
        let names = bsp.wad_names();
        for n in &names {
            wads.entry(n.to_ascii_lowercase()).or_insert_with(|| {
                ["cstrike", "valve"].iter().find_map(|d| Wad::load(root.join(d).join(n)).ok())
            });
        }
        let missing: Vec<_> = bsp.textures.iter()
            .filter(|t| !t.name.is_empty() && t.pixels.is_none())
            .filter(|t| !names.iter().any(|n| wads[&n.to_ascii_lowercase()].as_ref().is_some_and(|w| w.texture(&t.name).is_some())))
            .map(|t| t.name.clone()).collect();
        let absent_wads: Vec<_> = names.iter().filter(|n| wads[&n.to_ascii_lowercase()].is_none()).collect();
        println!("{:<20} wads={:<2} missing_tex={:<3} {:?} absent_wads={:?}",
            m.file_name().unwrap().to_string_lossy(), names.len(), missing.len(), &missing[..missing.len().min(4)], absent_wads);
        total_missing += missing.len();
    }
    std::process::exit((total_missing > 0) as i32);
}
