//! Where game files are looked up, like the engine does: the add-on (`cstrike_addon/` next to
//! `cstrike/`, or `BOOTLEG16_ADDON`; optional), then the mod's `cstrike/`, then `valve/`.
//! Everything that loads textures, models, sounds or sprites goes through here.
use std::path::{Path, PathBuf};

/// The folders that exist, first match wins.
pub fn roots(game: &Path) -> Vec<PathBuf> {
    let addon = std::env::var_os("BOOTLEG16_ADDON").map(PathBuf::from).unwrap_or_else(|| game.join("cstrike_addon"));
    [addon, game.join("cstrike"), game.join("valve")].into_iter().filter(|p| p.is_dir()).collect()
}

/// `rel` under each root, in lookup order.
pub fn paths(game: &Path, rel: &str) -> Vec<PathBuf> {
    roots(game).into_iter().map(|r| r.join(rel)).collect()
}
