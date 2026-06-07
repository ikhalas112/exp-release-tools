use crate::config::Config;
use crate::resolve::{artifact_file, resolve_release, Platform};
use anyhow::{bail, Result};
use md5::{Digest, Md5};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Checksum {
    pub md5: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtifactInfo {
    pub filename: String,
    pub size: u64,
    pub size_human: String,
    pub checksum: Checksum,
}

#[derive(Debug, Serialize)]
pub struct ManifestUrls {
    pub executable: String,
    pub manifest: String,
    pub base: String,
}

#[derive(Debug, Serialize)]
pub struct VersionManifest {
    pub game: String,
    pub distribution: String,
    /// None on windows — keeps the legacy manifest JSON byte-identical for the launcher.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    pub version: String,
    pub tag: String,
    pub build_number: Option<String>,
    pub channel: String,
    pub environment: String,
    pub released_at: String,
    pub is_critical: bool,
    pub release_notes: String,
    pub min_launcher_version: String,
    pub artifact: ArtifactInfo,
    pub urls: ManifestUrls,
}

#[derive(Debug, Serialize)]
pub struct ChannelManifestUrls {
    pub manifest: String,
    pub executable: String,
}

#[derive(Debug, Serialize)]
pub struct ChannelManifest {
    pub game: String,
    pub distribution: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    pub channel: String,
    pub latest_version: String,
    pub latest_tag: String,
    pub build_number: Option<String>,
    pub released_at: String,
    pub is_critical: bool,
    pub release_notes: String,
    pub min_launcher_version: String,
    pub artifact: ArtifactInfo,
    pub urls: ChannelManifestUrls,
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".into();
    }
    const K: f64 = 1024.0;
    const SIZES: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let i = (bytes as f64).log(K).floor() as usize;
    let i = i.min(SIZES.len() - 1);
    let value = bytes as f64 / K.powi(i as i32);
    format!("{value:.1} {}", SIZES[i])
}

pub fn md5_file(path: &Path) -> Result<String> {
    let data = fs::read(path)?;
    Ok(format!("{:x}", Md5::digest(data)))
}

pub fn generate_manifests(
    config: &Config,
    tag: &str,
    artifact_path: &Path,
    output_dir: &Path,
    platform: Platform,
) -> Result<(PathBuf, PathBuf)> {
    let r = resolve_release(tag, config, platform)?;
    if !artifact_path.is_file() {
        bail!("artifact not found: {}", artifact_path.display());
    }

    // None on windows keeps the legacy JSON unchanged for existing launchers.
    let platform_field = match platform {
        Platform::Windows => None,
        Platform::Macos => Some("macos".to_string()),
    };

    let meta = fs::metadata(artifact_path)?;
    let default_filename = artifact_file(config, platform);
    let filename = artifact_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&default_filename)
        .to_string();
    let md5 = md5_file(artifact_path)?;
    let released_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let release_notes = std::env::var("RELEASE_NOTES").unwrap_or_default();

    let artifact = ArtifactInfo {
        filename: filename.clone(),
        size: meta.len(),
        size_human: format_bytes(meta.len()),
        checksum: Checksum { md5: md5.clone() },
    };

    println!("=== Generating manifests ({}) ===", r.platform);
    println!("  Game:        {}", config.game);
    println!("  Tag:         {}", r.tag);
    println!("  Version:     {}", r.version);
    println!("  Environment: {}", r.env);
    println!("  Channel:     {}", r.channel);
    println!(
        "  artifact:    {} ({})",
        filename,
        format_bytes(meta.len())
    );

    let version_manifest = VersionManifest {
        game: config.game.clone(),
        distribution: config.distribution.clone(),
        platform: platform_field.clone(),
        version: r.version.clone(),
        tag: r.tag.clone(),
        build_number: r.build_number.clone(),
        channel: r.channel.clone(),
        environment: r.env.clone(),
        released_at: released_at.clone(),
        is_critical: false,
        release_notes: release_notes.clone(),
        min_launcher_version: config.launcher.min_version.clone(),
        artifact: artifact.clone(),
        urls: ManifestUrls {
            executable: format!("{}/{}", r.paths.releases, filename),
            manifest: format!("{}/manifest.json", r.paths.releases),
            base: format!("{}/", r.paths.releases),
        },
    };

    let channel_manifest = ChannelManifest {
        game: config.game.clone(),
        distribution: config.distribution.clone(),
        platform: platform_field,
        channel: r.channel.clone(),
        latest_version: r.version.clone(),
        latest_tag: r.tag.clone(),
        build_number: r.build_number.clone(),
        released_at,
        is_critical: false,
        release_notes,
        min_launcher_version: config.launcher.min_version.clone(),
        artifact,
        urls: ChannelManifestUrls {
            manifest: format!("{}/manifest.json", r.paths.releases),
            executable: format!("{}/{}", r.paths.releases, filename),
        },
    };

    fs::create_dir_all(output_dir)?;
    let manifest_path = output_dir.join("manifest.json");
    let channel_path = output_dir.join("channel-manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&version_manifest)?,
    )?;
    fs::write(
        &channel_path,
        serde_json::to_string_pretty(&channel_manifest)?,
    )?;
    println!("✓ {}", manifest_path.display());
    println!("✓ {}", channel_path.display());
    Ok((manifest_path, channel_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use tempfile::TempDir;

    fn test_config() -> Config {
        Config {
            game: "mygame".into(),
            source: "build".into(),
            prepare: String::new(),
            distribution: "single_exe".into(),
            build: Default::default(),
            protector: crate::config::ProtectorConfig {
                output_exe: "GameClient.exe".into(),
                ..Default::default()
            },
            macos: None,
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

    #[test]
    fn manifest_single_exe_schema() {
        let tmp = TempDir::new().unwrap();
        let artifact = tmp.path().join("GameClient.exe");
        fs::write(&artifact, b"fake exe content").unwrap();
        let out = tmp.path().join("output");
        let (manifest, channel) =
            generate_manifests(&test_config(), "v1.2.3-dev", &artifact, &out, Platform::Windows)
                .unwrap();
        let m: serde_json::Value = serde_json::from_str(&fs::read_to_string(manifest).unwrap()).unwrap();
        assert_eq!(m["distribution"], "single_exe");
        assert_eq!(m["artifact"]["filename"], "GameClient.exe");
        assert!(m["urls"]["executable"].as_str().unwrap().ends_with("GameClient.exe"));
        assert!(!m["urls"]["list"].is_string());
        // windows manifest must stay byte-compatible: no new platform key
        assert!(m.get("platform").is_none());
        let c: serde_json::Value = serde_json::from_str(&fs::read_to_string(channel).unwrap()).unwrap();
        assert_eq!(c["latest_tag"], "v1.2.3-dev");
        assert_eq!(c["channel"], "dev");
        assert!(c["urls"]["executable"].is_string());
        assert!(c.get("platform").is_none());
    }

    #[test]
    fn manifest_macos_schema() {
        let mut cfg = test_config();
        cfg.macos = Some(crate::config::MacosConfig {
            app_bundle: "Game.app".into(),
            output_file: "GameClient-macos.zip".into(),
            version_file_dir: "Contents/Resources".into(),
            mock_fixture: "fixtures/mock-game-macos".into(),
        });
        let tmp = TempDir::new().unwrap();
        let artifact = tmp.path().join("GameClient-macos.zip");
        fs::write(&artifact, b"fake zip content").unwrap();
        let out = tmp.path().join("output");
        let (manifest, channel) =
            generate_manifests(&cfg, "v1.2.3-dev", &artifact, &out, Platform::Macos).unwrap();
        let m: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(manifest).unwrap()).unwrap();
        assert_eq!(m["platform"], "macos");
        assert_eq!(m["artifact"]["filename"], "GameClient-macos.zip");
        assert_eq!(
            m["urls"]["executable"],
            "releases/v1.2.3-dev/macos/GameClient-macos.zip"
        );
        assert_eq!(m["urls"]["manifest"], "releases/v1.2.3-dev/macos/manifest.json");
        let c: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(channel).unwrap()).unwrap();
        assert_eq!(c["platform"], "macos");
        assert_eq!(
            c["urls"]["executable"],
            "releases/v1.2.3-dev/macos/GameClient-macos.zip"
        );
    }
}
