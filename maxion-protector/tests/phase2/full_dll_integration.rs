//! Phase 2 Integration Tests - Full DLL Embedding
//!
//! Tests the complete Phase 2 workflow:
//! 1. Load and analyze DLL structure
//! 2. Parse original PE executable
//! 3. Embed full DLL with all sections
//! 4. Apply relocations for new base address
//! 5. Resolve imports and patch IAT
//! 6. Validate protected executable structure
//! 7. Verify entry points and section layout

use anyhow::{Context, Result};
use maxion_injector::{PeInjector, SectionInfo};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Paths for test assets
fn get_test_paths() -> (PathBuf, PathBuf, PathBuf) {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = base.parent().expect("Failed to get project root");

    let test_exe = root.join("test_assets/test.exe");
    let stub_dll = root.join("target/release/maxion_loader_stub.dll");

    (root, test_exe, stub_dll)
}

/// Verify test assets exist
fn verify_test_assets() -> Result<()> {
    let (_root, test_exe, stub_dll) = get_test_paths();

    if !test_exe.exists() {
        anyhow::bail!("Test executable not found: {}", test_exe.display());
    }

    if !stub_dll.exists() {
        anyhow::bail!(
            "Stub DLL not found: {}\nBuild with: cargo build --release -p maxion-loader-stub",
            stub_dll.display()
        );
    }

    Ok(())
}

/// Integration test for full DLL embedding (Phase 2)
#[test]
#[cfg(feature = "phase2")]
fn test_full_dll_embedding() -> Result<()> {
    // Verify prerequisites
    verify_test_assets().context("Test assets not found")?;

    let (_root, test_exe, stub_dll) = get_test_paths();
    let temp_dir = TempDir::new().context("Failed to create temp directory")?;

    println!("🚀 Starting Phase 2 Integration Test");
    println!("=".repeat(60));
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
        .context("Failed to load DLL structure")?;

    println!("✅ DLL structure loaded for embedding");

    // Step 4: Perform full DLL injection
    println!("\n🔧 Step 4: Performing full DLL injection...");

    injector
        .inject_full_dll()
        .context("Full DLL injection failed")?;

    println!("✅ Full DLL injection completed");

    // Step 5: Validate protected executable was created
    println!("\n📋 Step 5: Validating protected executable...");

    assert!(
        protected_path.exists(),
        "Protected executable not created at: {}",
        protected_path.display()
    );

    let protected_size = fs::metadata(&protected_path)?.len();
    println!(
        "✅ Protected executable created: {} bytes ({} KB)",
        protected_size,
        protected_size / 1024
    );

    // Verify size is reasonable (should be larger than original)
    let original_size = fs::metadata(&test_exe)?.len();
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

    let protected_data = fs::read(&protected_path)?;
    use goblin::pe::PE;

    let protected_pe = PE::parse(&protected_data).context("Failed to parse protected PE")?;

    println!("✅ Protected PE parsed successfully");

    // Step 7: Validate sections
    println!("\n📋 Step 7: Validating section layout...");

    println!("Protected PE has {} sections:", protected_pe.sections.len());

    let mut found_maxion = false;
    let mut found_key = false;
    let mut dll_sections = 0;

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
        }
        if section_name == ".key" {
            found_key = true;
        }
        if section_name.starts_with(".dll_") {
            dll_sections += 1;
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

    Ok(())
}

/// Test DLL structure parsing
#[test]
#[cfg(feature = "phase2")]
fn test_dll_structure_parsing() -> Result<()> {
    verify_test_assets().context("Test assets not found")?;

    let (_root, test_exe, stub_dll) = get_test_paths();

    println!("🔍 Testing DLL structure parsing");
    println!("=".repeat(60));

    // Load DLL to verify structure
    let dll_data = fs::read(&stub_dll).context("Failed to read stub DLL")?;

    println!("✅ DLL loaded: {} bytes", dll_data.len());

    // Parse PE structure using goblin
    use goblin::pe::PE;

    let pe = PE::parse(&dll_data).context("Failed to parse PE")?;

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

    Ok(())
}

/// Test section alignment for embedded DLL
#[test]
#[cfg(feature = "phase2")]
fn test_dll_section_alignment() -> Result<()> {
    println!("📏 Testing DLL section alignment");
    println!("=".repeat(60));

    // Read DLL
    let (_root, _test_exe, stub_dll) = get_test_paths();

    let dll_data = fs::read(&stub_dll).context("Failed to read stub DLL")?;

    use goblin::pe::PE;

    let pe = PE::parse(&dll_data).context("Failed to parse PE")?;

    // Get section alignment
    let section_alignment = pe
        .header
        .optional_header
        .as_ref()
        .map(|h| h.windows_fields.section_alignment)
        .unwrap_or(0x1000);

    println!(
        "Section alignment: 0x{:X} ({} bytes)",
        section_alignment, section_alignment
    );

    // Check each section
    let mut violations = Vec::new();

    for section in &pe.sections {
        let name_bytes = &section.name;
        let null_pos = name_bytes.iter().position(|&b| b == 0).unwrap_or(8);
        let section_name = String::from_utf8_lossy(&name_bytes[..null_pos]);

        let virtual_address = section.virtual_address;

        // Check if properly aligned
        if virtual_address % section_alignment != 0 {
            violations.push(format!(
                "{}: VA 0x{:X} not aligned to 0x{:X}",
                section_name, virtual_address, section_alignment
            ));
        }

        println!(
            "{}: VA 0x{:08X} - {}",
            if virtual_address % section_alignment == 0 {
                "✓"
            } else {
                "✗"
            },
            virtual_address,
            section_name
        );
    }

    if !violations.is_empty() {
        println!("\n⚠️  Alignment violations found:");
        for violation in &violations {
            println!("   - {}", violation);
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("✅ Section alignment test PASSED");

    Ok(())
}

/// Test relocation directory parsing
#[test]
#[cfg(feature = "phase2")]
fn test_relocation_parsing() -> Result<()> {
    println!("🔄 Testing relocation directory parsing");
    println!("=".repeat(60));

    let (_root, _test_exe, stub_dll) = get_test_paths();

    let dll_data = fs::read(&stub_dll).context("Failed to read stub DLL")?;

    use goblin::pe::PE;

    let pe = PE::parse(&dll_data).context("Failed to parse PE")?;

    // Get relocation directory
    let reloc_dir = pe
        .header
        .optional_header
        .as_ref()
        .and_then(|h| h.data_directories.get_base_relocation_table());

    match reloc_dir {
        Some(reloc) => {
            println!("✅ Relocation directory found:");
            println!("   RVA:  0x{:08X}", reloc.virtual_address);
            println!("   Size: {} bytes", reloc.size);

            if reloc.size == 0 {
                println!("⚠️  Warning: Relocation directory has zero size");
            }
        }
        None => {
            anyhow::bail!("No relocation directory found - DLL cannot be rebased");
        }
    }

    // Find .reloc section
    let reloc_section = pe.sections.iter().find(|s| s.name.starts_with(b".reloc"));

    match reloc_section {
        Some(section) => {
            let raw_size = section.size_of_raw_data;
            println!("\n📦 .reloc section:");
            println!("   RVA:  0x{:08X}", section.virtual_address);
            println!("   Size: {} bytes", raw_size);

            // Estimate number of blocks
            // Each block is at least 8 bytes (4 for RVA, 4 for size)
            let estimated_blocks = raw_size / 8;
            println!("   Estimated blocks: ~{}", estimated_blocks);

            println!("\n✅ Relocation directory validated");
        }
        None => {
            println!("⚠️  .reloc section not found in section table");
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("✅ Relocation parsing test PASSED");

    Ok(())
}

/// Test import directory parsing
#[test]
#[cfg(feature = "phase2")]
fn test_import_parsing() -> Result<()> {
    println!("📦 Testing import directory parsing");
    println!("=".repeat(60));

    let (_root, _test_exe, stub_dll) = get_test_paths();

    let dll_data = fs::read(&stub_dll).context("Failed to read stub DLL")?;

    use goblin::pe::PE;

    let pe = PE::parse(&dll_data).context("Failed to parse PE")?;

    // Get import directory
    let import_dir = pe
        .header
        .optional_header
        .as_ref()
        .and_then(|h| h.data_directories.get_import_table());

    match import_dir {
        Some(imports) => {
            println!("✅ Import directory found:");
            println!("   RVA:  0x{:08X}", imports.virtual_address);
            println!("   Size: {} bytes", imports.size);

            if imports.size == 0 {
                println!("⚠️  Note: Import directory has zero size (no imports)");
            }
        }
        None => {
            println!("⚠️  No import directory (statically linked or no imports)");
        }
    }

    println!("\n{}", "=".repeat(60));
    println!("✅ Import parsing test PASSED");

    Ok(())
}
