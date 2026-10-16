//! With Steam: put Steam's libsteam_api.so next to the lobby binary and look for it there
//! (as the game does, see client/build.rs).
fn main() {
    if std::env::var_os("CARGO_FEATURE_STEAM").is_none() {
        return;
    }
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN");
    // OUT_DIR is target/<profile>/build/master-<hash>/out; steamworks-sys (a build dependency,
    // so already built) copied the library into its own out dir there.
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let profile = out.ancestors().nth(3).unwrap();
    let lib = std::fs::read_dir(profile.join("build")).unwrap().flatten().map(|d| d.path().join("out/libsteam_api.so")).find(|p| p.exists());
    if let Some(lib) = lib {
        let _ = std::fs::copy(lib, profile.join("libsteam_api.so"));
    }
}
