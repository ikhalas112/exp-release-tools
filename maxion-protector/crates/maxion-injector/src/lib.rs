//! Maxion PE Injector
//!
//! This module provides functionality for injecting runtime stub and
//! encrypted assets into Windows PE executables. It enables creation of
//! single, protected game executables that contain all necessary components.
//!
//! # Architecture
//!
//! ```text
//! Original PE                 Protected PE
//! ┌─────────────┐            ┌─────────────┐
//! │   .text     │            │   .text     │
//! │   .data     │            │   .data     │
//! │   .rsrc     │    +       │   .rsrc     │
//! └─────────────┘            │  .maxion    │ ← Encrypted archive
//!                            │  .stub      │ ← Injected stub code
//!                            │  .key       │ ← Encryption key
//!                            │  .dll_text  │ ← Embedded DLL .text (relocated)
//!                            │  .dll_data  │ ← Embedded DLL .data (relocated)
//!                            │  .dll_idata │ ← Embedded DLL .idata (patches applied)
//!                            └─────────────┘
//! ```
//!
//! # Injection Process
//!
//! 1. Parse original PE file
//! 2. Parse DLL structure (sections, imports, relocations)
//! 3. Create new sections (.maxion, .stub, .key, .dll_*)
//! 4. Map DLL sections to new addresses with relocations
//! 5. Resolve DLL imports and patch IAT
//! 6. Inject stub code into .stub section
//! 7. Embed encrypted archive into .maxion section
//! 8. Store encryption key in .key section (obfuscated)
//! 9. Modify entry point to call stub initialization
//! 10. Update PE headers and write protected executable

#[cfg(feature = "phase2")]
pub mod dll_loader;

use anyhow::{Context, Result};
use goblin::pe::PE;
use log::{debug, info, warn};
use maxion_core::{error::Error, MAGIC};
use memmap2::Mmap;
use rand::RngCore;
use std::fs::File;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::path::Path;
use std::path::PathBuf;

#[cfg(stub_compiled)]
include!(concat!(env!("OUT_DIR"), "/stub_embed.rs"));

#[cfg(feature = "phase2")]
use self::dll_loader::loader::{DllInjector, DllStructure};

/// Alignment for PE sections (must match PE specification)
const PE_SECTION_ALIGNMENT: u32 = 0x1000;

/// Alignment for file alignment in PE
const PE_FILE_ALIGNMENT: u32 = 0x200;

/// Size of PE section header
const SECTION_HEADER_SIZE: usize = 40;

/// Maximum size for stub code (16KB for DLL-based stub)
const MAX_STUB_SIZE: u32 = 16384;

/// Maximum size for encrypted key storage (256 bytes)
const MAX_KEY_SIZE: u32 = 256;
const KEY_BLOB_V2_MAGIC: &[u8; 4] = b"MXK2";
const KEY_BLOB_V3_MAGIC: &[u8; 4] = b"MXK3";
const KEY_BLOB_V2_SCHEME_COUNT: u8 = 3;

/// Stub loader for injecting runtime stub code
///
/// This structure handles loading and providing the compiled stub binary
/// that will be embedded into the protected PE file.
#[derive(Debug, Clone)]
pub struct StubLoader {
    /// Stub binary data
    data: Vec<u8>,
}

impl StubLoader {
    /// Create a new stub loader from binary data
    ///
    /// # Arguments
    ///
    /// * `data` - Compiled stub binary data
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Get stub binary data
    ///
    /// # Returns
    ///
    /// `&[u8]` - Stub binary data
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Get stub size
    ///
    /// # Returns
    ///
    /// `usize` - Size of stub binary in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Load stub from embedded bytes (compile-time)
    ///
    /// This is used when the stub is compiled into the packer binary.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Static byte slice containing stub binary
    pub fn from_embedded(bytes: &'static [u8]) -> Self {
        Self::new(bytes.to_vec())
    }
}

/// PE Injector for creating protected executables
///
/// This struct handles the complete injection process:
/// - Parsing PE files
/// - Creating new sections
/// - Injecting stub code
/// - Embedding encrypted assets
/// - Modifying entry points
#[allow(dead_code)]
pub struct PeInjector {
    /// Original PE file path
    pe_path: PathBuf,

    /// Protected PE file path (output)
    protected_path: PathBuf,

    /// Encrypted archive data to embed
    archive_data: Vec<u8>,

    /// Encryption key to store (obfuscated)
    encryption_key: [u8; 32],

    /// Nonce for encryption
    nonce: [u8; 24],

    /// Chunk size used for encryption
    chunk_size: u32,

    /// Stub loader for injecting runtime stub code
    stub_loader: Option<StubLoader>,

    /// Path to stub DLL (Phase 1 - Quick Fix)
    stub_dll_path: Option<PathBuf>,

    /// DLL structure for full embedding (Phase 2 - Production)
    #[cfg(feature = "phase2")]
    dll_structure: Option<DllStructure>,
}

/// PE section information for injection
#[derive(Debug, Clone)]
struct SectionInfo {
    /// Section name (8 bytes, null-padded)
    name: [u8; 8],

    /// Virtual size in memory
    virtual_size: u32,

    /// Virtual address (relative to image base)
    virtual_address: u32,

    /// Raw data size in file
    size_of_raw_data: u32,

    /// File offset to raw data
    pointer_to_raw_data: u32,

    /// Section characteristics (flags)
    characteristics: u32,
}

/// Type alias for complex return type to improve readability
type NewSectionsResult = Result<(Vec<(SectionInfo, Vec<u8>)>, u32)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PeKind {
    Pe32,
    Pe32Plus,
}

fn detect_pe_kind(optional_header: &[u8]) -> Result<PeKind> {
    if optional_header.len() < 2 {
        return Err(Error::Other("Optional header too short".to_string()).into());
    }

    let magic = u16::from_le_bytes([optional_header[0], optional_header[1]]);
    match magic {
        0x10b => Ok(PeKind::Pe32),
        0x20b => Ok(PeKind::Pe32Plus),
        _ => Err(Error::Other(format!(
            "Unsupported PE optional header magic=0x{:X} (expected 0x10B or 0x20B)",
            magic
        ))
        .into()),
    }
}

/// Helper function to serialize a goblin Section to PE section header bytes
///
/// # Arguments
///
/// * `section` - Reference to goblin PE section
///
/// # Returns
///
/// `Vec<u8>` - Serialized section header in PE format
fn section_to_bytes(section: &goblin::pe::section_table::SectionTable) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(SECTION_HEADER_SIZE);

    // Name (8 bytes)
    bytes.extend_from_slice(&section.name);

    // VirtualSize (4 bytes)
    bytes.extend_from_slice(&section.virtual_size.to_le_bytes());

    // VirtualAddress (4 bytes)
    bytes.extend_from_slice(&section.virtual_address.to_le_bytes());

    // SizeOfRawData (4 bytes)
    bytes.extend_from_slice(&section.size_of_raw_data.to_le_bytes());

    // PointerToRawData (4 bytes)
    bytes.extend_from_slice(&section.pointer_to_raw_data.to_le_bytes());

    // PointerToRelocations (4 bytes) - always 0
    bytes.extend_from_slice(&[0u8; 4]);

    // PointerToLinenumbers (4 bytes) - always 0
    bytes.extend_from_slice(&[0u8; 4]);

    // NumberOfRelocations (2 bytes) - always 0
    bytes.extend_from_slice(&[0u8; 2]);

    // NumberOfLinenumbers (2 bytes) - always 0
    bytes.extend_from_slice(&[0u8; 2]);

    // Characteristics (4 bytes)
    bytes.extend_from_slice(&section.characteristics.to_le_bytes());

    assert_eq!(bytes.len(), SECTION_HEADER_SIZE);
    bytes
}

impl SectionInfo {
    /// Create a new section info structure
    ///
    /// # Arguments
    ///
    /// * `name` - Section name (max 8 bytes)
    /// * `virtual_size` - Virtual size (already aligned to section alignment 0x1000)
    /// * `raw_size` - Raw file size (already aligned to file alignment 0x200)
    /// * `characteristics` - Section flags
    /// * `virtual_address` - Virtual address in memory
    /// * `raw_offset` - File offset
    fn new(
        name: &str,
        virtual_size: u32,
        raw_size: u32,
        characteristics: u32,
        virtual_address: u32,
        raw_offset: u32,
    ) -> Self {
        let mut name_bytes = [0u8; 8];
        // Copy name bytes up to 8 characters
        let name_len = name.len().min(8);
        name_bytes[..name_len].copy_from_slice(&name.as_bytes()[..name_len]);

        Self {
            name: name_bytes,
            virtual_size,
            virtual_address,
            size_of_raw_data: raw_size,
            pointer_to_raw_data: raw_offset,
            characteristics,
        }
    }

    /// Serialize section info to bytes (PE section header format)
    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(SECTION_HEADER_SIZE);

        // Name (8 bytes)
        bytes.extend_from_slice(&self.name);

        // VirtualSize (4 bytes)
        bytes.extend_from_slice(&self.virtual_size.to_le_bytes());

        // VirtualAddress (4 bytes)
        bytes.extend_from_slice(&self.virtual_address.to_le_bytes());

        // SizeOfRawData (4 bytes)
        bytes.extend_from_slice(&self.size_of_raw_data.to_le_bytes());

        // PointerToRawData (4 bytes)
        bytes.extend_from_slice(&self.pointer_to_raw_data.to_le_bytes());

        // PointerToRelocations (4 bytes) - always 0
        bytes.extend_from_slice(&[0u8; 4]);

        // PointerToLinenumbers (4 bytes) - always 0
        bytes.extend_from_slice(&[0u8; 4]);

        // NumberOfRelocations (2 bytes) - always 0
        bytes.extend_from_slice(&[0u8; 2]);

        // NumberOfLinenumbers (2 bytes) - always 0
        bytes.extend_from_slice(&[0u8; 2]);

        // Characteristics (4 bytes)
        bytes.extend_from_slice(&self.characteristics.to_le_bytes());

        bytes
    }
}

/// Section characteristics flags
mod section_flags {
    /// Code section (executable)
    pub const IMAGE_SCN_CNT_CODE: u32 = 0x00000020;

    /// Initialized data
    pub const IMAGE_SCN_CNT_INITIALIZED_DATA: u32 = 0x00000040;

    /// Section is readable
    pub const IMAGE_SCN_MEM_READ: u32 = 0x40000000;

    /// Section is writable
    #[allow(dead_code)]
    pub const IMAGE_SCN_MEM_WRITE: u32 = 0x80000000;

    /// Section is executable
    pub const IMAGE_SCN_MEM_EXECUTE: u32 = 0x20000000;

    /// Standard readable data section
    pub const DATA: u32 = IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ;

    /// Alias for DATA (readable)
    #[allow(dead_code)]
    pub const DATA_READ: u32 = DATA;

    /// Writable data section
    #[allow(dead_code)]
    pub const DATA_WRITE: u32 = DATA | IMAGE_SCN_MEM_WRITE;

    /// Executable code section
    pub const CODE: u32 = IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ;
}

/// Section layout for new PE
#[derive(Debug, Clone)]
struct SectionLayout {
    /// Virtual address for .maxion section
    maxion_va: u32,

    /// File offset for .maxion section
    maxion_offset: u32,

    /// Virtual address for .stub section
    stub_va: u32,

    /// File offset for .stub section
    stub_offset: u32,

    /// Size of .stub section (aligned)
    #[allow(dead_code)] // Reserved for future use
    stub_size: u32,

    /// Virtual address for .key section
    key_va: u32,

    /// File offset for .key section
    key_offset: u32,

    /// New entry point (stub entry)
    new_entry_point: u32,
}

impl PeInjector {
    /// Create a new PE injector
    ///
    /// # Arguments
    ///
    /// * `pe_path` - Path to original PE file
    /// * `protected_path` - Path for protected output PE file
    /// * `archive_data` - Encrypted archive data to embed
    /// * `encryption_key` - 32-byte encryption key
    /// * `nonce` - 24-byte nonce
    /// * `chunk_size` - Chunk size used for encryption
    pub fn new(
        pe_path: PathBuf,
        protected_path: PathBuf,
        archive_data: Vec<u8>,
        encryption_key: [u8; 32],
        nonce: [u8; 24],
        chunk_size: u32,
    ) -> Self {
        Self {
            pe_path,
            protected_path,
            archive_data,
            encryption_key,
            nonce,
            chunk_size,
            stub_loader: None,
            stub_dll_path: None,
            #[cfg(feature = "phase2")]
            dll_structure: None,
        }
    }

    /// Set the stub binary loader
    ///
    /// # Arguments
    ///
    /// * `stub_loader` - Stub loader containing compiled stub binary
    pub fn with_stub_loader(mut self, stub_loader: StubLoader) -> Self {
        self.stub_loader = Some(stub_loader);
        self
    }

    /// Use DLL loader approach (Phase 1 - Quick Fix)
    ///
    /// This will inject a minimal loader stub that loads maxion_stub.dll
    /// from the same directory and jumps to stub_entry.
    ///
    /// # Arguments
    ///
    /// * `dll_path` - Path to compiled maxion_stub.dll
    pub fn with_dll_loader(mut self, dll_path: PathBuf) -> Result<Self> {
        info!("Using DLL loader approach: {:?}", dll_path);

        // Verify DLL exists
        if !dll_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Stub DLL not found: {:?}", dll_path),
            )
            .into());
        }

        self.stub_dll_path = Some(dll_path);
        Ok(self)
    }

    /// Load DLL structure for full embedding (Phase 2 - Production)
    ///
    /// This parses the DLL structure for later use in full DLL injection.
    ///
    /// # Arguments
    ///
    /// * `dll_path` - Path to compiled maxion_stub.dll
    ///
    /// # Returns
    ///
    /// `Result<Self>` - Injector with loaded DLL structure
    #[cfg(feature = "phase2")]
    pub fn with_dll(mut self, dll_path: PathBuf) -> Result<Self> {
        info!("Loading DLL structure for full embedding: {:?}", dll_path);

        // Verify DLL exists
        if !dll_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Stub DLL not found: {:?}", dll_path),
            )
            .into());
        }

        // Parse DLL structure
        let dll_structure =
            DllStructure::parse(&dll_path).context("Failed to parse DLL structure")?;

        info!(
            "DLL structure loaded: {} sections, entry at 0x{:X}",
            dll_structure.sections.len(),
            dll_structure.entry_point
        );

        self.dll_structure = Some(dll_structure);
        self.stub_dll_path = Some(dll_path); // Keep track of DLL path
        Ok(self)
    }

    /// Stub implementation when phase2 feature is disabled
    #[cfg(not(feature = "phase2"))]
    pub fn with_dll(self, _dll_path: PathBuf) -> Result<Self> {
        Err(anyhow::anyhow!(
            "Phase 2 (full DLL embedding) is not enabled. Use --stub-dll flag for Phase 1 instead."
        ))
    }

    /// Use embedded stub binary (compiled at build time)
    ///
    /// This method loads the stub binary that was compiled and embedded
    /// during the build process by build.rs.
    ///
    /// # Returns
    ///
    /// `Self` - Modified PeInjector instance with embedded stub
    #[cfg(stub_compiled)]
    pub fn with_embedded_stub(mut self) -> Result<Self> {
        info!("Using embedded stub binary ({} bytes)", STUB_SIZE);

        // Verify stub integrity
        if !stub_integrity_check() {
            return Err(Error::Other(
                "Embedded stub integrity check failed".to_string(),
            ));
        }

        // Create stub loader from embedded binary
        let stub_loader = StubLoader::new(STUB_BINARY.to_vec());
        self.stub_loader = Some(stub_loader);

        Ok(self)
    }

    /// Perform the complete injection process
    ///
    /// This method:
    /// 1. Validates the input PE file
    /// 2. Parses PE structure
    /// 3. Calculates new sections
    /// 4. Injects stub code and archive
    /// 5. Modifies entry point
    /// 6. Writes protected executable
    ///
    /// # Returns
    ///
    /// `Result<()>` - Success or error
    pub fn inject(&self) -> Result<()> {
        info!("Starting PE injection for: {}", self.pe_path.display());

        // Validate stub loader
        if self.stub_loader.is_none() {
            #[cfg(not(stub_compiled))]
            {
                return Err(Error::Other(
                        "Stub loader not provided and embedded stub not available. \
                         Use with_stub_loader() or compile with stub support (requires Windows target and objcopy)".to_string()
                    ).into());
            }

            #[cfg(stub_compiled)]
            {
                return Err(Error::Other(
                    "Stub loader not provided. Use with_stub_loader() or with_embedded_stub()"
                        .to_string(),
                )
                .into());
            }
        }

        // Step 1: Validate and parse PE file
        let pe_data = self.load_pe_file()?;
        let pe = self.parse_pe(&pe_data)?;

        // Step 2: Validate PE structure
        self.validate_pe(&pe)?;

        // Step 3: Calculate stub size first for layout
        let stub_aligned_size = if let Some(loader) = &self.stub_loader {
            // Parse stub to get actual size
            let (extracted_data, _) = self.extract_stub_entry(loader.data())?;
            let size = extracted_data.len() as u32;
            size.div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT
        } else {
            // Use MAX_STUB_SIZE as fallback
            MAX_STUB_SIZE.div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT
        };

        // Step 4: Calculate new section layout with actual stub size
        let layout = self.calculate_section_layout(&pe, stub_aligned_size)?;

        debug!("Section layout: {:#?}", layout);

        // Step 5: Create new sections and get stub_entry offset
        info!("Step 5: Creating new sections...");
        let (new_sections, stub_entry_offset) = self.create_new_sections(&pe, &layout)?;

        info!("Created {} new sections:", new_sections.len());
        for (section_info, section_data) in &new_sections {
            let section_name = std::str::from_utf8(&section_info.name)
                .unwrap_or("???")
                .trim_end_matches('\0');
            info!(
                "  - Section '{}': VA=0x{:X}, file_offset=0x{:X}, data_size={} bytes, virtual_size=0x{:X}, raw_size=0x{:X}",
                section_name,
                section_info.virtual_address,
                section_info.pointer_to_raw_data,
                section_data.len(),
                section_info.virtual_size,
                section_info.size_of_raw_data
            );
        }
        info!(
            "stub_entry_offset from .text section: 0x{:X}",
            stub_entry_offset
        );

        // Calculate actual entry point: stub VA + stub_entry offset
        let actual_entry_point = layout.stub_va + stub_entry_offset;
        let layout_with_entry = SectionLayout {
            new_entry_point: actual_entry_point,
            ..layout.clone()
        };
        info!(
            "Actual stub_entry RVA: 0x{:X} (stub VA: 0x{:X} + offset: 0x{:X})",
            actual_entry_point, layout.stub_va, stub_entry_offset
        );

        // Step 6: Build protected PE with correct entry point
        info!("Step 6: Building protected PE...");
        info!(
            "Calling write_protected_pe with {} new sections...",
            new_sections.len()
        );
        match self.write_protected_pe(&pe_data, &pe, &new_sections, &layout_with_entry) {
            Ok(()) => info!("write_protected_pe completed successfully"),
            Err(e) => {
                return Err(e).context("Failed to write protected PE");
            }
        }

        info!(
            "Successfully created protected PE: {}",
            self.protected_path.display()
        );

        Ok(())
    }

    /// Inject with DLL loader approach (Phase 1 - Quick Fix)
    ///
    /// This performs the Phase 1 injection:
    /// 1. Injects tiny C loader stub into PE file
    /// 2. Copies maxion_stub.dll to output directory
    /// 3. Sets entry point to loader stub
    ///
    /// The loader stub will:
    /// - Load maxion_stub.dll from the same directory at runtime
    /// - Jump to stub_entry function in the DLL
    ///
    /// # Returns
    ///
    /// `Result<()>` - Success or error
    pub fn inject_with_dll(&self) -> Result<()> {
        info!("Starting Phase 1 DLL loader injection");

        // Step 1: Validate DLL path is set
        let dll_path = self
            .stub_dll_path
            .as_ref()
            .context("DLL path not set. Call with_dll_loader() first.")?;

        info!("Stub DLL: {}", dll_path.display());

        // Verify DLL exists
        if !dll_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Stub DLL not found: {}", dll_path.display()),
            )
            .into());
        }

        // Step 2: Read DLL file
        let dll_data = std::fs::read(dll_path).context("Failed to read stub DLL")?;

        info!("Stub DLL loaded: {} bytes", dll_data.len());

        // Step 3: Determine DLL output path (same directory as protected exe)
        let output_dir = self
            .protected_path
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let dll_output_path = output_dir.join("maxion_stub.dll");

        // Step 4: Copy DLL to output directory
        std::fs::write(&dll_output_path, &dll_data)
            .with_context(|| format!("Failed to write DLL to: {}", dll_output_path.display()))?;

        info!("Stub DLL copied to: {}", dll_output_path.display());

        // Step 5: Inject loader stub into PE (reuses existing inject method)
        self.inject()
            .context("Failed to inject loader stub into PE file")?;

        info!("Phase 1 DLL loader injection completed successfully!");
        info!("  Protected PE: {}", self.protected_path.display());
        info!("  Stub DLL: {}", dll_output_path.display());

        Ok(())
    }

    /// Inject with full DLL embedding (Phase 2 - Production)
    ///
    /// This performs production-grade injection:
    /// 1. Parses DLL structure (sections, imports, relocations)
    /// 2. Embeds all DLL sections with proper layout
    /// 3. Applies relocations for new base address
    /// 4. Resolves imports and patches IAT
    /// 5. Sets entry point to DLL's stub_entry
    ///
    /// # Returns
    ///
    /// `Result<()>` - Success or error
    #[cfg(feature = "phase2")]
    pub fn inject_full_dll(&self) -> Result<()> {
        info!("Starting full DLL injection (Phase 2 - Production)");

        // Check if DLL structure is loaded
        let dll_structure = self
            .dll_structure
            .as_ref()
            .context("DLL structure not loaded. Call with_dll() first")?;

        // Step 1: Load original PE
        let pe_data = self.load_pe_file()?;
        let pe = self.parse_pe(&pe_data)?;
        self.validate_pe(&pe)?;

        info!("Original PE loaded successfully");

        // Step 2: Calculate layout for archive sections
        // Phase 2 embeds full DLL, so use last section's aligned size as placeholder
        let last_section = pe.sections.last().context("PE has no sections")?;
        let last_section_aligned =
            last_section.virtual_size.div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
        let layout = self.calculate_section_layout(&pe, last_section_aligned)?;

        // Step 3: Create DLL injector with proper base address
        // Calculate new base address for embedded DLL
        let last_section = pe.sections.last().context("PE has no sections")?;
        let last_va = last_section.virtual_address + last_section.virtual_size;
        let new_base = pe.image_base + last_va as u64;

        info!("DLL will be loaded at base: 0x{:X}", new_base);

        let mut dll_injector = DllInjector::new(dll_structure.clone(), new_base);

        // Step 4: Map DLL sections with relocations and imports
        dll_injector
            .map_sections()
            .with_context(|| "Failed to map DLL sections with relocations/imports")?;

        info!("DLL sections mapped successfully");

        // Step 5: Calculate layout for embedded DLL sections
        let section_alignment = pe
            .header
            .optional_header
            .as_ref()
            .map(|h| h.windows_fields.section_alignment)
            .unwrap_or(0x1000);

        let dll_layout = dll_injector.calculate_layout(
            layout.key_va + MAX_KEY_SIZE,
            layout.key_offset + MAX_KEY_SIZE,
            section_alignment,
        );

        info!(
            "DLL section layout calculated: {} sections",
            dll_layout.len()
        );

        // Step 6: Create section headers for embedded DLL
        // We'll iterate directly over the layout to create sections
        info!("Processing {} DLL sections for embedding", dll_layout.len());

        // Step 7: Calculate entry point in embedded DLL
        let _dll_entry = dll_injector.get_entry_point()?;

        // Step 8: Create complete new sections list
        let mut new_sections = vec![(
            SectionInfo::new(
                ".maxion",
                self.archive_data.len() as u32,
                self.archive_data.len() as u32,
                section_flags::DATA,
                layout.maxion_va,
                layout.maxion_offset,
            ),
            self.archive_data.clone(),
        )];

        // Step 9: Create .key section with obfuscated encryption key
        // Get original entry point from PE for DLL to jump to
        let original_entry_point = pe.entry;
        let archive_hash = blake3::hash(&self.archive_data);
        let obfuscated_key =
            self.obfuscate_key(&self.encryption_key, original_entry_point, archive_hash.as_bytes())?;
        let key_aligned = MAX_KEY_SIZE.div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
        let mut key_padded = obfuscated_key;
        key_padded.resize(key_aligned as usize, 0);

        let key_section = SectionInfo::new(
            ".key",
            MAX_KEY_SIZE,
            MAX_KEY_SIZE,
            section_flags::DATA,
            layout.key_va,
            layout.key_offset,
        );

        new_sections.push((key_section, key_padded));
        info!(
            "Created .key section: {} bytes at 0x{:X}",
            MAX_KEY_SIZE, layout.key_va
        );

        // Step 10: Collect and sort DLL sections by virtual address
        // This ensures proper PE ordering when sections are written
        let mut dll_sections_ordered: Vec<(String, u32, u32)> = Vec::new();
        for (section_name, (rva, _offset)) in dll_layout.iter() {
            dll_sections_ordered.push((section_name.clone(), *rva, *_offset));
        }

        // Sort by virtual address to maintain PE section ordering
        dll_sections_ordered.sort_by_key(|(_, rva, _)| *rva);

        info!(
            "Embedding {} DLL sections in sorted order",
            dll_sections_ordered.len()
        );

        // Append DLL sections to new sections list in sorted order
        for (section_name, rva, offset) in dll_sections_ordered {
            let section_data = dll_injector
                .get_section_data(&section_name)
                .with_context(|| format!("Section {} not found in mapped DLL", section_name))?
                .to_vec();

            // Get original section characteristics from DLL
            let characteristics = dll_structure
                .get_section(&section_name)
                .map(|s| s.characteristics)
                .unwrap_or(0x40000040u32); // Default: Readable + Initialized Data

            let section_size = section_data.len();

            let section_info = SectionInfo::new(
                &section_name,
                section_size as u32,
                section_size as u32,
                characteristics,
                rva,
                offset,
            );

            new_sections.push((section_info, section_data));

            info!(
                "Section {} embedded: VA=0x{:X}, Offset=0x{:X}, Size={}",
                section_name, rva, offset, section_size
            );
        }

        // Step 11: Calculate entry point in embedded DLL
        let dll_entry = dll_injector.get_entry_point()?;

        let layout_with_entry = SectionLayout {
            new_entry_point: dll_entry as u32,
            ..layout
        };

        // Step 12: Build protected PE with all sections
        self.write_protected_pe(&pe_data, &pe, &new_sections, &layout_with_entry)?;

        info!("Full DLL injection completed successfully!");
        info!("Protected executable: {}", self.protected_path.display());

        Ok(())
    }

    /// Load PE file into memory
    ///
    /// # Returns
    ///
    /// `Result<Vec<u8>>` - PE file data
    fn load_pe_file(&self) -> Result<Vec<u8>> {
        debug!("Loading PE file: {}", self.pe_path.display());

        let file = File::open(&self.pe_path)
            .with_context(|| format!("Failed to open PE file: {}", self.pe_path.display()))?;

        let mmap = unsafe { Mmap::map(&file)? };
        Ok(mmap.to_vec())
    }

    /// Parse PE file structure
    ///
    /// # Arguments
    ///
    /// * `pe_data` - Raw PE file data
    ///
    /// # Returns
    ///
    /// `Result<PE<'_>>` - Parsed PE structure
    fn parse_pe<'a>(&self, pe_data: &'a [u8]) -> Result<PE<'a>> {
        debug!("Parsing PE structure");

        let pe =
            PE::parse(pe_data).with_context(|| "Failed to parse PE file - invalid or corrupted")?;

        info!(
            "PE parsed successfully: {} sections, entry at 0x{:X}",
            pe.sections.len(),
            pe.entry
        );

        Ok(pe)
    }

    /// Validate PE file for injection compatibility
    ///
    /// # Arguments
    ///
    /// * `pe` - Parsed PE structure
    ///
    /// # Returns
    ///
    /// `Result<()>` - Success or validation error
    fn validate_pe(&self, pe: &PE) -> Result<()> {
        debug!("Validating PE structure");

        // Check if PE has sections
        if pe.sections.is_empty() {
            return Err(Error::Other("PE file has no sections".to_string()).into());
        }

        // Check if PE is executable (has entry point)
        if pe.entry == 0 {
            return Err(Error::Other("PE file has no entry point".to_string()).into());
        }

        // Check section count (avoid too many sections)
        if pe.sections.len() >= 96 {
            warn!(
                "PE file has {} sections (close to maximum of 96)",
                pe.sections.len()
            );
        }

        debug!("PE validation passed");

        Ok(())
    }

    /// Calculate layout for new sections
    ///
    /// # Arguments
    ///
    /// * `pe` - Parsed PE structure
    ///
    /// # Returns
    ///
    /// `Result<SectionLayout>` - Calculated section layout
    fn calculate_section_layout(&self, pe: &PE, stub_size: u32) -> Result<SectionLayout> {
        debug!(
            "Calculating section layout with stub_size = {} bytes",
            stub_size
        );

        // Find last section to determine placement
        let last_section = pe.sections.last().context("PE has no sections")?;

        // Get last section's virtual address and size
        let last_va = last_section.virtual_address;
        let last_size = last_section.virtual_size;

        // Get last section's file offset and size
        let last_offset = last_section.pointer_to_raw_data;
        let last_raw_size = last_section.size_of_raw_data;

        // Calculate .maxion section (encrypted archive)
        let maxion_va = (last_va + last_size).div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
        let maxion_offset =
            (last_offset + last_raw_size).div_ceil(PE_FILE_ALIGNMENT) * PE_FILE_ALIGNMENT;

        debug!("Archive size: {} bytes", self.archive_data.len());

        // Calculate .stub section (stub code)
        // Use file alignment for file offset calculation, section alignment for virtual address
        let archive_aligned_va =
            (self.archive_data.len() as u32).div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
        let archive_aligned_file =
            (self.archive_data.len() as u32).div_ceil(PE_FILE_ALIGNMENT) * PE_FILE_ALIGNMENT;

        debug!(
            "Archive aligned VA: 0x{:X}, aligned file: 0x{:X}",
            archive_aligned_va, archive_aligned_file
        );
        debug!(
            ".maxion section: VA=0x{:X}, file_offset=0x{:X}",
            maxion_va, maxion_offset
        );

        let stub_va = maxion_va + archive_aligned_va;
        let stub_offset = maxion_offset + archive_aligned_file;

        debug!(
            ".stub section: VA=0x{:X}, file_offset=0x{:X}",
            stub_va, stub_offset
        );
        debug!(
            "Stub virtual offset from .maxion: 0x{:X}",
            stub_va - maxion_va
        );
        debug!(
            "Stub file offset from .maxion: 0x{:X}",
            stub_offset - maxion_offset
        );

        // Stub size should already be aligned
        let stub_aligned = stub_size;
        let stub_aligned_file = stub_aligned.div_ceil(PE_FILE_ALIGNMENT) * PE_FILE_ALIGNMENT;

        debug!(
            "Stub size: {} bytes (VA aligned to 0x{:X}, file aligned to 0x{:X})",
            stub_size, stub_aligned, stub_aligned_file
        );

        // Calculate .key section (encryption key)
        let key_va = stub_va + stub_aligned;
        let key_offset = stub_offset + stub_aligned_file;

        debug!(
            ".key section: VA=0x{:X}, file_offset=0x{:X}",
            key_va, key_offset
        );

        // We'll set the entry point after extracting stub_entry offset
        // For now, use stub section base as placeholder
        let layout = SectionLayout {
            maxion_va,
            maxion_offset,
            stub_va,
            stub_offset,
            stub_size,
            key_va,
            key_offset,
            new_entry_point: stub_va, // Will be adjusted in inject()
        };

        debug!("New entry point will be at: 0x{:X}", layout.new_entry_point);

        Ok(layout)
    }

    /// Create new PE sections
    ///
    /// # Arguments
    ///
    /// * `pe` - Parsed PE structure
    /// * `layout` - Section layout
    ///
    /// # Returns
    ///
    /// `NewSectionsResult` - (sections, stub_entry_offset)
    fn create_new_sections(&self, pe: &PE, layout: &SectionLayout) -> NewSectionsResult {
        debug!("Creating new PE sections");

        let mut sections = Vec::new();

        // 1. .maxion section - encrypted archive
        // Calculate aligned sizes: virtual uses section alignment, file uses file alignment
        let archive_aligned_va =
            (self.archive_data.len() as u32).div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
        let archive_aligned_file =
            (self.archive_data.len() as u32).div_ceil(PE_FILE_ALIGNMENT) * PE_FILE_ALIGNMENT;

        let maxion_section = SectionInfo::new(
            ".maxion",
            archive_aligned_va,   // Virtual size aligned to 0x1000
            archive_aligned_file, // Raw size aligned to 0x200
            section_flags::DATA,
            layout.maxion_va,
            layout.maxion_offset,
        );

        let mut archive_padded = self.archive_data.clone();
        archive_padded.resize(archive_aligned_file as usize, 0); // File data aligned to 0x200

        debug!(
            "Created .maxion section: {} bytes (VA: {} bytes, file: {} bytes) at VA 0x{:X}, file offset 0x{:X}",
            self.archive_data.len(), archive_aligned_va, archive_aligned_file, layout.maxion_va, layout.maxion_offset
        );
        debug!(
            ".maxion section data: first 16 bytes = {:02X?}",
            &archive_padded[..16.min(archive_padded.len())]
        );

        sections.push((maxion_section, archive_padded));

        // 2. .stub section - stub code
        let (stub_data, stub_entry_offset) = if let Some(loader) = &self.stub_loader {
            // Parse stub DLL to extract stub_entry function
            let (extracted_data, entry_offset) = self.extract_stub_entry(loader.data())?;
            // Log stub data for debugging
            debug!(
                "Stub data extracted: {} bytes, entry offset: 0x{:X}",
                extracted_data.len(),
                entry_offset
            );
            debug!(
                "Stub first 16 bytes: {:02X?}",
                &extracted_data[..16.min(extracted_data.len())]
            );

            let extracted_len = extracted_data.len();
            let mut data = extracted_data;
            // Pad to section alignment for both virtual and file
            let aligned_size =
                (data.len() as u32).div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
            data.resize(aligned_size as usize, 0);
            debug!(
                "Stub data after padding: {} bytes (was {})",
                data.len(),
                extracted_len
            );
            (data, entry_offset)
        } else {
            // Use MAX_STUB_SIZE aligned as placeholder
            let aligned_size = MAX_STUB_SIZE.div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
            (vec![0u8; aligned_size as usize], 0u32)
        };

        // Stub section: virtual and file sizes are same (both aligned to 0x1000)
        let stub_aligned = stub_data.len() as u32;

        let stub_section = SectionInfo::new(
            ".stub",
            stub_aligned, // Virtual size aligned to 0x1000
            stub_aligned, // Raw size same as virtual (both aligned to 0x1000)
            section_flags::CODE,
            layout.stub_va,
            layout.stub_offset,
        );

        sections.push((stub_section, stub_data.clone()));
        debug!(
            "Created .stub section: {} bytes at VA 0x{:X}, file offset 0x{:X}, stub_entry at offset 0x{:X}",
            stub_aligned, layout.stub_va, layout.stub_offset, stub_entry_offset
        );
        debug!(
            "Stub section data: first 16 bytes = {:02X?}, last 16 bytes = {:02X?}",
            &stub_data[..16.min(stub_data.len())],
            &stub_data[stub_data.len().saturating_sub(16)..]
        );
        // Verify stub data looks like valid code
        if stub_data.len() >= 3 {
            let first_bytes = &stub_data[..3];
            debug!(
                "Stub first 3 bytes: {:02X?} (expected 48 83 EC for valid x64 code)",
                first_bytes
            );
        }

        // 3. .key section - obfuscated encryption key
        // Key is small (256 bytes), so file and virtual alignment differ
        let key_aligned_va = MAX_KEY_SIZE.div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
        let key_aligned_file = MAX_KEY_SIZE.div_ceil(PE_FILE_ALIGNMENT) * PE_FILE_ALIGNMENT;

        info!(
            "Creating .key section: layout.key_va=0x{:X}, layout.key_offset=0x{:X}",
            layout.key_va, layout.key_offset
        );
        info!(
            "Key alignment: VA=0x{:X} (from MAX_KEY_SIZE={} / PE_SECTION_ALIGNMENT=0x{:X}), file=0x{:X} (from MAX_KEY_SIZE={} / PE_FILE_ALIGNMENT=0x{:X})",
            key_aligned_va, MAX_KEY_SIZE, PE_SECTION_ALIGNMENT,
            key_aligned_file, MAX_KEY_SIZE, PE_FILE_ALIGNMENT
        );

        let key_section = SectionInfo::new(
            ".key",
            key_aligned_va,   // Virtual size aligned to 0x1000
            key_aligned_file, // Raw size aligned to 0x200
            section_flags::DATA,
            layout.key_va,
            layout.key_offset,
        );

        // Get original entry point from PE for the stub to jump to
        let original_entry_point = pe.entry;
        let archive_hash = blake3::hash(&self.archive_data);
        let obfuscated_key =
            self.obfuscate_key(&self.encryption_key, original_entry_point, archive_hash.as_bytes())?;
        let obfuscated_len = obfuscated_key.len();
        let mut key_padded = obfuscated_key;
        key_padded.resize(key_aligned_file as usize, 0); // File data aligned to 0x200
        info!(
            ".key section data: {} bytes (obfuscated: {} -> padded: {})",
            key_padded.len(),
            obfuscated_len,
            key_aligned_file
        );
        info!(
            "Key section file_offset=0x{:X}, expected end=0x{:X} (0x{:X} + 0x{:X})",
            layout.key_offset,
            layout.key_offset + key_aligned_file,
            layout.key_offset,
            key_aligned_file
        );

        sections.push((key_section, key_padded));
        debug!(
            "Created .key section: {} bytes (VA: {} bytes, file: {} bytes) at VA 0x{:X}, file offset 0x{:X}",
            MAX_KEY_SIZE, key_aligned_va, key_aligned_file, layout.key_va, layout.key_offset
        );

        Ok((sections, stub_entry_offset))
    }
    /// Extract stub_entry function from stub DLL or raw binary
    ///
    /// Handles two cases:
    /// 1. Raw binary (loader stub) - already extracted .text section, entry_offset = 0
    /// 2. Full DLL (runtime stub) - parse and extract stub_entry function from .text section
    ///
    /// # Arguments
    ///
    /// * `stub_data` - Raw binary data (either full DLL or extracted .text section)
    ///
    /// # Returns
    ///
    /// `Result<(Vec<u8>, u32)>` - (stub code data, entry point offset)
    fn extract_stub_entry(&self, stub_data: &[u8]) -> Result<(Vec<u8>, u32)> {
        // Check if this is a raw binary (loader stub) or full DLL (runtime stub)
        // PE files start with "MZ" signature (0x5A4D)
        let is_pe = stub_data.len() >= 2 && stub_data[0] == 0x4D && stub_data[1] == 0x5A;

        if is_pe {
            info!("Stub appears to be a full DLL, parsing for stub_entry");
            self.extract_stub_from_dll(stub_data)
        } else {
            info!("Stub appears to be raw binary (loader stub), using directly");
            // Raw binary - already the code we want, entry point is at offset 0
            if stub_data.is_empty() {
                return Err(Error::Other("Raw stub binary is empty".to_string()).into());
            }
            // Log first and last 16 bytes for debugging
            let first_16 = if stub_data.len() >= 16 {
                format!("{:02X?}", &stub_data[..16])
            } else {
                format!("{:02X?}", stub_data)
            };
            let last_16 = if stub_data.len() >= 16 {
                let start = stub_data.len() - 16;
                format!("{:02X?}", &stub_data[start..])
            } else {
                format!("{:02X?}", stub_data)
            };
            info!(
                "Using raw stub binary: {} bytes, entry offset: 0",
                stub_data.len()
            );
            debug!("First 16 bytes: {}", first_16);
            debug!("Last 16 bytes: {}", last_16);
            Ok((stub_data.to_vec(), 0u32))
        }
    }

    /// Extract stub_entry from a full DLL by parsing PE structure
    fn extract_stub_from_dll(&self, stub_dll: &[u8]) -> Result<(Vec<u8>, u32)> {
        info!("Parsing stub DLL to extract stub_entry function");

        // Parse stub DLL as PE
        let pe = PE::parse(stub_dll)
            .map_err(|e| Error::Other(format!("Failed to parse stub DLL: {:?}", e)))?;

        // Find .text section (contains the code)
        let text_section = pe
            .sections
            .iter()
            .find(|s| s.name().unwrap_or("") == ".text")
            .ok_or_else(|| Error::Other("Stub DLL has no .text section".to_string()))?;

        info!(
            "Stub DLL .text section: VA=0x{:X}, size={} bytes",
            text_section.virtual_address, text_section.size_of_raw_data
        );

        // Find stub_entry export
        let stub_entry_rva = pe
            .exports
            .iter()
            .find(|e| e.name == Some("stub_entry"))
            .ok_or_else(|| Error::Other("stub_entry not found in DLL exports".to_string()))?
            .rva;

        info!("Found stub_entry export at RVA: 0x{:X}", stub_entry_rva);

        // Calculate offset within .text section
        let stub_entry_offset = stub_entry_rva
            .checked_sub(text_section.virtual_address as usize)
            .ok_or_else(|| {
                Error::Other(format!(
                    "stub_entry RVA (0x{:X}) is outside .text section (0x{:X})",
                    stub_entry_rva, text_section.virtual_address
                ))
            })?;

        // Extract .text section data
        let text_offset = text_section.pointer_to_raw_data as usize;
        let text_size = text_section.size_of_raw_data as usize;
        let text_data = &stub_dll[text_offset..text_offset + text_size];

        info!("Extracted {} bytes from .text section", text_data.len());

        Ok((text_data.to_vec(), stub_entry_offset as u32))
    }

    /// Obfuscate encryption key for storage in PE section
    ///
    /// Uses XOR obfuscation with magic bytes and BLAKE3 hash.
    /// Also stores the original entry point so the stub can jump to it.
    ///
    /// # Arguments
    ///
    /// * `key` - Raw encryption key
    /// * `original_entry_point` - Original PE entry point RVA
    ///
    /// # Returns
    ///
    /// `Result<Vec<u8>>` - Obfuscated key data with entry point
    fn obfuscate_key(
        &self,
        key: &[u8; 32],
        original_entry_point: u32,
        archive_hash: &[u8; 32],
    ) -> Result<Vec<u8>> {
        let mut rng = rand::thread_rng();
        let scheme_id = (rng.next_u32() % KEY_BLOB_V2_SCHEME_COUNT as u32) as u8;
        let mut mask = [0u8; 32];
        rng.fill_bytes(&mut mask);

        let mut obfuscated_key = [0u8; 32];
        for (i, byte) in key.iter().enumerate() {
            obfuscated_key[i] = match scheme_id {
                0 => byte ^ MAGIC[i % MAGIC.len()] ^ mask[i],
                1 => byte.wrapping_add(mask[i]) ^ self.nonce[i % self.nonce.len()],
                2 => byte.rotate_left((mask[i] & 0x07) as u32) ^ MAGIC[(i * 3) % MAGIC.len()],
                _ => unreachable!("scheme_id bounded by KEY_BLOB_V2_SCHEME_COUNT"),
            };
        }

        let mut blob = Vec::with_capacity(168);
        blob.extend_from_slice(KEY_BLOB_V3_MAGIC);
        blob.push(scheme_id);
        blob.push(0);
        blob.extend_from_slice(&0u16.to_le_bytes());
        blob.extend_from_slice(&mask);
        blob.extend_from_slice(&self.nonce);
        blob.extend_from_slice(&self.chunk_size.to_le_bytes());
        blob.extend_from_slice(&original_entry_point.to_le_bytes());
        blob.extend_from_slice(&obfuscated_key);
        blob.extend_from_slice(archive_hash);

        let checksum = blake3::hash(&blob);
        blob.extend_from_slice(checksum.as_bytes());

        Ok(blob)
    }

    /// Build protected PE file
    ///
    /// This method writes the complete protected PE file with:
    /// - Original PE sections
    /// - New .maxion, .stub, .key sections
    /// - Modified entry point
    /// - Updated headers
    ///
    /// # Arguments
    ///
    /// * `pe_data` - Original PE file data
    /// * `pe` - Parsed PE structure
    /// * `new_sections` - New sections to inject
    /// * `layout` - Layout information for new sections
    ///
    /// # Returns
    ///
    /// `Result<()>` - Success or error
    fn write_protected_pe(
        &self,
        pe_data: &[u8],
        pe: &PE,
        new_sections: &[(SectionInfo, Vec<u8>)],
        layout: &SectionLayout,
    ) -> Result<()> {
        info!("Building protected PE file");
        info!(
            "Input parameters: pe_data.len()={}, pe.sections.len()={}, new_sections.len()={}",
            pe_data.len(),
            pe.sections.len(),
            new_sections.len()
        );

        // Calculate new file size based on section's raw_size (padded size)
        let last_section = new_sections.last().context("No new sections to add")?;
        let last_offset = last_section.0.pointer_to_raw_data;
        let last_raw_size = last_section.0.size_of_raw_data;
        let last_data_size = last_section.1.len() as u32;
        let new_file_size = last_offset + last_raw_size;

        let last_name = std::str::from_utf8(&last_section.0.name).unwrap_or("???");
        info!(
            "New file size: {} bytes (last section '{}' at offset 0x{:X} + raw_size 0x{:X} = 0x{:X})",
            new_file_size, last_name, last_offset, last_raw_size, new_file_size
        );
        debug!(
            "Last section details: data_size={} bytes, raw_size={} bytes, padding={}",
            last_data_size,
            last_raw_size,
            last_raw_size - last_data_size
        );

        // Write original PE up to section table
        let _dos_stub_size = 64;
        let pe_header_offset = 60; // Offset to PE header in DOS stub

        // Read DOS stub and PE header offset
        if pe_data.len() < pe_header_offset + 4 {
            return Err(Error::Other("PE file too short".to_string()).into());
        }

        let mut pe_header_offset_bytes = [0u8; 4];
        pe_header_offset_bytes.copy_from_slice(&pe_data[pe_header_offset..pe_header_offset + 4]);
        let pe_offset = u32::from_le_bytes(pe_header_offset_bytes) as usize;

        // Modify and write optional header
        let optional_header_size = pe.header.coff_header.size_of_optional_header as usize;
        let sections_table_start = pe_offset + 24 + optional_header_size;
        let mut optional_header = pe_data[pe_offset + 24..sections_table_start].to_vec();
        let pe_kind = detect_pe_kind(&optional_header)?;

        // Calculate total file size: DOS + PE headers + section headers + section data
        let dos_and_pe_headers_size = pe_offset + 24 + optional_header_size;
        let section_headers_size = (pe.sections.len() + new_sections.len()) * SECTION_HEADER_SIZE;

        debug!(
            "File size calculation: DOS+PE=0x{:X} ({}), section_headers=0x{:X} ({}), last_section_end=0x{:X} ({}), total=0x{:X} ({})",
            dos_and_pe_headers_size, dos_and_pe_headers_size,
            section_headers_size, section_headers_size,
            new_file_size, new_file_size,
            new_file_size, new_file_size
        );

        debug!(
            "Last section: name={:?}, offset=0x{:X}, raw_size=0x{:X} ({}), data_size=0x{:X} ({}), file_end=0x{:X} ({})",
            std::str::from_utf8(&last_section.0.name).unwrap_or("???"),
            last_offset,
            last_raw_size, last_raw_size,
            last_data_size, last_data_size,
            new_file_size, new_file_size
        );

        debug!("Optional header size: {} bytes", optional_header.len());
        debug!("PE kind: {:?}", pe_kind);

        let current_entry = u32::from_le_bytes([
            optional_header[16],
            optional_header[17],
            optional_header[18],
            optional_header[19],
        ]);
        debug!("Current entry point at offset 16: 0x{:X}", current_entry);

        let current_size_of_image = u32::from_le_bytes([
            optional_header[56],
            optional_header[57],
            optional_header[58],
            optional_header[59],
        ]);
        debug!(
            "Current SizeOfImage at offset 56: 0x{:X}",
            current_size_of_image
        );

        let section_alignment = u32::from_le_bytes([
            optional_header[32],
            optional_header[33],
            optional_header[34],
            optional_header[35],
        ]);
        debug!("SectionAlignment: 0x{:X}", section_alignment);

        let image_base = match pe_kind {
            PeKind::Pe32 => u32::from_le_bytes([
                optional_header[28],
                optional_header[29],
                optional_header[30],
                optional_header[31],
            ]) as u64,
            PeKind::Pe32Plus => u64::from_le_bytes([
                optional_header[24],
                optional_header[25],
                optional_header[26],
                optional_header[27],
                optional_header[28],
                optional_header[29],
                optional_header[30],
                optional_header[31],
            ]),
        };
        debug!("ImageBase: 0x{:X}", image_base);

        let subsystem = u16::from_le_bytes([optional_header[68], optional_header[69]]);
        debug!("Subsystem: 0x{:X}", subsystem);

        info!(
            "Updating entry point from 0x{:X} to 0x{:X}",
            current_entry, layout.new_entry_point
        );
        optional_header[16..20].copy_from_slice(&layout.new_entry_point.to_le_bytes());

        // Update SizeOfImage in optional header (offset 56 from start of optional header, 4 bytes)
        // SizeOfImage = last section VA + last section virtual size, aligned to SectionAlignment
        let last_section_va = last_section.0.virtual_address;
        let last_section_size = last_section.0.virtual_size;
        let unaligned_size_of_image = last_section_va + last_section_size;
        let new_size_of_image =
            unaligned_size_of_image.div_ceil(section_alignment) * section_alignment;

        debug!(
            "Last section: VA=0x{:X}, VirtualSize=0x{:X}",
            last_section_va, last_section_size
        );
        debug!(
            "Unaligned SizeOfImage: 0x{:X}, Aligned to 0x{:X}",
            unaligned_size_of_image, new_size_of_image
        );

        info!(
            "Updating SizeOfImage from 0x{:X} to 0x{:X} (unaligned: 0x{:X}, aligned to SectionAlignment 0x{:X})",
            current_size_of_image, new_size_of_image, unaligned_size_of_image, section_alignment
        );

        // Verify alignment matches SectionAlignment
        if new_size_of_image % section_alignment != 0 {
            warn!(
                "SizeOfImage 0x{:X} is not aligned to SectionAlignment 0x{:X}",
                new_size_of_image, section_alignment
            );
        }

        optional_header[56..60].copy_from_slice(&new_size_of_image.to_le_bytes());

        // Verify what was written
        let written_size_of_image = u32::from_le_bytes([
            optional_header[56],
            optional_header[57],
            optional_header[58],
            optional_header[59],
        ]);
        debug!("Written SizeOfImage: 0x{:X}", written_size_of_image);
        if written_size_of_image != new_size_of_image {
            return Err(Error::Other(format!(
                "SizeOfImage write mismatch: expected 0x{:X}, wrote 0x{:X}",
                new_size_of_image, written_size_of_image
            ))
            .into());
        }

        // Zero the checksum to bypass Windows loader checksum validation
        // Checksum is at offset 64 in PE32+ optional header (4 bytes)
        // Setting to 0 allows Windows to accept the modified PE without proper checksum calculation
        optional_header[64..68].copy_from_slice(&0u32.to_le_bytes());
        debug!("Zeroed checksum at offset 64");

        // Also verify entry point was written correctly
        let written_entry = u32::from_le_bytes([
            optional_header[16],
            optional_header[17],
            optional_header[18],
            optional_header[19],
        ]);
        debug!("Written entry point: 0x{:X}", written_entry);
        if written_entry != layout.new_entry_point {
            return Err(Error::Other(format!(
                "Entry point write mismatch: expected 0x{:X}, wrote 0x{:X}",
                layout.new_entry_point, written_entry
            ))
            .into());
        }

        let output_file = File::create(&self.protected_path).with_context(|| {
            format!(
                "Failed to create output file: {}",
                self.protected_path.display()
            )
        })?;
        let mut writer = BufWriter::new(output_file);

        // Write DOS stub
        let dos_stub_bytes = writer.write(&pe_data[..pe_offset])?;
        debug!("Wrote {} bytes for DOS stub", dos_stub_bytes);

        // Write PE signature "PE\0\0" (4 bytes)
        let pe_signature_bytes = writer.write(&pe_data[pe_offset..pe_offset + 4])?;
        debug!(
            "Wrote PE signature at offset 0x{:X} ({} bytes)",
            pe_offset, pe_signature_bytes
        );

        // Modify and write COFF header (20 bytes after PE signature)
        let mut coff_header = pe_data[pe_offset + 4..pe_offset + 24].to_vec();

        let new_section_count = (pe.sections.len() + new_sections.len()) as u16;
        info!(
            "Preparing to write COFF header with section count: original={}, new={}, total={}",
            pe.sections.len(),
            new_sections.len(),
            new_section_count
        );
        coff_header[2..4].copy_from_slice(&new_section_count.to_le_bytes());
        info!(
            "Updated section count from {} to {}",
            pe.sections.len(),
            new_section_count
        );

        let coff_header_bytes = writer.write(&coff_header)?;
        debug!("Wrote {} bytes for COFF header", coff_header_bytes);

        let optional_header_bytes = writer.write(&optional_header)?;
        debug!("Wrote {} bytes for optional header", optional_header_bytes);

        // Write ALL section headers first (original + new)
        // Original section headers
        let mut total_bytes_written =
            dos_stub_bytes + pe_signature_bytes + coff_header_bytes + optional_header_bytes;
        debug!(
            "Total so far (DOS + COFF + optional): {} bytes",
            total_bytes_written
        );
        let mut original_header_bytes = 0;
        for section in &pe.sections {
            let header_bytes = writer.write(&section_to_bytes(section))?;
            total_bytes_written += header_bytes;
            original_header_bytes += header_bytes;
        }
        info!(
            "Wrote {} original section headers (total: {} bytes)",
            pe.sections.len(),
            original_header_bytes
        );

        // New section headers
        info!(
            "Preparing to write {} new section headers",
            new_sections.len()
        );
        let mut new_header_bytes = 0;
        for (section_info, _) in new_sections {
            let section_name = std::str::from_utf8(&section_info.name)
                .unwrap_or("???")
                .trim_end_matches('\0');
            debug!(
                "Writing header for section '{}': VA=0x{:X}, file_offset=0x{:X}, virtual_size=0x{:X}, raw_size=0x{:X}",
                section_name,
                section_info.virtual_address,
                section_info.pointer_to_raw_data,
                section_info.virtual_size,
                section_info.size_of_raw_data
            );
            let header_bytes = writer.write(&section_info.to_bytes())?;
            total_bytes_written += header_bytes;
            new_header_bytes += header_bytes;
        }
        debug!(
            "Wrote {} new section headers (total: {} bytes)",
            new_sections.len(),
            new_header_bytes
        );

        // Write ALL section data at their specified file offsets using seek()
        // Original section data
        let mut original_data_bytes = 0;
        for section in &pe.sections {
            let name = section.name().unwrap_or("???");
            debug!(
                "Seeking to offset 0x{:X} for original section '{}'",
                section.pointer_to_raw_data, name
            );
            writer.seek(SeekFrom::Start(section.pointer_to_raw_data as u64))?;

            let section_data = &pe_data[section.pointer_to_raw_data as usize..]
                [..section.size_of_raw_data as usize];
            let data_bytes = writer.write(section_data)?;
            let current_pos = writer.stream_position()?;
            if current_pos != (section.pointer_to_raw_data + section.size_of_raw_data) as u64 {
                warn!(
                    "Section '{}' write ended at 0x{:X}, expected 0x{:X}",
                    name,
                    current_pos,
                    section.pointer_to_raw_data + section.size_of_raw_data
                );
            }
            total_bytes_written = std::cmp::max(total_bytes_written, current_pos as usize);
            original_data_bytes += data_bytes;
            debug!(
                "Wrote original section '{}': {} bytes at offset 0x{:X}",
                name, data_bytes, section.pointer_to_raw_data
            );
        }
        debug!(
            "Wrote {} original sections data (total: {} bytes)",
            pe.sections.len(),
            original_data_bytes
        );

        // New section data
        debug!("Writing {} new sections to file", new_sections.len());
        let mut new_data_bytes = 0;
        for (section_info, section_data) in new_sections {
            let name = std::str::from_utf8(&section_info.name)
                .unwrap_or("???")
                .trim_end_matches('\0');
            debug!(
                "Seeking to offset 0x{:X} for section '{}'",
                section_info.pointer_to_raw_data, name
            );
            writer.seek(SeekFrom::Start(section_info.pointer_to_raw_data as u64))?;
            if name == ".stub" {
                // Log first and last 16 bytes of stub section
                let first_16 = if section_data.len() >= 16 {
                    format!("{:02X?}", &section_data[..16])
                } else {
                    format!("{:02X?}", section_data)
                };
                let last_16 = if section_data.len() >= 16 {
                    let start = section_data.len() - 16;
                    format!("{:02X?}", &section_data[start..])
                } else {
                    format!("{:02X?}", section_data)
                };
                info!(
                    "Stub section data being written: first 16 = {}, last 16 = {}",
                    first_16, last_16
                );
                // Verify it looks like valid code
                if section_data.len() >= 3 {
                    let first_bytes = &section_data[..3];
                    info!(
                        "Stub first 3 bytes being written: {:02X?} (expected 48 83 EC for valid x64)",
                        first_bytes
                    );
                }
            }

            // Write section data at the seeked position
            let bytes_written = writer.write(section_data)?;
            let raw_size = section_info.size_of_raw_data as usize;
            let padding_needed = raw_size.saturating_sub(section_data.len());

            debug!(
                "Wrote {} bytes for section '{}' (data_size={}, raw_size={}, padding_needed={})",
                bytes_written,
                name,
                section_data.len(),
                raw_size,
                padding_needed
            );

            if padding_needed > 0 {
                debug!(
                    "Padding section '{}' with {} zeros ({} -> {} bytes)",
                    name,
                    padding_needed,
                    section_data.len(),
                    raw_size
                );
                let padding_bytes = vec![0u8; padding_needed];
                let padding_written = writer.write(&padding_bytes)?;
                debug!("Wrote {} padding bytes", padding_written);
                total_bytes_written += bytes_written + padding_written;
                new_data_bytes += bytes_written + padding_written;
            } else {
                total_bytes_written += bytes_written;
                new_data_bytes += bytes_written;
            }

            let current_pos = writer.stream_position()?;
            if current_pos
                != (section_info.pointer_to_raw_data + section_info.size_of_raw_data) as u64
            {
                warn!(
                    "Section '{}' write ended at 0x{:X}, expected 0x{:X}",
                    name,
                    current_pos,
                    section_info.pointer_to_raw_data + section_info.size_of_raw_data
                );
            }

            if name == ".maxion" || name == ".stub" || name == ".key" {
                let section_end = section_info.pointer_to_raw_data + section_info.size_of_raw_data;
                info!(
                    "Section '{}': offset=0x{:X}, end=0x{:X}, total_bytes_so_far={}, expected_file_size=0x{:X}",
                    name,
                    section_info.pointer_to_raw_data,
                    section_end,
                    total_bytes_written,
                    new_file_size
                );
                if total_bytes_written != section_end as usize {
                    warn!(
                        "Mismatch: total_bytes_written={} != section_end=0x{:X} ({})",
                        total_bytes_written, section_end, section_end
                    );
                }
            }
        }

        // Write any trailing data from original file after last section
        // Some PE files have data (debug info, etc.) after the last section
        let original_last_section = pe.sections.last().context("No original sections")?;
        let original_end = original_last_section.pointer_to_raw_data as usize
            + original_last_section.size_of_raw_data as usize;
        if original_end < pe_data.len() {
            let trailing_size = pe_data.len() - original_end;
            info!(
                "Writing {} bytes of trailing data from original file (offset 0x{:X} to 0x{:X})",
                trailing_size,
                original_end,
                pe_data.len()
            );
            let trailing_data = &pe_data[original_end..];
            let trailing_written = writer.write(trailing_data)?;
            total_bytes_written += trailing_written;
            info!(
                "Trailing data written: {} bytes (total_bytes_written now: {} = 0x{:X})",
                trailing_written, total_bytes_written, total_bytes_written
            );
        } else {
            info!(
                "No trailing data in original file (ends at: {}, last section ends at: {})",
                pe_data.len(),
                original_end
            );
        }

        // Final verification before padding
        info!(
            "File size before padding: {} bytes (0x{:X}), expected: {} bytes (0x{:X})",
            total_bytes_written, total_bytes_written, new_file_size, new_file_size
        );

        // Write padding to reach expected file size
        // PE files often have padding at the end
        if total_bytes_written < new_file_size as usize {
            let padding_needed = new_file_size as usize - total_bytes_written;
            info!(
                "Adding {} bytes of padding (0x{:X}) to reach expected file size {} (0x{:X})",
                padding_needed, padding_needed, new_file_size, new_file_size
            );
            let padding = vec![0u8; padding_needed];
            let padding_written = writer.write(&padding)?;
            total_bytes_written += padding_written;
            debug!("Wrote {} padding bytes", padding_written);
        }

        writer.flush()?;

        // Final verification after padding
        if total_bytes_written != new_file_size as usize {
            warn!(
                "File size mismatch after padding: wrote {} bytes, expected {} bytes (diff: {})",
                total_bytes_written,
                new_file_size,
                (total_bytes_written as i64 - new_file_size as i64).abs()
            );
        } else {
            info!(
                "File size matches expected: {} bytes (0x{:X})",
                total_bytes_written, total_bytes_written
            );
        }

        // Summary of all bytes written
        // Calculate padding for display (if any was added)
        let expected_bytes = dos_stub_bytes
            + coff_header_bytes
            + optional_header_bytes
            + original_header_bytes
            + new_header_bytes
            + original_data_bytes
            + new_data_bytes;
        let padding_bytes = total_bytes_written.saturating_sub(expected_bytes);

        info!(
            "Byte count breakdown: DOS={} + COFF={} + optional={} + orig_headers={} + new_headers={} + orig_data={} + new_data={} + padding={} = {}",
            dos_stub_bytes,
            coff_header_bytes,
            optional_header_bytes,
            original_header_bytes,
            new_header_bytes,
            original_data_bytes,
            new_data_bytes,
            padding_bytes,
            total_bytes_written
        );
        info!(
            "Protected PE file written successfully. Total bytes written: {}, expected file size: {}",
            total_bytes_written, new_file_size
        );

        if total_bytes_written != new_file_size as usize {
            warn!(
                "File size mismatch! Written: {} (0x{:X}), Expected: {} (0x{:X}), Difference: {}",
                total_bytes_written,
                total_bytes_written,
                new_file_size,
                new_file_size,
                (new_file_size as i64) - (total_bytes_written as i64)
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_section_info_creation() {
        // SectionInfo::new() now expects pre-aligned values
        // virtual_size should be aligned to PE_SECTION_ALIGNMENT (0x1000 = 4096)
        // raw_size should be aligned to PE_FILE_ALIGNMENT (0x200 = 512)
        let section = SectionInfo::new(
            ".test",
            4096, // Pre-aligned virtual size
            1536, // Pre-aligned raw size
            section_flags::DATA_READ,
            0x10000,
            0x400,
        );

        // Verify pre-aligned values are stored correctly
        assert_eq!(section.virtual_size, 4096);
        assert_eq!(section.size_of_raw_data, 1536);
        assert_eq!(section.virtual_address, 0x10000);
        assert_eq!(section.pointer_to_raw_data, 0x400);
        assert_eq!(&section.name[..5], b".test");
    }

    #[test]
    fn test_section_info_serialization() {
        let section = SectionInfo::new(
            ".test",
            4096,
            4096,
            section_flags::DATA_READ,
            0x10000,
            0x400,
        );

        let bytes = section.to_bytes();
        assert_eq!(bytes.len(), SECTION_HEADER_SIZE);
    }

    #[test]
    fn test_key_obfuscation() {
        let injector = PeInjector::new(
            PathBuf::from("test.exe"),
            PathBuf::from("protected.exe"),
            vec![0u8; 100],
            [1u8; 32],
            [2u8; 24],
            65536,
        );

        let key = [42u8; 32];
        let original_entry_point = 0x12345678u32;
        let archive_hash = blake3::hash(b"archive-data");
        let obfuscated = injector
            .obfuscate_key(&key, original_entry_point, archive_hash.as_bytes())
            .unwrap();

        // Obfuscated key should be larger (includes nonce + chunk size + entry point + archive hash + checksum)
        assert!(obfuscated.len() > 32);

        // 4 magic + 4 reserved + 32 mask + 24 nonce + 4 chunk + 4 entry + 32 key + 32 archive hash + 32 checksum
        assert_eq!(obfuscated.len(), 168);
    }

    #[test]
    fn test_section_alignment() {
        // Test virtual alignment (0x1000)
        assert_eq!(PE_SECTION_ALIGNMENT, 0x1000);

        // Test file alignment (0x200)
        assert_eq!(PE_FILE_ALIGNMENT, 0x200);

        // Calculate aligned sizes
        let size1: u32 = 100;
        let aligned1 = size1.div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
        assert_eq!(aligned1, 0x1000);

        let size2: u32 = 4096;
        let aligned2 = size2.div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
        assert_eq!(aligned2, 0x1000);

        let size3: u32 = 4100;
        let aligned3 = size3.div_ceil(PE_SECTION_ALIGNMENT) * PE_SECTION_ALIGNMENT;
        assert_eq!(aligned3, 0x2000);
    }
}
