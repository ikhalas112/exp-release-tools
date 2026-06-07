//! PE Relocation Parser and Processor
//!
//! This module handles parsing and applying PE relocations, which are necessary
//! when embedding a DLL into a PE file at a different base address.

pub mod import;
pub mod loader;
//
// # Relocation Overview
//
// When a DLL is compiled, it assumes a preferred base address (default 0x10000000).
// If the Windows loader can't load it at that address, it applies relocations
// to adjust all absolute addresses. When we manually embed a DLL, we must
// apply these relocations ourselves.
//
// # Relocation Block Structure
//
// ```text
// Relocation Directory (.reloc section)
// ┌─────────────────────────────────────┐
// │ Page RVA (4 bytes)              │ ← Virtual address of 4KB page
// │ Block Size (4 bytes)             │ ← Size of this block in bytes
// ├─────────────────────────────────────┤
// │ TypeOffset Entry 1 (2 bytes)     │ │
// │ TypeOffset Entry 2 (2 bytes)     │ │ ── Repeated entries
// │ ...                             │ │
// ├─────────────────────────────────────┤
// │ Page RVA (next block)            │
// │ Block Size                       │
// │ ...                             │
// └─────────────────────────────────────┘
// ```
//
// # TypeOffset Encoding
//
// Each entry is 2 bytes: `[3-bit type][12-bit offset]`
//
// Common types:
// - `0x0000` (0): ABSOLUTE - No relocation, skip
// - `0x3000` (3): HIGHLOW - 32-bit relocation (most common)
// - `0xA000` (10): DIR64 - 64-bit relocation (x64 only)
// - `0x0001` (1): HIGH - High 16 bits of 32-bit address
// - `0x0002` (2): LOW - Low 16 bits of 32-bit address

use std::collections::HashMap;

/// Relocation type constants (from PE specification)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum RelocType {
    /// Absolute - No relocation required
    Absolute = 0,
    /// High - Add high 16 bits of delta to 16-bit field at offset
    High = 1,
    /// Low - Add low 16 bits of delta to 16-bit field at offset
    Low = 2,
    /// HighLow - Add 32-bit delta to 32-bit field at offset
    HighLow = 3,
    /// HighAdj - Add high 16 bits of delta + sign bit to 16-bit field
    HighAdj = 4,
    /// MIPS Jump Address - 5
    MipsJump = 5,
    /// Section - 6
    Section = 6,
    /// Rel32 - 32-bit relative relocation (x86)
    Rel32 = 7,
    /// Dir64 - 64-bit absolute relocation (x64)
    Dir64 = 10,
}

impl RelocType {
    /// Parse relocation type from TypeOffset entry
    ///
    /// # Arguments
    ///
    /// * `type_offset` - 16-bit TypeOffset entry
    ///
    /// # Returns
    ///
    /// `RelocType` - Parsed relocation type
    fn from_u16(value: u16) -> Self {
        // Extract upper 4 bits (type field)
        let type_bits = (value >> 12) & 0xF;
        match type_bits {
            0 => RelocType::Absolute,
            1 => RelocType::High,
            2 => RelocType::Low,
            3 => RelocType::HighLow,
            4 => RelocType::HighAdj,
            5 => RelocType::MipsJump,
            6 => RelocType::Section,
            7 => RelocType::Rel32,
            10 => RelocType::Dir64,
            _ => RelocType::Absolute, // Unknown type, treat as absolute
        }
    }

    /// Check if relocation type is valid for processing
    ///
    /// # Returns
    ///
    /// `bool` - True if valid relocation type
    pub fn is_valid(&self) -> bool {
        !matches!(self, RelocType::Absolute)
    }
}

/// Single relocation entry within a page
#[derive(Debug, Clone)]
pub struct RelocEntry {
    /// Type of relocation
    pub reloc_type: RelocType,
    /// Offset within the page (0-4095)
    pub offset: u16,
    /// Target address (RVA) that needs adjustment
    pub target_rva: u32,
}

/// Block of relocations for a single 4KB page
#[derive(Debug, Clone)]
pub struct RelocBlock {
    /// Page RVA (4KB aligned virtual address)
    pub page_rva: u32,
    /// Relocation entries in this page
    pub entries: Vec<RelocEntry>,
    /// Total block size in bytes
    pub block_size: u32,
}

impl RelocBlock {
    /// Create a new relocation block
    ///
    /// # Arguments
    ///
    /// * `page_rva` - Page RVA (must be 4KB aligned)
    ///
    /// # Returns
    ///
    /// `RelocBlock` - New relocation block
    pub fn new(page_rva: u32) -> Self {
        Self {
            page_rva,
            entries: Vec::new(),
            block_size: 8, // Header size (page RVA + block size)
        }
    }

    /// Add a relocation entry to this block
    ///
    /// # Arguments
    ///
    /// * `reloc_type` - Type of relocation
    /// * `offset` - Offset within page (0-4095)
    pub fn add_entry(&mut self, reloc_type: RelocType, offset: u16) {
        assert!(offset < 4096, "Offset must be within page (0-4095)");

        self.entries.push(RelocEntry {
            reloc_type,
            offset,
            target_rva: self.page_rva + offset as u32,
        });
        self.block_size += 2; // Each entry is 2 bytes
    }

    /// Apply all relocations in this block that target a specific section
    ///
    /// # Arguments
    ///
    /// * `code_data` - Mutable slice of code/data to modify
    /// * `delta` - Address difference (new_base - old_base)
    /// * `section_rva` - Virtual address of the section being modified
    ///
    /// # Returns
    ///
    /// `Result<usize>` - Number of relocations applied
    pub fn apply(
        &self,
        code_data: &mut [u8],
        delta: i64,
        section_rva: u32,
    ) -> anyhow::Result<usize> {
        let mut applied = 0;

        for entry in &self.entries {
            // Calculate section-relative offset
            let section_offset = entry.target_rva.wrapping_sub(section_rva);

            // Only apply if this relocation targets this section
            if entry.target_rva >= section_rva && section_offset < code_data.len() as u32 {
                let addr = section_offset as usize;

                match entry.reloc_type {
                    RelocType::HighLow => {
                        // 32-bit relocation
                        let current = u32::from_le_bytes([
                            code_data[addr],
                            code_data[addr + 1],
                            code_data[addr + 2],
                            code_data[addr + 3],
                        ]);

                        let relocated = (current as i64 + delta) as u32;

                        code_data[addr..addr + 4].copy_from_slice(&relocated.to_le_bytes());
                    }

                    RelocType::Dir64 => {
                        // 64-bit relocation
                        let current = u64::from_le_bytes([
                            code_data[addr],
                            code_data[addr + 1],
                            code_data[addr + 2],
                            code_data[addr + 3],
                            code_data[addr + 4],
                            code_data[addr + 5],
                            code_data[addr + 6],
                            code_data[addr + 7],
                        ]);

                        let relocated = (current as i64 + delta) as u64;

                        code_data[addr..addr + 8].copy_from_slice(&relocated.to_le_bytes());
                    }

                    RelocType::High => {
                        // High 16 bits of 32-bit field
                        let current = u16::from_le_bytes([code_data[addr], code_data[addr + 1]]);

                        let relocated = ((current as i32 + delta as i32) >> 16) as u16;

                        code_data[addr..addr + 2].copy_from_slice(&relocated.to_le_bytes());
                    }

                    RelocType::Low => {
                        // Low 16 bits of 32-bit field
                        let current = u16::from_le_bytes([code_data[addr], code_data[addr + 1]]);

                        let relocated = ((current as i32 + delta as i32) & 0xFFFF) as u16;

                        code_data[addr..addr + 2].copy_from_slice(&relocated.to_le_bytes());
                    }

                    RelocType::Rel32 => {
                        // 32-bit relative relocation (less common)
                        let current = u32::from_le_bytes([
                            code_data[addr],
                            code_data[addr + 1],
                            code_data[addr + 2],
                            code_data[addr + 3],
                        ]);

                        let relocated = (current as i64 + delta) as u32;

                        code_data[addr..addr + 4].copy_from_slice(&relocated.to_le_bytes());
                    }

                    RelocType::Absolute => {
                        // Skip absolute relocations (no action needed)
                    }

                    _ => {
                        anyhow::bail!("Unsupported relocation type: {:?}", entry.reloc_type);
                    }
                }

                applied += 1;
            }
        }

        Ok(applied)
    }
}

/// Complete relocation directory parsed from PE file
#[derive(Debug, Clone)]
pub struct RelocDirectory {
    /// All relocation blocks
    pub blocks: Vec<RelocBlock>,
    /// Total number of relocation entries
    pub entry_count: usize,
}

impl Default for RelocDirectory {
    fn default() -> Self {
        Self::new()
    }
}

impl RelocDirectory {
    /// Create empty relocation directory
    ///
    /// # Returns
    ///
    /// `RelocDirectory` - Empty directory
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            entry_count: 0,
        }
    }

    /// Parse relocation directory from PE section data
    ///
    /// # Arguments
    ///
    /// * `data` - Raw .reloc section data
    ///
    /// # Returns
    ///
    /// `Result<RelocDirectory>` - Parsed relocations or error
    pub fn parse(data: &[u8]) -> anyhow::Result<Self> {
        let mut directory = Self::new();
        let mut offset = 0;

        while offset + 8 <= data.len() {
            // Read page RVA (4 bytes)
            let page_rva = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);

            // Read block size (4 bytes)
            let block_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            // End of relocations (page_rva = 0, block_size = 0)
            if page_rva == 0 && block_size == 0 {
                break;
            }

            // Validate block size
            if block_size < 8 || block_size as usize > data.len() - offset {
                anyhow::bail!(
                    "Invalid relocation block size: {} (offset: {})",
                    block_size,
                    offset
                );
            }

            // Parse entries in this block
            let mut block = RelocBlock::new(page_rva);

            let entries_end = offset + block_size as usize;
            let mut entry_offset = offset + 8; // Skip header

            while entry_offset + 2 <= entries_end {
                // Read TypeOffset entry (2 bytes)
                let type_offset = u16::from_le_bytes([data[entry_offset], data[entry_offset + 1]]);

                // Extract type (upper 4 bits) and offset (lower 12 bits)
                let reloc_type = RelocType::from_u16(type_offset);
                let entry_offset_field = type_offset & 0xFFF;

                if reloc_type.is_valid() {
                    block.add_entry(reloc_type, entry_offset_field);
                    directory.entry_count += 1;
                }

                entry_offset += 2;
            }

            directory.blocks.push(block);

            // Move to next block
            offset += block_size as usize;

            // Align to 4-byte boundary
            offset = (offset + 3) & !3;
        }

        log::info!(
            "Parsed {} relocation blocks with {} entries",
            directory.blocks.len(),
            directory.entry_count
        );

        Ok(directory)
    }

    /// Apply all relocations to code/data
    ///
    /// # Arguments
    ///
    /// * `code_data` - Mutable slice of code/data to modify
    /// * `delta` - Address difference (new_base - old_base)
    ///
    /// # Returns
    ///
    /// `Result<()>` - Success or error
    pub fn apply(&self, code_data: &mut [u8], delta: i64, section_rva: u32) -> anyhow::Result<()> {
        log::info!(
            "Applying {} relocation blocks with delta: {:#X} for section at 0x{:X}",
            self.blocks.len(),
            delta,
            section_rva
        );

        let mut applied_count = 0;
        for block in &self.blocks {
            applied_count += block.apply(code_data, delta, section_rva)?;
        }

        log::info!("Applied {} relocations successfully", applied_count);

        Ok(())
    }

    /// Get statistics about relocations
    ///
    /// # Returns
    ///
    /// `HashMap<String, usize>` - Count of each relocation type
    pub fn get_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();

        for block in &self.blocks {
            for entry in &block.entries {
                let type_name = format!("{:?}", entry.reloc_type);
                *stats.entry(type_name).or_insert(0) += 1;
            }
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reloc_type_parsing() {
        // TypeOffset 0x3001 = HIGHLOW (type 3) at offset 1
        let type_offset = 0x3001;
        let reloc_type = RelocType::from_u16(type_offset);

        assert_eq!(reloc_type, RelocType::HighLow);
        assert!(reloc_type.is_valid());
    }

    #[test]
    fn test_absolute_reloc() {
        // Absolute reloc should not be processed
        let reloc_type = RelocType::from_u16(0x0000);
        assert_eq!(reloc_type, RelocType::Absolute);
        assert!(!reloc_type.is_valid());
    }

    #[test]
    fn test_reloc_block_creation() {
        let mut block = RelocBlock::new(0x1000);
        assert_eq!(block.page_rva, 0x1000);
        assert_eq!(block.block_size, 8);
        assert!(block.entries.is_empty());

        block.add_entry(RelocType::HighLow, 0x10);
        assert_eq!(block.entries.len(), 1);
        assert_eq!(block.entries[0].offset, 0x10);
        assert_eq!(block.entries[0].target_rva, 0x1010);
        assert_eq!(block.block_size, 10); // 8 + 2
    }

    #[test]
    fn test_highlow_relocation_application() {
        let mut code = vec![0u8; 8];
        // Store original address: 0x10001000
        code[0..4].copy_from_slice(&0x10001000u32.to_le_bytes());

        let mut block = RelocBlock::new(0x1000);
        block.add_entry(RelocType::HighLow, 0x0);

        // Apply delta: +0x1000
        block.apply(&mut code, 0x1000, 0x1000).unwrap();

        // Check relocated address: 0x10002000
        let relocated = u32::from_le_bytes([code[0], code[1], code[2], code[3]]);
        assert_eq!(relocated, 0x10002000);
    }

    #[test]
    fn test_dir64_relocation_application() {
        let mut code = vec![0u8; 16];
        // Store original address: 0x0000000100000000
        code[0..8].copy_from_slice(&0x0000000100000000u64.to_le_bytes());

        let mut block = RelocBlock::new(0x1000);
        block.add_entry(RelocType::Dir64, 0x0);

        // Apply delta: +0x1000
        block.apply(&mut code, 0x1000, 0x1000).unwrap();

        // Check relocated address: 0x0000000100001000
        let relocated = u64::from_le_bytes([
            code[0], code[1], code[2], code[3], code[4], code[5], code[6], code[7],
        ]);
        assert_eq!(relocated, 0x0000000100001000);
    }

    #[test]
    fn test_reloc_directory_parse_empty() {
        let data = [0u8; 8]; // Empty directory (terminating block)
        let dir = RelocDirectory::parse(&data).unwrap();

        assert!(dir.blocks.is_empty());
        assert_eq!(dir.entry_count, 0);
    }

    #[test]
    fn test_reloc_directory_parse_single() {
        // Single block with one HIGHLOW relocation at offset 0x10
        let mut data = vec![0u8; 16];
        // Page RVA: 0x1000
        data[0..4].copy_from_slice(&0x00001000u32.to_le_bytes());
        // Block size: 8 + 2 = 10
        data[4..8].copy_from_slice(&10u32.to_le_bytes());
        // TypeOffset: HIGHLOW (3) at offset 0x10 = 0x3010
        data[8..10].copy_from_slice(&0x3010u16.to_le_bytes());

        let dir = RelocDirectory::parse(&data).unwrap();

        assert_eq!(dir.blocks.len(), 1);
        assert_eq!(dir.blocks[0].page_rva, 0x1000);
        assert_eq!(dir.blocks[0].entries.len(), 1);
        assert_eq!(dir.blocks[0].entries[0].reloc_type, RelocType::HighLow);
        assert_eq!(dir.blocks[0].entries[0].offset, 0x10);
        assert_eq!(dir.entry_count, 1);
    }

    #[test]
    #[ignore = "Prototype test - run with: cargo test -p maxion-injector test_dll_mapper_prototype -- --ignored"]
    fn test_dll_mapper_prototype() {
        use crate::dll_loader::loader::DllStructure;
        use std::path::PathBuf;

        // Path to loader stub DLL (16KB, minimal and easier to debug)
        let dll_path = PathBuf::from("../../../target/release/maxion_loader_stub.dll");

        if !dll_path.exists() {
            println!("⚠️  DLL not found at: {}", dll_path.display());
            println!("   Run: cargo build --release -p maxion-loader-stub");
            return;
        }

        println!("📦 Loading DLL from: {}", dll_path.display());

        // Parse DLL structure (DllStructure::parse handles reading the file)
        let dll_structure = match DllStructure::parse(&dll_path) {
            Ok(structure) => {
                println!("✅ DLL parsed successfully");
                structure
            }
            Err(e) => {
                println!("❌ Failed to parse DLL: {}", e);
                return;
            }
        };

        // Print section information (Step 1: Mapper output)
        println!("\n📋 Section Analysis:");
        println!("┌─────────────┬──────────────┬──────────────┬──────────────┬───────────┐");
        println!("│ Section     │ Virtual Size │ Raw Size     │ RVA          │ Flags     │");
        println!("├─────────────┼──────────────┼──────────────┼──────────────┼───────────┤");

        let mut found_reloc = false;
        for section in &dll_structure.sections {
            println!(
                "│ {:<11} │ {:>12} │ {:>12} │ 0x{:>08X} │ 0x{:>04X} │",
                section.name,
                section.virtual_size,
                section.data.len(),
                section.virtual_address,
                section.characteristics
            );

            if section.name == ".reloc" {
                found_reloc = true;
                assert!(!section.data.is_empty(), ".reloc section should have data");
            }
        }
        println!("└─────────────┴──────────────┴──────────────┴──────────────┴───────────┘");

        assert!(found_reloc, "DLL should have .reloc section");

        // Print overall DLL info
        println!("\n🔧 DLL Information:");
        println!("   Image Base:    0x{:016X}", dll_structure.image_base);
        println!("   Entry Point:   0x{:08X}", dll_structure.entry_point);
        println!(
            "   Architecture:  {}",
            if dll_structure.is_64bit { "x64" } else { "x86" }
        );
        println!(
            "   Section Align: 0x{:08X}",
            dll_structure.section_alignment
        );
        println!("   File Align:    0x{:08X}", dll_structure.file_alignment);
        println!("   Total Sections: {}", dll_structure.sections.len());

        // Print import information
        println!("\n📦 Import Analysis:");
        if let Some(ref import_dir) = dll_structure.import_directory {
            println!(
                "   DLL imports {} functions from {} modules",
                import_dir.total_imports,
                import_dir.descriptors.len()
            );
            for desc in &import_dir.descriptors {
                println!("      - {}", desc.name);
            }
        } else {
            println!("   ⚠️  No import directory found");
        }

        // Print relocation information (Step 1 verification: confirm .reloc exists)
        println!("\n🔄 Relocation Analysis:");
        if let Some(ref reloc_dir) = dll_structure.relocations {
            let reloc_stats = reloc_dir.get_stats();
            println!("   Relocation Blocks: {}", reloc_dir.blocks.len());

            let total_relocations: usize = reloc_stats.values().sum();
            println!("   Total Relocations: {}", total_relocations);

            if total_relocations > 0 {
                println!("   ✅ Relocations found - DLL can be embedded at new base address");
            } else {
                println!("   ⚠️  No relocations - DLL must be loaded at preferred base address");
            }
        } else {
            println!("   ⚠️  No relocation directory found");
        }

        // Calculate total memory size (for embedding)
        let total_size = dll_structure.calculate_total_size();
        println!("\n📏 Memory Layout:");
        println!(
            "   Total Size (aligned): {} bytes (0x{:X})",
            total_size, total_size
        );
        println!(
            "   Code size (.text):   {} bytes",
            dll_structure
                .get_section(".text")
                .map(|s| s.data.len())
                .unwrap_or(0)
        );

        println!("\n✅ Prototype test complete - Mapper step validated");
    }
}
