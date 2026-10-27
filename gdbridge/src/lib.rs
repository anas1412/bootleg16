//! bootleg16's GDExtension: Godot loads this library and gets the `Session` class.
use godot::prelude::*;

mod session;
mod units;

struct Bootleg16;

#[gdextension]
unsafe impl ExtensionLibrary for Bootleg16 {}
