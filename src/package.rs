use crate::config::Config;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pack a macOS .app bundle into a single zip via `ditto -c -k`
/// (preserves symlinks, permissions, and any existing code signature).
pub fn package_macos(config: &Config, build_dir: &Path, output_dir: &Path) -> Result<PathBuf> {
    let Some(mac) = config.macos.as_ref() else {
        bail!(
            "package --platform=macos requires a \"macos\" block in {}",
            config.config_path.display()
        );
    };

    let app = build_dir.join(&mac.app_bundle);
    let output = output_dir.join(&mac.output_file);

    if !app.is_dir() {
        bail!("app bundle not found: {}", app.display());
    }
    if !cfg!(target_os = "macos") {
        bail!("package --platform=macos requires a macOS host (uses `ditto`)");
    }
    std::fs::create_dir_all(output_dir)?;
    if output.exists() {
        std::fs::remove_file(&output)?;
    }

    println!("=== package (macos) ===");
    println!("  app:    {}", app.display());
    println!("  output: {}", output.display());

    let status = Command::new("ditto")
        .arg("-c")
        .arg("-k")
        .arg("--sequesterRsrc")
        .arg("--keepParent")
        .arg(&app)
        .arg(&output)
        .status()
        .context("run ditto")?;
    if !status.success() {
        bail!("ditto exited with status {status}");
    }
    if !output.is_file() {
        bail!("packaged zip was not created: {}", output.display());
    }

    println!("✓ packaged app: {}", output.display());
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, MacosConfig};
    use tempfile::TempDir;

    fn test_config(macos: Option<MacosConfig>) -> Config {
        Config {
            game: "mygame".into(),
            source: "build".into(),
            prepare: String::new(),
            distribution: "single_exe".into(),
            build: Default::default(),
            protector: Default::default(),
            macos,
            r2: crate::config::R2Config {
                bucket: "test".into(),
            },
            launcher: crate::config::LauncherConfig {
                min_version: "1.0.0".into(),
            },
            lfs: Default::default(),
            concurrency: Default::default(),
            cdn_base_url: String::new(),
            runner: Default::default(),
            config_path: "release.config.json".into(),
        }
    }

    fn macos_config() -> MacosConfig {
        MacosConfig {
            app_bundle: "Game.app".into(),
            output_file: "Game-macos.zip".into(),
            version_file_dir: "Contents/Resources".into(),
            mock_fixture: "fixtures/mock-game-macos".into(),
        }
    }

    #[test]
    fn requires_macos_block() {
        let tmp = TempDir::new().unwrap();
        let err = package_macos(&test_config(None), tmp.path(), tmp.path()).unwrap_err();
        assert!(format!("{err:#}").contains("macos"));
    }

    #[test]
    fn requires_app_bundle_dir() {
        let tmp = TempDir::new().unwrap();
        let err = package_macos(&test_config(Some(macos_config())), tmp.path(), tmp.path())
            .unwrap_err();
        assert!(format!("{err:#}").contains("app bundle not found"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn packages_app_to_zip() {
        let tmp = TempDir::new().unwrap();
        let build = tmp.path().join("build");
        let app = build.join("Game.app/Contents/MacOS");
        std::fs::create_dir_all(&app).unwrap();
        std::fs::write(app.join("game"), b"#!/bin/sh\necho hi\n").unwrap();
        let out_dir = tmp.path().join("output");
        let zip =
            package_macos(&test_config(Some(macos_config())), &build, &out_dir).unwrap();
        assert!(zip.is_file());
        assert!(std::fs::metadata(&zip).unwrap().len() > 0);
    }
}
