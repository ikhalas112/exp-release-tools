use crate::config::Config;
use crate::r2_config::maxion_build_secret;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn resolve_build_secret() -> Option<String> {
    maxion_build_secret()
}

pub struct ProtectOptions<'a> {
    pub config: &'a Config,
    pub build_dir: &'a Path,
    pub output_dir: &'a Path,
    pub build_secret: Option<&'a str>,
    pub stub_dll: Option<&'a Path>,
}

pub fn protect(opts: &ProtectOptions<'_>) -> Result<PathBuf> {
    let p = &opts.config.protector;
    let input = opts.build_dir.join(&p.input_exe);
    let assets = opts.build_dir.join(&p.assets_dir);
    let output = opts.output_dir.join(&p.output_exe);

    if !input.is_file() {
        bail!("input executable not found: {}", input.display());
    }
    if !assets.is_dir() {
        bail!("assets directory not found: {}", assets.display());
    }
    std::fs::create_dir_all(opts.output_dir)?;

    let stub_dll = resolve_stub_dll(opts.stub_dll, p.stub_dll.as_deref())?;
    let pnp = resolve_pnp_binary()?;

    println!("=== protect ===");
    println!("  input:  {}", input.display());
    println!("  assets: {}", assets.display());
    println!("  output: {}", output.display());
    println!("  stub:   {}", stub_dll.display());

    let mut cmd = Command::new(&pnp);
    cmd.arg("protect")
        .arg("--input")
        .arg(&input)
        .arg("--assets")
        .arg(&assets)
        .arg("--output")
        .arg(&output)
        .arg("--chunk-size")
        .arg(p.chunk_size.to_string())
        .arg("--compression-level")
        .arg(p.compression_level.to_string())
        .arg("--stub-dll")
        .arg(&stub_dll);

    if p.phase2 {
        cmd.arg("--phase2");
    }
    if p.enable_protected_all {
        cmd.arg("--enable-protected-all");
    }
    if let Some(secret) = opts.build_secret {
        cmd.arg("--build-secret").arg(secret);
    }

    let status = cmd.status().context("run pnp protect")?;
    if !status.success() {
        bail!("pnp protect exited with status {status}");
    }
    if !output.is_file() {
        bail!("protected executable was not created: {}", output.display());
    }

    println!("✓ protected executable: {}", output.display());
    Ok(output)
}

fn bundle_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

fn resolve_pnp_binary() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidates = Vec::new();
    if let Some(dir) = bundle_dir() {
        candidates.push(dir.join("pnp.exe"));
        candidates.push(dir.join("pnp"));
    }
    candidates.extend([
        manifest_dir.join("maxion-protector/target/release/pnp.exe"),
        manifest_dir.join("maxion-protector/target/release/pnp"),
        manifest_dir.join("target/release/pnp.exe"),
        manifest_dir.join("target/release/pnp"),
    ]);
    for c in &candidates {
        if c.is_file() {
            return Ok(c.clone());
        }
    }

    println!("Building maxion-packer (pnp)...");
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "-p",
            "maxion-packer",
            "--manifest-path",
            manifest_dir
                .join("maxion-protector/Cargo.toml")
                .to_str()
                .unwrap(),
        ])
        .status()
        .context("build maxion-packer")?;
    if !status.success() {
        bail!("failed to build maxion-packer");
    }

    for c in &candidates {
        if c.is_file() {
            return Ok(c.clone());
        }
    }
    bail!("pnp binary not found after build");
}

fn resolve_stub_dll(explicit: Option<&Path>, config_path: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if p.is_file() {
            return Ok(p.to_path_buf());
        }
        bail!("stub dll not found: {}", p.display());
    }
    if let Some(rel) = config_path {
        let p = PathBuf::from(rel);
        if p.is_file() {
            return Ok(p);
        }
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut candidates = Vec::new();
    if let Some(dir) = bundle_dir() {
        candidates.push(dir.join("maxion_stub.dll"));
    }
    candidates.extend([
        manifest_dir.join("maxion-protector/target/release/maxion_stub.dll"),
        manifest_dir.join("maxion-protector/target/x86_64-pc-windows-msvc/release/maxion_stub.dll"),
        PathBuf::from("target/release/maxion_stub.dll"),
    ]);
    for c in &candidates {
        if c.is_file() {
            return Ok(c.clone());
        }
    }

    println!("Building maxion-stub...");
    let status = Command::new("cargo")
        .args([
            "build",
            "--release",
            "-p",
            "maxion-stub",
            "--manifest-path",
            manifest_dir
                .join("maxion-protector/Cargo.toml")
                .to_str()
                .unwrap(),
        ])
        .status()
        .context("build maxion-stub")?;
    if !status.success() {
        bail!("failed to build maxion-stub");
    }

    for c in &candidates {
        if c.is_file() {
            return Ok(c.clone());
        }
    }
    bail!("maxion_stub.dll not found — protect requires Windows + maxion-stub build");
}
