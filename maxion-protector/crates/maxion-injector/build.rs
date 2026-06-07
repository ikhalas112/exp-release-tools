//! Build script for maxion-injector
//!
//! This build script handles compilation of the maxion-stub crate
//! and prepares it for embedding into protected PE files.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../maxion-stub/src");
    println!("cargo:rerun-if-changed=../maxion-stub/Cargo.toml");
    println!("cargo:rerun-if-changed=build.rs");

    // Determine target architecture
    let target = env::var("TARGET").expect("TARGET not set");

    // Only compile stub for Windows targets
    if !target.contains("windows") {
        println!("cargo:warning=Stub compilation skipped: non-Windows target");
        // Don't set cfg flag - code using #[cfg(stub_compiled)] will be excluded
        return;
    }

    // Skip automatic stub compilation to avoid circular dependency
    // The stub should be pre-compiled separately using: cargo build --release -p maxion-stub
    println!(
        "cargo:info=Skipping automatic maxion-stub compilation (to avoid circular dependency)"
    );
    println!("cargo:warning=Pre-compile stub with: cargo build --release -p maxion-stub");

    // Get profile for locating the pre-compiled stub
    let profile = env::var("PROFILE").unwrap_or_else(|_| "release".to_string());

    // Commented out: automatic stub compilation causes build timeout due to circular dependency
    // let mut build_cmd = Command::new("cargo");
    // build_cmd.args(["build", "--package", "maxion-stub", "--target", &target]);
    // if profile == "release" {
    //     build_cmd.arg("--release");
    //     let target_upper = target.to_uppercase().replace("-", "_");
    //     let rustflags_key = format!("CARGO_TARGET_{}_RUSTFLAGS", target_upper);
    //     env::set_var(
    //         &rustflags_key,
    //         format!(
    //             "{} -C opt-level=z -C lto=fat -C codegen-units=1 -C panic=abort",
    //             env::var(&rustflags_key).unwrap_or_default()
    //         ),
    //     );
    // }
    // let status = build_cmd
    //     .status()
    //     .expect("Failed to execute cargo build for maxion-stub");
    // if !status.success() {
    //     panic!("Failed to compile maxion-stub");
    // }

    // Locate compiled stub library
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();

    // Windows MSVC uses .lib extension, Unix systems use .a
    let lib_name = if target.contains("windows-msvc") {
        "maxion_stub.lib"
    } else {
        "libmaxion_stub.a"
    };

    // Check both target-specific and default target directories
    // Native builds use target/profile/, cross-compilation uses target/{triple}/profile/
    let profile_dir = if profile == "release" {
        "release"
    } else {
        "debug"
    };

    let target_specific_path = workspace_root.join(format!("target/{}/{}", target, profile_dir));
    let default_path = workspace_root.join(format!("target/{}", profile_dir));

    // On Windows, we have both static (.lib) and DLL import library (.dll.lib)
    // Try both versions
    let possible_lib_names: Vec<&str> = if target.contains("windows-msvc") {
        vec!["maxion_stub.dll.lib", "maxion_stub.lib"]
    } else {
        vec![lib_name]
    };

    let mut stub_lib_path: Option<PathBuf> = None;

    for lib_name_candidate in possible_lib_names.iter() {
        let path = target_specific_path.join(lib_name_candidate);
        if path.exists() {
            stub_lib_path = Some(path);
            break;
        }

        let default_path_candidate = default_path.join(lib_name_candidate);
        if default_path_candidate.exists() {
            stub_lib_path = Some(default_path_candidate);
            break;
        }

        // If building debug, also try release directory
        if profile_dir == "debug" {
            let release_path = workspace_root
                .join("target/release")
                .join(lib_name_candidate);
            if release_path.exists() {
                println!("cargo:info=Using stub from release directory");
                stub_lib_path = Some(release_path);
                break;
            }
        }
    }

    let stub_lib_path = stub_lib_path.expect(
        "Stub library not found. Please compile the stub first: cargo build --release -p maxion-stub"
    );

    println!("cargo:info=Stub library: {:?}", stub_lib_path);

    // Attempt to extract raw binary using objcopy
    let objcopy_available = Command::new("objcopy")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if objcopy_available {
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let stub_bin_path = out_dir.join("stub.bin");

        let objcopy_status = Command::new("objcopy")
            .args(["-O", "binary"])
            .arg(&stub_lib_path)
            .arg(&stub_bin_path)
            .status();

        match objcopy_status {
            Ok(status) if status.success() => {
                // Read the binary and create a Rust file that includes it
                let stub_bytes = fs::read(&stub_bin_path).expect("Failed to read stub binary");

                let stub_bytes_str = format!("{:?}", stub_bytes);

                // Generate the embed file
                let embed_file = out_dir.join("stub_embed.rs");
                let embed_content = format!(
                    r#"//! Auto-generated stub binary embedding
//!
//! This file is automatically generated by build.rs and contains
//! the compiled maxion-stub binary data.

/// Stub binary data compiled from maxion-stub crate
pub const STUB_BINARY: &[u8] = &{};

/// Stub binary size in bytes
pub const STUB_SIZE: usize = {};

/// Verify stub binary integrity
pub fn stub_integrity_check() -> bool {{
    // Basic check: ensure binary is not empty and has reasonable size
    !STUB_BINARY.is_empty() && STUB_SIZE < 1024 * 1024 // < 1MB
}}
"#,
                    stub_bytes_str,
                    stub_bytes.len()
                );

                fs::write(&embed_file, embed_content).expect("Failed to write stub embed file");

                // Tell cargo to recompile if the stub binary changes
                println!("cargo:rerun-if-changed={}", stub_bin_path.display());

                // Set cfg flag indicating stub was compiled
                println!("cargo:rustc-cfg=stub_compiled");
                println!(
                    "cargo:info=Stub binary embedded: {} bytes",
                    stub_bytes.len()
                );
            }
            _ => {
                println!("cargo:warning=objcopy extraction failed, stub not embedded");
                // Don't set cfg flag - code using #[cfg(stub_compiled)] will be excluded
            }
        }
    } else {
        println!("cargo:warning=objcopy not available, stub not embedded");
        println!("cargo:info=Install objcopy to enable stub binary embedding");
        println!("cargo:info=  macOS: brew install binutils");
        println!("cargo:info=  Linux: apt-get install binutils");
        // Don't set cfg flag - code using #[cfg(stub_compiled)] will be excluded
    }
    // Note: Static library linking removed to prevent duplicate symbols
    // The stub is embedded via objcopy above when available
}
