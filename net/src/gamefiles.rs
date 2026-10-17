//! Where game files are looked up: the add-on (`cstrike_addon/` next to the install's
//! `cstrike/`, or `BOOTLEG16_ADDON`). Textures, models, sounds and sprites come only from
//! there; Valve's own folders (`cstrike/`, `valve/`) are not read, except for the maps.
use std::path::{Path, PathBuf};

/// The folders to look in (just the add-on, when it exists).
pub fn roots(game: &Path) -> Vec<PathBuf> {
    let addon = std::env::var_os("BOOTLEG16_ADDON").map(PathBuf::from).unwrap_or_else(|| game.join("cstrike_addon"));
    Some(addon).filter(|p| p.is_dir()).into_iter().collect()
}

/// `rel` under each root, in lookup order.
pub fn paths(game: &Path, rel: &str) -> Vec<PathBuf> {
    roots(game).into_iter().map(|r| r.join(rel)).collect()
}
