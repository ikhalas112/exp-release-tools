use crate::config::Config;
use crate::r2_config::{r2_credentials, s3_client};
use crate::resolve::{resolve_release, Platform};
use crate::version_cmp::cmp_release;
use anyhow::Result;
use mime_guess::from_path;
use std::path::Path;

pub async fn update_channel(
    config: &Config,
    tag: &str,
    local_manifest: &Path,
    force: bool,
    platform: Platform,
) -> Result<()> {
    let creds = r2_credentials()?;
    let bucket = creds.bucket_name;
    let r = resolve_release(tag, config, platform)?;
    let key = &r.paths.channel_key;
    let client = s3_client().await?;

    let existing = match client
        .get_object()
        .bucket(&bucket)
        .key(key)
        .send()
        .await
    {
        Ok(out) => {
            let data = out.body.collect().await?.into_bytes();
            Some(serde_json::from_slice::<serde_json::Value>(&data)?)
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("NoSuchKey") || msg.contains("404") {
                None
            } else {
                return Err(e.into());
            }
        }
    };

    let (allow, reason) = if let Some(ref ex) = existing {
        let ex_ver = ex["latest_version"].as_str().unwrap_or("");
        let ex_build = ex["build_number"].as_str();
        let ex_tag = ex["latest_tag"].as_str().unwrap_or("");
        let cmp = cmp_release(
            &r.version,
            r.build_number.as_deref(),
            ex_ver,
            ex_build,
        );
        if force {
            (true, format!("--force (override; current={ex_tag})"))
        } else if cmp >= 0 {
            (
                true,
                format!("newer-or-equal than current ({ex_tag})"),
            )
        } else {
            (
                false,
                format!(
                    "OLDER than current channel version {ex_tag} ({ex_ver}); not moving channel"
                ),
            )
        }
    } else {
        (true, "first release on this channel".to_string())
    };

    println!(
        "channel {}: incoming {} ({}{})",
        r.channel,
        r.tag,
        r.version,
        r.build_number
            .as_ref()
            .map(|b| format!(".{b}"))
            .unwrap_or_default()
    );

    if !allow {
        println!("⏭️  channel NOT updated — {reason}");
        println!(
            "   (release artifacts were still uploaded to releases/{}; use --force to override)",
            r.tag
        );
        return Ok(());
    }

    let body = tokio::fs::read(local_manifest).await?;
    let ct = from_path(local_manifest)
        .first_or_octet_stream()
        .essence_str()
        .to_string();
    client
        .put_object()
        .bucket(&bucket)
        .key(key)
        .body(body.into())
        .content_type(ct)
        .send()
        .await?;
    println!("✅ channel updated → {bucket}/{key} — {reason}");
    Ok(())
}
