use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn mock_build(fixture: &Path, output: &Path) -> Result<()> {
    if !fixture.exists() {
        bail!("mock fixture not found: {}", fixture.display());
    }

    if output.exists() {
        fs::remove_dir_all(output).with_context(|| format!("clean {}", output.display()))?;
    }
    fs::create_dir_all(output)?;

    let assets_src = fixture.join("assets");
    let assets_dst = output.join("assets");
    if assets_src.is_dir() {
        copy_dir_recursive(&assets_src, &assets_dst)?;
    }

    let game_exe = resolve_game_exe(fixture)?;
    fs::copy(&game_exe, output.join("game.exe"))
        .with_context(|| format!("copy {} → build/game.exe", game_exe.display()))?;

    println!("=== mock-build complete ===");
    println!("  output: {}", output.display());
    println!("  game.exe: {} bytes", fs::metadata(output.join("game.exe"))?.len());
    println!(
        "  assets: {} files",
        WalkDir::new(&assets_dst)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .count()
    );
    Ok(())
}

/// macOS mock build: copy fixture's .app bundle → build/{app_bundle}.
/// `copy_dir_recursive` preserves permission bits (fs::copy), so the
/// executable inside Contents/MacOS stays runnable. Fixtures contain no symlinks.
pub fn mock_build_macos(fixture: &Path, output: &Path, app_bundle: &str) -> Result<()> {
    let app_src = fixture.join(app_bundle);
    if !app_src.is_dir() {
        bail!(
            "mock fixture app bundle not found: {} (generate it first)",
            app_src.display()
        );
    }

    if output.exists() {
        fs::remove_dir_all(output).with_context(|| format!("clean {}", output.display()))?;
    }
    fs::create_dir_all(output)?;

    let app_dst = output.join(app_bundle);
    copy_dir_recursive(&app_src, &app_dst)?;

    println!("=== mock-build complete (macos) ===");
    println!("  output: {}", app_dst.display());
    println!(
        "  files: {}",
        WalkDir::new(&app_dst)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .count()
    );
    Ok(())
}

fn resolve_game_exe(fixture: &Path) -> Result<PathBuf> {
    let fixture_exe = fixture.join("game.exe");
    if fixture_exe.is_file() {
        return Ok(fixture_exe);
    }

    if cfg!(windows) {
        if let Some(built) = try_build_hello_world()? {
            return Ok(built);
        }
    }

    bail!(
        "game.exe not found in {} and auto-build is only supported on Windows.\n\
         Place a Windows PE at fixtures/mock-game/game.exe or run on windows-latest CI.",
        fixture.display()
    );
}

fn try_build_hello_world() -> Result<Option<PathBuf>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let hello_dir = manifest_dir.join("maxion-protector/examples/hello-world");
    if !hello_dir.join("Cargo.toml").exists() {
        return Ok(None);
    }

    println!("Building hello-world example for mock game.exe...");
    let status = std::process::Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&hello_dir)
        .status()
        .context("spawn cargo build hello-world")?;
    if !status.success() {
        bail!("cargo build hello-world failed");
    }

    let candidates = [
        hello_dir.join("target/release/hello.exe"),
        hello_dir.join("target/release/hello"),
    ];
    for c in candidates {
        if c.is_file() {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in WalkDir::new(src).min_depth(1) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn mock_build_macos_copies_app_bundle() {
        let tmp = TempDir::new().unwrap();
        let fixture = tmp.path().join("fixture");
        let macos_dir = fixture.join("Game.app/Contents/MacOS");
        fs::create_dir_all(&macos_dir).unwrap();
        fs::write(macos_dir.join("game"), b"binary").unwrap();
        fs::create_dir_all(fixture.join("Game.app/Contents/Resources")).unwrap();
        fs::write(fixture.join("Game.app/Contents/Info.plist"), b"<plist/>").unwrap();

        let out = tmp.path().join("build");
        mock_build_macos(&fixture, &out, "Game.app").unwrap();
        assert!(out.join("Game.app/Contents/MacOS/game").is_file());
        assert!(out.join("Game.app/Contents/Info.plist").is_file());
    }

    #[test]
    fn mock_build_macos_missing_bundle_errors() {
        let tmp = TempDir::new().unwrap();
        let err = mock_build_macos(tmp.path(), &tmp.path().join("build"), "Game.app")
            .unwrap_err();
        assert!(format!("{err:#}").contains("app bundle not found"));
    }

    #[test]
    fn mock_build_copies_assets() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/mock-game");
        let tmp = TempDir::new().unwrap();
        if fixture.join("game.exe").exists() {
            mock_build(&fixture, tmp.path()).unwrap();
            assert!(tmp.path().join("assets/config.json").exists());
            assert!(tmp.path().join("game.exe").exists());
        }
    }
}
