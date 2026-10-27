//! bootleg16's GDExtension: Godot loads this library and gets the `Session` and `Lobby` classes.
use godot::prelude::*;

mod lobby;
mod session;
mod units;

struct Bootleg16;

#[gdextension]
unsafe impl ExtensionLibrary for Bootleg16 {}
