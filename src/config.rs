use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectorConfig {
    #[serde(default = "default_input_exe")]
    pub input_exe: String,
    #[serde(default = "default_assets_dir")]
    pub assets_dir: String,
    #[serde(default = "default_output_exe")]
    pub output_exe: String,
    #[serde(default = "default_true")]
    pub phase2: bool,
    #[serde(default = "default_compression_level")]
    pub compression_level: u32,
    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,
    #[serde(default = "default_true")]
    pub enable_protected_all: bool,
    #[serde(default)]
    pub stub_dll: Option<String>,
}

fn default_input_exe() -> String {
    "game.exe".into()
}
fn default_assets_dir() -> String {
    "assets".into()
}
fn default_output_exe() -> String {
    "GameClient.exe".into()
}
fn default_true() -> bool {
    true
}
fn default_compression_level() -> u32 {
    6
}
fn default_chunk_size() -> u32 {
    65536
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacosConfig {
    pub app_bundle: String,
    #[serde(default = "default_macos_output_file")]
    pub output_file: String,
    #[serde(default = "default_macos_version_file_dir")]
    pub version_file_dir: String,
    #[serde(default = "default_macos_mock_fixture")]
    pub mock_fixture: String,
}

fn default_macos_output_file() -> String {
    "GameClient-macos.zip".into()
}
fn default_macos_version_file_dir() -> String {
    "Contents/Resources".into()
}
fn default_macos_mock_fixture() -> String {
    "fixtures/mock-game-macos".into()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct BuildConfig {
    #[serde(default = "default_build_dir")]
    pub dir: String,
    #[serde(default = "default_mock_fixture")]
    pub mock_fixture: String,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            dir: default_build_dir(),
            mock_fixture: default_mock_fixture(),
        }
    }
}

fn default_build_dir() -> String {
    "build".into()
}
fn default_mock_fixture() -> String {
    "fixtures/mock-game".into()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RawConfig {
    /// Optional in the file: the game name normally comes from the GAME_NAME env
    /// var (set by the reusable workflow's `game_name` input), keeping the
    /// inline config_json focused on build/deploy settings only.
    #[serde(default)]
    pub game: String,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub prepare: String,
    #[serde(default = "default_distribution")]
    pub distribution: String,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub protector: ProtectorConfig,
    #[serde(default)]
    pub macos: Option<MacosConfig>,
    pub r2: R2Config,
    #[serde(default)]
    pub launcher: LauncherConfig,
    #[serde(default)]
    pub lfs: LfsConfig,
    #[serde(default)]
    pub concurrency: ConcurrencyConfig,
    #[serde(default)]
    pub cdn_base_url: String,
    #[serde(default)]
    pub runner: RunnerConfig,
}

fn default_source() -> String {
    "client".into()
}
fn default_distribution() -> String {
    "single_exe".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct R2Config {
    pub bucket: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LauncherConfig {
    #[serde(default = "default_min_version")]
    pub min_version: String,
}

fn default_min_version() -> String {
    "1.0.0".into()
}

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct LfsConfig {
    #[serde(default)]
    pub binary_extensions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct ConcurrencyConfig {
    #[serde(default = "default_concurrency")]
    pub gzip: u32,
    #[serde(default = "default_concurrency")]
    pub upload: u32,
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            gzip: 30,
            upload: 30,
        }
    }
}

fn default_concurrency() -> u32 {
    30
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunnerConfig {
    #[serde(default = "default_workspace_mode")]
    pub workspace_mode: String,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            workspace_mode: default_workspace_mode(),
        }
    }
}

fn default_workspace_mode() -> String {
    "fresh".into()
}

impl Default for ProtectorConfig {
    fn default() -> Self {
        Self {
            input_exe: default_input_exe(),
            assets_dir: default_assets_dir(),
            output_exe: default_output_exe(),
            phase2: true,
            compression_level: 6,
            chunk_size: 65536,
            enable_protected_all: true,
            stub_dll: None,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Config {
    pub game: String,
    pub source: String,
    pub prepare: String,
    pub distribution: String,
    pub build: BuildConfig,
    pub protector: ProtectorConfig,
    pub macos: Option<MacosConfig>,
    pub r2: R2Config,
    pub launcher: LauncherConfig,
    pub lfs: LfsConfig,
    pub concurrency: ConcurrencyConfig,
    pub cdn_base_url: String,
    pub runner: RunnerConfig,
    pub config_path: PathBuf,
}

/// The game name is the top-level R2 key prefix (`{game}/releases/...`), so it
/// must be path-safe: start alphanumeric, then only `[A-Za-z0-9._-]`. No
/// slashes, spaces, or other characters that would mangle the key namespace.
fn validate_game_name(game: &str) -> Result<()> {
    if game.is_empty() {
        bail!("game name is required (set config.game or the GAME_NAME env var)");
    }
    let valid = game.chars().enumerate().all(|(i, c)| {
        if i == 0 {
            c.is_ascii_alphanumeric()
        } else {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')
        }
    });
    if !valid {
        bail!(
            "invalid game name {game:?}: must start with a letter/digit and contain \
             only [A-Za-z0-9._-] (it becomes the R2 key prefix)"
        );
    }
    Ok(())
}

pub fn load_config(config_path: Option<&Path>) -> Result<Config> {
    let path = config_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("release.config.json"));
    let abs = std::env::current_dir()
        .context("cwd")?
        .join(&path);
    if !abs.exists() {
        bail!("release config not found: {}", abs.display());
    }
    let raw: RawConfig = serde_json::from_str(&fs::read_to_string(&abs)?)
        .with_context(|| format!("invalid JSON in {}", abs.display()))?;
    // game name resolves runtime env → config (mirrors the R2 credential model),
    // so CI/forks/tests can re-scope R2 keys without editing the checked-in config.
    let game = std::env::var("GAME_NAME")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| raw.game.clone());
    validate_game_name(&game)?;
    if raw.r2.bucket.is_empty() {
        bail!("config.r2.bucket is required");
    }
    if let Some(ref m) = raw.macos {
        if m.app_bundle.is_empty() {
            bail!("config.macos.appBundle is required when macos block is present");
        }
    }

    Ok(Config {
        game,
        source: if raw.source.is_empty() {
            "client".into()
        } else {
            raw.source
        },
        prepare: raw.prepare,
        distribution: raw.distribution,
        build: raw.build,
        protector: raw.protector,
        macos: raw.macos,
        r2: raw.r2,
        launcher: LauncherConfig {
            min_version: if raw.launcher.min_version.is_empty() {
                default_min_version()
            } else {
                raw.launcher.min_version
            },
        },
        lfs: raw.lfs,
        concurrency: raw.concurrency,
        cdn_base_url: raw.cdn_base_url,
        runner: raw.runner,
        config_path: abs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn macos_block_defaults() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("release.config.json");
        std::fs::write(
            &path,
            r#"{
  "game": "test",
  "r2": { "bucket": "b" },
  "macos": { "appBundle": "Game.app" }
}"#,
        )
        .unwrap();
        let cfg = load_config(Some(&path)).unwrap();
        let m = cfg.macos.expect("macos block");
        assert_eq!(m.app_bundle, "Game.app");
        assert_eq!(m.output_file, "GameClient-macos.zip");
        assert_eq!(m.version_file_dir, "Contents/Resources");
        assert_eq!(m.mock_fixture, "fixtures/mock-game-macos");
    }

    #[test]
    fn macos_block_requires_app_bundle() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("release.config.json");
        std::fs::write(
            &path,
            r#"{
  "game": "test",
  "r2": { "bucket": "b" },
  "macos": { "appBundle": "" }
}"#,
        )
        .unwrap();
        let err = load_config(Some(&path)).unwrap_err();
        assert!(format!("{err:#}").contains("appBundle"));
    }

    #[test]
    fn config_without_macos_block_loads() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("release.config.json");
        std::fs::write(&path, r#"{ "game": "test", "r2": { "bucket": "b" } }"#).unwrap();
        let cfg = load_config(Some(&path)).unwrap();
        assert!(cfg.macos.is_none());
    }

    #[test]
    fn missing_game_without_env_errors() {
        // game may legitimately come from GAME_NAME; only assert the file-only path.
        if std::env::var("GAME_NAME").is_ok() {
            return;
        }
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("release.config.json");
        std::fs::write(&path, r#"{ "r2": { "bucket": "b" } }"#).unwrap();
        let err = load_config(Some(&path)).unwrap_err();
        assert!(format!("{err:#}").contains("game name is required"));
    }

    #[test]
    fn game_name_validation() {
        assert!(validate_game_name("snake").is_ok());
        assert!(validate_game_name("exp-window-game").is_ok());
        assert!(validate_game_name("Game_2.0").is_ok());
        assert!(validate_game_name("").is_err());
        assert!(validate_game_name("bad/name").is_err());
        assert!(validate_game_name("has space").is_err());
        assert!(validate_game_name("-leadingdash").is_err());
    }

    #[test]
    fn reject_unknown_channels_field() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("release.config.json");
        std::fs::write(
            &path,
            r#"{
  "game": "test",
  "r2": { "bucket": "b" },
  "channels": { "dev": "alpha" }
}"#,
        )
        .unwrap();
        let err = load_config(Some(&path)).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("channels"),
            "expected reject unknown channels field, got: {msg}"
        );
    }
}
