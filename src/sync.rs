use crate::r2_config::{r2_credentials, s3_client};
use anyhow::{Context, Result};
use aws_sdk_s3::Client;
use mime_guess::from_path;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use walkdir::WalkDir;

struct SyncConfig {
    bucket: String,
    concurrency: usize,
}

fn load_sync(upload_concurrency: u32) -> Result<SyncConfig> {
    let creds = r2_credentials()?;
    let concurrency = std::env::var("SYNC_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(upload_concurrency as usize);
    Ok(SyncConfig {
        bucket: creds.bucket_name,
        concurrency,
    })
}

async fn list_r2(client: &Client, bucket: &str, prefix: &str) -> Result<HashMap<String, u64>> {
    let mut map = HashMap::new();
    let mut token = None;
    loop {
        let mut req = client.list_objects_v2().bucket(bucket).prefix(prefix);
        if let Some(t) = &token {
            req = req.continuation_token(t);
        }
        let res = req.send().await?;
        for obj in res.contents() {
            if let (Some(key), Some(size)) = (obj.key(), obj.size()) {
                map.insert(key.to_string(), size as u64);
            }
        }
        token = res.next_continuation_token().map(str::to_string);
        if token.is_none() {
            break;
        }
    }
    Ok(map)
}

/// A remote key is deleted only if it's missing locally AND not under an
/// excluded prefix (relative to the dest base). Exclusion keeps another
/// platform's nested artifacts (e.g. `macos/`) safe from the mirror delete.
fn should_delete(
    key: &str,
    base: &str,
    local_keys: &HashSet<String>,
    exclude_prefixes: &[String],
) -> bool {
    if local_keys.contains(key) {
        return false;
    }
    let rel = key
        .strip_prefix(base)
        .map(|r| r.trim_start_matches('/'))
        .unwrap_or(key);
    !exclude_prefixes.iter().any(|p| rel.starts_with(p.as_str()))
}

pub async fn sync(
    local_dir: &Path,
    dest_paths: &[String],
    upload_concurrency: u32,
    exclude_prefixes: &[String],
) -> Result<()> {
    let cfg = load_sync(upload_concurrency)?;
    let client = s3_client().await?;
    let root = local_dir.canonicalize().context("local dir")?;
    let start = std::time::Instant::now();

    println!("🚀 sync {}", root.display());
    println!("   bucket: {}", cfg.bucket);
    for d in dest_paths {
        println!("   dest:   {d}");
    }

    let mut local_map: HashMap<String, (PathBuf, u64)> = HashMap::new();
    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(&root)?
                .to_string_lossy()
                .replace('\\', "/");
            local_map.insert(rel, (entry.path().to_path_buf(), entry.metadata()?.len()));
        }
    }
    println!("📁 local files: {}", local_map.len());

    let mut r2_maps = Vec::with_capacity(dest_paths.len());
    for d in dest_paths {
        r2_maps.push(list_r2(&client, &cfg.bucket, d).await?);
    }

    let mut upload_tasks = Vec::new();
    let mut delete_keys = Vec::new();
    let mut skip = 0usize;

    for (i, base) in dest_paths.iter().enumerate() {
        let r2_files = &r2_maps[i];
        let mut local_keys = HashSet::new();
        for (rel, (path, size)) in &local_map {
            let key = format!("{base}/{rel}");
            local_keys.insert(key.clone());
            match r2_files.get(&key) {
                Some(remote_size) if *remote_size == *size => skip += 1,
                _ => upload_tasks.push((path.clone(), key)),
            }
        }
        for key in r2_files.keys() {
            if should_delete(key, base, &local_keys, exclude_prefixes) {
                delete_keys.push(key.clone());
            }
        }
    }

    println!(
        "📊 upload {} | skip {} | delete {}",
        upload_tasks.len(),
        skip,
        delete_keys.len()
    );

    let sem = Arc::new(Semaphore::new(cfg.concurrency));
    let mut uploaded = 0usize;
    let mut handles = Vec::new();
    for (path, key) in upload_tasks {
        let client = client.clone();
        let bucket = cfg.bucket.clone();
        let sem = sem.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            let body = tokio::fs::read(&path).await?;
            let ct = from_path(&path)
                .first_or_octet_stream()
                .essence_str()
                .to_string();
            client
                .put_object()
                .bucket(&bucket)
                .key(&key)
                .body(body.into())
                .content_type(ct)
                .send()
                .await?;
            Ok::<_, anyhow::Error>(())
        }));
    }
    for handle in handles {
        handle.await??;
        uploaded += 1;
        if uploaded.is_multiple_of(10) || uploaded == 1 {
            print!("\r   📤 {uploaded}");
        }
    }
    if uploaded > 0 {
        println!();
    }

    let del_sem = Arc::new(Semaphore::new(cfg.concurrency));
    let mut deleted = 0usize;
    for key in delete_keys {
        let _permit = del_sem.acquire().await.unwrap();
        client
            .delete_object()
            .bucket(&cfg.bucket)
            .key(&key)
            .send()
            .await?;
        deleted += 1;
    }

    let secs = start.elapsed().as_secs_f64();
    println!("✨ done in {secs:.1}s — uploaded {uploaded}, skipped {skip}, deleted {deleted}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delete_filter_respects_exclude_prefix() {
        let base = "releases/v1.0.0";
        let mut local = HashSet::new();
        local.insert("releases/v1.0.0/GameClient.exe".to_string());
        let excludes = vec!["macos/".to_string()];

        // present locally → keep
        assert!(!should_delete(
            "releases/v1.0.0/GameClient.exe",
            base,
            &local,
            &excludes
        ));
        // stale windows-side key → delete
        assert!(should_delete(
            "releases/v1.0.0/old-file.bin",
            base,
            &local,
            &excludes
        ));
        // other platform's nested keys → never delete
        assert!(!should_delete(
            "releases/v1.0.0/macos/GameClient-macos.zip",
            base,
            &local,
            &excludes
        ));
        assert!(!should_delete(
            "releases/v1.0.0/macos/manifest.json",
            base,
            &local,
            &excludes
        ));
        // no excludes → mirror semantics unchanged
        assert!(should_delete(
            "releases/v1.0.0/macos/GameClient-macos.zip",
            base,
            &local,
            &[]
        ));
    }
}
