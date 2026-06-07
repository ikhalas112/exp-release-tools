use crate::config::Config;
use crate::r2_config::{r2_credentials, s3_client};
use crate::resolve::{resolve_release, Platform};
use anyhow::{Context, Result};
use std::path::Path;

/// Upload the per-version manifest to `{game}/manifests/{tag}.json` — a flat,
/// append-only history of every version ever released. Unlike the channel
/// pointer, this is never mirror-deleted and is keyed by the (sanitized) tag,
/// so the full release history stays queryable in one place.
pub async fn archive_manifest(
    config: &Config,
    tag: &str,
    local_manifest: &Path,
    platform: Platform,
) -> Result<()> {
    let creds = r2_credentials()?;
    let bucket = creds.bucket_name;
    let r = resolve_release(tag, config, platform)?;
    let key = &r.paths.manifest_archive_key;
    let client = s3_client().await?;

    let body = tokio::fs::read(local_manifest)
        .await
        .with_context(|| format!("read manifest {}", local_manifest.display()))?;

    client
        .put_object()
        .bucket(&bucket)
        .key(key)
        .body(body.into())
        .content_type("application/json")
        .send()
        .await?;

    println!("✅ archived manifest → {bucket}/{key}");
    Ok(())
}
