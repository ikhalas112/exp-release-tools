use anyhow::{bail, Result};
use chrono::Utc;
use std::fs;
use std::path::Path;

pub fn inject_version(source: &Path, tag: &str, channel: &str, date: Option<&str>) -> Result<()> {
    if tag.is_empty() || channel.is_empty() {
        bail!("--source, --tag, and --channel are required");
    }
    let build_date = date
        .map(str::to_string)
        .unwrap_or_else(|| Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string());
    fs::create_dir_all(source)?;
    let out_path = source.join("version.txt");
    let content = format!("{tag}\n{channel}\n{build_date}\n");
    fs::write(&out_path, &content)?;
    println!(
        "version.txt → {}: {tag} / {channel} / {build_date}",
        out_path.display()
    );
    Ok(())
}
