//! Dev builds link Bevy as a shared library in target/<profile>/deps: let the game binary
//! (one directory up) find it there when run directly. With Steam, Steam's libsteam_api.so
//! goes next to the binary (as it ships), and the binary looks there too.
fn main() {
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN/deps");
    if std::env::var_os("CARGO_FEATURE_STEAM").is_some() {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
        // OUT_DIR is target/<profile>/build/client-<hash>/out; steamworks-sys (a build
        // dependency, so already built) copied the library into its own out dir there.
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let profile = out.ancestors().nth(3).unwrap();
        let lib = std::fs::read_dir(profile.join("build")).unwrap().flatten().map(|d| d.path().join("out/libsteam_api.so")).find(|p| p.exists());
        if let Some(lib) = lib {
            let _ = std::fs::copy(lib, profile.join("libsteam_api.so"));
        }
    }
}
