fn main() {
    // Set default output directory for Windows cross-compilation
    if std::env::var("CARGO_CFG_TARGET")
        .unwrap_or_default()
        .contains("windows")
    {
        println!("cargo:rerun-if-changed=build.rs");
    }
}
