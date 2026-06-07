use crate::config::Config;
use anyhow::Result;
use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

/// Target platform for a release. Windows is the default and keeps all
/// existing R2 keys/output names; macOS gets platform-suffixed keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Platform {
    Windows,
    Macos,
}

impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::Windows => "windows",
            Platform::Macos => "macos",
        }
    }
}

fn tag_re() -> &'static Regex {
    static TAG_RE: OnceLock<Regex> = OnceLock::new();
    TAG_RE.get_or_init(|| {
        Regex::new(r"^v(\d+\.\d+\.\d+)(?:-(dev|sit|uat|staging)(?:\.(\d+))?)?$")
            .expect("TAG_RE")
    })
}

/// Tag suffix → (deploy env, channel name). No suffix → prod / prod.
fn env_and_channel(suffix: Option<&str>) -> (&'static str, &'static str) {
    match suffix {
        None => ("prod", "prod"),
        Some("dev") => ("dev", "dev"),
        Some("sit") => ("sit", "sit"),
        Some("uat") => ("uat", "uat"),
        Some("staging") => ("staging", "staging"),
        _ => unreachable!("regex-validated suffix"),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ReleasePaths {
    pub releases: String,
    #[serde(rename = "channelKey")]
    pub channel_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedRelease {
    pub tag: String,
    pub version: String,
    #[serde(rename = "buildNumber")]
    pub build_number: Option<String>,
    pub env: String,
    pub channel: String,
    pub platform: String,
    pub game: String,
    pub source: String,
    pub prepare: String,
    #[serde(rename = "workspaceMode")]
    pub workspace_mode: String,
    pub paths: ReleasePaths,
}

pub fn resolve_release(tag: &str, config: &Config, platform: Platform) -> Result<ResolvedRelease> {
    let caps = tag_re()
        .captures(tag)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "tag \"{tag}\" does not match vX.Y.Z[-(dev|sit|uat|staging)[.N]]"
            )
        })?;
    if platform == Platform::Macos && config.macos.is_none() {
        anyhow::bail!(
            "--platform=macos requires a \"macos\" block in {}",
            config.config_path.display()
        );
    }
    let version = caps.get(1).unwrap().as_str().to_string();
    let suffix = caps.get(2).map(|m| m.as_str());
    let build_number = caps.get(3).map(|m| m.as_str().to_string());
    let (env, channel) = env_and_channel(suffix);

    let paths = match platform {
        Platform::Windows => ReleasePaths {
            releases: format!("releases/{tag}"),
            channel_key: format!("channels/{channel}/manifest.json"),
        },
        Platform::Macos => ReleasePaths {
            releases: format!("releases/{tag}/macos"),
            channel_key: format!("channels/{channel}/manifest-macos.json"),
        },
    };

    Ok(ResolvedRelease {
        tag: tag.to_string(),
        version,
        build_number,
        env: env.to_string(),
        channel: channel.to_string(),
        platform: platform.as_str().to_string(),
        game: config.game.clone(),
        source: config.source.clone(),
        prepare: config.prepare.clone(),
        workspace_mode: config.runner.workspace_mode.clone(),
        paths,
    })
}

/// Artifact filename for the platform (windows: protected exe, macos: app zip).
pub fn artifact_file(config: &Config, platform: Platform) -> String {
    match platform {
        Platform::Windows => config.protector.output_exe.clone(),
        Platform::Macos => config
            .macos
            .as_ref()
            .map(|m| m.output_file.clone())
            .unwrap_or_else(|| "GameClient-macos.zip".into()),
    }
}

/// Directory (relative to the workspace) where inject-version writes version.txt.
pub fn version_inject_dir(config: &Config, platform: Platform) -> String {
    match platform {
        Platform::Windows => format!("build/{}", config.protector.assets_dir),
        Platform::Macos => {
            let m = config.macos.as_ref();
            let bundle = m.map(|m| m.app_bundle.as_str()).unwrap_or("Game.app");
            let dir = m
                .map(|m| m.version_file_dir.as_str())
                .unwrap_or("Contents/Resources");
            format!("build/{bundle}/{dir}")
        }
    }
}

pub fn shquote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

pub fn print_env(r: &ResolvedRelease, artifact_file: &str, version_inject_dir: &str) {
    let artifact_path = format!("output/{artifact_file}");
    let lines = [
        ("TAG", r.tag.as_str()),
        ("VERSION", r.version.as_str()),
        ("DEPLOY_ENV", r.env.as_str()),
        ("CHANNEL", r.channel.as_str()),
        ("PLATFORM", r.platform.as_str()),
        ("GAME", r.game.as_str()),
        ("SOURCE_DIR", r.source.as_str()),
        ("PREPARE_CMD", r.prepare.as_str()),
        ("WORKSPACE_MODE", r.workspace_mode.as_str()),
        ("R2_RELEASES_PATH", r.paths.releases.as_str()),
        ("R2_CHANNEL_KEY", r.paths.channel_key.as_str()),
        ("BUILD_DIR", "build"),
        ("OUTPUT_DIR", "output"),
        ("OUTPUT_EXE", artifact_file),
        ("ARTIFACT_PATH", artifact_path.as_str()),
        ("VERSION_INJECT_DIR", version_inject_dir),
    ];
    for (k, v) in lines {
        println!("export {k}={}", shquote(v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_config;

    fn test_config() -> Config {
        Config {
            game: "snake".into(),
            source: "client".into(),
            prepare: String::new(),
            distribution: "single_exe".into(),
            build: Default::default(),
            protector: Default::default(),
            macos: None,
            r2: crate::config::R2Config {
                bucket: "democlient".into(),
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

    fn test_config_macos() -> Config {
        let mut cfg = test_config();
        cfg.macos = Some(crate::config::MacosConfig {
            app_bundle: "Snake.app".into(),
            output_file: "Snake-macos.zip".into(),
            version_file_dir: "Contents/Resources".into(),
            mock_fixture: "fixtures/mock-game-macos".into(),
        });
        cfg
    }

    #[test]
    fn prod_tag() {
        let r = resolve_release("v1.2.3", &test_config(), Platform::Windows).unwrap();
        assert_eq!(r.env, "prod");
        assert_eq!(r.channel, "prod");
        assert_eq!(r.version, "1.2.3");
        assert_eq!(r.platform, "windows");
        assert_eq!(r.paths.releases, "releases/v1.2.3");
        assert_eq!(r.paths.channel_key, "channels/prod/manifest.json");
    }

    #[test]
    fn dev_sit_uat_staging_tags() {
        let cfg = test_config();
        let dev = resolve_release("v1.2.3-dev", &cfg, Platform::Windows).unwrap();
        assert_eq!(dev.env, "dev");
        assert_eq!(dev.channel, "dev");

        let sit = resolve_release("v1.2.3-sit", &cfg, Platform::Windows).unwrap();
        assert_eq!(sit.env, "sit");
        assert_eq!(sit.channel, "sit");

        let uat = resolve_release("v1.2.3-uat", &cfg, Platform::Windows).unwrap();
        assert_eq!(uat.env, "uat");
        assert_eq!(uat.channel, "uat");

        let staging = resolve_release("v1.2.3-staging", &cfg, Platform::Windows).unwrap();
        assert_eq!(staging.env, "staging");
        assert_eq!(staging.channel, "staging");
    }

    #[test]
    fn build_number() {
        let r = resolve_release("v1.2.3-dev.2", &test_config(), Platform::Windows).unwrap();
        assert_eq!(r.env, "dev");
        assert_eq!(r.channel, "dev");
        assert_eq!(r.build_number.as_deref(), Some("2"));
        assert_eq!(r.paths.releases, "releases/v1.2.3-dev.2");
    }

    #[test]
    fn malformed_tag() {
        let cfg = test_config();
        assert!(resolve_release("v1.2", &cfg, Platform::Windows).is_err());
        assert!(resolve_release("1.2.3", &cfg, Platform::Windows).is_err());
        assert!(resolve_release("v1.2.3-nope", &cfg, Platform::Windows).is_err());
        assert!(resolve_release("v1.2.3-alpha", &cfg, Platform::Windows).is_err());
        assert!(resolve_release("v1.2.3-beta", &cfg, Platform::Windows).is_err());
        assert!(resolve_release("v1.2.3-rc", &cfg, Platform::Windows).is_err());
    }

    #[test]
    fn macos_paths() {
        let r = resolve_release("v1.2.3-dev", &test_config_macos(), Platform::Macos).unwrap();
        assert_eq!(r.platform, "macos");
        assert_eq!(r.paths.releases, "releases/v1.2.3-dev/macos");
        assert_eq!(r.paths.channel_key, "channels/dev/manifest-macos.json");
    }

    #[test]
    fn macos_requires_config_block() {
        let err = resolve_release("v1.2.3", &test_config(), Platform::Macos).unwrap_err();
        assert!(format!("{err:#}").contains("macos"));
    }

    #[test]
    fn artifact_and_inject_dir_per_platform() {
        let cfg = test_config_macos();
        assert_eq!(artifact_file(&cfg, Platform::Windows), "GameClient.exe");
        assert_eq!(artifact_file(&cfg, Platform::Macos), "Snake-macos.zip");
        assert_eq!(version_inject_dir(&cfg, Platform::Windows), "build/assets");
        assert_eq!(
            version_inject_dir(&cfg, Platform::Macos),
            "build/Snake.app/Contents/Resources"
        );
    }

    #[test]
    fn load_example_config() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/release.config.json");
        if path.exists() {
            let cfg = load_config(Some(&path)).unwrap();
            let r = resolve_release("v1.0.0-dev", &cfg, Platform::Windows).unwrap();
            assert_eq!(r.channel, "dev");
        }
    }
}
