//! PE Import Resolver
//!
//! This module handles parsing and resolving imports in a DLL, which is necessary
//! when embedding a DLL into a PE file. The resolver patches the Import Address
//! Table (IAT) with actual function addresses from system DLLs.
//!
//! # Import Overview
//!
//! When a DLL is loaded, the Windows loader:
//! 1. Reads the Import Directory (.idata section)
//! 2. For each imported DLL:
//!    - Loads the DLL (LoadLibrary)
//!    - Resolves each function (GetProcAddress)
//!    - Writes addresses to IAT
//! 3. Code references IAT entries to call imported functions
//!
//! When we manually embed a DLL, we must perform step 2 ourselves.
//!
//! # Import Directory Structure
//!
//! ```text
//! Import Directory (array of IMAGE_IMPORT_DESCRIPTOR)
//! ┌─────────────────────────────────────┐
//! │ OriginalFirstThunk (4 bytes)    │ ← RVA to Lookup Table
//! │ TimeDateStamp (4 bytes)         │ ← 0 = bind at runtime
//! │ ForwarderChain (4 bytes)        │ ← Forwarder chain index
//! │ Name (4 bytes)                 │ ← RVA to DLL name string
//! │ FirstThunk (4 bytes)            │ ← RVA to IAT
//! ├─────────────────────────────────────┤
//! │ Next descriptor...               │
//! │ ...                             │
//! └─────────────────────────────────────┘
//! ```
//!
//! # Import Lookup Table (ILT) / IAT
//!
//! ```text
//! Lookup Table / IAT (array of u32/u64)
//! ┌─────────────────────────────────────┐
//! │ Hint/Name Ordinal (4/8 bytes)  │ │ ── Repeated for each function
//! │ ...                             │ │
//! │ 0 (NULL terminator)              │ │
//! └─────────────────────────────────────┘
//!
//! If high bit is set: Import by ordinal
//! Otherwise: RVA to function name (IMAGE_IMPORT_BY_NAME)
//! ```

use std::ffi::CStr;

#[cfg(windows)]
use std::ffi::CString;

#[cfg(windows)]
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryA};

/// Import descriptor from PE Import Directory
///
/// Represents a single DLL that the PE imports from.
#[derive(Debug, Clone)]
pub struct ImportDescriptor {
    /// RVA to OriginalFirstThunk (Lookup Table)
    pub original_first_thunk: u32,
    /// Time/date stamp (0 = bind at runtime)
    pub time_date_stamp: u32,
    /// Forwarder chain index (0 = no forwarding)
    pub forwarder_chain: u32,
    /// RVA to DLL name (e.g., "KERNEL32.dll\0")
    pub name: u32,
    /// RVA to FirstThunk (Import Address Table - IAT)
    pub first_thunk: u32,
}

impl ImportDescriptor {
    /// Size of ImportDescriptor struct in PE file
    pub const SIZE: usize = 20;

    /// Parse import descriptor from bytes
    ///
    /// # Arguments
    ///
    /// * `data` - PE file data or section data
    /// * `offset` - Offset to import descriptor
    ///
    /// # Returns
    ///
    /// `Result<ImportDescriptor>` - Parsed descriptor or error
    pub fn parse(data: &[u8], offset: usize) -> anyhow::Result<Self> {
        if offset + Self::SIZE > data.len() {
            anyhow::bail!(
                "Import descriptor at offset 0x{:X} exceeds data size",
                offset
            );
        }

        let desc_offset = offset;

        Ok(Self {
            original_first_thunk: u32::from_le_bytes([
                data[desc_offset],
                data[desc_offset + 1],
                data[desc_offset + 2],
                data[desc_offset + 3],
            ]),
            time_date_stamp: u32::from_le_bytes([
                data[desc_offset + 4],
                data[desc_offset + 5],
                data[desc_offset + 6],
                data[desc_offset + 7],
            ]),
            forwarder_chain: u32::from_le_bytes([
                data[desc_offset + 8],
                data[desc_offset + 9],
                data[desc_offset + 10],
                data[desc_offset + 11],
            ]),
            name: u32::from_le_bytes([
                data[desc_offset + 12],
                data[desc_offset + 13],
                data[desc_offset + 14],
                data[desc_offset + 15],
            ]),
            first_thunk: u32::from_le_bytes([
                data[desc_offset + 16],
                data[desc_offset + 17],
                data[desc_offset + 18],
                data[desc_offset + 19],
            ]),
        })
    }

    /// Check if this is the terminating descriptor (all zeros)
    ///
    /// # Returns
    ///
    /// `bool` - True if terminating descriptor
    pub fn is_null(&self) -> bool {
        self.original_first_thunk == 0
            && self.time_date_stamp == 0
            && self.forwarder_chain == 0
            && self.name == 0
            && self.first_thunk == 0
    }
}

/// Import by name structure (for named imports)
#[derive(Debug, Clone)]
pub struct ImportByName {
    /// Hint index into export name table (optional, can be 0)
    pub hint: u16,
    /// Function name (null-terminated, follows immediately after hint)
    pub name: String,
}

impl ImportByName {
    /// Size of hint field
    const HINT_SIZE: usize = 2;

    /// Parse import by name from bytes
    ///
    /// # Arguments
    ///
    /// * `data` - PE file data or section data
    /// * `rva` - RVA to import by name structure
    ///
    /// # Returns
    ///
    /// `Result<ImportByName>` - Parsed import or error
    pub fn parse(data: &[u8], rva: u32) -> anyhow::Result<Self> {
        let offset = rva as usize;

        if offset + Self::HINT_SIZE + 1 > data.len() {
            anyhow::bail!("Import by name at RVA 0x{:X} exceeds data size", rva);
        }

        // Read hint
        let hint = u16::from_le_bytes([data[offset], data[offset + 1]]);

        // Read null-terminated function name
        let name_start = offset + Self::HINT_SIZE;
        let name_end = data[name_start..]
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| anyhow::anyhow!("Import name not null-terminated"))?;

        let name_bytes = data[name_start..name_start + name_end].to_vec();
        let name = String::from_utf8(name_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in import name: {}", e))?;

        Ok(Self { hint, name })
    }
}

/// Import entry (either by name or by ordinal)
#[derive(Debug, Clone, PartialEq)]
pub enum ImportEntry {
    /// Import by name (most common)
    ByName { name: String, hint: u16 },
    /// Import by ordinal (direct index into export table)
    ByOrdinal { ordinal: u16 },
}

impl ImportEntry {
    /// Parse import entry from lookup table (32-bit or 64-bit)
    ///
    /// # Arguments
    ///
    /// * `data` - PE file data or section data
    /// * `rva` - RVA to import entry in lookup table
    /// * `is_64bit` - True for x64 PE, false for x86
    ///
    /// # Returns
    ///
    /// `Result<ImportEntry>` - Parsed import entry or error
    pub fn parse(data: &[u8], rva: u32, is_64bit: bool) -> anyhow::Result<Self> {
        let offset = rva as usize;
        let entry_size = if is_64bit { 8 } else { 4 };

        if offset + entry_size > data.len() {
            anyhow::bail!("Import entry at RVA 0x{:X} exceeds data size", rva);
        }

        if is_64bit {
            // 64-bit: u64
            let value = u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            if value == 0 {
                anyhow::bail!("Import entry is NULL (terminator)");
            }

            // Check ordinal bit (high bit set)
            if value & 0x8000000000000000 != 0 {
                // Import by ordinal (lower 16 bits)
                Ok(ImportEntry::ByOrdinal {
                    ordinal: (value & 0xFFFF) as u16,
                })
            } else {
                // Import by name (low 31 bits are RVA)
                let import_by_name = ImportByName::parse(data, (value & 0x7FFFFFFF) as u32)?;
                Ok(ImportEntry::ByName {
                    name: import_by_name.name,
                    hint: import_by_name.hint,
                })
            }
        } else {
            // 32-bit: u32
            let value = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);

            if value == 0 {
                anyhow::bail!("Import entry is NULL (terminator)");
            }

            // Check ordinal bit (high bit set)
            if value & 0x80000000 != 0 {
                // Import by ordinal (lower 16 bits)
                Ok(ImportEntry::ByOrdinal {
                    ordinal: (value & 0xFFFF) as u16,
                })
            } else {
                // Import by name (low 31 bits are RVA)
                let import_by_name = ImportByName::parse(data, value & 0x7FFFFFFF)?;
                Ok(ImportEntry::ByName {
                    name: import_by_name.name,
                    hint: import_by_name.hint,
                })
            }
        }
    }

    /// Get function name if import is by name
    ///
    /// # Returns
    ///
    /// `Option<&str>` - Function name or None if by ordinal
    pub fn name(&self) -> Option<&str> {
        match self {
            ImportEntry::ByName { name, .. } => Some(name.as_str()),
            ImportEntry::ByOrdinal { .. } => None,
        }
    }
}

/// Complete import directory parsed from PE file
#[derive(Debug, Clone)]
pub struct ImportDirectory {
    /// All import descriptors
    pub descriptors: Vec<ImportDescriptor>,
    /// Import entries per descriptor
    pub imports: Vec<Vec<ImportEntry>>,
    /// Total number of imported functions
    pub total_imports: usize,

    /// IAT RVAs (one per descriptor, from FirstThunk)
    pub iat_rvas: Vec<u32>,
}

impl Default for ImportDirectory {
    fn default() -> Self {
        Self::new()
    }
}

impl ImportDirectory {
    /// Create empty import directory
    ///
    /// # Returns
    ///
    /// `ImportDirectory` - Empty directory
    pub fn new() -> Self {
        Self {
            descriptors: Vec::new(),
            imports: Vec::new(),
            total_imports: 0,
            iat_rvas: Vec::new(),
        }
    }

    /// Parse import directory from PE file data
    ///
    /// # Arguments
    ///
    /// * `data` - PE file data or section data
    /// * `pe` - Parsed PE structure for RVA conversion
    /// * `import_dir_offset` - File offset to import directory
    /// * `is_64bit` - True for x64 PE, false for x86
    ///
    /// # Returns
    ///
    /// `Result<ImportDirectory>` - Parsed import directory or error
    pub fn parse(
        data: &[u8],
        pe: &goblin::pe::PE,
        import_dir_offset: u32,
        is_64bit: bool,
    ) -> anyhow::Result<Self> {
        let mut directory = Self::new();
        let mut offset = import_dir_offset as usize;

        if offset + 20 > data.len() {
            log::warn!(
                "Import directory offset 0x{:X} exceeds file bounds (len={})",
                import_dir_offset,
                data.len()
            );
            return Ok(Self::new());
        }

        log::debug!(
            "Parsing import directory at file offset 0x{:X}",
            import_dir_offset
        );
        // Parse all import descriptors
        loop {
            // Check bounds
            if offset + ImportDescriptor::SIZE > data.len() {
                break;
            }

            // Parse import descriptor
            let descriptor = ImportDescriptor::parse(data, offset)?;

            // Check for terminating descriptor
            if descriptor.is_null() {
                break;
            }

            // Parse DLL name
            let dll_name_rva = descriptor.name;

            // Convert RVA to file offset
            let dll_name_offset =
                crate::dll_loader::loader::DllStructure::rva_to_offset(pe, dll_name_rva)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "Cannot map DLL name RVA 0x{:X} to file offset",
                            dll_name_rva
                        )
                    })?;

            if dll_name_offset >= data.len() {
                anyhow::bail!("DLL name offset 0x{:X} exceeds data size", dll_name_offset);
            }

            let dll_name = CStr::from_bytes_until_nul(&data[dll_name_offset..])
                .map_err(|e| anyhow::anyhow!("Invalid DLL name string: {}", e))?
                .to_string_lossy()
                .into_owned();

            log::debug!("Found DLL import: {}", dll_name);

            // Parse import entries from lookup table
            let mut import_entries = Vec::new();

            let lookup_table_rva = descriptor.original_first_thunk;
            if lookup_table_rva != 0 {
                let lookup_offset =
                    crate::dll_loader::loader::DllStructure::rva_to_offset(pe, lookup_table_rva)
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Cannot map lookup table RVA 0x{:X} to file offset",
                                lookup_table_rva
                            )
                        })?;

                let mut current_lookup_offset = lookup_offset;
                let entry_size = if is_64bit { 8 } else { 4 };

                loop {
                    if current_lookup_offset + entry_size > data.len() {
                        break;
                    }

                    match ImportEntry::parse(data, current_lookup_offset as u32, is_64bit) {
                        Ok(entry) => {
                            log::debug!("  Import: {:?}", entry.name());
                            import_entries.push(entry);
                            current_lookup_offset += entry_size;
                        }
                        Err(_) => {
                            // NULL terminator reached
                            break;
                        }
                    }
                }
            }

            // Get length before moving import_entries
            let import_count = import_entries.len();

            directory.descriptors.push(descriptor.clone());
            directory.imports.push(import_entries);
            directory.total_imports += import_count;

            // Collect IAT RVA from FirstThunk field
            directory.iat_rvas.push(descriptor.first_thunk);

            // Move to next import descriptor
            offset += ImportDescriptor::SIZE;
        }

        log::info!(
            "Parsed {} DLLs with {} total imports",
            directory.descriptors.len(),
            directory.total_imports
        );

        Ok(directory)
    }

    /// Resolve all imports and patch IAT
    ///
    /// This function:
    /// 1. Loads each imported DLL
    /// 2. Resolves each function via GetProcAddress
    /// 3. Patches the IAT with resolved addresses
    ///
    /// # Arguments
    ///
    /// * `data` - Mutable PE file data
    /// * `iat_rvas` - IAT RVAs for each descriptor
    /// * `is_64bit` - True for x64 PE, false for x86
    ///
    /// # Returns
    /// Get IAT RVAs for each import descriptor
    ///
    /// # Returns
    ///
    /// `&Vec<u32>` - IAT RVAs (one per descriptor)
    pub fn get_iat_rvas(&self) -> &Vec<u32> {
        &self.iat_rvas
    }

    /// Resolve all imports and patch IAT
    ///
    /// This function:
    /// 1. Loads each imported DLL
    /// 2. Resolves each function via GetProcAddress
    /// 3. Patches the IAT with resolved addresses
    ///
    /// # Arguments
    ///
    /// * `data` - Mutable PE file data
    /// * `iat_rvas` - IAT RVAs for each descriptor
    /// * `is_64bit` - True for x64 PE, false for x86
    ///
    /// # Returns
    ///
    /// `Result<()>` - Success or error
    #[cfg(windows)]
    pub fn resolve_and_patch(
        &self,
        data: &mut [u8],
        iat_rvas: Vec<u32>,
        is_64bit: bool,
    ) -> anyhow::Result<()> {
        log::info!("Resolving {} imported DLLs...", self.descriptors.len());

        if self.descriptors.len() != self.iat_rvas.len() {
            anyhow::bail!(
                "Descriptor count {} != IAT RVA count {}",
                self.descriptors.len(),
                self.iat_rvas.len()
            );
        }

        for (idx, descriptor) in self.descriptors.iter().enumerate() {
            let _iat_rva = self.iat_rvas[idx];
            // Parse DLL name
            let dll_name_rva = descriptor.name;

            // Validate DLL name RVA is within bounds
            if dll_name_rva as usize >= data.len() {
                log::warn!(
                    "DLL name RVA 0x{:X} exceeds data length {}",
                    dll_name_rva,
                    data.len()
                );
                continue;
            }

            let dll_name_cstr =
                unsafe { CStr::from_ptr(data.as_ptr().add(dll_name_rva as usize).cast()) };
            // Use to_string_lossy() because DLL names may not be valid UTF-8
            let dll_name = dll_name_cstr.to_string_lossy().into_owned();
            log::debug!("Loading DLL: {}", dll_name);

            // Load DLL
            let dll_handle = unsafe { LoadLibraryA(dll_name.as_ptr()) };

            if dll_handle.is_null() {
                anyhow::bail!("Failed to load DLL: {}", dll_name);
            }

            // Get IAT location
            let iat_rva = iat_rvas[idx];
            let mut iat_offset = iat_rva as usize;

            // Resolve each import
            for import in &self.imports[idx] {
                let func_name = match import {
                    ImportEntry::ByName { name, .. } => name.clone(),
                    ImportEntry::ByOrdinal { ordinal } => {
                        // Ordinal imports use MAKEINTRESOURCE macro
                        let ordinal_value = (*ordinal as usize) | 0x8000;
                        format!("#{}", ordinal_value)
                    }
                };

                log::debug!("  Resolving: {}", func_name);

                // Get function address
                let func_address = unsafe {
                    match import {
                        ImportEntry::ByName { name, .. } => {
                            let name_cstr = CString::new(name.as_str())
                                .map_err(|e| anyhow::anyhow!("Invalid function name: {}", e))?;
                            GetProcAddress(dll_handle, name_cstr.as_ptr().cast())
                        }
                        ImportEntry::ByOrdinal { ordinal } => {
                            GetProcAddress(dll_handle, (*ordinal as usize | 0x8000) as *const u8)
                        }
                    }
                };

                if func_address.is_none() {
                    anyhow::bail!("Failed to resolve function: {} in {}", func_name, dll_name);
                }

                let func_ptr = func_address.unwrap() as *const ();

                // Patch IAT with resolved address
                if is_64bit {
                    // 64-bit: u64 address
                    let addr = func_ptr as u64;
                    if iat_offset + 8 > data.len() {
                        anyhow::bail!("IAT offset exceeds data size");
                    }
                    data[iat_offset..iat_offset + 8].copy_from_slice(&addr.to_le_bytes());
                    iat_offset += 8;
                } else {
                    // 32-bit: u32 address
                    let addr = func_ptr as u32;
                    if iat_offset + 4 > data.len() {
                        anyhow::bail!("IAT offset exceeds data size");
                    }
                    data[iat_offset..iat_offset + 4].copy_from_slice(&addr.to_le_bytes());
                    iat_offset += 4;
                }
            }
        }

        log::info!("All imports resolved and IAT patched successfully");

        Ok(())
    }

    /// Get statistics about imports
    ///
    /// # Returns
    ///
    /// `Vec<(String, usize)>` - List of (DLL name, import count)
    pub fn get_stats(&self) -> Vec<(String, usize)> {
        let mut stats = Vec::new();

        for (idx, descriptor) in self.descriptors.iter().enumerate() {
            if let Ok(dll_name_cstr) =
                unsafe { CStr::from_ptr(descriptor.name as *const i8).to_str() }
            {
                stats.push((dll_name_cstr.to_string(), self.imports[idx].len()));
            }
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_descriptor_null() {
        let data = [0u8; 20];
        let desc = ImportDescriptor::parse(&data, 0).unwrap();
        assert!(desc.is_null());
    }

    #[test]
    fn test_import_descriptor_not_null() {
        let mut data = [0u8; 20];
        data[12] = 1; // name RVA = 1
        let desc = ImportDescriptor::parse(&data, 0).unwrap();
        assert!(!desc.is_null());
        assert_eq!(desc.name, 1);
    }

    #[test]
    fn test_import_by_name_parse() {
        let mut data = vec![0u8; 20];
        // Hint: 0x1234
        data[0] = 0x34;
        data[1] = 0x12;
        // Name: "TestFunc\0"
        data[2..11].copy_from_slice(b"TestFunc\0");

        let import = ImportByName::parse(&data, 0).unwrap();
        assert_eq!(import.hint, 0x1234);
        assert_eq!(import.name, "TestFunc");
    }

    #[test]
    fn test_import_entry_by_name_32bit() {
        let mut data = vec![0u8; 300];
        // RVA to ImportByName: 0x100 (ordinal bit not set)
        data[0..4].copy_from_slice(&0x00000100u32.to_le_bytes());
        // ImportByName at offset 0x100
        data[0x100] = 0x12; // hint
        data[0x101] = 0x34;
        data[0x102..0x109].copy_from_slice(b"MyFunc\0");

        let import = ImportEntry::parse(&data, 0, false).unwrap();
        assert_eq!(
            import,
            ImportEntry::ByName {
                name: "MyFunc".to_string(),
                hint: 0x3412
            }
        );
    }

    #[test]
    fn test_import_entry_by_ordinal_32bit() {
        let mut data = vec![0u8; 100];
        // Ordinal import (high bit set): 0x80001234
        data[0..4].copy_from_slice(&0x80001234u32.to_le_bytes());

        let import = ImportEntry::parse(&data, 0, false).unwrap();
        assert_eq!(import, ImportEntry::ByOrdinal { ordinal: 0x1234 });
    }

    #[test]
    fn test_import_entry_by_name_64bit() {
        let mut data = vec![0u8; 300];
        // RVA to ImportByName: 0x100 (ordinal bit not set)
        data[0..8].copy_from_slice(&0x0000000000000100u64.to_le_bytes());
        // ImportByName at offset 0x100
        data[0x100] = 0x12; // hint
        data[0x101] = 0x34;
        data[0x102..0x109].copy_from_slice(b"MyFunc\0");

        let import = ImportEntry::parse(&data, 0, true).unwrap();
        assert_eq!(
            import,
            ImportEntry::ByName {
                name: "MyFunc".to_string(),
                hint: 0x3412
            }
        );
    }

    #[test]
    fn test_import_entry_null_terminator() {
        let data = [0u8; 4];
        let result = ImportEntry::parse(&data, 0, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("NULL"));
    }
}
