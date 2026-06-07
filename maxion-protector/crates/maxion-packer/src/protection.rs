//! File protection strategy module for Maxion Packer
//!
//! This module implements smart file protection and compression strategies based on
//! file types and sizes, following the recommendations from benchmark analysis.

use std::collections::HashSet;
use std::path::Path;

use maxion_core::AssetFile;

/// File protection strategy based on benchmark recommendations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionStrategy {
    /// Always protect and compress (recommended for configs, scripts, textures, audio)
    ProtectAndCompress,
    /// Protect but don't compress (for already-compressed formats)
    ProtectOnly,
    /// Skip this file entirely
    Skip,
}

impl ProtectionStrategy {
    /// Get strategy based on file extension and size using smart defaults
    ///
    /// Follows recommendations from benchmark analysis:
    /// - Config files, scripts, JSON, XML: Always compress (88-99.9% space savings)
    /// - Textures, audio clips: Always compress (99.9% space savings)
    /// - Already-compressed (PNG, JPEG, MP4): Protect only (no benefit)
    /// - Large files (>100MB): Protect only (compression time dominates)
    pub fn from_file_smart(file_path: &Path, file_size: u64) -> Self {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        // Large files (>100MB) - don't compress by default
        // Recommendation: "Consider Disabling Compression For: Ultra-large files (>100MB)"
        if file_size > 100 * 1024 * 1024 {
            return Self::ProtectOnly;
        }

        // Already-compressed formats - protect only, no compression
        // Recommendation: "Consider Disabling Compression For: Already-compressed assets (PNG, JPEG, MP4)"
        let already_compressed = matches!(
            ext.as_str(),
            // Images
            "png" | "jpg" | "jpeg" | "webp" | "avif" | "heic" | "heif" |
            // Video
            "mp4" | "m4v" | "mov" | "avi" | "wmv" | "mkv" |
            // Audio (compressed)
            "ogg" | "oga" | "mp3" | "m4a" | "flac" |
            // Archives (already compressed/encrypted)
            "zip" | "rar" | "7z" | "gz" | "bz2" | "xz" | "lzma" | "zst"
        );

        if already_compressed {
            return Self::ProtectOnly;
        }

        // Always protect and compress for:
        // Config files: JSON, XML, TOML, YAML, INI, CFG
        // Scripts: JS, TS, PY, LUA, CS, C, CPP, H, HPP, RS, GO
        // Textures (uncompressed): BMP, TGA, DDS, PSD, TIF
        // Audio (uncompressed): WAV, AU, RAW
        // Models/Assets: OBJ, FBX, GLTF, GLB, COLLADA
        // Misc: TXT, MD, CSV, DATA, BIN
        // Recommendation: "ALWAYS Enable Protection For: Config files, scripts, JSON, XML"
        // Recommendation: "ALWAYS Enable Protection For: Textures, audio clips, animations"
        let compressible = matches!(
            ext.as_str(),
            // Config files
            "json" | "xml" | "toml" | "yaml" | "yml" | "ini" | "cfg" | "conf" | "properties" |
            // Scripts
            "js" | "ts" | "py" | "lua" | "cs" | "c" | "cpp" | "h" | "hpp" | "rs" | "go" |
            "java" | "kt" | "swift" | "rb" | "php" | "sh" | "bat" | "ps1" |
            // Textures (uncompressed)
            "bmp" | "tga" | "dds" | "psd" | "tif" | "tiff" | "pnm" | "pbm" | "pgm" | "ppm" |
            // Audio (uncompressed)
            "wav" | "aiff" | "au" | "raw" | "pcm" |
            // Models/Assets
            "obj" | "fbx" | "gltf" | "glb" | "dae" | "blend" | "ma" | "mb" | "max" |
            // Misc
            "txt" | "md" | "csv" | "log" | "data" | "bin" | "dat" | "db" | "sqlite" | "sql"
        );

        if compressible {
            Self::ProtectAndCompress
        } else {
            // Unknown format - protect but don't compress
            Self::ProtectOnly
        }
    }
}

/// Configuration for file protection and compression
#[derive(Debug, Clone)]
pub struct FileProtectionConfig {
    /// Whether to use smart defaults (file type-based)
    pub use_smart_defaults: bool,

    /// Compress all files (overrides smart defaults)
    pub compress_all: bool,

    /// Disable compression for all files (overrides smart defaults)
    pub compress_none: bool,

    /// Extensions to force compress (if use_smart_defaults, overrides smart defaults)
    pub force_compress_types: HashSet<String>,

    /// Extensions to exclude from compression (overrides smart defaults)
    pub no_compress_types: HashSet<String>,

    /// Extensions to skip entirely (no protection, no compression)
    pub skip_types: HashSet<String>,

    /// Only protect these extensions (if set, all others are skipped)
    pub protect_only_types: Option<HashSet<String>>,

    /// Enable protection of all files - ignores skip_types and protect_only_types when true
    pub enable_protected_all: bool,

    /// File size threshold for skipping compression (default: 100MB)
    #[allow(dead_code)]
    pub compress_size_threshold: u64,
}

impl Default for FileProtectionConfig {
    fn default() -> Self {
        Self {
            use_smart_defaults: true,
            compress_all: false,
            compress_none: false,
            force_compress_types: HashSet::new(),
            no_compress_types: HashSet::new(),
            skip_types: HashSet::new(),
            protect_only_types: None,
            enable_protected_all: false,
            compress_size_threshold: 100 * 1024 * 1024, // 100MB
        }
    }
}

impl FileProtectionConfig {
    /// Determine the protection strategy for a file
    pub fn get_strategy(&self, file_path: &Path, file_size: u64) -> ProtectionStrategy {
        // Check if we should protect all files - ignores skip_types and protect_only_types
        if self.enable_protected_all {
            // Use smart defaults for compression, but force protection
            if self.use_smart_defaults {
                return ProtectionStrategy::from_file_smart(file_path, file_size);
            } else {
                return ProtectionStrategy::ProtectOnly;
            }
        }

        // Large files (>100MB) - don't compress by default
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        // Check if file should be skipped entirely
        if self.skip_types.contains(&ext) {
            return ProtectionStrategy::Skip;
        }

        // If protect_only_types is set, only protect those
        if let Some(ref only_types) = self.protect_only_types {
            if !only_types.contains(&ext) {
                return ProtectionStrategy::Skip;
            }
        }

        // If compress_all is set, compress all files
        if self.compress_all {
            return ProtectionStrategy::ProtectAndCompress;
        }

        // If compress_none is set, never compress
        if self.compress_none {
            return ProtectionStrategy::ProtectOnly;
        }

        // Check if compression is forced for this type
        if self.force_compress_types.contains(&ext) {
            return ProtectionStrategy::ProtectAndCompress;
        }

        // Check if compression is excluded for this type
        if self.no_compress_types.contains(&ext) {
            return ProtectionStrategy::ProtectOnly;
        }

        // Use smart defaults if enabled
        if self.use_smart_defaults {
            return ProtectionStrategy::from_file_smart(file_path, file_size);
        }

        // Default: protect but don't compress if smart defaults disabled
        ProtectionStrategy::ProtectOnly
    }

    /// Check if a file should be compressed
    pub fn should_compress(&self, file_path: &Path, file_size: u64) -> bool {
        matches!(
            self.get_strategy(file_path, file_size),
            ProtectionStrategy::ProtectAndCompress
        )
    }

    /// Check if a file should be protected
    pub fn should_protect(&self, file_path: &Path, file_size: u64) -> bool {
        !matches!(
            self.get_strategy(file_path, file_size),
            ProtectionStrategy::Skip
        )
    }
}

/// Builder for creating FileProtectionConfig
pub struct ProtectionConfigBuilder {
    smart_defaults: bool,
    compress_all: bool,
    compress_none: bool,
    compress_types: Option<String>,
    no_compress_types: Option<String>,
    protect_only_types: Option<String>,
    skip_types: Option<String>,
    enable_protected_all: bool,
}

impl Default for ProtectionConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtectionConfigBuilder {
    pub fn new() -> Self {
        Self {
            smart_defaults: true,
            compress_all: false,
            compress_none: false,
            compress_types: None,
            no_compress_types: None,
            protect_only_types: None,
            skip_types: None,
            enable_protected_all: false,
        }
    }

    pub fn smart_defaults(mut self, value: bool) -> Self {
        self.smart_defaults = value;
        self
    }

    pub fn compress_all(mut self, value: bool) -> Self {
        self.compress_all = value;
        self
    }

    pub fn compress_none(mut self, value: bool) -> Self {
        self.compress_none = value;
        self
    }

    pub fn compress_types(mut self, value: Option<String>) -> Self {
        self.compress_types = value;
        self
    }

    pub fn no_compress_types(mut self, value: Option<String>) -> Self {
        self.no_compress_types = value;
        self
    }

    pub fn protect_only_types(mut self, value: Option<String>) -> Self {
        self.protect_only_types = value;
        self
    }

    pub fn skip_types(mut self, value: Option<String>) -> Self {
        self.skip_types = value;
        self
    }

    pub fn enable_protected_all(mut self, value: bool) -> Self {
        self.enable_protected_all = value;
        self
    }

    pub fn build(self) -> FileProtectionConfig {
        // Determine if smart defaults should be enabled
        // Disabled if compress_all, compress_none, or compress_types is set
        let has_compress_types = self.compress_types.is_some();
        let use_smart_defaults =
            self.smart_defaults && !self.compress_all && !self.compress_none && !has_compress_types;

        // Parse extension lists
        let force_compress_types = self
            .compress_types
            .map(|types| parse_extension_list(&types))
            .unwrap_or_default();

        FileProtectionConfig {
            use_smart_defaults,
            compress_all: self.compress_all,
            compress_none: self.compress_none,
            force_compress_types,
            no_compress_types: self
                .no_compress_types
                .map(|types| parse_extension_list(&types))
                .unwrap_or_default(),
            skip_types: self
                .skip_types
                .map(|types| parse_extension_list(&types))
                .unwrap_or_default(),
            protect_only_types: self
                .protect_only_types
                .map(|types| parse_extension_list(&types)),
            enable_protected_all: self.enable_protected_all,
            compress_size_threshold: 100 * 1024 * 1024, // 100MB
        }
    }
}

/// Parse a comma-separated list of extensions into a HashSet
pub fn parse_extension_list(input: &str) -> HashSet<String> {
    input
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Create file protection config from CLI arguments
/// CLI protection configuration parameters
pub struct CLIProtectionConfig {
    pub smart_defaults: bool,
    pub compress_all: bool,
    pub compress_none: bool,
    pub compress_types: Option<String>,
    pub no_compress_types: Option<String>,
    pub protect_only_types: Option<String>,
    pub skip_types: Option<String>,
    pub enable_protected_all: bool,
}

/// Create file protection config from CLI arguments
pub fn create_protection_config(config: CLIProtectionConfig) -> FileProtectionConfig {
    ProtectionConfigBuilder::new()
        .smart_defaults(config.smart_defaults)
        .compress_all(config.compress_all)
        .compress_none(config.compress_none)
        .compress_types(config.compress_types)
        .no_compress_types(config.no_compress_types)
        .protect_only_types(config.protect_only_types)
        .skip_types(config.skip_types)
        .enable_protected_all(config.enable_protected_all)
        .build()
}

/// Display verification of what will be protected/compressed
pub fn display_verification(
    files: &[AssetFile],
    protection_config: &FileProtectionConfig,
    assets_dir: &Path,
) {
    println!("📋 File Protection Verification");
    println!("{}", "=".repeat(80));
    println!();

    let mut protect_compress = Vec::new();
    let mut protect_only = Vec::new();
    let mut skipped = Vec::new();
    let mut total_original = 0u64;

    for file in files {
        total_original += file.original_size;
        let full_path = assets_dir.join(&file.path);
        let strategy = protection_config.get_strategy(&full_path, file.original_size);

        match strategy {
            ProtectionStrategy::ProtectAndCompress => {
                protect_compress.push((file.clone(), "✅ Protect + Compress"));
            }
            ProtectionStrategy::ProtectOnly => {
                protect_only.push((file.clone(), "⚠️  Protect Only"));
            }
            ProtectionStrategy::Skip => {
                skipped.push((file.clone(), "⏭️  Skipped"));
            }
        }
    }

    // Sort each list by path for consistent output
    protect_compress.sort_by(|a, b| a.0.path.cmp(&b.0.path));
    protect_only.sort_by(|a, b| a.0.path.cmp(&b.0.path));
    skipped.sort_by(|a, b| a.0.path.cmp(&b.0.path));

    // Calculate sizes
    let pc_size: u64 = protect_compress.iter().map(|(f, _)| f.original_size).sum();
    let po_size: u64 = protect_only.iter().map(|(f, _)| f.original_size).sum();
    let skip_size: u64 = skipped.iter().map(|(f, _)| f.original_size).sum();

    // Display summary
    println!("📊 Summary:");
    println!("  Total files: {}", files.len());
    println!(
        "  Total size: {:.2} MB",
        total_original as f64 / 1024.0 / 1024.0
    );
    println!();
    println!(
        "  ✅ Protect + Compress: {} files ({:.2} MB, {:.1}%)",
        protect_compress.len(),
        pc_size as f64 / 1024.0 / 1024.0,
        if total_original > 0 {
            (pc_size as f64 / total_original as f64) * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  ⚠️  Protect Only: {} files ({:.2} MB, {:.1}%)",
        protect_only.len(),
        po_size as f64 / 1024.0 / 1024.0,
        if total_original > 0 {
            (po_size as f64 / total_original as f64) * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  ⏭️  Skipped: {} files ({:.2} MB, {:.1}%)",
        skipped.len(),
        skip_size as f64 / 1024.0 / 1024.0,
        if total_original > 0 {
            (skip_size as f64 / total_original as f64) * 100.0
        } else {
            0.0
        }
    );
    println!();

    // Display detailed lists if there are files in each category
    if !protect_compress.is_empty() {
        println!("📦 Files to be Protected and Compressed:");
        for (file, action) in protect_compress.iter().take(50) {
            println!(
                "  {} {} ({:.2} KB)",
                action,
                file.path.display(),
                file.original_size as f64 / 1024.0
            );
        }
        if protect_compress.len() > 50 {
            println!("  ... and {} more files", protect_compress.len() - 50);
        }
        println!();
    }

    if !protect_only.is_empty() {
        println!("🔒 Files to be Protected Only (no compression):");
        for (file, action) in protect_only.iter().take(50) {
            println!(
                "  {} {} ({:.2} KB)",
                action,
                file.path.display(),
                file.original_size as f64 / 1024.0
            );
        }
        if protect_only.len() > 50 {
            println!("  ... and {} more files", protect_only.len() - 50);
        }
        println!();
    }

    if !skipped.is_empty() {
        println!("⏭️  Files to be Skipped:");
        for (file, action) in skipped.iter().take(50) {
            println!(
                "  {} {} ({:.2} KB)",
                action,
                file.path.display(),
                file.original_size as f64 / 1024.0
            );
        }
        if skipped.len() > 50 {
            println!("  ... and {} more files", skipped.len() - 50);
        }
        println!();
    }

    println!("{}", "=".repeat(80));
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_extension_list() {
        let list = parse_extension_list("json,xml,lua");
        assert_eq!(list.len(), 3);
        assert!(list.contains("json"));
        assert!(list.contains("xml"));
        assert!(list.contains("lua"));
    }

    #[test]
    fn test_parse_extension_list_empty() {
        let list = parse_extension_list("");
        assert!(list.is_empty());
    }

    #[test]
    fn test_parse_extension_list_case_insensitive() {
        let list = parse_extension_list("PNG,JPG,MP4");
        assert!(list.contains("png"));
        assert!(list.contains("jpg"));
        assert!(list.contains("mp4"));
    }

    #[test]
    fn test_protection_strategy_json() {
        let path = PathBuf::from("config.json");
        let strategy = ProtectionStrategy::from_file_smart(&path, 1024);
        assert_eq!(strategy, ProtectionStrategy::ProtectAndCompress);
    }

    #[test]
    fn test_protection_strategy_png() {
        let path = PathBuf::from("texture.png");
        let strategy = ProtectionStrategy::from_file_smart(&path, 1024 * 1024);
        assert_eq!(strategy, ProtectionStrategy::ProtectOnly);
    }

    #[test]
    fn test_protection_strategy_large_file() {
        let path = PathBuf::from("large_file.bin");
        let large_size = 200 * 1024 * 1024; // 200MB
        let strategy = ProtectionStrategy::from_file_smart(&path, large_size);
        assert_eq!(strategy, ProtectionStrategy::ProtectOnly);
    }

    #[test]
    fn test_protection_strategy_wav() {
        let path = PathBuf::from("audio.wav");
        let strategy = ProtectionStrategy::from_file_smart(&path, 10 * 1024 * 1024);
        assert_eq!(strategy, ProtectionStrategy::ProtectAndCompress);
    }

    #[test]
    fn test_file_protection_config_default() {
        let config = FileProtectionConfig::default();
        assert!(config.use_smart_defaults);
        assert!(config.force_compress_types.is_empty());
        assert!(config.no_compress_types.is_empty());
        assert!(config.skip_types.is_empty());
        assert!(config.protect_only_types.is_none());
    }

    #[test]
    fn test_should_compress() {
        let config = FileProtectionConfig::default();
        let path = PathBuf::from("config.json");
        assert!(config.should_compress(&path, 1024));
    }

    #[test]
    fn test_should_protect() {
        let config = FileProtectionConfig::default();
        let path = PathBuf::from("config.json");
        assert!(config.should_protect(&path, 1024));
    }

    #[test]
    fn test_skip_types() {
        let mut config = FileProtectionConfig::default();
        config.skip_types.insert("log".to_string());

        let path = PathBuf::from("debug.log");
        let strategy = config.get_strategy(&path, 1024);
        assert_eq!(strategy, ProtectionStrategy::Skip);
    }

    #[test]
    fn test_force_compress() {
        let mut config = FileProtectionConfig {
            use_smart_defaults: false,
            ..Default::default()
        };
        config.force_compress_types.insert("png".to_string());

        let path = PathBuf::from("texture.png");
        let strategy = config.get_strategy(&path, 1024 * 1024);
        assert_eq!(strategy, ProtectionStrategy::ProtectAndCompress);
    }

    #[test]
    fn test_protect_only_types() {
        let config = FileProtectionConfig {
            protect_only_types: Some(["json".to_string()].iter().cloned().collect()),
            ..Default::default()
        };

        let path1 = PathBuf::from("config.json");
        let path2 = PathBuf::from("texture.png");

        assert_eq!(
            config.get_strategy(&path1, 1024),
            ProtectionStrategy::ProtectAndCompress
        );
        assert_eq!(
            config.get_strategy(&path2, 1024 * 1024),
            ProtectionStrategy::Skip
        );
    }

    #[test]
    fn test_compress_all() {
        let config = FileProtectionConfig {
            compress_all: true,
            ..Default::default()
        };

        // Should compress all files, even non-compressible ones
        let path1 = PathBuf::from("config.json");
        let path2 = PathBuf::from("texture.png");
        let path3 = PathBuf::from("video.mp4");

        assert_eq!(
            config.get_strategy(&path1, 1024),
            ProtectionStrategy::ProtectAndCompress
        );
        assert_eq!(
            config.get_strategy(&path2, 1024 * 1024),
            ProtectionStrategy::ProtectAndCompress
        );
        assert_eq!(
            config.get_strategy(&path3, 10 * 1024 * 1024),
            ProtectionStrategy::ProtectAndCompress
        );
    }

    #[test]
    fn test_compress_none() {
        let config = FileProtectionConfig {
            compress_none: true,
            ..Default::default()
        };

        // Should not compress any files, even compressible ones
        let path1 = PathBuf::from("config.json");
        let path2 = PathBuf::from("script.js");
        let path3 = PathBuf::from("texture.png");

        assert_eq!(
            config.get_strategy(&path1, 1024),
            ProtectionStrategy::ProtectOnly
        );
        assert_eq!(
            config.get_strategy(&path2, 1024),
            ProtectionStrategy::ProtectOnly
        );
        assert_eq!(
            config.get_strategy(&path3, 1024 * 1024),
            ProtectionStrategy::ProtectOnly
        );
    }

    #[test]
    fn test_create_protection_config() {
        let config = create_protection_config(CLIProtectionConfig {
            smart_defaults: false,
            compress_all: false,
            compress_none: false,
            compress_types: Some("json,xml".to_string()),
            no_compress_types: Some("png,jpg".to_string()),
            protect_only_types: None,
            skip_types: None,
            enable_protected_all: false,
        });

        assert!(!config.use_smart_defaults);
        assert!(config.force_compress_types.contains("json"));
        assert!(config.no_compress_types.contains("png"));
    }

    #[test]
    fn test_create_protection_config_compress_all() {
        let config = create_protection_config(CLIProtectionConfig {
            smart_defaults: true, // Should be overridden
            compress_all: true,
            compress_none: false,
            compress_types: None,
            no_compress_types: None,
            protect_only_types: None,
            skip_types: None,
            enable_protected_all: false,
        });

        assert!(!config.use_smart_defaults); // Should be disabled when compress_all is set
        assert!(config.compress_all);
        assert!(!config.compress_none);
    }

    #[test]
    fn test_create_protection_config_compress_none() {
        let config = create_protection_config(CLIProtectionConfig {
            smart_defaults: true, // Should be overridden
            compress_all: false,
            compress_none: true,
            compress_types: None,
            no_compress_types: None,
            protect_only_types: None,
            skip_types: None,
            enable_protected_all: true,
        });

        assert!(!config.use_smart_defaults); // Should be disabled when compress_none is set
        assert!(!config.compress_all);
        assert!(config.compress_none);
    }

    #[test]
    fn test_create_protection_config_smart_defaults_false() {
        let config = create_protection_config(CLIProtectionConfig {
            smart_defaults: false,
            compress_all: false,
            compress_none: false,
            compress_types: None,
            no_compress_types: None,
            protect_only_types: None,
            skip_types: None,
            enable_protected_all: false,
        });

        assert!(!config.use_smart_defaults);
        assert!(!config.compress_all);
        assert!(!config.compress_none);
    }

    #[test]
    fn test_protected_all_protection() {
        let config = FileProtectionConfig {
            enable_protected_all: true,
            skip_types: parse_extension_list("json,xml,txt"),
            ..FileProtectionConfig::default()
        };

        // With protected_all enabled, all files should be protected

        let test_files = vec![
            PathBuf::from("config.json"),
            PathBuf::from("data.xml"),
            PathBuf::from("readme.txt"),
            PathBuf::from("test.bin"),
        ];

        for file in test_files {
            let strategy = config.get_strategy(&file, 1024);
            assert_ne!(
                strategy,
                ProtectionStrategy::Skip,
                "{} should be protected when enable_protected_all is true",
                file.display()
            );
        }
    }

    #[test]
    fn test_protected_all_without_smart_defaults() {
        let config = FileProtectionConfig {
            enable_protected_all: true,
            use_smart_defaults: false,
            ..FileProtectionConfig::default()
        };

        let test_file = PathBuf::from("config.json");
        let strategy = config.get_strategy(&test_file, 1024);

        // Should be protected but not compressed (no smart defaults)
        assert_eq!(strategy, ProtectionStrategy::ProtectOnly);
    }

    #[test]
    fn test_create_protection_config_with_protected_all() {
        let config = create_protection_config(CLIProtectionConfig {
            smart_defaults: true,
            compress_all: false,
            compress_none: false,
            compress_types: None,
            no_compress_types: None,
            protect_only_types: None,
            skip_types: None,
            enable_protected_all: true,
        });

        assert!(config.enable_protected_all);
        assert!(config.use_smart_defaults);
    }
}
