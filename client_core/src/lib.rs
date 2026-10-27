//! Client logic shared by every front end (the Bevy client today, the Godot one next): no
//! engine types, only numbers in and out.
pub mod assistant;
pub mod reverb;
pub mod view;
pub mod voice;

use std::path::PathBuf;

/// Where the player's own files go (settings, key binds, voice): `%APPDATA%\bootleg16` on
/// Windows, `~/.config/bootleg16` elsewhere.
pub fn config_dir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join("bootleg16")
    } else {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/bootleg16")
    }
}
