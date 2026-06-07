//! Build script for maxion-loader-stub
//!
//! This script enforces Position-Independent Code (PIC) compilation
//! to ensure the stub can be injected as raw binary without import tables.

fn main() {
    // Enforce PIC compilation
    println!("cargo:rustc-cfg=pic");

    // Inform user about compilation settings
    println!("cargo:warning=Maxion Loader Stub: Enforcing PIC compilation");
    println!("cargo:warning=  - relocation-model=pic");

    // Set optimization level for release builds
    #[cfg(not(debug_assertions))]
    {
        println!("cargo:rustc-cfg=opt_level=\"z\"");
        println!("cargo:warning=  - opt-level=z (release mode)");
    }

    // Rebuild if build script changes
    println!("cargo:rerun-if-changed=build.rs");
}
