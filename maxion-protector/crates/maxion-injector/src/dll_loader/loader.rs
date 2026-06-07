//! DLL Loader for Manual Embedding
//!
//! This module provides the core functionality for embedding a complete DLL
//! into a PE file and manually loading it at runtime. This is the
//! production-grade fix (Option B) that achieves true single-file protection.
//!
//! # Overview
//!
//! When embedding a DLL into a PE file, we cannot rely on Windows loader
//! because the DLL is not a separate file. Instead, we must:
//!
//! 1. **Parse DLL structure** - Extract all sections, imports, relocations
//! 2. **Embed sections** - Copy all sections to new locations in target PE
//! 3. **Apply relocations** - Adjust absolute addresses for new base address
//! 4. **Resolve imports** - Patch Import Address Table with real function addresses
//! 5. **Jump to entry** - Call DLL's stub_entry function
//!
//! # Injection Flow
//!
//! ```text
//! Original DLL                        Target PE
//! ┌─────────────┐                 ┌─────────────┐
//! │ .text       │    Copy        │ .text       │
//! │ .data       │  ─────────→   │ .data       │
//! │ .rdata      │                 │ .rdata      │
//! │ .idata       │                 │ .idata       │ ← Patches needed
//! │ .reloc      │                 │ .reloc      │ ← Apply relocations
//! │ .pdata      │                 │ .pdata      │
//! └─────────────┘                 └─────────────┘
//!  Base: 0x10000000                Base: 0x140000000
//!  (DLL's preferred)               (Target PE's base)
//! ```
//!
//! # Memory Layout
//!
//! After injection, the DLL sections are remapped:
//!
//! ```text
//! Target PE Memory Layout (after protection)
//! ┌─────────────────────────────┐
//! │ Original PE Sections        │ ← Unchanged
//! │ (.text, .data, .rsrc)    │
//! ├─────────────────────────────┤
//! │ Embedded .maxion section    │ ← Encrypted assets
//! ├─────────────────────────────┤
//! │ Embedded DLL .text         │ ← Stub code
//! │ (relocated)              │
//! ├─────────────────────────────┤
//! │ Embedded DLL .data         │ ← Global variables
//! │ (relocated)              │
//! ├─────────────────────────────┤
//! │ Embedded DLL .rdata        │ ← String literals
//! │ (relocated)              │
//! ├─────────────────────────────┤
//! │ Embedded DLL .idata        │ ← Import table (patched)
//! │ (IAT patched)            │
//! ├─────────────────────────────┤
//! │ Embedded DLL .reloc        │ ← Unused after relocation
//! │ (applied, discarded)      │
//! └─────────────────────────────┘
//! ```

use super::import::ImportDirectory;
use super::RelocDirectory;
use anyhow::{Context, Result};
use goblin::pe::PE;
use log::{debug, info, warn};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Section data extracted from DLL
#[derive(Debug, Clone)]
pub struct DllSection {
    /// Section name (8 bytes, null-terminated)
    pub name: String,
    /// Virtual address where section should be loaded
    pub virtual_address: u32,
    /// Size in memory (with alignment)
    pub virtual_size: u32,
    /// Raw data bytes from DLL file
    pub data: Vec<u8>,
    /// Section characteristics (flags)
    pub characteristics: u32,
}

impl DllSection {
    /// Create new DLL section
    ///
    /// # Arguments
    ///
    /// * `name` - Section name
    /// * `virtual_address` - RVA for loading
    /// * `virtual_size` - Size in memory
    /// * `characteristics` - Section flags
    ///
    /// # Returns
    ///
    /// `DllSection` - New section structure
    pub fn new(name: &str, virtual_address: u32, virtual_size: u32, characteristics: u32) -> Self {
        Self {
            name: name.to_string(),
            virtual_address,
            virtual_size,
            data: Vec::new(),
            characteristics,
        }
    }

    /// Check if section contains executable code
    ///
    /// # Returns
    ///
    /// `bool` - True if executable
    pub fn is_executable(&self) -> bool {
        self.characteristics & 0x20000000 != 0 // IMAGE_SCN_MEM_EXECUTE
    }

    /// Check if section is readable
    ///
    /// # Returns
    ///
    /// `bool` - True if readable
    pub fn is_readable(&self) -> bool {
        self.characteristics & 0x40000000 != 0 // IMAGE_SCN_MEM_READ
    }

    /// Check if section is writable
    ///
    /// # Returns
    ///
    /// `bool` - True if writable
    pub fn is_writable(&self) -> bool {
        self.characteristics & 0x80000000 != 0 // IMAGE_SCN_MEM_WRITE
    }
}

/// Complete DLL structure parsed for embedding
#[derive(Debug, Clone)]
pub struct DllStructure {
    /// All sections extracted from DLL
    pub sections: Vec<DllSection>,
    /// Import directory (for resolution)
    pub import_directory: Option<ImportDirectory>,
    /// Relocation directory (for address adjustment)
    pub relocations: Option<RelocDirectory>,
    /// Original entry point RVA (for stub_entry)
    pub entry_point: u32,
    /// Preferred image base address
    pub image_base: u64,
    /// Is this a 64-bit DLL?
    pub is_64bit: bool,
    /// Section alignment
    pub section_alignment: u32,
    /// File alignment
    pub file_alignment: u32,
}

impl DllStructure {
    /// Parse DLL file into embeddable structure
    ///
    /// # Arguments
    ///
    /// * `dll_path` - Path to compiled maxion_stub.dll
    ///
    /// # Returns
    ///
    /// `Result<DllStructure>` - Parsed structure or error
    pub fn parse<P: AsRef<Path>>(dll_path: P) -> Result<Self> {
        let path = dll_path.as_ref();
        info!("Parsing DLL: {}", path.display());

        // Read entire DLL file
        let mut file =
            File::open(path).with_context(|| format!("Failed to open DLL: {}", path.display()))?;

        let mut dll_data = Vec::new();
        file.read_to_end(&mut dll_data)
            .with_context(|| "Failed to read DLL file")?;

        // Parse PE structure
        let pe = PE::parse(&dll_data).with_context(|| "Failed to parse PE structure")?;

        info!(
            "DLL parsed: {} sections, entry at 0x{:X}",
            pe.sections.len(),
            pe.entry
        );

        // Extract sections
        let sections = Self::extract_sections(&pe, &dll_data)?;

        // Parse import directory
        let import_directory = Self::parse_imports(&pe, &dll_data)?;

        // Parse relocation directory
        let relocations = Self::parse_relocations(&pe, &dll_data)?;

        // Get PE characteristics
        let is_64bit = pe.is_64;
        let image_base = pe.image_base;
        let section_alignment = pe
            .header
            .optional_header
            .as_ref()
            .map(|h| h.windows_fields.section_alignment)
            .unwrap_or(0x1000);
        let file_alignment = pe
            .header
            .optional_header
            .as_ref()
            .map(|h| h.windows_fields.file_alignment)
            .unwrap_or(0x200);

        debug!("DLL properties:");
        debug!("  64-bit: {}", is_64bit);
        debug!("  Image base: 0x{:X}", image_base);
        debug!("  Section alignment: 0x{:X}", section_alignment);
        debug!("  File alignment: 0x{:X}", file_alignment);

        Ok(Self {
            sections,
            import_directory,
            relocations,
            entry_point: pe.entry,
            image_base,
            is_64bit,
            section_alignment,
            file_alignment,
        })
    }

    /// Extract all sections from DLL
    ///
    /// # Arguments
    ///
    /// * `pe` - Parsed PE structure
    /// * `dll_data` - Raw DLL file data
    ///
    /// # Returns
    ///
    /// `Result<Vec<DllSection>>` - Extracted sections or error
    fn extract_sections(pe: &PE, dll_data: &[u8]) -> Result<Vec<DllSection>> {
        let mut sections = Vec::new();

        for section in &pe.sections {
            // Get section name (8 bytes, padded with nulls)
            let name_bytes = section.name.to_vec();
            let name = String::from_utf8(name_bytes)
                .map(|s| s.trim_matches('\0').to_string())
                .unwrap_or_else(|_| format!("section_{}", sections.len()));

            // Get section data
            let start = section.pointer_to_raw_data as usize;
            let size = section.size_of_raw_data as usize;

            if start + size > dll_data.len() {
                warn!(
                    "Section {} extends beyond file (start={}, size={}, file_len={})",
                    name,
                    start,
                    size,
                    dll_data.len()
                );
                continue;
            }

            let data = dll_data[start..start + size].to_vec();

            debug!(
                "Extracted section: {} (VA=0x{:X}, size={})",
                name,
                section.virtual_address,
                data.len()
            );

            sections.push(DllSection {
                name,
                virtual_address: section.virtual_address,
                virtual_size: section.virtual_size,
                data,
                characteristics: section.characteristics,
            });
        }

        info!("Extracted {} sections from DLL", sections.len());
        Ok(sections)
    }

    /// Convert RVA to file offset using section mapping
    ///
    /// # Arguments
    ///
    /// * `pe` - Parsed PE structure
    /// * `rva` - Relative Virtual Address
    ///
    /// # Returns
    ///
    /// `Option<usize>` - File offset if RVA maps to a section, None otherwise
    pub fn rva_to_offset(pe: &PE, rva: u32) -> Option<usize> {
        for section in &pe.sections {
            let section_va = section.virtual_address;
            let section_size = section.virtual_size.max(section.size_of_raw_data);

            // Check if RVA falls within this section
            if rva >= section_va && rva < section_va + section_size {
                let offset = (rva - section_va) as usize;
                let file_offset = section.pointer_to_raw_data as usize + offset;
                return Some(file_offset);
            }
        }
        None
    }

    /// Parse import directory from DLL
    ///
    /// # Arguments
    ///
    /// * `pe` - Parsed PE structure
    /// * `dll_data` - Raw DLL file data
    ///
    /// # Returns
    ///
    /// `Option<ImportDirectory>` - Parsed imports or None if not found
    fn parse_imports(pe: &PE, dll_data: &[u8]) -> Result<Option<ImportDirectory>> {
        let import_dir_rva = if let Some(opt) = &pe.header.optional_header {
            opt.data_directories
                .get_import_table()
                .map(|d| d.virtual_address)
                .unwrap_or(0)
        } else {
            0
        };

        if import_dir_rva == 0 {
            info!("DLL has no import directory (static linking)");
            return Ok(None);
        }

        info!("Parsing import directory at RVA 0x{:X}", import_dir_rva);

        // Convert RVA to file offset
        let import_dir_offset = Self::rva_to_offset(pe, import_dir_rva).ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot map import directory RVA 0x{:X} to file offset",
                import_dir_rva
            )
        })?;

        let import_dir = ImportDirectory::parse(dll_data, pe, import_dir_offset as u32, pe.is_64)?;

        Ok(Some(import_dir))
    }

    /// Parse relocation directory from DLL
    ///
    /// # Arguments
    ///
    /// * `pe` - Parsed PE structure
    /// * `dll_data` - Raw DLL file data
    ///
    /// # Returns
    ///
    /// `Option<RelocDirectory>` - Parsed relocations or None if not found
    fn parse_relocations(pe: &PE, dll_data: &[u8]) -> Result<Option<RelocDirectory>> {
        // Relocation RVA is in DataDirectory[5] of Optional Header
        let reloc_dir_rva = if let Some(opt) = &pe.header.optional_header {
            opt.data_directories
                .get_base_relocation_table()
                .map(|d| d.virtual_address)
                .unwrap_or(0)
        } else {
            0
        };

        if reloc_dir_rva == 0 {
            info!("DLL has no relocation directory (no relocations needed)");
            return Ok(None);
        }

        info!("Parsing relocation directory at RVA 0x{:X}", reloc_dir_rva);

        // Use rva_to_offset to find the relocation data
        let reloc_offset = match Self::rva_to_offset(pe, reloc_dir_rva) {
            Some(offset) => offset,
            None => {
                warn!("Cannot map relocation directory RVA to file offset");
                return Ok(None);
            }
        };

        // Relocation data is in .reloc section
        // Find .reloc section to get size
        let reloc_section = pe.sections.iter().find(|s| s.name.starts_with(b".reloc"));

        if let Some(section) = reloc_section {
            let start = reloc_offset;
            let size = section.size_of_raw_data as usize;

            if start + size > dll_data.len() {
                warn!("Relocation section extends beyond file");
                return Ok(None);
            }

            let reloc_data = &dll_data[start..start + size];
            let relocations = RelocDirectory::parse(reloc_data)?;

            Ok(Some(relocations))
        } else {
            warn!("No .reloc section found in DLL");
            Ok(None)
        }
    }

    /// Get section by name
    ///
    /// # Arguments
    ///
    /// * `name` - Section name (without leading dot)
    ///
    /// # Returns
    ///
    /// `Option<&DllSection>` - Section if found
    pub fn get_section(&self, name: &str) -> Option<&DllSection> {
        let name_with_dot = format!(".{}", name);
        self.sections
            .iter()
            .find(|s| s.name == name_with_dot || s.name == name)
    }

    /// Calculate total size needed for all sections
    ///
    /// # Returns
    ///
    /// `u32` - Total virtual size (with alignment)
    pub fn calculate_total_size(&self) -> u32 {
        let mut total = 0u32;

        for section in &self.sections {
            // Align to section alignment
            let aligned =
                section.virtual_size.div_ceil(self.section_alignment) * self.section_alignment;
            total += aligned;
        }

        total
    }

    /// Get statistics about DLL structure
    ///
    /// # Returns
    ///
    /// `HashMap<String, String>` - Various statistics
    pub fn get_stats(&self) -> HashMap<String, String> {
        let mut stats = HashMap::new();

        stats.insert("sections".to_string(), self.sections.len().to_string());
        stats.insert(
            "entry_point".to_string(),
            format!("0x{:X}", self.entry_point),
        );
        stats.insert("image_base".to_string(), format!("0x{:X}", self.image_base));
        stats.insert("is_64bit".to_string(), self.is_64bit.to_string());
        stats.insert(
            "total_size".to_string(),
            format!("{} bytes", self.calculate_total_size()),
        );

        // Import stats
        if let Some(ref imports) = self.import_directory {
            stats.insert(
                "import_dlls".to_string(),
                imports.descriptors.len().to_string(),
            );
            stats.insert(
                "total_imports".to_string(),
                imports.total_imports.to_string(),
            );
        }

        // Relocation stats
        if let Some(ref relocs) = self.relocations {
            stats.insert("reloc_blocks".to_string(), relocs.blocks.len().to_string());
            stats.insert("reloc_entries".to_string(), relocs.entry_count.to_string());
        }

        stats
    }
}

/// DLL Injector - Manually loads embedded DLL
///
/// This structure handles the runtime loading of embedded DLL sections,
/// applying relocations and resolving imports before calling stub_entry.
pub struct DllInjector {
    /// Parsed DLL structure
    dll_structure: DllStructure,
    /// New base address where DLL will be loaded
    new_base: u64,
    /// Section data mapped to new addresses
    sections_data: HashMap<String, Vec<u8>>,
}

impl DllInjector {
    /// Create new DLL injector
    ///
    /// # Arguments
    ///
    /// * `dll_structure` - Parsed DLL structure
    /// * `new_base` - New base address in target PE
    ///
    /// # Returns
    ///
    /// `DllInjector` - New injector
    pub fn new(dll_structure: DllStructure, new_base: u64) -> Self {
        Self {
            dll_structure,
            new_base,
            sections_data: HashMap::new(),
        }
    }

    /// Map all sections to new memory layout
    ///
    /// This function:
    /// 1. Allocates memory for each section at new addresses
    /// 2. Copies section data to new locations
    /// 3. Applies relocations (if available)
    /// 4. Resolves imports (if available)
    ///
    /// # Returns
    ///
    /// `Result<()>` - Success or error
    pub fn map_sections(&mut self) -> Result<()> {
        info!("Mapping {} DLL sections", self.dll_structure.sections.len());

        let delta = self.new_base as i64 - self.dll_structure.image_base as i64;
        info!("Address delta: {:#X} (new_base - old_base)", delta);

        // Step 1: Copy all sections to new locations
        for section in &self.dll_structure.sections {
            let mut section_data = section.data.clone();

            // Pad to virtual size
            while section_data.len() < section.virtual_size as usize {
                section_data.push(0);
            }

            let data_len = section_data.len();

            self.sections_data
                .insert(section.name.clone(), section_data);

            debug!(
                "Mapped section: {} at VA 0x{:X} ({} bytes)",
                section.name,
                self.new_base + section.virtual_address as u64,
                data_len
            );
        }

        // Step 2: Apply relocations to all sections
        if let Some(ref relocations) = self.dll_structure.relocations {
            info!("Applying relocations to DLL sections...");

            // Apply relocations to each section's data
            for section in &self.dll_structure.sections {
                if let Some(data) = self.sections_data.get_mut(&section.name) {
                    relocations.apply(data, delta, section.virtual_address)?;
                    info!("Applied relocations to section {}", section.name);
                }
            }
        } else {
            info!("No relocations to apply");
        }

        // Step 3: Resolve imports (patch IAT) - Windows only
        #[cfg(windows)]
        if let Some(ref imports) = self.dll_structure.import_directory {
            info!("Resolving imports in DLL...");

            // Resolve imports and patch IAT in .idata section
            if let Some(idata) = self.sections_data.get_mut(".idata") {
                imports.resolve_and_patch(
                    idata,
                    imports.get_iat_rvas().to_vec(),
                    self.dll_structure.is_64bit,
                )?;
                info!("Resolved {} imports", imports.total_imports);
            } else {
                warn!("No .idata section found for import resolution");
            }
        }

        #[cfg(not(windows))]
        {
            if self.dll_structure.import_directory.is_some() {
                info!("Import resolution skipped (non-Windows platform)");
            } else {
                info!("No imports to resolve");
            }
        }

        Ok(())
    }

    /// Get entry point function address
    ///
    /// Calculates the actual address of stub_entry in the embedded DLL.
    ///
    /// # Returns
    ///
    /// `Result<u64>` - Entry point address in memory
    pub fn get_entry_point(&self) -> Result<u64> {
        let entry_rva = self.dll_structure.entry_point;

        if entry_rva == 0 {
            anyhow::bail!("DLL has no entry point");
        }

        let entry_addr = self.new_base + entry_rva as u64;

        info!(
            "Entry point: RVA 0x{:X} -> VA 0x{:X}",
            entry_rva, entry_addr
        );

        Ok(entry_addr)
    }

    /// Get section data for embedding
    ///
    /// # Arguments
    ///
    /// * `section_name` - Section name (e.g., ".text", ".data")
    ///
    /// # Returns
    ///
    /// `Option<&[u8]>` - Section data if found
    pub fn get_section_data(&self, section_name: &str) -> Option<&[u8]> {
        self.sections_data.get(section_name).map(|v| v.as_slice())
    }

    /// Get all section names
    ///
    /// # Returns
    ///
    /// `Vec<String>` - List of section names
    pub fn get_section_names(&self) -> Vec<String> {
        self.sections_data.keys().cloned().collect()
    }

    /// Calculate section layout for embedding into target PE
    ///
    /// Calculates where each embedded section should be placed in the
    /// target PE file, maintaining proper alignment.
    ///
    /// # Arguments
    ///
    /// * `last_rva` - Last section RVA in target PE
    /// * `last_offset` - Last section file offset in target PE
    /// * `section_alignment` - Section alignment for target PE
    ///
    /// # Returns
    ///
    /// `HashMap<String, (u32, u32)>` - Map of (section_name -> (rva, offset))
    pub fn calculate_layout(
        &self,
        last_rva: u32,
        last_offset: u32,
        section_alignment: u32,
    ) -> HashMap<String, (u32, u32)> {
        let mut layout = HashMap::new();
        let mut current_rva = last_rva;
        let mut current_offset = last_offset;

        for section in &self.dll_structure.sections {
            // Align RVA to section alignment
            let aligned_rva = current_rva.div_ceil(section_alignment) * section_alignment;

            // Align file offset to 512 bytes (typical PE file alignment)
            let file_alignment = 512u32;
            let aligned_offset = current_offset.div_ceil(file_alignment) * file_alignment;

            layout.insert(section.name.clone(), (aligned_rva, aligned_offset));

            debug!(
                "Section {} layout: RVA=0x{:X}, Offset=0x{:X}",
                section.name, aligned_rva, aligned_offset
            );

            // Advance to next section
            current_rva = aligned_rva + section.virtual_size;
            current_offset = aligned_offset + section.data.len() as u32;
        }

        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    #[test]
    #[ignore] // Requires actual DLL file
    fn test_dll_parsing() {
        // This test requires an actual maxion_stub.dll file
        let dll_path = PathBuf::from("target/release/maxion_stub.dll");

        if !dll_path.exists() {
            println!("Skipping test: {} not found", dll_path.display());
            return;
        }

        let dll_structure = DllStructure::parse(&dll_path).unwrap();

        assert!(!dll_structure.sections.is_empty());
        assert_ne!(dll_structure.entry_point, 0);

        // Check for essential sections
        assert!(dll_structure.get_section(".text").is_some());
        assert!(dll_structure.get_section(".data").is_some());

        println!(
            "DLL parsed successfully: {} sections",
            dll_structure.sections.len()
        );
        println!("Entry point: 0x{:X}", dll_structure.entry_point);
    }

    #[test]
    fn test_dll_section_properties() {
        let section = DllSection::new(
            ".text", 0x1000, 0x2000, 0x60000020, // Execute + Read
        );

        assert!(section.is_executable());
        assert!(section.is_readable());
        assert!(!section.is_writable());
    }

    #[test]
    fn test_dll_injector_creation() {
        let sections = vec![
            // Create a mock .text section
            DllSection {
                name: ".text".to_string(),
                virtual_address: 0x1000,
                virtual_size: 0x1000,
                data: vec![0xCC; 0x1000],    // INT3 instructions
                characteristics: 0x60000020, // Execute + Read
            },
            // Create a mock .data section
            DllSection {
                name: ".data".to_string(),
                virtual_address: 0x2000,
                virtual_size: 0x1000,
                data: vec![0u8; 0x1000],
                characteristics: 0xC0000040, // Read + Write
            },
        ];

        let dll_structure = DllStructure {
            sections,
            import_directory: None,
            relocations: None,
            entry_point: 0x1000,
            image_base: 0x10000000,
            is_64bit: true,
            section_alignment: 0x1000,
            file_alignment: 0x200,
        };

        // Create injector with new base address
        let new_base = 0x140000000u64;
        let mut injector = DllInjector::new(dll_structure, new_base);

        // Map sections (no relocations in this test)
        injector.map_sections().unwrap();

        // Verify entry point
        let entry = injector.get_entry_point().unwrap();
        assert_eq!(entry, 0x140001000);

        // Verify sections mapped
        assert!(injector.get_section_data(".text").is_some());
        assert!(injector.get_section_data(".data").is_some());
    }

    #[test]
    fn test_calculate_total_size() {
        let sections = vec![
            // Section 1: 0x1000 bytes (4KB)
            DllSection {
                name: ".text".to_string(),
                virtual_address: 0x1000,
                virtual_size: 0x1000,
                data: vec![0u8; 0x1000],
                characteristics: 0x60000020,
            },
            // Section 2: 0x1500 bytes (aligned to 0x2000)
            DllSection {
                name: ".data".to_string(),
                virtual_address: 0x2000,
                virtual_size: 0x1500,
                data: vec![0u8; 0x1500],
                characteristics: 0xC0000040,
            },
        ];

        let dll_structure = DllStructure {
            sections,
            import_directory: None,
            relocations: None,
            entry_point: 0x1000,
            image_base: 0x10000000,
            is_64bit: true,
            section_alignment: 0x1000,
            file_alignment: 0x200,
        };

        // Calculate total size
        // Section 1: 0x1000 (already aligned)
        // Section 2: 0x1500 -> aligned to 0x2000
        // Total: 0x1000 + 0x2000 = 0x3000
        let total = dll_structure.calculate_total_size();
        assert_eq!(total, 0x3000);
    }

    #[test]
    fn test_layout_calculation() {
        let sections = vec![
            DllSection {
                name: ".text".to_string(),
                virtual_address: 0x1000,
                virtual_size: 0x1000,
                data: vec![0u8; 0x1000],
                characteristics: 0x60000020,
            },
            DllSection {
                name: ".data".to_string(),
                virtual_address: 0x2000,
                virtual_size: 0x1000,
                data: vec![0u8; 0x1000],
                characteristics: 0xC0000040,
            },
        ];

        let dll_structure = DllStructure {
            sections,
            import_directory: None,
            relocations: None,
            entry_point: 0x1000,
            image_base: 0x10000000,
            is_64bit: true,
            section_alignment: 0x1000,
            file_alignment: 0x200,
        };

        let mut injector = DllInjector::new(dll_structure, 0x140000000);
        injector.map_sections().unwrap();

        // Calculate layout
        let layout = injector.calculate_layout(0x5000, 0x4000, 0x1000);

        // First section (.text)
        let text_layout = layout.get(".text").unwrap();
        assert_eq!(text_layout.0, 0x5000); // RVA
        assert_eq!(text_layout.1, 0x4000); // Offset

        // Second section (.data)
        let data_layout = layout.get(".data").unwrap();
        // After .text (0x1000), aligned to 0x1000: 0x5000 + 0x1000 = 0x6000
        assert_eq!(data_layout.0, 0x6000);
        // After .text offset (0x1000), aligned to 0x200: 0x4000 + 0x1000 = 0x5000
        assert_eq!(data_layout.1, 0x5000);
    }
}
