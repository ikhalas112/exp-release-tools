use crate::config::Config;
use crate::resolve::{artifact_file, resolve_release, Platform};
use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ChannelManifest {
    latest_tag: String,
    urls: ChannelUrls,
}

#[derive(Debug, Deserialize)]
struct ChannelUrls {
    executable: Option<String>,
}

pub async fn verify(config: &Config, tag: &str, platform: Platform) -> Result<()> {
    let r = resolve_release(tag, config, platform)?;
    let base = std::env::var("VERIFY_CDN_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if config.cdn_base_url.is_empty() {
                None
            } else {
                Some(config.cdn_base_url.clone())
            }
        })
        .map(|s| s.trim_end_matches('/').to_string());

    if base.is_none() {
        println!("⏭️  verify skipped: no cdnBaseUrl in config (set cdnBaseUrl or VERIFY_CDN_URL)");
        return Ok(());
    }
    let base = base.unwrap();
    let exe_name = artifact_file(config, platform);
    let exe_url = format!("{base}/{}/{}", r.paths.releases, exe_name);
    let chan_url = format!("{base}/{}", r.paths.channel_key);

    println!("🔍 verify {} ({}/{}/{})", r.tag, r.env, r.channel, r.platform);
    println!("   executable: {exe_url}");
    println!("   channel:    {chan_url}");

    let client = reqwest::Client::new();
    let exe_resp = client.get(&exe_url).send().await?;
    if !exe_resp.status().is_success() {
        bail!("HTTP {} for {exe_url}", exe_resp.status());
    }
    let exe_bytes = exe_resp.bytes().await?;
    if exe_bytes.is_empty() {
        bail!("release executable is empty");
    }

    let chan_resp = client.get(&chan_url).send().await?;
    if !chan_resp.status().is_success() {
        bail!("HTTP {} for {chan_url}", chan_resp.status());
    }
    let chan: ChannelManifest = chan_resp.json().await?;

    let mut problems = Vec::new();
    if chan.latest_tag != r.tag {
        problems.push(format!(
            "channel latest_tag=\"{}\" != \"{}\"",
            chan.latest_tag, r.tag
        ));
    }
    let expect_exe = format!("{}/{}", r.paths.releases, exe_name);
    if chan.urls.executable.as_deref() != Some(expect_exe.as_str()) {
        problems.push(format!(
            "channel urls.executable={:?} != \"{expect_exe}\"",
            chan.urls.executable
        ));
    }

    println!(
        "   executable size: {} bytes | channel latest_tag: {}",
        exe_bytes.len(),
        chan.latest_tag
    );

    if !problems.is_empty() {
        eprintln!("❌ verify FAILED:");
        for p in &problems {
            eprintln!("   - {p}");
        }
        bail!("verification failed");
    }
    println!("✅ verify OK");
    Ok(())
}
