//! Dev builds link Bevy as a shared library in target/<profile>/deps: let the game binary
//! (one directory up) find it there when run directly.
fn main() {
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN/deps");
}
