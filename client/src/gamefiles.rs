//! `net::gamefiles` for the game's own install folder.
use std::path::PathBuf;

pub fn roots() -> Vec<PathBuf> {
    net::gamefiles::roots(&crate::game_dir())
}

pub fn paths(rel: &str) -> Vec<PathBuf> {
    net::gamefiles::paths(&crate::game_dir(), rel)
}
