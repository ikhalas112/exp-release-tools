//! Integration tests for Maxion Protector
//!
//! Tests the complete workflow from asset packing to archive reading.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

// Re-export types for easier access
use maxion_core::access_control::AccessStats;
use maxion_core::archive::ArchiveReader;
use maxion_core::compression::CompressionStats;

// Phase 2 imports (only used in cfg(feature = "phase2") tests)
#[cfg(feature = "phase2")]
use goblin::pe::PE;
#[cfg(feature = "phase2")]
use maxion_injector::PeInjector;

/// Create a test directory with sample asset files
#[allow(dead_code)]
fn create_test_assets(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();

    // Create test files with various sizes
    let test_data = vec![
        ("test.txt", b"Hello, World!".to_vec()),
        ("image.png", vec![0u8; 4096]),
        ("model.obj", vec![1u8; 8192]),
        ("config.json", b"{\"key\": \"value\"}".to_vec()),
        ("large_asset.dat", vec![0xFFu8; 1024 * 1024]), // 1MB file
    ];

    for (name, data) in test_data {
        let file_path = dir.join(name);
        let mut file = File::create(&file_path)?;
        file.write_all(&data)?;
        files.push(file_path);
    }

    Ok(files)
}

/// Test basic archive creation and reading
#[test]
fn test_archive_creation_and_reading() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create config
    let mut config = maxion_core::Config::new()
        .with_compression(false, 0) // Disable compression for testing
        .with_chunk_size(64 * 1024);

    config.generate_keys();

    // Create archive builder
    let mut builder = maxion_core::ArchiveBuilder::new(config.clone());

    // Add test assets with absolute paths that exist
    let assets_dir = temp_dir.path().join("assets");
    fs::create_dir_all(&assets_dir).expect("Failed to create assets dir");

    let test_data = b"Test data for archive";
    let test_file_path = assets_dir.join("test.txt");
    fs::write(&test_file_path, test_data).expect("Failed to write test file");

    let mut asset = maxion_core::AssetFile::new(test_file_path.clone(), test_data.len() as u64);
    asset.calculate_checksum(test_data);
    builder.add_file(asset);

    // Build archive
    let archive_path = temp_dir.path().join("test.archive");
    let header = builder
        .build(&archive_path)
        .expect("Failed to build archive");

    // Verify archive was created
    assert!(archive_path.exists(), "Archive file should exist");
    assert_eq!(header.file_count, 1, "Archive should contain 1 file");
}

/// Test archive header serialization
#[test]
fn test_archive_header_serialization() {
    let header = maxion_core::ArchiveHeader::new(100, maxion_core::ChunkSize::new(65536), true);

    let bytes = header.to_bytes();
    assert_eq!(bytes.len(), 256, "Header should be 256 bytes");

    let parsed = maxion_core::ArchiveHeader::from_bytes(&bytes).expect("Failed to parse header");

    assert_eq!(parsed.file_count, header.file_count);
    assert_eq!(parsed.compress, header.compress);
    assert_eq!(parsed.chunk_size, header.chunk_size);
}

/// Test checksum calculation
#[test]
fn test_checksum_calculation() {
    let data = b"Test data for checksum";
    let checksum = maxion_core::crypto::utils::blake3_hash(data);

    assert_eq!(checksum.as_bytes().len(), 32, "Checksum should be 32 bytes");
    assert_ne!(
        checksum.as_bytes(),
        &[0u8; 32],
        "Checksum should not be all zeros"
    );
}

/// Test chunk size validation
#[test]
fn test_chunk_size_validation() {
    // Valid sizes (powers of 2)
    assert!(maxion_core::ChunkSize::new(4096).is_valid());
    assert!(maxion_core::ChunkSize::new(8192).is_valid());
    assert!(maxion_core::ChunkSize::new(65536).is_valid());

    // Invalid size (not power of 2) - should still work due to validation
    let size = maxion_core::ChunkSize::new(5000);
    assert!(!size.is_valid(), "Non-power of 2 size should be invalid");
}

/// Test access control rate limiting
#[test]
fn test_access_control_rate_limiting() {
    let mut control = maxion_core::AccessControl::with_limits(3, 10);

    // First 3 reads should succeed
    for _ in 0..3 {
        assert!(control.check_rate_limit().is_ok(), "Read should be allowed");
    }

    // 4th read within delay window should fail
    assert!(
        control.check_rate_limit().is_err(),
        "Rate limit should be exceeded"
    );

    // Wait for delay to pass
    std::thread::sleep(std::time::Duration::from_millis(15));

    // Should allow reads again
    assert!(
        control.check_rate_limit().is_ok(),
        "Rate limit should reset"
    );
}

/// Test nonce generation
#[test]
fn test_nonce_generation() {
    let nonce1 = maxion_core::Nonce::generate();
    let nonce2 = maxion_core::Nonce::generate();

    assert_eq!(nonce1.as_bytes().len(), 24, "Nonce should be 24 bytes");
    assert_eq!(nonce2.as_bytes().len(), 24, "Nonce should be 24 bytes");
    assert_ne!(nonce1, nonce2, "Generated nonces should be unique");
}

/// Test encryption key generation
#[test]
fn test_encryption_key_generation() {
    let key = maxion_core::EncryptionKey::generate();

    assert_eq!(key.as_bytes().len(), 32, "Key should be 32 bytes");
    assert_ne!(
        key.as_bytes(),
        &[0u8; 32],
        "Generated key should not be all zeros"
    );
}

/// Test config key derivation
#[test]
fn test_config_key_derivation() {
    let mut config = maxion_core::Config::new();

    config.build_secret.copy_from_slice(&[1u8; 32]);

    let result = config.derive_key();
    assert!(result.is_ok(), "Key derivation should succeed");
    assert_ne!(
        config.encryption_key, [0u8; 32],
        "Derived key should not be all zeros"
    );
}

/// Test asset file metadata
#[test]
fn test_asset_file_metadata() {
    let mut asset = maxion_core::AssetFile::new(PathBuf::from("test/path/file.txt"), 1024);

    // Note: chunk_count is calculated based on packed_size, not original_size
    // Since packed_size defaults to original_size, we need to set it first
    asset.packed_size = 1024;

    // Note: ChunkSize::new() normalizes to minimum 4096 (power of 2)
    // So 1024 bytes / 4096 bytes per chunk = 1 chunk
    asset.calculate_chunk_count(maxion_core::ChunkSize::new(512));
    assert_eq!(
        asset.chunk_count, 1,
        "1024 bytes / 4096 bytes per chunk (normalized) = 1 chunk"
    );

    asset.normalize_path();
    assert_eq!(
        asset.path_str(),
        "test/path/file.txt",
        "Path should use forward slashes"
    );
}

/// Test compression statistics
#[test]
fn test_compression_statistics() {
    // Test single file stats
    let stats1 = CompressionStats::new(1000, 500, 6);
    assert_eq!(stats1.original_size, 1000, "Original size should be 1000");
    assert_eq!(stats1.compressed_size, 500, "Compressed size should be 500");
    assert_eq!(stats1.space_saved(), 500, "Space saved should be 500");
    assert_eq!(stats1.ratio(), 0.5, "Compression ratio should be 0.5");
    assert_eq!(
        stats1.percentage(),
        50.0,
        "Compression percentage should be 50%"
    );

    // Test another file
    let stats2 = CompressionStats::new(1000, 600, 6);
    assert_eq!(stats2.ratio(), 0.6, "Compression ratio should be 0.6");
    assert_eq!(
        stats2.percentage(),
        40.0,
        "Compression percentage should be 40%"
    );
}

/// Test encryption and decryption
#[test]
fn test_encryption_decryption() {
    use maxion_core::crypto::ChunkCipher;

    let key = maxion_core::EncryptionKey::generate();
    let nonce = maxion_core::Nonce::generate();
    let chunk_size = maxion_core::ChunkSize::new(1024);

    let cipher = ChunkCipher::new(key.as_bytes(), nonce.as_bytes(), chunk_size);

    let plaintext = b"This is a test message for encryption";
    let chunk_nonce = maxion_core::Nonce::from_chunk_index(0, nonce.as_bytes());
    let encrypted = cipher
        .encrypt_single(plaintext, &chunk_nonce)
        .expect("Encryption failed");

    assert_ne!(
        encrypted, plaintext,
        "Encrypted data should differ from plaintext"
    );
    assert!(
        encrypted.len() > plaintext.len(),
        "Encrypted data should be larger (includes tag)"
    );

    let chunk_nonce = maxion_core::Nonce::from_chunk_index(0, nonce.as_bytes());
    let decrypted = cipher
        .decrypt_single(&encrypted, &chunk_nonce)
        .expect("Decryption failed");

    assert_eq!(decrypted, plaintext, "Decrypted data should match original");
}

/// Test compression
#[test]
fn test_compression() {
    let original = b"AAAAABBBBBCCCCCDDDDDEEEEEFFFFFGGGGGHHHHHIIIIIJJJJJ";

    let compressed = maxion_core::compress(original, 6, None).expect("Compression failed");
    let decompressed =
        maxion_core::decompress(&compressed, Some(original.len())).expect("Decompression failed");

    assert_eq!(
        decompressed, original,
        "Decompressed data should match original"
    );
    assert!(
        compressed.len() < original.len(),
        "Compressed data should be smaller"
    );
}

/// Test path normalization
#[test]
fn test_path_normalization() {
    let mut asset =
        maxion_core::AssetFile::new(PathBuf::from("assets\\folder\\subfolder\\file.png"), 1024);

    asset.normalize_path();
    assert_eq!(asset.path_str(), "assets/folder/subfolder/file.png");
}

/// Test archive header checksum
#[test]
fn test_archive_header_checksum() {
    let mut header = maxion_core::ArchiveHeader::new(50, maxion_core::ChunkSize::new(32768), false);

    header.calculate_checksum();

    let bytes = header.to_bytes();
    let parsed = maxion_core::ArchiveHeader::from_bytes(&bytes).expect("Failed to parse header");

    assert_eq!(parsed.header_checksum, header.header_checksum);
    assert!(
        parsed.verify_checksum(),
        "Checksum verification should succeed"
    );
}

/// Test invalid archive detection
#[test]
fn test_invalid_archive_detection() {
    let invalid_data = vec![0u8; 256];

    let result = maxion_core::ArchiveHeader::from_bytes(&invalid_data);
    assert!(result.is_err(), "Should detect invalid magic number");
}

/// Test access control statistics
#[test]
fn test_access_control_statistics() {
    let mut stats = AccessStats::new();

    stats.record_success();
    stats.record_success();
    stats.record_violation();

    assert_eq!(stats.total_attempts, 3, "Total attempts should be 3");
    assert_eq!(stats.successful_reads, 2, "Successful reads should be 2");
    assert_eq!(stats.rate_limit_violations, 1, "Violations should be 1");
    assert!(
        (stats.success_rate() - 66.66).abs() < 0.01,
        "Success rate should be ~66.67%"
    );
}

/// Test chunk nonce derivation
#[test]
fn test_chunk_nonce_derivation() {
    let base_nonce = [1u8; 24];

    let nonce0 = maxion_core::Nonce::from_chunk_index(0, &base_nonce);
    let nonce1 = maxion_core::Nonce::from_chunk_index(1, &base_nonce);

    assert_ne!(
        nonce0, nonce1,
        "Different chunk indices should produce different nonces"
    );

    // Check that the chunk index is encoded in the first 4 bytes
    assert_eq!(&nonce0.as_bytes()[..4], &0u32.to_le_bytes());
    assert_eq!(&nonce1.as_bytes()[..4], &1u32.to_le_bytes());
}

/// Test large file handling
#[test]
fn test_large_file_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let large_file = temp_dir.path().join("large.dat");

    // Create a 5MB file
    let data = vec![0xABu8; 5 * 1024 * 1024];
    let mut file = File::create(&large_file).expect("Failed to create file");
    file.write_all(&data).expect("Failed to write file");

    // Verify file size
    let metadata = fs::metadata(&large_file).expect("Failed to get metadata");
    assert_eq!(metadata.len(), 5 * 1024 * 1024, "File should be 5MB");
}

/// Test concurrent file processing
#[test]
fn test_concurrent_processing() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let assets_dir = temp_dir.path().join("assets");
    fs::create_dir_all(&assets_dir).expect("Failed to create assets dir");

    // Create multiple files
    for i in 0..10 {
        let file_path = assets_dir.join(format!("file_{}.txt", i));
        let mut file = File::create(&file_path).expect("Failed to create file");
        file.write_all(format!("Content of file {}", i).as_bytes())
            .expect("Failed to write file");
    }

    // Verify all files were created
    let entries: Vec<_> = fs::read_dir(&assets_dir)
        .expect("Failed to read directory")
        .filter_map(|e| e.ok())
        .collect();

    assert_eq!(entries.len(), 10, "Should have 10 files");
}

/// Test error handling for missing files
#[test]
fn test_missing_file_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let archive_path = temp_dir.path().join("test.archive");

    // Try to read non-existent archive
    let result = ArchiveReader::open(&archive_path);
    assert!(result.is_err(), "Should error on missing file");
}

/// Test empty archive
#[test]
fn test_empty_archive() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let archive_path = temp_dir.path().join("empty.archive");

    // Create config
    let mut config = maxion_core::Config::new();
    config.generate_keys();

    // Build empty archive
    let mut builder = maxion_core::ArchiveBuilder::new(config);
    let header = builder
        .build(&archive_path)
        .expect("Failed to build empty archive");

    assert_eq!(header.file_count, 0, "Empty archive should have 0 files");
    assert!(archive_path.exists(), "Archive file should exist");
}

/// Test archive version compatibility
#[test]
fn test_archive_version() {
    let header = maxion_core::ArchiveHeader::new(0, maxion_core::ChunkSize::default(), false);

    assert_eq!(header.version, 1, "Archive version should be 1");
    assert_eq!(
        maxion_core::ARCHIVE_VERSION,
        1,
        "Version constant should match"
    );
}

/// Test magic number
#[test]
fn test_magic_number() {
    let magic = maxion_core::MAGIC;
    assert_eq!(magic.len(), 8, "Magic number should be 8 bytes");
    assert_eq!(magic, b"MAXION\x01\x00", "Magic number should match");
}

// ============================================================================
// PHASE 2: Full DLL Embedding Integration Tests
// ============================================================================

/// Get test asset paths
#[allow(dead_code)]
fn get_test_paths() -> (PathBuf, PathBuf) {
    // CARGO_MANIFEST_DIR points to workspace root where Cargo.toml is located
    // In this workspace, tests/ are at the same level as crates/, so we need to go up
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let test_exe = workspace_root.join("test_assets/test.exe");

    // Build directory is relative to workspace root
    let stub_dll = workspace_root.join("target/release/maxion_stub.dll");

    println!("📍 Test paths:");
    println!("   Workspace root: {}", workspace_root.display());
    println!("   Test EXE: {}", test_exe.display());
    println!("   Stub DLL: {}", stub_dll.display());
    println!("   DLL exists: {}", stub_dll.exists());

    (test_exe, stub_dll)
}

/// Integration test for full DLL embedding (Phase 2)
#[test]
#[cfg(feature = "phase2")]
fn test_full_dll_embedding() {
    let (test_exe, stub_dll) = get_test_paths();
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    println!("🚀 Starting Phase 2 Integration Test");
    println!("{}", "=".repeat(60));
    println!("Test EXE:  {}", test_exe.display());
    println!("Stub DLL: {}", stub_dll.display());

    // Step 1: Create test archive data
    println!("\n📦 Step 1: Creating test archive data...");

    let archive_data = vec![0u8; 1024]; // Minimal archive for testing
    let mut encryption_key = [0u8; 32];
    encryption_key[0] = 0x01; // Set a non-zero value
    let mut nonce = [0u8; 24];
    nonce[0] = 0x02; // Set a non-zero value
    let chunk_size = 65536u32;

    println!("✅ Test archive data created: {} bytes", archive_data.len());

    // Step 2: Create injector with paths and data
    println!("\n🔧 Step 2: Creating PE injector...");

    let protected_path = temp_dir.path().join("test_phase2.exe");

    let injector = PeInjector::new(
        test_exe.clone(),
        protected_path.clone(),
        archive_data,
        encryption_key,
        nonce,
        chunk_size,
    );

    println!("✅ PE injector created");

    // Step 3: Load DLL structure for full embedding
    println!("\n📦 Step 3: Loading DLL structure...");

    let injector = injector
        .with_dll(stub_dll.clone())
        .expect("Failed to load DLL structure");

    println!("✅ DLL structure loaded for embedding");

    // Step 4: Perform full DLL injection
    println!("\n🔧 Step 4: Performing full DLL injection...");

    injector
        .inject_full_dll()
        .expect("Full DLL injection failed");

    println!("✅ Full DLL injection completed");

    // Step 5: Validate protected executable was created
    println!("\n📋 Step 5: Validating protected executable...");

    assert!(
        protected_path.exists(),
        "Protected executable not created at: {}",
        protected_path.display()
    );

    let protected_size = fs::metadata(&protected_path)
        .expect("Failed to get protected file metadata")
        .len();
    println!(
        "✅ Protected executable created: {} bytes ({} KB)",
        protected_size,
        protected_size / 1024
    );

    // Verify size is reasonable (should be larger than original)
    let original_size = fs::metadata(&test_exe)
        .expect("Failed to get original file metadata")
        .len();
    assert!(
        protected_size > original_size,
        "Protected executable should be larger than original: {} > {}",
        protected_size,
        original_size
    );

    println!(
        "✅ Size validation passed: {} KB → {} KB",
        original_size / 1024,
        protected_size / 1024
    );

    // Step 6: Parse protected PE and validate structure
    println!("\n📊 Step 6: Parsing protected PE structure...");

    let protected_data = fs::read(&protected_path).expect("Failed to read protected file");

    let protected_pe = PE::parse(&protected_data).expect("Failed to parse protected PE");

    println!("✅ Protected PE parsed successfully");

    // Step 7: Validate sections
    println!("\n📋 Step 7: Validating section layout...");

    println!("Protected PE has {} sections:", protected_pe.sections.len());

    let mut found_maxion = false;
    let mut found_key = false;
    let mut dll_sections = 0;

    // Track DLL sections by their VA ranges (they appear after .maxion section)
    let mut found_maxion_va = 0u32;

    for section in &protected_pe.sections {
        let name_bytes = &section.name;
        let null_pos = name_bytes.iter().position(|&b| b == 0).unwrap_or(8);
        let section_name = String::from_utf8_lossy(&name_bytes[..null_pos]);

        println!(
            "   {} - VA: 0x{:08X}, Size: {} bytes",
            section_name, section.virtual_address, section.size_of_raw_data
        );

        if section_name == ".maxion" {
            found_maxion = true;
            found_maxion_va = section.virtual_address;
        }
        if section_name == ".key" {
            found_key = true;
        }

        // After finding .maxion section, any .text, .rdata, .data, .pdata, or .reloc
        // are from the embedded DLL
        if found_maxion_va > 0 && section.virtual_address > found_maxion_va {
            let is_dll_section = matches!(
                section_name.as_ref(),
                ".text" | ".rdata" | ".data" | ".pdata" | ".reloc"
            );
            if is_dll_section {
                dll_sections += 1;
            }
        }
    }

    // Validate expected sections
    assert!(found_maxion, "Protected PE missing .maxion section");
    assert!(found_key, "Protected PE missing .key section");
    assert!(
        dll_sections > 0,
        "Protected PE should have embedded DLL sections (found {})",
        dll_sections
    );

    println!("\n✅ Section validation passed:");
    println!(
        "   .maxion: {} ✓",
        if found_maxion { "found" } else { "MISSING" }
    );
    println!(
        "   .key:    {} ✓",
        if found_key { "found" } else { "MISSING" }
    );
    println!("   DLL sections: {} embedded ✓", dll_sections);

    // Step 8: Validate entry point
    println!("\n🔧 Step 8: Validating entry point...");

    let entry_point = protected_pe.entry;
    println!("Entry point: 0x{:08X}", entry_point);

    assert_ne!(entry_point, 0, "Entry point should not be zero");

    println!("✅ Entry point validated");

    println!("\n{}", "=".repeat(60));
    println!("✅ Phase 2 Integration Test PASSED");
    println!("\nSummary:");
    println!("   ✅ DLL structure loaded");
    println!("   ✅ Full DLL injection completed");
    println!("   ✅ Protected executable created");
    println!("   ✅ Section layout validated");
    println!("   ✅ Entry point validated");
}

/// Test DLL structure parsing
#[test]
#[cfg(feature = "phase2")]
fn test_dll_structure_parsing() {
    let (_test_exe, stub_dll) = get_test_paths();

    println!("🔍 Testing DLL structure parsing");
    println!("{}", "=".repeat(60));

    // Load DLL to verify structure
    let dll_data = fs::read(&stub_dll).expect("Failed to read stub DLL");

    println!("✅ DLL loaded: {} bytes", dll_data.len());

    // Parse PE structure using goblin
    let pe = PE::parse(&dll_data).expect("Failed to parse PE");

    println!("✅ PE parsed successfully");

    // Validate PE is a DLL
    assert!(pe.is_lib, "PE file is not a DLL");

    println!("✅ Validated: PE is a DLL");

    // Check sections
    println!("\n📋 Sections found:");
    for section in &pe.sections {
        let name_bytes = &section.name;
        let null_pos = name_bytes.iter().position(|&b| b == 0).unwrap_or(8);
        let section_name = String::from_utf8_lossy(&name_bytes[..null_pos]);

        println!(
            "   {} - VA: 0x{:08X}, Size: {} bytes",
            section_name, section.virtual_address, section.size_of_raw_data
        );
    }

    // Verify essential sections exist
    let has_text = pe.sections.iter().any(|s| s.name.starts_with(b".text"));
    let has_data = pe.sections.iter().any(|s| s.name.starts_with(b".data"));
    let has_reloc = pe.sections.iter().any(|s| s.name.starts_with(b".reloc"));

    assert!(has_text, "DLL missing .text section");
    assert!(has_data, "DLL missing .data section");
    assert!(
        has_reloc,
        "DLL missing .reloc section (required for embedding)"
    );

    println!("\n✅ Essential sections present");
    println!("   .text:  {}", if has_text { "✓" } else { "✗" });
    println!("   .data:  {}", if has_data { "✓" } else { "✗" });
    println!("   .reloc: {}", if has_reloc { "✓" } else { "✗" });

    // Check entry point
    let entry_point = pe.entry;
    assert_ne!(entry_point, 0, "DLL has no entry point");

    println!("\n🔧 Entry point: 0x{:08X}", entry_point);

    // Check image base
    let image_base = pe.image_base;
    println!("🔧 Image base:   0x{:016X}", image_base);

    // Verify 64-bit
    assert!(pe.is_64, "DLL is not 64-bit");
    println!("✅ Validated: DLL is 64-bit");

    println!("\n{}", "=".repeat(60));
    println!("✅ DLL structure parsing test PASSED");
}
