use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use goblin::pe::PE;
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::model::{AssetSelection, AssetSelectionKind, GuiConfig};

const CONFIG_FILE_NAME: &str = "maxion_protector_gui_config.json";

#[derive(Debug, Clone)]
struct RuntimePaths {
    pnp_exe: PathBuf,
    stub_dll: PathBuf,
    loader_stub_dll: PathBuf,
}

pub fn run_protect(config: &GuiConfig) -> Result<String> {
    let input_exe = PathBuf::from(config.input_exe.trim());
    if !input_exe.is_file() {
        anyhow::bail!("Input executable not found: {}", input_exe.display());
    }
    if config.asset_entries.is_empty() {
        anyhow::bail!("Add at least one file or folder before protecting.");
    }

    let output_dir = PathBuf::from(config.output_dir.trim());
    if output_dir.as_os_str().is_empty() {
        anyhow::bail!("Output folder is empty.");
    }
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create output folder {}", output_dir.display()))?;

    let is_x64 = detect_pe_architecture(&input_exe)?;
    let runtime_paths = locate_runtime_paths(is_x64)?;
    let output_exe = expected_output_exe(&config.input_exe, &config.output_dir)
        .context("failed to build output executable path")?;
    let stage_dir = stage_selected_assets(&input_exe, &config.asset_entries)?;
    let resolved_protect_types = resolve_protect_types(config, stage_dir.path())?;
    let resolved_skip_types = normalize_csv(&config.skip_types);

    let mut command = Command::new(&runtime_paths.pnp_exe);
    command
        .arg("protect")
        .arg("--input")
        .arg(&input_exe)
        .arg("--assets")
        .arg(stage_dir.path())
        .arg("--output")
        .arg(&output_exe)
        .arg("--stub-dll")
        .arg(&runtime_paths.stub_dll)
        .arg("--loader-stub")
        .arg(&runtime_paths.loader_stub_dll)
        .arg("--compression-level")
        .arg(config.compression_level.to_string());

    if !resolved_protect_types.is_empty() {
        command
            .arg("--protect-only-types")
            .arg(&resolved_protect_types);
    }
    if !resolved_skip_types.is_empty() {
        command.arg("--skip-types").arg(&resolved_skip_types);
    }
    if config.enable_compress {
        command.arg("--compress");
    }
    if config.use_phase2 && is_x64 {
        command.arg("--phase2");
    }

    let command_output = command
        .output()
        .with_context(|| format!("failed to run {}", runtime_paths.pnp_exe.display()))?;

    let stdout = String::from_utf8_lossy(&command_output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&command_output.stderr).to_string();
    let mut full_log = String::new();
    full_log.push_str("Protect command completed.\n\n");
    full_log.push_str(&format!("Input: {}\n", input_exe.display()));
    full_log.push_str(&format!("Output: {}\n", output_exe.display()));
    full_log.push_str(&format!("Stage: {}\n", stage_dir.path().display()));
    full_log.push_str(&format!("Architecture: {}\n", if is_x64 { "x64" } else { "x86" }));
    full_log.push_str(&format!(
        "Phase 2 Embedding: {}\n",
        if config.use_phase2 && is_x64 {
            "Enabled"
        } else if config.use_phase2 && !is_x64 {
            "Disabled automatically for x86"
        } else {
            "Disabled"
        }
    ));
    full_log.push_str(&format!(
        "Protect File Types: {}\n",
        if resolved_protect_types.is_empty() {
            "Smart defaults".to_string()
        } else {
            resolved_protect_types.clone()
        }
    ));
    full_log.push_str(&format!(
        "Skip File Types: {}\n\n",
        if resolved_skip_types.is_empty() {
            "(none)".to_string()
        } else {
            resolved_skip_types.clone()
        }
    ));

    if !stdout.trim().is_empty() {
        full_log.push_str("[stdout]\n");
        full_log.push_str(&stdout);
        full_log.push('\n');
    }
    if !stderr.trim().is_empty() {
        full_log.push_str("[stderr]\n");
        full_log.push_str(&stderr);
        full_log.push('\n');
    }

    if !command_output.status.success() {
        anyhow::bail!("{full_log}");
    }

    let copied = copy_runtime_support_files(&runtime_paths, &output_dir)?;
    if !copied.is_empty() {
        full_log.push_str("\n[runtime files copied]\n");
        for path in copied {
            full_log.push_str(&format!("{}\n", path.display()));
        }
    }

    Ok(full_log)
}

pub fn expected_output_exe(input_exe: &str, output_dir: &str) -> Option<PathBuf> {
    let input = Path::new(input_exe.trim());
    let output = Path::new(output_dir.trim());
    let stem = input.file_stem()?.to_string_lossy();
    Some(output.join(format!("{stem}_protected.exe")))
}

pub fn default_output_path(input_exe: &str) -> Option<PathBuf> {
    input_parent_dir(input_exe)
}

pub fn input_parent_dir(input_exe: &str) -> Option<PathBuf> {
    let path = Path::new(input_exe.trim());
    path.parent().map(Path::to_path_buf)
}

pub fn detect_pe_architecture(path: &Path) -> Result<bool> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read executable {}", path.display()))?;
    let pe = PE::parse(&bytes)
        .with_context(|| format!("failed to parse PE executable {}", path.display()))?;
    Ok(pe.is_64)
}

pub fn normalize_csv(value: &str) -> String {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

pub fn dedupe_entries(entries: Vec<AssetSelection>) -> Vec<AssetSelection> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();

    for entry in entries {
        if seen.insert((entry.kind, entry.path.to_ascii_lowercase())) {
            deduped.push(entry);
        }
    }

    deduped
}

pub fn migrate_legacy_defaults(config: &mut GuiConfig) {
    const LEGACY_PROTECT_TYPES: &str =
        "dat,lua,gui,ani,sod,tga,vsh,psh,fx,txt,xml,scheme,looknfeel,imageset,font,hyb,zfm,nsb,nsf,nif";
    const LEGACY_SKIP_TYPES: &str =
        "exe,dll,pdb,bat,lnk,tmp,log,hsh,ini,mod,img,jpg,jpeg,png,webp,mp3,ogg,wav";

    if normalize_csv(&config.protect_types) == LEGACY_PROTECT_TYPES {
        config.protect_types.clear();
    }
    if normalize_csv(&config.skip_types) == LEGACY_SKIP_TYPES {
        config.skip_types.clear();
    }
    if !config.input_exe.trim().is_empty() {
        let input_path = Path::new(config.input_exe.trim());
        if input_path.is_file() {
            if let Ok(is_x64) = detect_pe_architecture(input_path) {
                if !is_x64 {
                    config.use_phase2 = false;
                }
            }
        }
    }
}

pub fn resolve_protect_types(config: &GuiConfig, stage_root: &Path) -> Result<String> {
    let explicit = normalize_csv(&config.protect_types);
    if !explicit.is_empty() {
        return Ok(explicit);
    }

    let mut extensions = BTreeSet::new();
    for entry in WalkDir::new(stage_root) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        if let Some(ext) = entry.path().extension().and_then(|ext| ext.to_str()) {
            let ext = ext.trim().to_ascii_lowercase();
            if !ext.is_empty() {
                extensions.insert(ext);
            }
        }
    }

    Ok(extensions.into_iter().collect::<Vec<_>>().join(","))
}

pub fn load_config() -> Result<GuiConfig> {
    let path = config_path()?;
    if !path.is_file() {
        return Ok(GuiConfig::default());
    }

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    let config = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse config {}", path.display()))?;
    Ok(config)
}

pub fn save_config(config: &GuiConfig) -> Result<()> {
    let path = config_path()?;
    let contents =
        serde_json::to_string_pretty(config).context("failed to serialize GUI config")?;
    fs::write(&path, contents)
        .with_context(|| format!("failed to write config {}", path.display()))?;
    Ok(())
}

fn config_path() -> Result<PathBuf> {
    let exe_dir = std::env::current_exe()
        .context("failed to resolve current executable path")?
        .parent()
        .context("current executable has no parent directory")?
        .to_path_buf();
    Ok(exe_dir.join(CONFIG_FILE_NAME))
}

fn copy_runtime_support_files(runtime_paths: &RuntimePaths, output_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut copied = Vec::new();
    for source in [&runtime_paths.stub_dll, &runtime_paths.loader_stub_dll] {
        if !source.is_file() {
            continue;
        }
        let destination = output_dir.join(
            source
                .file_name()
                .context("runtime file is missing its file name")?,
        );
        fs::copy(source, &destination).with_context(|| {
            format!(
                "failed to copy runtime file {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        copied.push(destination);
    }
    Ok(copied)
}

fn locate_runtime_paths(input_is_64: bool) -> Result<RuntimePaths> {
    let exe_dir = std::env::current_exe()
        .context("failed to resolve GUI executable path")?
        .parent()
        .context("GUI executable has no parent directory")?
        .to_path_buf();

    let workspace_release = exe_dir.clone();
    let x86_release = exe_dir
        .parent()
        .map(|parent| parent.join("i686-pc-windows-msvc").join("release"));

    let pnp_exe = workspace_release.join("pnp.exe");
    let stub_dll = if input_is_64 {
        workspace_release.join("maxion_stub.dll")
    } else {
        x86_release
            .clone()
            .context("missing x86 release directory")?
            .join("maxion_stub.dll")
    };
    let loader_stub_dll = if input_is_64 {
        workspace_release.join("maxion_loader_stub.dll")
    } else {
        x86_release
            .context("missing x86 release directory")?
            .join("maxion_loader_stub.dll")
    };

    for path in [&pnp_exe, &stub_dll, &loader_stub_dll] {
        if !path.is_file() {
            anyhow::bail!("Required runtime file not found: {}", path.display());
        }
    }

    Ok(RuntimePaths {
        pnp_exe,
        stub_dll,
        loader_stub_dll,
    })
}

fn stage_selected_assets(input_exe: &Path, entries: &[AssetSelection]) -> Result<TempDir> {
    let temp_dir = tempfile::Builder::new()
        .prefix("maxion-gui-stage-")
        .tempdir()
        .context("failed to create staging directory")?;
    let input_root = input_exe
        .parent()
        .context("input executable has no parent directory")?;
    let mut staged_paths: HashMap<PathBuf, PathBuf> = HashMap::new();

    for entry in entries {
        let source = Path::new(&entry.path);
        if !source.exists() {
            anyhow::bail!("Selected asset does not exist: {}", source.display());
        }
        match entry.kind {
            AssetSelectionKind::File => {
                stage_file(source, input_root, temp_dir.path(), &mut staged_paths)?
            }
            AssetSelectionKind::Folder => {
                stage_folder(source, input_root, temp_dir.path(), &mut staged_paths)?
            }
        }
    }

    Ok(temp_dir)
}

fn stage_file(
    source: &Path,
    input_root: &Path,
    stage_root: &Path,
    staged_paths: &mut HashMap<PathBuf, PathBuf>,
) -> Result<()> {
    let relative = relative_stage_path(source, input_root)?;
    register_stage_path(&relative, source, staged_paths)?;
    let destination = stage_root.join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create stage folder {}", parent.display()))?;
    }
    fs::copy(source, &destination).with_context(|| {
        format!(
            "failed to stage file {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn stage_folder(
    source: &Path,
    input_root: &Path,
    stage_root: &Path,
    staged_paths: &mut HashMap<PathBuf, PathBuf>,
) -> Result<()> {
    let base_relative = relative_stage_path(source, input_root)?;
    for entry in WalkDir::new(source) {
        let entry = entry?;
        let path = entry.path();
        let relative_inside = path
            .strip_prefix(source)
            .with_context(|| format!("failed to strip folder prefix for {}", path.display()))?;
        let destination = stage_root.join(&base_relative).join(relative_inside);
        let staged_relative = base_relative.join(relative_inside);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination).with_context(|| {
                format!("failed to create stage directory {}", destination.display())
            })?;
            continue;
        }

        register_stage_path(&staged_relative, path, staged_paths)?;

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create stage folder {}", parent.display()))?;
        }
        fs::copy(path, &destination).with_context(|| {
            format!(
                "failed to stage file {} to {}",
                path.display(),
                destination.display()
            )
        })?;
    }
    Ok(())
}

fn relative_stage_path(source: &Path, input_root: &Path) -> Result<PathBuf> {
    if let Ok(relative) = source.strip_prefix(input_root) {
        return Ok(relative.to_path_buf());
    }

    if let Some(file_name) = source.file_name() {
        return Ok(PathBuf::from(file_name));
    }

    anyhow::bail!("failed to derive stage path for {}", source.display())
}

fn register_stage_path(
    staged_relative: &Path,
    source: &Path,
    staged_paths: &mut HashMap<PathBuf, PathBuf>,
) -> Result<()> {
    match staged_paths.get(staged_relative) {
        Some(existing) if existing == source => Ok(()),
        Some(existing) => anyhow::bail!(
            "Multiple selected files or folders map to the same staged path: {}.\nExisting source: {}\nConflicting source: {}\n\nMove both items under the game folder or select a parent folder that already contains both.",
            staged_relative.display(),
            existing.display(),
            source.display()
        ),
        None => {
            staged_paths.insert(staged_relative.to_path_buf(), source.to_path_buf());
            Ok(())
        }
    }
}
