//! `net::gamefiles` for the game's own install folder.
use std::path::PathBuf;

pub fn roots() -> Vec<PathBuf> {
    net::gamefiles::roots(&crate::game_dir())
}

pub fn paths(rel: &str) -> Vec<PathBuf> {
    net::gamefiles::paths(&crate::game_dir(), rel)
}

/// Where the player's own files go (settings, key binds, voice): `%APPDATA%\bootleg16` on
/// Windows, `~/.config/bootleg16` elsewhere.
pub fn config_dir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("bootleg16")
    } else {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/bootleg16")
    }
}
