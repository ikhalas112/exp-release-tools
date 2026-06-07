//! Maxion Packer CLI
//!
//! Command-line tool for encrypting and packing game assets into a virtual archive.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use goblin::pe::PE;

use indicatif::{ProgressBar, ProgressStyle};
use maxion_core::{
    archive::ArchiveBuilder,
    compression::compress,
    types::{AssetFile, ChunkSize, Config},
};
use maxion_injector::{PeInjector, StubLoader};
use rayon::prelude::*;
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

mod protection;

/// Maxion Packer - Asset Protection Tool
#[derive(Parser, Debug)]
#[command(name = "maxion-packer")]
#[command(author = "Maxion Team")]
#[command(version = "0.1.0")]
#[command(about = "Pack and encrypt game assets into virtual archives", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

/// Configuration for pack command
struct PackConfig {
    assets_dir: PathBuf,
    output: PathBuf,
    chunk_size: u32,
    compress: bool,
    compression_level: u32,
    build_secret: Option<String>,
    simd_mode: String,
    protection_config: protection::FileProtectionConfig,
    verify: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Pack assets into an encrypted archive
    Pack {
        /// Path to the assets directory
        #[arg(short, long)]
        assets: PathBuf,

        /// Path to the output archive file
        #[arg(short, long)]
        output: PathBuf,

        /// Chunk size for encryption (in bytes, must be power of 2)
        #[arg(short, long, default_value = "65536")]
        chunk_size: u32,

        /// Enable compression
        #[arg(short = 'z', long, default_value = "true")]
        compress: bool,

        /// Compression level (0-11)
        #[arg(short = 'l', long, default_value = "6")]
        compression_level: u32,

        /// Build secret for key derivation (hex string)
        #[arg(long)]
        build_secret: Option<String>,

        /// SIMD optimization mode: auto (detect), on (force), off (disable)
        #[arg(long, default_value = "auto")]
        simd: String,

        /// Use smart defaults for file protection based on file type
        #[arg(long, default_value_t = true, num_args = 0..=1, value_name = "BOOL")]
        smart_defaults: bool,

        /// Compress all files (overrides smart defaults)
        #[arg(long)]
        compress_all: bool,

        /// Disable compression for all files (overrides smart defaults)
        #[arg(long)]
        compress_none: bool,

        /// Compress only specific file extensions (comma-separated, e.g., "json,xml,lua")
        #[arg(long)]
        compress_types: Option<String>,

        /// Exclude specific file extensions from compression (comma-separated, e.g., "png,jpg,mp4")
        #[arg(long)]
        no_compress_types: Option<String>,

        /// Only protect specific file extensions (comma-separated, others are skipped)
        #[arg(long)]
        protect_only_types: Option<String>,

        /// Skip specific file extensions entirely (comma-separated)
        #[arg(long)]
        skip_types: Option<String>,

        /// Enable protection of all files - ignores skip_types and protect_only_types when true
        #[arg(long)]
        enable_protected_all: bool,

        /// Verify what will be protected/compressed before processing
        #[arg(long)]
        verify: bool,
    },

    /// Extract files from an archive (for testing/debugging)
    Extract {
        /// Path to the archive file
        #[arg(short, long)]
        archive: PathBuf,

        /// Output directory for extracted files
        #[arg(short, long)]
        output: PathBuf,
    },

    /// List files in an archive
    List {
        /// Path to the archive file
        #[arg(short, long)]
        archive: PathBuf,
    },

    /// Show information about an archive
    Info {
        /// Path to the archive file
        #[arg(short, long)]
        archive: PathBuf,
    },

    /// Protect a game executable by embedding encrypted assets
    Protect {
        /// Path to the input game executable
        #[arg(short, long)]
        input: PathBuf,

        /// Path to the assets directory
        #[arg(short, long)]
        assets: PathBuf,

        /// Path to the output protected executable
        #[arg(short, long)]
        output: PathBuf,

        /// Chunk size for encryption (in bytes, must be power of 2)
        #[arg(short, long, default_value = "65536")]
        chunk_size: u32,

        /// Enable compression
        #[arg(short = 'z', long, default_value = "true")]
        compress: bool,

        /// Compression level (0-11)
        #[arg(short = 'l', long, default_value = "6")]
        compression_level: u32,

        /// Build secret for key derivation (hex string)
        #[arg(long)]
        build_secret: Option<String>,

        /// Path to stub DLL (Phase 1 - Quick Fix)
        #[arg(long)]
        stub_dll: Option<PathBuf>,

        /// Path to loader stub DLL matching target architecture (optional)
        #[arg(long)]
        loader_stub: Option<PathBuf>,

        /// Enable Phase 2 full DLL embedding (Production)
        #[arg(long)]
        phase2: bool,

        /// SIMD optimization mode: auto (detect), on (force), off (disable)
        #[arg(long, default_value = "auto")]
        simd: String,

        /// Use smart defaults for file protection based on file type
        #[arg(long, default_value_t = true, num_args = 0..=1, value_name = "BOOL")]
        smart_defaults: bool,

        /// Compress all files (overrides smart defaults)
        #[arg(long)]
        compress_all: bool,

        /// Disable compression for all files (overrides smart defaults)
        #[arg(long)]
        compress_none: bool,

        /// Compress only specific file extensions (comma-separated, e.g., "json,xml,lua")
        #[arg(long)]
        compress_types: Option<String>,

        /// Exclude specific file extensions from compression (comma-separated, e.g., "png,jpg,mp4")
        #[arg(long)]
        no_compress_types: Option<String>,

        /// Only protect specific file extensions (comma-separated, others are skipped)
        #[arg(long)]
        protect_only_types: Option<String>,

        /// Skip specific file extensions entirely (comma-separated)
        #[arg(long)]
        skip_types: Option<String>,

        /// Enable protection of all files - ignores skip_types and protect_only_types when true
        #[arg(long)]
        enable_protected_all: bool,

        /// Verify what will be protected/compressed before processing
        #[arg(long)]
        verify: bool,

        /// Enable trap checking for honeypot anti-cheat protection (default: true)
        #[arg(long, default_value = "true")]
        enable_trap: bool,
    },
}

fn main() -> Result<()> {
    // Initialize logger for debug output
    env_logger::init();

    let args = Args::parse();

    match args.command {
        Commands::Pack {
            assets,
            output,
            chunk_size,
            compress,
            compression_level,
            build_secret,
            simd,
            smart_defaults,
            compress_all,
            compress_none,
            compress_types,
            no_compress_types,
            protect_only_types,
            skip_types,
            enable_protected_all,
            verify,
        } => {
            let protection_config =
                protection::create_protection_config(protection::CLIProtectionConfig {
                    smart_defaults,
                    compress_all,
                    compress_none,
                    compress_types,
                    no_compress_types,
                    protect_only_types,
                    skip_types,
                    enable_protected_all,
                });
            cmd_pack(PackConfig {
                assets_dir: assets,
                output,
                chunk_size,
                compress,
                compression_level,
                build_secret,
                simd_mode: simd,
                protection_config,
                verify,
            })
        }
        Commands::Extract { archive, output } => cmd_extract(archive, output),
        Commands::List { archive } => cmd_list(archive),
        Commands::Info { archive } => cmd_info(archive),
        Commands::Protect {
            input,
            assets,
            output,
            chunk_size,
            compress,
            compression_level,
            build_secret,
            stub_dll,
            loader_stub,
            phase2,
            simd,
            smart_defaults,
            compress_all,
            compress_none,
            compress_types,
            no_compress_types,
            protect_only_types,
            skip_types,
            enable_protected_all,
            verify,
            enable_trap,
        } => {
            let protection_config =
                protection::create_protection_config(protection::CLIProtectionConfig {
                    smart_defaults,
                    compress_all,
                    compress_none,
                    compress_types,
                    no_compress_types,
                    protect_only_types,
                    skip_types,
                    enable_protected_all,
                });
            cmd_protect(
                input,
                assets,
                output,
                chunk_size,
                compress,
                compression_level,
                build_secret,
                stub_dll,
                loader_stub,
                phase2,
                simd,
                protection_config,
                verify,
                enable_trap,
            )
        }
    }
}

/// Protect a game executable by embedding encrypted assets
#[allow(clippy::too_many_arguments)]
fn cmd_protect(
    input_path: PathBuf,
    assets_dir: PathBuf,
    output_path: PathBuf,
    chunk_size: u32,
    compress: bool,
    compression_level: u32,
    build_secret: Option<String>,
    stub_dll_path: Option<PathBuf>,
    loader_stub_path: Option<PathBuf>,
    use_phase2: bool,
    simd_mode: String,
    protection_config: protection::FileProtectionConfig,
    verify: bool,
    enable_trap: bool,
) -> Result<()> {
    println!("Maxion Packer v0.1.0 - Protect Mode");
    println!();

    // Validate SIMD mode
    let simd_config = maxion_core::simd::validate_simd_mode(&simd_mode)
        .map_err(|e| anyhow::anyhow!("Invalid SIMD mode: {}", e))?;

    println!("SIMD Configuration: {}", simd_config);

    // Configure trap checking
    maxion_core::set_trap_enabled(enable_trap);
    println!(
        "Trap Checking: {}",
        if enable_trap { "ENABLED" } else { "DISABLED" }
    );
    println!();

    // Validate inputs
    if !input_path.exists() {
        anyhow::bail!("Input executable does not exist: {}", input_path.display());
    }

    if !assets_dir.exists() {
        anyhow::bail!("Assets directory does not exist: {}", assets_dir.display());
    }

    println!("Input executable: {}", input_path.display());
    println!("Assets directory: {}", assets_dir.display());
    println!("Output: {}", output_path.display());
    println!();

    let input_is_64 = detect_pe_architecture(&input_path)
        .with_context(|| format!("Failed to determine architecture: {}", input_path.display()))?;
    println!(
        "Target architecture: {}",
        if input_is_64 { "x64" } else { "x86" }
    );
    println!();

    // Display protection mode information
    if protection_config.compress_all {
        println!("Protection Mode: Compress All Files");
    } else if protection_config.compress_none {
        println!("Protection Mode: Protect Only (no compression)");
    } else if protection_config.use_smart_defaults {
        println!("Protection Mode: Smart Defaults (file type-based)");
    } else if !protection_config.force_compress_types.is_empty() {
        println!(
            "Protection Mode: Custom compress types: {:?}",
            protection_config.force_compress_types
        );
    } else {
        println!("Protection Mode: Protect Only");
    }
    if !protection_config.no_compress_types.is_empty() {
        println!("  No compress: {:?}", protection_config.no_compress_types);
    }
    if !protection_config.skip_types.is_empty() {
        println!("  Skip types: {:?}", protection_config.skip_types);
    }
    if let Some(ref only) = protection_config.protect_only_types {
        println!("  Protect only: {:?}", only);
    }
    println!();

    // Step 1: Create encrypted archive
    println!("Step 1: Creating encrypted archive...");

    // Generate one build secret up front so the archive and injected key blob
    // use identical key/nonce material.
    let mut config = Config::new();
    config.chunk_size = ChunkSize::new(chunk_size);
    let compress_enabled = compress && !protection_config.compress_none;
    config.compress = compress_enabled;
    config.compression_level = compression_level;
    let build_secret_hex = if let Some(secret_hex) = build_secret.clone() {
        let secret_bytes = hex::decode(&secret_hex).context("Invalid hex string for build_secret")?;
        if secret_bytes.len() != 32 {
            anyhow::bail!("Build secret must be 64 hex characters (32 bytes)");
        }
        config.build_secret.copy_from_slice(&secret_bytes);
        config.derive_key()?;
        secret_hex
    } else {
        config.generate_keys();
        config.derive_key()?;
        hex::encode(config.build_secret)
    };

    // Create temporary archive file
    let temp_archive = tempfile::NamedTempFile::new()?;
    let temp_archive_path = temp_archive.path().to_path_buf();

    // Call cmd_pack to create the archive
    cmd_pack(PackConfig {
        assets_dir: assets_dir.clone(),
        output: temp_archive_path.clone(),
        chunk_size,
        compress,
        compression_level,
        build_secret: Some(build_secret_hex),
        simd_mode: simd_mode.clone(),
        protection_config: protection_config.clone(),
        verify,
    })?;

    println!();

    // Step 2: Load archive data
    println!("Step 2: Loading encrypted archive...");
    let archive_data = std::fs::read(&temp_archive_path)?;
    println!("Archive size: {} bytes", archive_data.len());
    println!();

    // Step 3: Generate encryption key
    println!("Step 3: Generating encryption keys...");
    let encryption_key = config.encryption_key;
    let nonce = config.nonce;
    println!("Encryption key generated");
    println!();

    // Step 4: Inject archive into PE file
    println!("Step 4: Injecting archive into executable...");

    // Create injector
    let mut injector = PeInjector::new(
        input_path,
        output_path.clone(),
        archive_data,
        encryption_key,
        nonce,
        chunk_size,
    );

    // Phase 2: Full DLL embedding (Production)
    if use_phase2 {
        println!("Using Phase 2 full DLL embedding (Production)");
        println!("  Full DLL will be embedded into PE file");
        println!("  Single-file protection: No external DLL needed");

        // Validate that DLL path is provided
        if let Some(dll_path) = stub_dll_path {
            validate_stub_architecture(&dll_path, input_is_64)?;
            // Load DLL structure for full embedding
            injector = injector.with_dll(dll_path)?;

            // Perform full DLL embedding
            injector.inject_full_dll()?;
        } else {
            anyhow::bail!("Phase 2 requires --stub-dll flag with path to maxion_stub.dll");
        }
    } else if let Some(dll_path) = stub_dll_path {
        // Phase 1: Use DLL loader approach
        println!("Using Phase 1 DLL loader approach");
        println!("  Loader stub: Tiny stub injected into PE file");
        println!(
            "  Stub DLL: {} (loaded by loader at runtime)",
            dll_path.display()
        );

        validate_stub_architecture(&dll_path, input_is_64)?;

        // Load the tiny loader stub that will be injected
        injector = load_loader_stub(injector, input_is_64, loader_stub_path)?;

        // Configure injector to copy DLL to output directory
        injector = injector.with_dll_loader(dll_path)?;

        // Inject loader stub and copy DLL
        injector.inject_with_dll()?;
    } else {
        // Load stub binary (Phase 2 approach)
        #[cfg(stub_compiled)]
        {
            // Try to use embedded stub (if available via objcopy)
            match injector.with_embedded_stub() {
                Ok(inj) => injector = inj,
                Err(_) => {
                    println!("Warning: Embedded stub not available, trying manual stub loading");
                    injector = load_stub_manually(injector)?;
                }
            }
        }

        #[cfg(not(stub_compiled))]
        {
            // Load stub manually (no embedded stub available)
            injector = load_stub_manually(injector)?;
        }

        injector.inject()?;
    }

    println!();
    println!("✓ Protection complete!");
    println!("Protected executable: {}", output_path.display());

    Ok(())
}

/// Load stub binary manually from pre-compiled library
fn load_stub_manually(injector: PeInjector) -> Result<PeInjector> {
    // Try to locate the compiled loader stub DLL (tiny stub, not full stub)
    let stub_paths = vec![
        // Try workspace target directory
        PathBuf::from("target/release/maxion_loader_stub.dll"),
        PathBuf::from("target/debug/maxion_loader_stub.dll"),
        // Try relative paths from current directory
        PathBuf::from("../target/release/maxion_loader_stub.dll"),
        PathBuf::from("../target/debug/maxion_loader_stub.dll"),
    ];

    let stub_path = stub_paths
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| anyhow::anyhow!("Could not find maxion_loader_stub.dll. Please build it first: cargo build --release -p maxion-loader-stub"))?;

    println!("Loading loader stub from: {}", stub_path.display());

    // Read stub DLL
    let stub_dll_data = std::fs::read(&stub_path)?;

    if stub_dll_data.is_empty() {
        anyhow::bail!("Loader stub DLL is empty: {}", stub_path.display());
    }

    println!(
        "Loader stub DLL loaded: {} bytes from {}",
        stub_dll_data.len(),
        stub_path.display()
    );
    println!(
        "  First 16 bytes: {:02X?}",
        &stub_dll_data[..16.min(stub_dll_data.len())]
    );

    // Extract .text section from DLL (this is the actual stub code)
    let pe = goblin::pe::PE::parse(&stub_dll_data)
        .map_err(|e| anyhow::anyhow!("Failed to parse loader stub DLL: {:?}", e))?;

    let text_section = pe
        .sections
        .iter()
        .find(|s| s.name().unwrap_or("") == ".text")
        .ok_or_else(|| anyhow::anyhow!("Loader stub DLL has no .text section"))?;

    let text_offset = text_section.pointer_to_raw_data as usize;
    let text_size = text_section.size_of_raw_data as usize;
    let stub_data = stub_dll_data[text_offset..text_offset + text_size].to_vec();

    println!(
        "Extracted .text section: {} bytes ({} KB)",
        stub_data.len(),
        stub_data.len() / 1024
    );

    Ok(injector.with_stub_loader(StubLoader::new(stub_data)))
}

/// Load loader stub for Phase 1 DLL loader approach
///
/// This loads the tiny loader stub that will be injected into the PE file.
/// The loader stub then loads maxion_stub.dll at runtime.
fn load_loader_stub(
    injector: PeInjector,
    input_is_64: bool,
    explicit_path: Option<PathBuf>,
) -> Result<PeInjector> {
    // Try to locate the loader stub DLL
    let mut stub_dll_paths = Vec::new();
    if let Some(path) = explicit_path {
        stub_dll_paths.push(path);
    }

    let target_folder = if input_is_64 {
        "x86_64-pc-windows-msvc"
    } else {
        "i686-pc-windows-msvc"
    };

    stub_dll_paths.extend([
        PathBuf::from(format!(
            "target/{target_folder}/release/maxion_loader_stub.dll"
        )),
        PathBuf::from(format!(
            "target/{target_folder}/debug/maxion_loader_stub.dll"
        )),
        PathBuf::from("target/release/maxion_loader_stub.dll"),
        PathBuf::from("target/debug/maxion_loader_stub.dll"),
        PathBuf::from(format!(
            "../target/{target_folder}/release/maxion_loader_stub.dll"
        )),
        PathBuf::from(format!(
            "../target/{target_folder}/debug/maxion_loader_stub.dll"
        )),
        PathBuf::from("../target/release/maxion_loader_stub.dll"),
        PathBuf::from("../target/debug/maxion_loader_stub.dll"),
        PathBuf::from(format!(
            "../../target/{target_folder}/release/maxion_loader_stub.dll"
        )),
        PathBuf::from(format!(
            "../../target/{target_folder}/debug/maxion_loader_stub.dll"
        )),
        PathBuf::from("../../target/release/maxion_loader_stub.dll"),
        PathBuf::from("../../target/debug/maxion_loader_stub.dll"),
    ]);

    let stub_dll_path = stub_dll_paths
        .into_iter()
        .find(|p| p.exists())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Could not find maxion_loader_stub.dll for {}. Please build it first.",
                if input_is_64 { "x64" } else { "x86" }
            )
        })?;

    println!("Loading loader stub from: {}", stub_dll_path.display());

    // Parse DLL to extract .text section
    let stub_dll_data = std::fs::read(&stub_dll_path)?;

    if stub_dll_data.is_empty() {
        anyhow::bail!("Loader stub DLL is empty: {}", stub_dll_path.display());
    }

    println!("Loader stub DLL data loaded: {} bytes", stub_dll_data.len());
    println!(
        "  First 16 bytes: {:02X?}",
        &stub_dll_data[..16.min(stub_dll_data.len())]
    );

    let pe = goblin::pe::PE::parse(&stub_dll_data)
        .map_err(|e| anyhow::anyhow!("Failed to parse loader stub DLL: {:?}", e))?;
    let text_section = pe
        .sections
        .iter()
        .find(|s| s.name().unwrap_or("") == ".text")
        .ok_or_else(|| anyhow::anyhow!("Loader stub DLL has no .text section"))?;
    println!(
        "Found .text section in loader stub DLL: {} bytes",
        text_section.size_of_raw_data
    );

    println!(
        "Loader stub DLL loaded for export parsing: {} bytes",
        stub_dll_data.len()
    );

    Ok(injector.with_stub_loader(StubLoader::new(stub_dll_data)))
}

fn detect_pe_architecture(path: &Path) -> Result<bool> {
    let data = fs::read(path)?;
    let pe = PE::parse(&data)
        .with_context(|| format!("Failed to parse PE structure: {}", path.display()))?;
    Ok(pe.is_64)
}

fn validate_stub_architecture(stub_dll_path: &Path, input_is_64: bool) -> Result<()> {
    let stub_is_64 = detect_pe_architecture(stub_dll_path)?;
    if stub_is_64 != input_is_64 {
        anyhow::bail!(
            "Stub DLL architecture mismatch: executable is {}, but stub DLL is {} ({})",
            if input_is_64 { "x64" } else { "x86" },
            if stub_is_64 { "x64" } else { "x86" },
            stub_dll_path.display()
        );
    }

    Ok(())
}

/// Pack assets into an encrypted archive
fn cmd_pack(config: PackConfig) -> Result<()> {
    println!("Maxion Packer v0.1.0");
    println!();

    // Validate inputs
    if !config.assets_dir.exists() {
        anyhow::bail!(
            "Assets directory does not exist: {}",
            config.assets_dir.display()
        );
    }

    // Validate SIMD mode
    let simd_config = maxion_core::simd::validate_simd_mode(&config.simd_mode)
        .map_err(|e| anyhow::anyhow!("Invalid SIMD mode: {}", e))?;

    let chunk_size = ChunkSize::new(config.chunk_size);
    println!("Chunk size: {} bytes", chunk_size.as_u32());
    println!("SIMD Configuration: {}", simd_config);
    println!();

    // Display protection mode information
    if config.protection_config.use_smart_defaults {
        println!("Protection Mode: Smart Defaults (file type-based)");
    } else if config.protection_config.compress_all {
        println!("Protection Mode: Compress All Files");
    } else if config.protection_config.compress_none {
        println!("Protection Mode: Protect Only (no compression)");
    } else if !config.protection_config.force_compress_types.is_empty() {
        println!(
            "Protection Mode: Custom compress types: {:?}",
            config.protection_config.force_compress_types
        );
    } else {
        println!("Protection Mode: Protect Only (no compression)");
    }
    if !config.protection_config.no_compress_types.is_empty() {
        println!(
            "  No compress: {:?}",
            config.protection_config.no_compress_types
        );
    }
    if !config.protection_config.skip_types.is_empty() {
        println!("  Skip types: {:?}", config.protection_config.skip_types);
    }
    if let Some(ref only) = config.protection_config.protect_only_types {
        println!("  Protect only: {:?}", only);
    }
    println!();

    // Create configuration
    // If compress_none is set, disable compression globally
    let compress_enabled = config.compress && !config.protection_config.compress_none;
    let mut config_obj = Config::new()
        .with_chunk_size(chunk_size.as_u32())
        .with_compression(compress_enabled, config.compression_level);

    // Add SIMD configuration
    match config.simd_mode.to_lowercase().as_str() {
        "auto" => config_obj = config_obj.with_simd_auto(),
        "on" | "enabled" | "true" => config_obj = config_obj.with_simd_enabled(),
        "off" | "disabled" | "false" => config_obj = config_obj.with_simd_disabled(),
        _ => config_obj = config_obj.with_simd_auto(),
    }

    // Handle build secret
    if let Some(secret_hex) = config.build_secret {
        let secret_bytes =
            hex::decode(&secret_hex).context("Invalid hex string for build_secret")?;
        if secret_bytes.len() != 32 {
            anyhow::bail!("Build secret must be 64 hex characters (32 bytes)");
        }
        config_obj.build_secret.copy_from_slice(&secret_bytes);
        config_obj.derive_key()?;
    } else {
        config_obj.generate_keys();
    }

    // Scan for assets
    println!("Scanning assets...");
    let start_time = Instant::now();
    let files = scan_assets(&config.assets_dir)?;
    let scan_duration = start_time.elapsed();

    println!(
        "Found {} files in {:.2}s",
        files.len(),
        scan_duration.as_secs_f64()
    );
    println!();

    // Calculate total size
    let total_size: u64 = files.iter().map(|f| f.original_size).sum();
    println!("Total uncompressed size: {} MB", total_size / 1024 / 1024);
    println!();

    // Display verification if requested
    if config.verify {
        protection::display_verification(&files, &config.protection_config, &config.assets_dir);
        println!("Press Enter to continue or Ctrl+C to cancel...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        println!();
    }

    // Process files (compress and calculate checksums)
    println!("Processing assets...");
    let progress = ProgressBar::new(files.len() as u64);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .expect("Invalid progress bar template")
            .progress_chars("#>-"),
    );

    let processed_files = process_files(
        files,
        &config.assets_dir,
        &config_obj,
        &progress,
        &config.protection_config,
    )?;
    progress.finish();

    println!();

    // Calculate compression statistics
    let compressed_size: u64 = processed_files.iter().map(|f| f.packed_size).sum();
    let compression_ratio = if total_size > 0 {
        compressed_size as f64 / total_size as f64
    } else {
        1.0
    };

    // Count files by protection type
    let mut compressed_count = 0u32;
    let mut protect_only_count = 0u32;
    for file in &processed_files {
        let full_path = config.assets_dir.join(&file.path);
        if config
            .protection_config
            .should_compress(&full_path, file.original_size)
        {
            compressed_count += 1;
        } else {
            protect_only_count += 1;
        }
    }

    println!();
    println!("📊 Final Statistics:");
    println!(
        "Total compressed size: {} MB",
        compressed_size / 1024 / 1024
    );
    println!(
        "Compression ratio: {:.2}%",
        (1.0 - compression_ratio) * 100.0
    );
    println!(
        "Space saved: {} MB",
        (total_size.saturating_sub(compressed_size)) / 1024 / 1024
    );
    println!("  ✅ Compressed: {} files", compressed_count);
    println!("  ⚠️  Protected only: {} files", protect_only_count);
    println!();

    // Build archive
    println!("Building archive...");
    let start_time = Instant::now();
    let mut builder = ArchiveBuilder::new(config_obj.clone()).with_base_dir(&config.assets_dir);
    builder.add_files(processed_files);

    let header = builder.build(&config.output)?;
    let build_duration = start_time.elapsed();

    println!("Archive created: {}", config.output.display());
    println!("Build time: {:.2}s", build_duration.as_secs_f64());
    println!("Archive version: {}", header.version);
    println!("Files: {}", header.file_count);
    println!("File table size: {} bytes", header.file_table_size);
    println!();

    println!();
    println!("✓ Packing complete!");
    println!();
    println!("Note: To embed the archive into an executable, you'll need to:");
    println!("1. Use a PE injection tool or");
    println!("2. Append the archive to the executable and");
    println!("3. Update the stub to read from the embedded section");

    Ok(())
}

/// Extract files from an archive (for testing)
/// Scan directory for asset files
fn scan_assets(dir: &Path) -> Result<Vec<AssetFile>> {
    let mut files = Vec::new();

    for entry in walkdir::WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();

        // Skip directories
        if path.is_dir() {
            continue;
        }

        // Get metadata
        let metadata: std::fs::Metadata = fs::metadata(path)
            .with_context(|| format!("Failed to read metadata for {}", path.display()))?;

        // Calculate relative path
        let relative_path: PathBuf = path
            .strip_prefix(dir)
            .with_context(|| format!("Failed to get relative path for {}", path.display()))?
            .to_path_buf();

        // Create asset file entry
        let mut asset = AssetFile::new(relative_path.to_path_buf(), metadata.len());

        // Store modification time
        if let Ok(modified) = metadata.modified() {
            asset.modified = modified
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }

        files.push(asset);
    }

    // Normalize paths (use forward slashes)
    for file in &mut files {
        file.normalize_path();
    }

    // Sort by path for consistency
    files.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(files)
}

/// Process files: compress, calculate checksums, and update metadata
fn process_files(
    files: Vec<AssetFile>,
    assets_dir: &Path,
    config: &Config,
    progress: &ProgressBar,
    protection_config: &protection::FileProtectionConfig,
) -> Result<Vec<AssetFile>> {
    let chunk_size = config.chunk_size;
    let compress_enabled = config.compress;
    let compression_level = config.compression_level;
    let simd_config = config.simd_config.as_ref();

    let processed: Vec<AssetFile> = files
        .into_par_iter()
        .filter_map(|mut file| -> Option<Result<AssetFile>> {
            let full_path = assets_dir.join(&file.path);

            // Check if file should be skipped
            if !protection_config.should_protect(&full_path, file.original_size) {
                progress.inc(1);
                return None; // Skip this file
            }

            Some((|| -> Result<AssetFile> {
                // Read file using full path
                let mut data = Vec::new();
                File::open(&full_path)?
                    .read_to_end(&mut data)
                    .context("Failed to read file")?;

                // Calculate checksum of original data
                file.calculate_checksum(&data);

                // Determine if this file should be compressed
                let should_compress =
                    protection_config.should_compress(&full_path, file.original_size);

                // Compress if enabled for this file
                let processed_data = if compress_enabled && should_compress {
                    compress(&data, compression_level, simd_config)?
                } else {
                    data
                };

                // Update metadata
                file.packed_size = processed_data.len() as u64;
                file.calculate_chunk_count(chunk_size);

                progress.inc(1);

                Ok(file)
            })())
        })
        .collect::<Result<Vec<_>>>()
        .context("Failed to process files")?;

    Ok(processed)
}

/// Extract files from an archive (for testing)
fn cmd_extract(archive_path: PathBuf, output_dir: PathBuf) -> Result<()> {
    println!("Extracting from: {}", archive_path.display());
    println!("Output directory: {}", output_dir.display());
    println!();

    // Create output directory
    fs::create_dir_all(&output_dir).context("Failed to create output directory")?;

    // Open archive
    let archive_data = fs::read(&archive_path)?;
    let header = maxion_core::archive::ArchiveHeader::from_bytes(&archive_data)?;

    // Note: Full extraction would require decryption keys
    // This is a stub implementation for the CLI interface
    println!("Archive version: {}", header.version);
    println!("File count: {}", header.file_count);
    println!("Chunk size: {}", header.chunk_size);
    println!(
        "Compression: {}",
        if header.compress {
            "enabled"
        } else {
            "disabled"
        }
    );

    println!();
    println!("Note: Full extraction requires decryption keys.");
    println!("This is a diagnostic view only.");

    Ok(())
}

/// List files in an archive
fn cmd_list(archive_path: PathBuf) -> Result<()> {
    println!("Listing archive: {}", archive_path.display());
    println!();

    // Open archive
    let archive_data = fs::read(&archive_path)?;
    let header = maxion_core::archive::ArchiveHeader::from_bytes(&archive_data)?;

    // Note: Full listing would require decrypting the file table
    println!("Archive version: {}", header.version);
    println!("File count: {}", header.file_count);
    println!("Chunk size: {}", header.chunk_size);
    println!(
        "Compression: {}",
        if header.compress {
            "enabled"
        } else {
            "disabled"
        }
    );

    println!();
    println!("Note: Full file listing requires decryption keys.");

    Ok(())
}

/// Show information about an archive
fn cmd_info(archive_path: PathBuf) -> Result<()> {
    println!("Archive info: {}", archive_path.display());
    println!();

    // Read archive
    let metadata = fs::metadata(&archive_path)?;
    println!("File size: {} bytes", metadata.len());
    println!();

    // Read header
    let archive_data = fs::read(&archive_path)?;
    let header = maxion_core::archive::ArchiveHeader::from_bytes(&archive_data)?;

    // Verify checksum
    let checksum_valid = header.verify_checksum();
    println!("Magic: {}", String::from_utf8_lossy(&header.magic));
    println!("Version: {}", header.version);
    println!("File count: {}", header.file_count);
    println!("Chunk size: {} bytes", header.chunk_size);
    println!(
        "Compression: {}",
        if header.compress {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("File table offset: {} bytes", header.file_table_offset);
    println!("File table size: {} bytes", header.file_table_size);
    println!(
        "Header checksum: {}",
        if checksum_valid {
            "✓ Valid"
        } else {
            "✗ Invalid"
        }
    );

    Ok(())
}
