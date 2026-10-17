//! Our own game data: textures, models, sounds, sprites, maps and radar overviews live in one
//! folder (`data/` beside the executable; in the repo, the private `dist/gamedata/`). Set
//! `BOOTLEG16_DIR` to use another. Nothing is read from anywhere else.
use std::path::{Path, PathBuf};

/// The game data folder.
pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BOOTLEG16_DIR") {
        return dir.into();
    }
    let beside_exe = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("data"))).filter(|p| p.is_dir());
    beside_exe.unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../dist/gamedata"))
}

/// The folders to look in (just the data folder, when it exists).
pub fn roots(game: &Path) -> Vec<PathBuf> {
    Some(game.to_path_buf()).filter(|p| p.is_dir()).into_iter().collect()
}

/// The one naming rule (same as `tools/normalize_names.py`): lowercase, spaces become
/// underscores, quotes are dropped. Models name their sounds in the old style ("Galil ARM/draw.wav").
pub fn clean(name: &str) -> String {
    name.to_lowercase().replace(' ', "_").replace('\'', "")
}

/// `rel` under each root, in lookup order. All game file names follow `clean`, so a request
/// in the old style finds the file.
pub fn paths(game: &Path, rel: &str) -> Vec<PathBuf> {
    let rel = clean(rel);
    roots(game).into_iter().map(|r| r.join(&rel)).collect()
}
