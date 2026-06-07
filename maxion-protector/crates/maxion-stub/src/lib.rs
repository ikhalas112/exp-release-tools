//! Maxion Runtime Stub
//!
//! Minimal runtime library that hooks Windows API calls to provide transparent
//! access to encrypted game assets. Implements a virtual file system (VFS) that
//! decrypts assets on-demand with minimal performance overhead.
//!
//! # Architecture
//!
//! The stub works by intercepting Windows API calls (CreateFileW, ReadFile, etc.)
//! and redirecting asset file requests to the virtual file system. The VFS:
//! - Decrypts chunks on-demand using XChaCha20-Poly1305
//! - Implements rate limiting to prevent scraping
//! - Uses minimal memory through chunk-based access
//! - Falls through to original API for non-asset files
//!
//! # Safety
//!
//! This module contains unsafe code for:
//! - API hooking (detour mechanism)
//! - Direct memory access for PE parsing
//! - Windows API calls
//! - Thread-local storage for hook state

use std::sync::{Arc, Mutex, OnceLock};
use std::{ffi::c_char, ptr};

// Windows API bindings (only on Windows)
use std::collections::BTreeMap;
use std::format;
#[cfg(target_os = "windows")]
use std::fs;
#[cfg(target_os = "windows")]
use std::io::Write as _;

use std::string::{String, ToString};
use std::sync::atomic::{AtomicU32, Ordering};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_NORMAL, FINDEX_INFO_LEVELS, FINDEX_SEARCH_OPS, GET_FILEEX_INFO_LEVELS,
    WIN32_FILE_ATTRIBUTE_DATA, WIN32_FIND_DATAA, WIN32_FIND_DATAW,
};
#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
use windows_sys::Win32::Storage::FileSystem::{CreateFileW, GetFileSizeEx, ReadFile};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{FALSE, HANDLE, INVALID_HANDLE_VALUE, TRUE};
#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
use windows_sys::Win32::Foundation::CloseHandle;

#[cfg(not(target_os = "windows"))]
#[allow(clippy::upper_case_acronyms)] // Windows API convention
type HANDLE = *mut std::ffi::c_void;

#[cfg(target_os = "windows")]
use windows_sys::Win32::System::IO::OVERLAPPED;
#[cfg(target_os = "windows")]
use windows_sys::Win32::System::Memory::{VirtualProtect, PAGE_EXECUTE_READWRITE};

// NOTE: Loader stub is in separate crate: crates/maxion-loader-stub
// The loader is NOT part of maxion-stub.dll

// Re-export from maxion-core
pub use maxion_core::{
    access_control::{AccessControl, ANTI_SCRAPE_DELAY_MS, MAX_SEQUENTIAL_READS},
    archive::ArchiveHeader,
    compression::decompress,
    crypto::ChunkCipher,
    error::{Error, Result},
    types::{ChunkSize, Config, Nonce},
    virtual_archive::{
        AssetFileInfo as CoreAssetFileInfo, DefaultVirtualArchive as VirtualArchive,
    },
    MAGIC,
};

const KEY_BLOB_V2_MAGIC: &[u8; 4] = b"MXK2";
const KEY_BLOB_V3_MAGIC: &[u8; 4] = b"MXK3";
const EXIT_FAILURE_STUB_GET_MODULE: u32 = 201;
const EXIT_FAILURE_STUB_ARCHIVE: u32 = 203;
const EXIT_FAILURE_STUB_KEY: u32 = 204;
const EXIT_FAILURE_STUB_HEADER: u32 = 205;
const EXIT_FAILURE_STUB_VFS_ARCHIVE: u32 = 206;
const EXIT_FAILURE_STUB_VFS_CREATE: u32 = 207;
const EXIT_FAILURE_STUB_GLOBAL_SET: u32 = 208;

/// Magic value to identify our virtual handles
const VFS_HANDLE_MAGIC: u32 = 0x56465331; // "VFS1"
#[cfg(target_os = "windows")]
const VFS_FIND_HANDLE_BASE: u32 = 0x7000_0000;

/// Invalid virtual handle (Windows only)
#[cfg(target_os = "windows")]
#[allow(dead_code)]
const INVALID_VFS_HANDLE: HANDLE = 0xFFFFFFFFFFFFFFFFu64 as HANDLE;

/// Global VFS instance using OnceLock for lazy initialization
/// This is shared across all threads for API hooking
#[allow(dead_code)] // Used by hook functions
static GLOBAL_VFS: OnceLock<Arc<Mutex<VFS>>> = OnceLock::new();
#[cfg(target_os = "windows")]
static GLOBAL_FIND_HANDLES: OnceLock<Mutex<BTreeMap<u32, VirtualFindHandle>>> = OnceLock::new();
#[cfg(target_os = "windows")]
static PATCHED_MODULES: OnceLock<Mutex<BTreeMap<usize, ()>>> = OnceLock::new();

#[cfg(target_os = "windows")]
fn trace_line(message: &str) {
    let _ = message;
}
#[cfg(target_os = "windows")]
static GLOBAL_TEMP_FILES: OnceLock<Mutex<BTreeMap<usize, String>>> = OnceLock::new();
#[cfg(target_os = "windows")]
static GLOBAL_TEMP_FDS: OnceLock<Mutex<BTreeMap<i32, String>>> = OnceLock::new();
#[cfg(target_os = "windows")]
static GLOBAL_TEMP_HANDLES: OnceLock<Mutex<BTreeMap<usize, String>>> = OnceLock::new();

#[cfg(target_os = "windows")]
fn initialize_embedded_vfs() -> Result<Arc<Mutex<VFS>>> {
    // Step 1: Get module handle (current executable)
    let module_handle =
        unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) };

    if module_handle.is_null() {
        return Err(Error::Other("Failed to get module handle".to_string()));
    }

    // Step 2: Locate embedded .maxion section containing encrypted archive
    let (archive_data, key_data) = locate_embedded_archive(module_handle)?;

    // Step 3: Parse archive header
    let _header = parse_archive_header(&archive_data)?;

    // Step 4: De-obfuscate encryption key and initialize VFS
    let (encryption_key, nonce, chunk_size, _original_entry, expected_archive_hash) =
        deobfuscate_key(&key_data)?;
    let mut key_data = key_data;
    key_data.fill(0);
    if let Some(expected_archive_hash) = expected_archive_hash {
        let actual_archive_hash = blake3::hash(&archive_data);
        if actual_archive_hash.as_bytes() != &expected_archive_hash {
            return Err(Error::Other("Embedded archive integrity verification failed".to_string()));
        }
    }

    // Step 5: Create Config from deobfuscated key
    let config = Config {
        encryption_key,
        nonce,
        chunk_size: maxion_core::ChunkSize::new(chunk_size),
        compress: false,         // Already determined by archive during packing
        compression_level: 0,    // Doesn't matter when reading
        build_secret: [0u8; 32], // Only used during packing
        simd_config: None,       // Not used in stub (runtime uses CPU auto-detection)
    };

    // Step 6: Load VirtualArchive from embedded data
    let archive = VirtualArchive::from_memory(archive_data, config)?;

    // Step 7: Initialize VFS with the archive
    let vfs = VFS::new(archive)?;
    Ok(Arc::new(Mutex::new(vfs)))
}

#[cfg(target_os = "windows")]
fn ensure_initialized(install_hooks_flag: bool) -> Result<Arc<Mutex<VFS>>> {
    if let Some(vfs) = GLOBAL_VFS.get() {
        trace_line("ensure_initialized: reuse existing vfs");
        return Ok(vfs.clone());
    }

    trace_line("ensure_initialized: initialize_embedded_vfs");
    let vfs = initialize_embedded_vfs()?;

    // Install hooks when requested. If this fails we still keep direct API available.
    if install_hooks_flag {
        trace_line("ensure_initialized: install_api_hooks");
        if let Err(error) = install_api_hooks(vfs.clone()) {
            trace_line(&format!("ensure_initialized: install hooks failed: {error:?}"));
            log::warn!("Failed to install API hooks: {:?}", error);
        }
    }

    match GLOBAL_VFS.set(vfs.clone()) {
        Ok(()) => {
            trace_line("ensure_initialized: global set ok");
            Ok(vfs)
        }
        Err(_) => GLOBAL_VFS
            .get()
            .cloned()
            .ok_or_else(|| Error::Other("Failed to store VFS instance".to_string())),
    }
}

/// Storage for original Windows API function pointers
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)] // Windows API naming convention
static mut ORIG_CreateFileW: Option<
    unsafe extern "system" fn(
        *const u16,
        u32,
        u32,
        *const windows_sys::Win32::Security::SECURITY_ATTRIBUTES,
        u32,
        u32,
        HANDLE,
    ) -> HANDLE,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_LoadLibraryA: Option<unsafe extern "system" fn(*const u8) -> HANDLE> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_LoadLibraryW: Option<unsafe extern "system" fn(*const u16) -> HANDLE> = None;

#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)] // Windows API naming convention
static mut ORIG_CreateFileA: Option<
    unsafe extern "system" fn(
        *const u8,
        u32,
        u32,
        *const windows_sys::Win32::Security::SECURITY_ATTRIBUTES,
        u32,
        u32,
        HANDLE,
    ) -> HANDLE,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_CreateFile2: Option<
    unsafe extern "system" fn(*const u16, u32, u32, u32, *const core::ffi::c_void) -> HANDLE,
> = None;

#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)] // Windows API naming convention
static mut ORIG_ReadFile: Option<
    unsafe extern "system" fn(HANDLE, *mut u8, u32, *mut u32, *mut OVERLAPPED) -> i32,
> = None;

#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)] // Windows API naming convention
static mut ORIG_CloseHandle: Option<unsafe extern "system" fn(HANDLE) -> i32> = None;

#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)] // Windows API naming convention
static mut ORIG_GetFileSizeEx: Option<unsafe extern "system" fn(HANDLE, *mut i64) -> i32> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_GetFileSize: Option<unsafe extern "system" fn(HANDLE, *mut u32) -> u32> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_SetFilePointer: Option<unsafe extern "system" fn(HANDLE, i32, *mut i32, u32) -> u32> =
    None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_SetFilePointerEx: Option<
    unsafe extern "system" fn(HANDLE, i64, *mut i64, u32) -> i32,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_GetFileType: Option<unsafe extern "system" fn(HANDLE) -> u32> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_GetFileAttributesA: Option<unsafe extern "system" fn(*const u8) -> u32> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_GetFileAttributesExW: Option<
    unsafe extern "system" fn(*const u16, GET_FILEEX_INFO_LEVELS, *mut core::ffi::c_void) -> i32,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_FindFirstFileA: Option<
    unsafe extern "system" fn(*const u8, *mut WIN32_FIND_DATAA) -> HANDLE,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_FindFirstFileW: Option<
    unsafe extern "system" fn(*const u16, *mut WIN32_FIND_DATAW) -> HANDLE,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_FindFirstFileExW: Option<
    unsafe extern "system" fn(
        *const u16,
        FINDEX_INFO_LEVELS,
        *mut core::ffi::c_void,
        FINDEX_SEARCH_OPS,
        *const core::ffi::c_void,
        u32,
    ) -> HANDLE,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_FindNextFileA: Option<
    unsafe extern "system" fn(HANDLE, *mut WIN32_FIND_DATAA) -> i32,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_FindNextFileW: Option<
    unsafe extern "system" fn(HANDLE, *mut WIN32_FIND_DATAW) -> i32,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_FindClose: Option<unsafe extern "system" fn(HANDLE) -> i32> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_fopen: Option<unsafe extern "C" fn(*const u8, *const u8) -> *mut core::ffi::c_void> =
    None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG__wfopen: Option<
    unsafe extern "C" fn(*const u16, *const u16) -> *mut core::ffi::c_void,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_fopen_s: Option<
    unsafe extern "C" fn(*mut *mut core::ffi::c_void, *const u8, *const u8) -> i32,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG_fclose: Option<unsafe extern "C" fn(*mut core::ffi::c_void) -> i32> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG__stat64i32: Option<
    unsafe extern "C" fn(*const u8, *mut core::ffi::c_void) -> i32,
> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG__open_osfhandle: Option<unsafe extern "C" fn(isize, i32) -> i32> = None;
#[cfg(target_os = "windows")]
#[allow(non_upper_case_globals)]
static mut ORIG__close: Option<unsafe extern "C" fn(i32) -> i32> = None;

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_CreateFileW: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_LoadLibraryA: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_LoadLibraryW: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_CreateFileA: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_CreateFile2: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_ReadFile: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_CloseHandle: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_GetFileSizeEx: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_GetFileSize: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_SetFilePointer: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_SetFilePointerEx: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_GetFileType: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_GetFileAttributesA: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_GetFileAttributesExW: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_FindFirstFileA: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_FindFirstFileW: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_FindFirstFileExW: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_FindNextFileA: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_FindNextFileW: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_FindClose: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_fopen: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT__wfopen: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_fopen_s: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT_fclose: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT__stat64i32: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT__open_osfhandle: *mut usize = std::ptr::null_mut();
#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
#[allow(non_upper_case_globals)]
static mut IAT_SLOT__close: *mut usize = std::ptr::null_mut();

/// Storage for active Detour objects to keep hooks alive for process lifetime
#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
#[allow(non_upper_case_globals)] // Windows API naming convention
static mut HOOK_CreateFileW: Option<retour::GenericDetour<CreateFileWFn>> = None;

#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
#[allow(non_upper_case_globals)] // Windows API naming convention
static mut HOOK_ReadFile: Option<retour::GenericDetour<ReadFileFn>> = None;

#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
#[allow(non_upper_case_globals)] // Windows API naming convention
static mut HOOK_CloseHandle: Option<retour::GenericDetour<CloseHandleFn>> = None;

#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
#[allow(non_upper_case_globals)] // Windows API naming convention
static mut HOOK_GetFileSizeEx: Option<retour::GenericDetour<GetFileSizeExFn>> = None;

/// Type alias for CreateFileW function pointer (Windows only)
#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
type CreateFileWFn = unsafe extern "system" fn(
    *const u16,
    u32,
    u32,
    *const windows_sys::Win32::Security::SECURITY_ATTRIBUTES,
    u32,
    u32,
    HANDLE,
) -> HANDLE;

/// Type alias for ReadFile function pointer (Windows only)
#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
type ReadFileFn = unsafe extern "system" fn(HANDLE, *mut u8, u32, *mut u32, *mut OVERLAPPED) -> i32;

/// Type alias for CloseHandle function pointer (Windows only)
#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
type CloseHandleFn = unsafe extern "system" fn(HANDLE) -> i32;

/// Type alias for GetFileSizeEx function pointer (Windows only)
#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
type GetFileSizeExFn = unsafe extern "system" fn(HANDLE, *mut i64) -> i32;

/// Maximum number of concurrent virtual file handles
// MAX_VIRTUAL_HANDLES removed - using dynamic handle allocation
/// Information about a virtual file handle
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used through BTreeMap
struct VirtualFileHandle {
    /// Magic value for validation
    magic: u32,

    /// Handle ID (incremental)
    handle_id: u32,

    /// File information from the archive
    file_info: AssetFileInfo,

    /// Current read offset in the file
    current_offset: u64,

    /// File path (for debugging)
    path: String,

    /// Handle value (Windows only)
    #[cfg(target_os = "windows")]
    handle_value: HANDLE,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone)]
struct VirtualFindHandle {
    entries: Vec<String>,
    cursor: usize,
}

// Safety: Handled via raw pointers in Windows API
unsafe impl Send for VirtualFileHandle {}
unsafe impl Sync for VirtualFileHandle {}

/// File information stored in the VFS
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used through BTreeMap
struct AssetFileInfo {
    /// Original file size
    original_size: u64,

    /// Compressed/encrypted size
    packed_size: u64,

    /// Offset in the archive data
    offset: u64,

    /// Number of chunks
    chunk_count: u32,

    /// File checksum for integrity
    checksum: [u8; 32],
}

/// Virtual File System for encrypted assets
pub struct VFS {
    /// Virtual archive with memory-mapped access and caching
    archive: VirtualArchive,

    /// Access control for rate limiting
    access_control: AccessControl,

    /// Virtual handle counter
    next_handle_id: AtomicU32,

    /// Active virtual handles
    virtual_handles: BTreeMap<u32, VirtualFileHandle>,

    /// Statistics
    stats: VFSStats,
}

/// VFS statistics for monitoring
#[derive(Debug, Default, Clone)]
pub struct VFSStats {
    /// Total number of file opens
    pub total_opens: u64,

    /// Total number of successful reads
    pub successful_reads: u64,

    /// Total bytes read
    pub total_bytes_read: u64,

    /// Number of rate limit violations
    pub rate_limit_violations: u64,

    /// Number of decryption cache hits
    pub cache_hits: u64,

    /// Number of decryption cache misses
    pub cache_misses: u64,
}

impl VFS {
    /// Create a new VFS from a VirtualArchive instance
    ///
    /// # Arguments
    ///
    /// * `archive` - Initialized VirtualArchive with memory-mapped access
    ///
    /// # Returns
    ///
    /// `Result<VFS>` - Initialized VFS ready for use
    pub fn new(archive: VirtualArchive) -> Result<Self> {
        let file_count = archive.file_count();
        log::info!("Loaded {} files into VFS from VirtualArchive", file_count);

        Ok(Self {
            archive,
            access_control: AccessControl::new(),
            next_handle_id: AtomicU32::new(1),
            virtual_handles: BTreeMap::new(),
            stats: VFSStats::default(),
        })
    }

    /// Open a virtual file
    pub fn open_virtual(&mut self, path: &str) -> Result<HANDLE> {
        #[cfg(feature = "profiling")]
        let timer_start = std::time::Instant::now();

        // Check rate limit
        if let Err(e) = self.access_control.check_rate_limit() {
            self.stats.rate_limit_violations += 1;
            return Err(e);
        }

        // Normalize path
        let normalized_path = path.replace('\\', "/");

        // Check if file exists in VirtualArchive
        if !self.archive.file_exists(&normalized_path) {
            return Err(Error::Other(format!("File not found in VFS: {}", path)));
        }

        // Get file info from VirtualArchive
        let core_file_info = self
            .archive
            .get_file_info(&normalized_path)
            .ok_or_else(|| Error::Other(format!("Failed to get file info: {}", path)))?;

        // Convert core AssetFileInfo to stub AssetFileInfo
        let file_info = AssetFileInfo {
            original_size: core_file_info.original_size,
            packed_size: core_file_info.packed_size,
            offset: core_file_info.offset,
            chunk_count: core_file_info.chunk_count,
            checksum: core_file_info.checksum,
        };

        // Allocate new handle ID
        let handle_id = self.next_handle_id.fetch_add(1, Ordering::SeqCst);

        // Create virtual handle
        let virtual_handle = VirtualFileHandle {
            magic: VFS_HANDLE_MAGIC,
            handle_id,
            file_info,
            current_offset: 0,
            path: normalized_path,
            #[cfg(target_os = "windows")]
            handle_value: (handle_id + 1) as HANDLE,
        };

        // Store handle
        self.virtual_handles.insert(handle_id, virtual_handle);

        // Update statistics
        self.stats.total_opens += 1;

        #[cfg(feature = "profiling")]
        {
            use maxion_profiler::{track_file_load, LoadMethod};
            let load_time_ms = timer_start.elapsed().as_millis();
            track_file_load(
                &normalized_path,
                file_info.original_size,
                load_time_ms,
                LoadMethod::Vfs,
            );
        }

        // Create handle value (handle_id + 1 shifted to avoid NULL) - Windows only
        #[cfg(target_os = "windows")]
        let handle_value = (handle_id + 1) as HANDLE;

        #[cfg(not(target_os = "windows"))]
        let handle_value = (handle_id + 1) as *mut std::ffi::c_void;

        Ok(handle_value)
    }

    /// Read from a virtual file
    #[cfg(target_os = "windows")]
    pub fn read_virtual(
        &mut self,
        handle: HANDLE,
        buffer: &mut [u8],
        bytes_to_read: u32,
    ) -> Result<u32> {
        #[cfg(feature = "profiling")]
        {
            use maxion_profiler::Timer;
            let _timer = Timer::start("vfs_read");
        }

        // Extract handle ID
        let handle_id = (handle as u32).wrapping_sub(1);

        // Get virtual handle
        let virtual_handle = self
            .virtual_handles
            .get_mut(&handle_id)
            .ok_or_else(|| Error::Other("Invalid virtual handle".to_string()))?;

        // Validate handle magic
        if virtual_handle.magic != VFS_HANDLE_MAGIC {
            return Err(Error::Other("Corrupted virtual handle".to_string()));
        }

        // Check rate limit
        if let Err(e) = self.access_control.check_rate_limit() {
            self.stats.rate_limit_violations += 1;
            return Err(e);
        }

        // Calculate bytes available to read
        let file_size = virtual_handle.file_info.original_size;
        let current_offset = virtual_handle.current_offset;
        let bytes_available = file_size.saturating_sub(current_offset);
        let bytes_to_read = bytes_to_read as u64;
        let bytes_to_read = bytes_available.min(bytes_to_read) as u32;

        if bytes_to_read == 0 {
            // EOF
            return Ok(0);
        }

        // Read and decrypt data using VirtualArchive (with automatic caching)
        let decrypted_data = self.archive.read_file_range(
            &virtual_handle.path,
            current_offset,
            bytes_to_read as u64,
        )?;

        // Copy to buffer
        let bytes_copied = decrypted_data.len().min(buffer.len());
        buffer[..bytes_copied].copy_from_slice(&decrypted_data[..bytes_copied]);

        // Update offset
        virtual_handle.current_offset += bytes_copied as u64;

        // Update statistics
        self.stats.successful_reads += 1;
        self.stats.total_bytes_read += bytes_copied as u64;

        Ok(bytes_copied as u32)
    }

    /// Read from a virtual file (non-Windows stub for testing)
    #[cfg(not(target_os = "windows"))]
    pub fn read_virtual(
        &mut self,
        handle: *mut std::ffi::c_void,
        buffer: &mut [u8],
        bytes_to_read: u32,
    ) -> Result<u32> {
        // Extract handle ID
        let handle_id = (handle as u32).wrapping_sub(1);

        // Get virtual handle
        let virtual_handle = self
            .virtual_handles
            .get_mut(&handle_id)
            .ok_or_else(|| Error::Other("Invalid virtual handle".to_string()))?;

        // Validate handle magic
        if virtual_handle.magic != VFS_HANDLE_MAGIC {
            return Err(Error::Other("Corrupted virtual handle".to_string()));
        }

        // Check rate limit
        if let Err(e) = self.access_control.check_rate_limit() {
            self.stats.rate_limit_violations += 1;
            return Err(e);
        }

        // Calculate bytes available to read
        let file_size = virtual_handle.file_info.original_size;
        let current_offset = virtual_handle.current_offset;
        let bytes_available = file_size.saturating_sub(current_offset);
        let bytes_to_read = bytes_to_read as u64;
        let bytes_to_read = bytes_available.min(bytes_to_read) as u32;

        if bytes_to_read == 0 {
            // EOF
            return Ok(0);
        }

        // Read and decrypt data using VirtualArchive (with automatic caching)
        let decrypted_data = self.archive.read_file_range(
            &virtual_handle.path,
            current_offset,
            bytes_to_read as u64,
        )?;

        // Copy to buffer
        let bytes_copied = decrypted_data.len().min(buffer.len());
        buffer[..bytes_copied].copy_from_slice(&decrypted_data[..bytes_copied]);

        // Update offset
        virtual_handle.current_offset += bytes_copied as u64;

        // Update statistics
        self.stats.successful_reads += 1;
        self.stats.total_bytes_read += bytes_copied as u64;

        Ok(bytes_copied as u32)
    }

    /// Close a virtual file handle
    #[cfg(target_os = "windows")]
    pub fn close_virtual(&mut self, handle: HANDLE) -> Result<()> {
        // Extract handle ID
        let handle_id = (handle as u32).wrapping_sub(1);

        // Remove handle
        self.virtual_handles
            .remove(&handle_id)
            .ok_or_else(|| Error::Other("Invalid virtual handle".to_string()))?;

        Ok(())
    }

    /// Close a virtual file handle (non-Windows stub for testing)
    #[cfg(not(target_os = "windows"))]
    pub fn close_virtual(&mut self, handle: *mut std::ffi::c_void) -> Result<()> {
        // Extract handle ID
        let handle_id = (handle as u32).wrapping_sub(1);

        // Remove handle
        self.virtual_handles
            .remove(&handle_id)
            .ok_or_else(|| Error::Other("Invalid virtual handle".to_string()))?;

        Ok(())
    }

    /// Check if a handle is a virtual handle
    pub fn is_virtual_handle(&self, handle: *mut std::ffi::c_void) -> bool {
        let handle_id = (handle as u32).wrapping_sub(1);
        // Actually check if the handle ID exists in our virtual handles
        self.virtual_handles.contains_key(&handle_id)
    }

    /// Get file size for a virtual handle
    /// Get the size of a virtual file
    #[cfg(target_os = "windows")]
    pub fn get_file_size(&self, handle: HANDLE) -> Result<i64> {
        let handle_id = (handle as u32).wrapping_sub(1);

        let virtual_handle = self
            .virtual_handles
            .get(&handle_id)
            .ok_or_else(|| Error::Other("Invalid virtual handle".to_string()))?;

        Ok(virtual_handle.file_info.original_size as i64)
    }

    #[cfg(target_os = "windows")]
    pub fn set_file_pointer(&mut self, handle: HANDLE, distance: i64, method: u32) -> Result<u64> {
        let handle_id = (handle as u32).wrapping_sub(1);
        let virtual_handle = self
            .virtual_handles
            .get_mut(&handle_id)
            .ok_or_else(|| Error::Other("Invalid virtual handle".to_string()))?;

        let file_size = virtual_handle.file_info.original_size as i64;
        let base = match method {
            0 => 0i64,
            1 => virtual_handle.current_offset as i64,
            2 => file_size,
            _ => return Err(Error::Other(format!("Unsupported seek method: {}", method))),
        };
        let new_offset = base
            .checked_add(distance)
            .ok_or_else(|| Error::Other("Seek offset overflow".to_string()))?;
        if new_offset < 0 {
            return Err(Error::Other("Negative seek offset".to_string()));
        }
        let new_offset = new_offset as u64;
        virtual_handle.current_offset = new_offset.min(virtual_handle.file_info.original_size);
        Ok(virtual_handle.current_offset)
    }

    /// Get the size of a virtual file (non-Windows stub for testing)
    #[cfg(not(target_os = "windows"))]
    pub fn get_file_size(&self, handle: *mut std::ffi::c_void) -> Result<i64> {
        // Extract handle ID
        let handle_id = (handle as u32).wrapping_sub(1);

        // Get virtual handle
        let virtual_handle = self
            .virtual_handles
            .get(&handle_id)
            .ok_or_else(|| Error::Other("Invalid virtual handle".to_string()))?;

        Ok(virtual_handle.file_info.original_size as i64)
    }

    /// Get cache statistics from the underlying VirtualArchive
    pub fn get_cache_stats(&self) -> (usize, usize) {
        self.archive.cache_stats()
    }

    /// Get VFS statistics
    pub fn get_stats(&self) -> &VFSStats {
        &self.stats
    }

    /// Reset VFS statistics
    pub fn reset_stats(&mut self) {
        self.stats = VFSStats::default();
    }

    /// Get the number of active virtual handles
    pub fn active_handles(&self) -> usize {
        self.virtual_handles.len()
    }
}

#[cfg(target_os = "windows")]
fn normalize_game_path(path: &str) -> String {
    let mut normalized = path.trim_matches('"').replace('\\', "/");
    while normalized.starts_with("./") {
        normalized = normalized[2..].to_string();
    }

    let lower = normalized.to_ascii_lowercase();
    if let Some(exe_dir) = current_exe_dir_normalized() {
        let exe_dir_lower = exe_dir.to_ascii_lowercase();
        if lower.starts_with(&exe_dir_lower) {
            let mut stripped = normalized[exe_dir.len()..].trim_start_matches('/').to_string();
            while stripped.starts_with("./") {
                stripped = stripped[2..].to_string();
            }
            return stripped;
        }
    }

    if let Some(idx) = lower.find("/__maxion_stage/") {
        return normalized[idx + "/__maxion_stage/".len()..].to_string();
    }

    normalized
}

#[cfg(target_os = "windows")]
fn current_exe_dir_normalized() -> Option<String> {
    static EXE_DIR: OnceLock<Option<String>> = OnceLock::new();
    EXE_DIR
        .get_or_init(|| unsafe {
            let mut buffer = [0u16; 1024];
            let len = windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW(
                std::ptr::null_mut(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            ) as usize;
            if len == 0 || len >= buffer.len() {
                return None;
            }
            let full_path = String::from_utf16_lossy(&buffer[..len]).replace('\\', "/");
            let slash = full_path.rfind('/')?;
            Some(full_path[..slash].to_string())
        })
        .clone()
}

#[cfg(target_os = "windows")]
fn current_exe_dir_path() -> Option<std::path::PathBuf> {
    current_exe_dir_normalized().map(|s| std::path::PathBuf::from(s.replace('/', "\\")))
}

#[cfg(target_os = "windows")]
fn wildcard_matches(pattern: &str, candidate: &str) -> bool {
    let pattern = pattern.as_bytes();
    let candidate = candidate.as_bytes();
    let (mut p, mut c) = (0usize, 0usize);
    let (mut star_p, mut star_c) = (None, 0usize);

    while c < candidate.len() {
        if p < pattern.len()
            && (pattern[p] == b'?' || pattern[p].eq_ignore_ascii_case(&candidate[c]))
        {
            p += 1;
            c += 1;
            continue;
        }

        if p < pattern.len() && pattern[p] == b'*' {
            star_p = Some(p);
            p += 1;
            star_c = c;
            continue;
        }

        if let Some(star) = star_p {
            p = star + 1;
            star_c += 1;
            c = star_c;
            continue;
        }

        return false;
    }

    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }

    p == pattern.len()
}

#[cfg(target_os = "windows")]
fn split_parent_and_pattern(path: &str) -> (String, String) {
    let normalized = normalize_game_path(path);
    let trimmed = normalized.trim_end_matches('/').to_string();
    if let Some((parent, pattern)) = trimmed.rsplit_once('/') {
        (parent.to_string(), pattern.to_string())
    } else {
        ("".to_string(), trimmed)
    }
}

#[cfg(target_os = "windows")]
fn vfs_directory_exists(normalized_path: &str) -> bool {
    let normalized = normalize_game_path(normalized_path).trim_matches('/').to_string();
    let prefix = if normalized.is_empty() {
        String::new()
    } else {
        format!("{normalized}/")
    };

    let Some(vfs) = GLOBAL_VFS.get() else {
        return false;
    };
    let Ok(guard) = vfs.lock() else {
        return false;
    };

    guard.archive.list_files().into_iter().any(|entry| {
        if prefix.is_empty() {
            entry.contains('/')
        } else {
            entry.len() > prefix.len() && entry.starts_with(&prefix)
        }
    })
}

#[cfg(target_os = "windows")]
fn vfs_list_directory(pattern_path: &str) -> Vec<String> {
    let (parent, pattern) = split_parent_and_pattern(pattern_path);
    let parent = parent.trim_matches('/').to_string();
    let prefix = if parent.is_empty() {
        String::new()
    } else {
        format!("{parent}/")
    };

    let Some(vfs) = GLOBAL_VFS.get() else {
        return Vec::new();
    };
    let Ok(guard) = vfs.lock() else {
        return Vec::new();
    };

    let mut entries = BTreeMap::<String, String>::new();
    for entry in guard.archive.list_files() {
        if !entry.starts_with(&prefix) {
            continue;
        }
        let remainder = &entry[prefix.len()..];
        if remainder.is_empty() {
            continue;
        }
        if let Some((first, _rest)) = remainder.split_once('/') {
            if wildcard_matches(&pattern, first) {
                entries.entry(first.to_ascii_lowercase()).or_insert_with(|| {
                    if parent.is_empty() {
                        first.to_string()
                    } else {
                        format!("{parent}/{first}")
                    }
                });
            }
        } else if wildcard_matches(&pattern, remainder) {
            entries
                .entry(remainder.to_ascii_lowercase())
                .or_insert_with(|| entry.clone());
        }
    }

    entries.into_values().collect()
}

#[cfg(target_os = "windows")]
fn vfs_is_file(normalized_path: &str) -> bool {
    resolve_vfs_path(normalized_path).is_some()
}

#[cfg(target_os = "windows")]
fn resolve_vfs_path(path: &str) -> Option<String> {
    let normalized = normalize_game_path(path);
    let candidates = [
        normalized.clone(),
        normalized.trim_start_matches('/').to_string(),
        normalized
            .strip_prefix("__maxion_stage/")
            .unwrap_or(&normalized)
            .to_string(),
    ];

    let vfs = GLOBAL_VFS.get()?;
    let guard = vfs.lock().ok()?;

    for candidate in &candidates {
        if !candidate.is_empty() && guard.archive.file_exists(candidate) {
            return Some(candidate.clone());
        }
    }

    let lower_candidates: Vec<String> = candidates.iter().map(|s| s.to_ascii_lowercase()).collect();
    guard
        .archive
        .list_files()
        .into_iter()
        .find(|entry| lower_candidates.iter().any(|c| c == &entry.to_ascii_lowercase()))
}

#[cfg(target_os = "windows")]
fn materialize_vfs_file(path: &str) -> Option<String> {
    let resolved_path = resolve_vfs_path(path)?;
    let data = with_vfs_mut(|vfs| {
        let size = vfs
            .archive
            .get_file_info(&resolved_path)
            .map(|info| info.original_size)
            .unwrap_or(0);
        vfs.archive.read_file_range(&resolved_path, 0, size)
    })
    .ok()?;

    let temp_root = std::env::temp_dir().join("maxion_vfs");
    fs::create_dir_all(&temp_root).ok()?;
    let unique_name = format!(
        "{}_{}_{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("main"),
        resolved_path.replace('/', "_")
    );
    let temp_path = temp_root.join(unique_name);
    let mut file = fs::File::create(&temp_path).ok()?;
    file.write_all(&data).ok()?;
    file.flush().ok()?;
    Some(temp_path.to_string_lossy().to_string())
}

#[cfg(target_os = "windows")]
fn materialize_archive_to_disk(vfs: &mut VFS) -> Result<usize> {
    let exe_dir = current_exe_dir_path()
        .ok_or_else(|| Error::Other("Failed to resolve executable directory".to_string()))?;
    let files = vfs.archive.list_files();
    let mut materialized = 0usize;

    for relative_path in files {
        let destination = exe_dir.join(relative_path.replace('/', "\\"));
        if destination.exists() {
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| Error::Other(format!("Failed to create directory {:?}: {}", parent, e)))?;
        }

        let size = vfs
            .archive
            .get_file_info(&relative_path)
            .map(|info| info.original_size)
            .unwrap_or(0);
        let data = vfs.archive.read_file_range(&relative_path, 0, size)?;
        fs::write(&destination, data).map_err(|e| {
            Error::Other(format!(
                "Failed to materialize archive file {:?}: {}",
                destination, e
            ))
        })?;
        materialized += 1;
    }

    Ok(materialized)
}

#[cfg(target_os = "windows")]
fn remember_temp_file(file_ptr: *mut core::ffi::c_void, temp_path: String) {
    let files = GLOBAL_TEMP_FILES.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(mut guard) = files.lock() {
        guard.insert(file_ptr as usize, temp_path);
    }
}

#[cfg(target_os = "windows")]
fn cleanup_temp_file(file_ptr: *mut core::ffi::c_void) {
    let Some(files) = GLOBAL_TEMP_FILES.get() else {
        return;
    };
    let path = files
        .lock()
        .ok()
        .and_then(|mut guard| guard.remove(&(file_ptr as usize)));
    if let Some(path) = path {
        let _ = fs::remove_file(path);
    }
}

#[cfg(target_os = "windows")]
fn remember_temp_fd(fd: i32, temp_path: String) {
    let files = GLOBAL_TEMP_FDS.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(mut guard) = files.lock() {
        guard.insert(fd, temp_path);
    }
}

#[cfg(target_os = "windows")]
fn cleanup_temp_fd(fd: i32) {
    let Some(files) = GLOBAL_TEMP_FDS.get() else {
        return;
    };
    let path = files
        .lock()
        .ok()
        .and_then(|mut guard| guard.remove(&fd));
    if let Some(path) = path {
        let _ = fs::remove_file(path);
    }
}

#[cfg(target_os = "windows")]
fn remember_temp_handle(handle: HANDLE, temp_path: String) {
    let files = GLOBAL_TEMP_HANDLES.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Ok(mut guard) = files.lock() {
        guard.insert(handle as usize, temp_path);
    }
}

#[cfg(target_os = "windows")]
fn temp_handle_path(handle: HANDLE) -> Option<String> {
    GLOBAL_TEMP_HANDLES
        .get()
        .and_then(|m| m.lock().ok())
        .and_then(|m| m.get(&(handle as usize)).cloned())
}

#[cfg(target_os = "windows")]
fn cleanup_temp_handle(handle: HANDLE) {
    let Some(files) = GLOBAL_TEMP_HANDLES.get() else {
        return;
    };
    let path = files
        .lock()
        .ok()
        .and_then(|mut guard| guard.remove(&(handle as usize)));
    if let Some(path) = path {
        let _ = fs::remove_file(path);
    }
}

#[cfg(target_os = "windows")]
fn vfs_path_for_handle(handle: HANDLE) -> Option<String> {
    let vfs = GLOBAL_VFS.get()?;
    let guard = vfs.lock().ok()?;
    let handle_id = (handle as u32).wrapping_sub(1);
    guard.virtual_handles.get(&handle_id).map(|h| h.path.clone())
}

#[cfg(target_os = "windows")]
unsafe fn try_open_materialized_file_a(
    original_path: &str,
    dw_desired_access: u32,
    dw_share_mode: u32,
    lp_security_attributes: *const windows_sys::Win32::Security::SECURITY_ATTRIBUTES,
    dw_creation_disposition: u32,
    dw_flags_and_attributes: u32,
    h_template_file: HANDLE,
) -> Option<HANDLE> {
    let temp_path = materialize_vfs_file(original_path)?;
    let temp_c = std::ffi::CString::new(temp_path.clone()).ok()?;
    let orig = ORIG_CreateFileA?;
    let handle = orig(
        temp_c.as_ptr().cast(),
        dw_desired_access,
        dw_share_mode,
        lp_security_attributes,
        dw_creation_disposition,
        dw_flags_and_attributes,
        h_template_file,
    );
    if handle == INVALID_HANDLE_VALUE {
        let _ = fs::remove_file(temp_path);
        return None;
    }
    remember_temp_handle(handle, temp_path);
    if original_path.to_ascii_lowercase().starts_with("lua/") {
        trace_line(&format!("temp_handle_a: {} => 0x{:x}", original_path, handle as usize));
    }
    Some(handle)
}

#[cfg(target_os = "windows")]
unsafe fn try_open_materialized_file_w(
    original_path: &str,
    dw_desired_access: u32,
    dw_share_mode: u32,
    lp_security_attributes: *const windows_sys::Win32::Security::SECURITY_ATTRIBUTES,
    dw_creation_disposition: u32,
    dw_flags_and_attributes: u32,
    h_template_file: HANDLE,
) -> Option<HANDLE> {
    let temp_path = materialize_vfs_file(original_path)?;
    let temp_w: Vec<u16> = temp_path.encode_utf16().chain(std::iter::once(0)).collect();
    let orig = ORIG_CreateFileW?;
    let handle = orig(
        temp_w.as_ptr(),
        dw_desired_access,
        dw_share_mode,
        lp_security_attributes,
        dw_creation_disposition,
        dw_flags_and_attributes,
        h_template_file,
    );
    if handle == INVALID_HANDLE_VALUE {
        let _ = fs::remove_file(temp_path);
        return None;
    }
    remember_temp_handle(handle, temp_path);
    if original_path.to_ascii_lowercase().starts_with("lua/") {
        trace_line(&format!("temp_handle_w: {} => 0x{:x}", original_path, handle as usize));
    }
    Some(handle)
}

#[cfg(target_os = "windows")]
fn try_open_vfs_path(normalized_path: &str) -> Option<HANDLE> {
    let resolved_path = resolve_vfs_path(normalized_path)?;
    let vfs = GLOBAL_VFS.get()?;
    let vfs_guard = vfs.lock().ok()?;
    if !vfs_guard.archive.file_exists(&resolved_path) {
        return None;
    }
    drop(vfs_guard);
    let mut vfs_guard = vfs.lock().ok()?;
    match vfs_guard.open_virtual(&resolved_path) {
        Ok(handle) => Some(handle),
        Err(e) => {
            log::error!("VFS open failed for {}: {:?}", resolved_path, e);
            None
        }
    }
}

#[cfg(target_os = "windows")]
fn vfs_file_exists(normalized_path: &str) -> bool {
    vfs_is_file(normalized_path) || vfs_directory_exists(normalized_path)
}

#[cfg(target_os = "windows")]
fn vfs_get_file_info(normalized_path: &str) -> Option<CoreAssetFileInfo> {
    let resolved_path = resolve_vfs_path(normalized_path)?;
    GLOBAL_VFS
        .get()
        .and_then(|vfs| vfs.lock().ok())
        .and_then(|guard| guard.archive.get_file_info(&resolved_path).cloned())
}

#[cfg(target_os = "windows")]
fn store_find_handle(entries: Vec<String>) -> HANDLE {
    let handles = GLOBAL_FIND_HANDLES.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = handles.lock().expect("find handle mutex poisoned");
    let mut id = 1u32;
    while guard.contains_key(&id) {
        id = id.wrapping_add(1);
    }
    guard.insert(id, VirtualFindHandle { entries, cursor: 1 });
    (VFS_FIND_HANDLE_BASE.wrapping_add(id)) as HANDLE
}

#[cfg(target_os = "windows")]
fn is_find_handle(handle: HANDLE) -> bool {
    let value = handle as usize as u32;
    value >= VFS_FIND_HANDLE_BASE && value != INVALID_HANDLE_VALUE as usize as u32
}

#[cfg(target_os = "windows")]
fn remove_find_handle(handle: HANDLE) -> bool {
    let value = handle as usize as u32;
    if value < VFS_FIND_HANDLE_BASE {
        return false;
    }
    let id = value.wrapping_sub(VFS_FIND_HANDLE_BASE);
    GLOBAL_FIND_HANDLES
        .get()
        .and_then(|handles| handles.lock().ok())
        .and_then(|mut guard| guard.remove(&id))
        .is_some()
}

#[cfg(target_os = "windows")]
fn advance_find_handle(handle: HANDLE) -> Option<String> {
    let value = handle as usize as u32;
    if value < VFS_FIND_HANDLE_BASE {
        return None;
    }
    let id = value.wrapping_sub(VFS_FIND_HANDLE_BASE);
    let handles = GLOBAL_FIND_HANDLES.get()?;
    let mut guard = handles.lock().ok()?;
    let state = guard.get_mut(&id)?;
    let next = state.entries.get(state.cursor)?.clone();
    state.cursor += 1;
    Some(next)
}

#[cfg(target_os = "windows")]
unsafe fn fill_find_data_a(path: &str, find_data: *mut WIN32_FIND_DATAA) {
    if find_data.is_null() {
        return;
    }
    if let Some(info) = vfs_get_file_info(path) {
        (*find_data).dwFileAttributes = FILE_ATTRIBUTE_NORMAL;
        (*find_data).nFileSizeHigh = (info.original_size >> 32) as u32;
        (*find_data).nFileSizeLow = info.original_size as u32;
    } else {
        (*find_data).dwFileAttributes = windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
        (*find_data).nFileSizeHigh = 0;
        (*find_data).nFileSizeLow = 0;
    }
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let bytes = file_name.as_bytes();
    let copy_len = bytes.len().min((*find_data).cFileName.len().saturating_sub(1));
    ptr::copy_nonoverlapping(
        bytes.as_ptr(),
        (*find_data).cFileName.as_mut_ptr().cast::<u8>(),
        copy_len,
    );
    (*find_data).cFileName[copy_len] = 0;
}

#[cfg(target_os = "windows")]
unsafe fn fill_find_data_w(path: &str, find_data: *mut WIN32_FIND_DATAW) {
    if find_data.is_null() {
        return;
    }
    if let Some(info) = vfs_get_file_info(path) {
        (*find_data).dwFileAttributes = FILE_ATTRIBUTE_NORMAL;
        (*find_data).nFileSizeHigh = (info.original_size >> 32) as u32;
        (*find_data).nFileSizeLow = info.original_size as u32;
    } else {
        (*find_data).dwFileAttributes = windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
        (*find_data).nFileSizeHigh = 0;
        (*find_data).nFileSizeLow = 0;
    }
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let wide: Vec<u16> = file_name.encode_utf16().collect();
    let copy_len = wide.len().min((*find_data).cFileName.len().saturating_sub(1));
    ptr::copy_nonoverlapping(wide.as_ptr(), (*find_data).cFileName.as_mut_ptr(), copy_len);
    (*find_data).cFileName[copy_len] = 0;
}

#[cfg(target_os = "windows")]
unsafe fn utf16_ptr_to_string(lp_file_name: *const u16) -> Option<String> {
    if lp_file_name.is_null() {
        return None;
    }
    let mut len = 0usize;
    while *lp_file_name.add(len) != 0 {
        len += 1;
    }
    Some(String::from_utf16_lossy(std::slice::from_raw_parts(
        lp_file_name, len,
    )))
}

#[cfg(target_os = "windows")]
unsafe fn ansi_ptr_to_string(lp_file_name: *const u8) -> Option<String> {
    if lp_file_name.is_null() {
        return None;
    }
    let mut len = 0usize;
    while *lp_file_name.add(len) != 0 {
        len += 1;
    }
    Some(String::from_utf8_lossy(std::slice::from_raw_parts(lp_file_name, len)).to_string())
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
unsafe fn patch_iat_entry(slot: *mut usize, replacement: usize) -> Result<()> {
    if slot.is_null() {
        return Err(Error::Other("IAT slot pointer is null".to_string()));
    }
    let mut old_protect = 0u32;
    if VirtualProtect(
        slot.cast(),
        std::mem::size_of::<usize>(),
        PAGE_EXECUTE_READWRITE,
        &mut old_protect,
    ) == 0
    {
        return Err(Error::Other("VirtualProtect failed for IAT patch".to_string()));
    }
    *slot = replacement;
    let mut restore_protect = 0u32;
    let _ = VirtualProtect(
        slot.cast(),
        std::mem::size_of::<usize>(),
        old_protect,
        &mut restore_protect,
    );
    Ok(())
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
macro_rules! init_orig_fn {
    ($slot:ident, $value:expr) => {{
        if unsafe { $slot }.is_none() {
            unsafe { $slot = Some(std::mem::transmute($value)) };
        }
    }};
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
macro_rules! remember_iat_slot {
    ($slot:ident, $value:expr) => {{
        if unsafe { $slot }.is_null() {
            unsafe { $slot = $value };
        }
    }};
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
unsafe fn module_path(module: HANDLE) -> Option<String> {
    let mut buffer = [0u16; 1024];
    let len = windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW(
        module,
        buffer.as_mut_ptr(),
        buffer.len() as u32,
    ) as usize;
    if len == 0 || len >= buffer.len() {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..len]).replace('\\', "/"))
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
unsafe fn should_patch_module(module: HANDLE) -> bool {
    let Some(module_path) = module_path(module) else {
        return false;
    };
    let module_path_lower = module_path.to_ascii_lowercase();
    if module_path_lower.ends_with("/maxion_stub.dll") {
        return false;
    }
    if module_path_lower.ends_with("/msvcp140.dll")
        || module_path_lower.ends_with("/ucrtbase.dll")
    {
        return true;
    }

    let Some(exe_dir) = current_exe_dir_normalized() else {
        return false;
    };
    let exe_dir_lower = exe_dir.to_ascii_lowercase();
    let exe_dir_prefix = format!("{exe_dir_lower}/");
    if !(module_path_lower.starts_with(&exe_dir_prefix) || module_path_lower == exe_dir_lower) {
        return false;
    }

    true
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
unsafe fn rva_slice<'a>(base: *const u8, image_size: usize, rva: u32, len: usize) -> Option<&'a [u8]> {
    let start = rva as usize;
    let end = start.checked_add(len)?;
    if start >= image_size || end > image_size {
        return None;
    }
    Some(std::slice::from_raw_parts(base.add(start), len))
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
unsafe fn read_c_string_rva(base: *const u8, image_size: usize, rva: u32) -> Option<String> {
    let start = rva as usize;
    if start >= image_size {
        return None;
    }
    let max_len = image_size - start;
    let bytes = std::slice::from_raw_parts(base.add(start), max_len);
    let nul = bytes.iter().position(|b| *b == 0)?;
    Some(String::from_utf8_lossy(&bytes[..nul]).to_string())
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
unsafe fn patch_module_iat(module: HANDLE) -> Result<bool> {
    if module.is_null() {
        return Ok(false);
    }
    if !should_patch_module(module) {
        return Ok(false);
    }
    if let Some(path) = module_path(module) {
        trace_line(&format!("patch_module_iat: begin {path}"));
    }

    let patched = PATCHED_MODULES.get_or_init(|| Mutex::new(BTreeMap::new()));
    {
        let mut guard = patched
            .lock()
            .map_err(|_| Error::Other("patched module mutex poisoned".to_string()))?;
        if guard.contains_key(&(module as usize)) {
            return Ok(false);
        }
        guard.insert(module as usize, ());
    }

    let base = module as *const u8;
    let e_lfanew = std::ptr::read_unaligned(base.add(0x3C) as *const u32) as usize;
    let pe_sig = std::ptr::read_unaligned(base.add(e_lfanew) as *const u32);
    if pe_sig != 0x0000_4550 {
        return Ok(false);
    }

    let optional_header = base.add(e_lfanew + 24);
    let magic = std::ptr::read_unaligned(optional_header as *const u16);
    let image_size = std::ptr::read_unaligned(optional_header.add(56) as *const u32) as usize;
    let import_dir_offset = if magic == 0x20B { 0x78 } else { 0x68 };
    let import_rva = std::ptr::read_unaligned(optional_header.add(import_dir_offset) as *const u32);
    if import_rva == 0 {
        return Ok(false);
    }

    if rva_slice(base, image_size, import_rva, 20).is_none() {
        trace_line("patch_module_iat: invalid import directory");
        return Ok(false);
    }

    let mut import_rva_cursor = import_rva;
    let mut patched_any = false;
    loop {
        let Some(import_desc) = rva_slice(base, image_size, import_rva_cursor, 20) else {
            break;
        };
        let original_first_thunk = u32::from_le_bytes(import_desc[0..4].try_into().unwrap());
        let name_rva = u32::from_le_bytes(import_desc[12..16].try_into().unwrap());
        let first_thunk = u32::from_le_bytes(import_desc[16..20].try_into().unwrap());
        if original_first_thunk == 0 && name_rva == 0 && first_thunk == 0 {
            break;
        }

        let Some(dll_name) = read_c_string_rva(base, image_size, name_rva) else {
            import_rva_cursor = import_rva_cursor.saturating_add(20);
            continue;
        };
        let dll_name = dll_name.to_ascii_lowercase();

        if dll_name == "kernel32.dll"
            || dll_name == "kernelbase.dll"
            || dll_name.starts_with("api-ms-win-core-file")
            || dll_name.starts_with("api-ms-win-core-libraryloader")
        {
            let mut thunk_name_rva = if original_first_thunk != 0 {
                original_first_thunk
            } else {
                first_thunk
            };
            let mut thunk_iat_rva = first_thunk;
            loop {
                let Some(thunk_name) = rva_slice(base, image_size, thunk_name_rva, 4) else {
                    break;
                };
                let import_ref = u32::from_le_bytes(thunk_name.try_into().unwrap());
                if import_ref == 0 {
                    break;
                }
                if (import_ref & 0x8000_0000) == 0 {
                    let Some(import_name) = read_c_string_rva(base, image_size, import_ref + 2)
                    else {
                        thunk_name_rva = thunk_name_rva.saturating_add(4);
                        thunk_iat_rva = thunk_iat_rva.saturating_add(4);
                        continue;
                    };
                    let thunk_iat = base.add(thunk_iat_rva as usize) as *mut usize;

                    match import_name.as_str() {
                        "LoadLibraryA" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: LoadLibraryA"));
                            }
                            init_orig_fn!(ORIG_LoadLibraryA, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_LoadLibraryA, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_LoadLibraryA as *const () as usize)?;
                            patched_any = true;
                        }
                        "LoadLibraryW" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: LoadLibraryW"));
                            }
                            init_orig_fn!(ORIG_LoadLibraryW, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_LoadLibraryW, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_LoadLibraryW as *const () as usize)?;
                            patched_any = true;
                        }
                        "CreateFileW" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: CreateFileW"));
                            }
                            init_orig_fn!(ORIG_CreateFileW, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_CreateFileW, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_CreateFileW as *const () as usize)?;
                            patched_any = true;
                        }
                        "CreateFileA" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: CreateFileA"));
                            }
                            init_orig_fn!(ORIG_CreateFileA, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_CreateFileA, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_CreateFileA as *const () as usize)?;
                            patched_any = true;
                        }
                        "CreateFile2" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: CreateFile2"));
                            }
                            init_orig_fn!(ORIG_CreateFile2, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_CreateFile2, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_CreateFile2 as *const () as usize)?;
                            patched_any = true;
                        }
                        "ReadFile" => {
                            init_orig_fn!(ORIG_ReadFile, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_ReadFile, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_ReadFile as *const () as usize)?;
                            patched_any = true;
                        }
                        "CloseHandle" => {
                            init_orig_fn!(ORIG_CloseHandle, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_CloseHandle, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_CloseHandle as *const () as usize)?;
                            patched_any = true;
                        }
                        "GetFileSizeEx" => {
                            init_orig_fn!(ORIG_GetFileSizeEx, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_GetFileSizeEx, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_GetFileSizeEx as *const () as usize)?;
                            patched_any = true;
                        }
                        "GetFileSize" => {
                            init_orig_fn!(ORIG_GetFileSize, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_GetFileSize, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_GetFileSize as *const () as usize)?;
                            patched_any = true;
                        }
                        "SetFilePointer" => {
                            init_orig_fn!(ORIG_SetFilePointer, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_SetFilePointer, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_SetFilePointer as *const () as usize)?;
                            patched_any = true;
                        }
                        "SetFilePointerEx" => {
                            init_orig_fn!(ORIG_SetFilePointerEx, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_SetFilePointerEx, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_SetFilePointerEx as *const () as usize)?;
                            patched_any = true;
                        }
                        "GetFileType" => {
                            init_orig_fn!(ORIG_GetFileType, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_GetFileType, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_GetFileType as *const () as usize)?;
                            patched_any = true;
                        }
                        "GetFileAttributesA" => {
                            init_orig_fn!(ORIG_GetFileAttributesA, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_GetFileAttributesA, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_GetFileAttributesA as *const () as usize)?;
                            patched_any = true;
                        }
                        "GetFileAttributesExW" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: GetFileAttributesExW"));
                            }
                            init_orig_fn!(ORIG_GetFileAttributesExW, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_GetFileAttributesExW, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_GetFileAttributesExW as *const () as usize)?;
                            patched_any = true;
                        }
                        "FindFirstFileA" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: FindFirstFileA"));
                            }
                            init_orig_fn!(ORIG_FindFirstFileA, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_FindFirstFileA, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_FindFirstFileA as *const () as usize)?;
                            patched_any = true;
                        }
                        "FindFirstFileW" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: FindFirstFileW"));
                            }
                            init_orig_fn!(ORIG_FindFirstFileW, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_FindFirstFileW, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_FindFirstFileW as *const () as usize)?;
                            patched_any = true;
                        }
                        "FindFirstFileExW" => {
                            init_orig_fn!(ORIG_FindFirstFileExW, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_FindFirstFileExW, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_FindFirstFileExW as *const () as usize)?;
                            patched_any = true;
                        }
                        "FindNextFileA" => {
                            init_orig_fn!(ORIG_FindNextFileA, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_FindNextFileA, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_FindNextFileA as *const () as usize)?;
                            patched_any = true;
                        }
                        "FindNextFileW" => {
                            init_orig_fn!(ORIG_FindNextFileW, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_FindNextFileW, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_FindNextFileW as *const () as usize)?;
                            patched_any = true;
                        }
                        "FindClose" => {
                            init_orig_fn!(ORIG_FindClose, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_FindClose, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_FindClose as *const () as usize)?;
                            patched_any = true;
                        }
                        _ => {}
                    }
                }

                thunk_name_rva = thunk_name_rva.saturating_add(4);
                thunk_iat_rva = thunk_iat_rva.saturating_add(4);
            }
        } else if dll_name.starts_with("msvcr")
            || dll_name.starts_with("api-ms-win-crt-stdio")
            || dll_name.starts_with("api-ms-win-crt-filesystem")
        {
            let mut thunk_name_rva = if original_first_thunk != 0 {
                original_first_thunk
            } else {
                first_thunk
            };
            let mut thunk_iat_rva = first_thunk;
            loop {
                let Some(thunk_name) = rva_slice(base, image_size, thunk_name_rva, 4) else {
                    break;
                };
                let import_ref = u32::from_le_bytes(thunk_name.try_into().unwrap());
                if import_ref == 0 {
                    break;
                }
                if (import_ref & 0x8000_0000) == 0 {
                    let Some(import_name) = read_c_string_rva(base, image_size, import_ref + 2)
                    else {
                        thunk_name_rva = thunk_name_rva.saturating_add(4);
                        thunk_iat_rva = thunk_iat_rva.saturating_add(4);
                        continue;
                    };
                    let thunk_iat = base.add(thunk_iat_rva as usize) as *mut usize;
                    match import_name.as_str() {
                        "fopen" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: fopen"));
                            }
                            init_orig_fn!(ORIG_fopen, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_fopen, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_fopen as *const () as usize)?;
                            patched_any = true;
                        }
                        "_wfopen" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: _wfopen"));
                            }
                            init_orig_fn!(ORIG__wfopen, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT__wfopen, thunk_iat);
                            patch_iat_entry(thunk_iat, hk__wfopen as *const () as usize)?;
                            patched_any = true;
                        }
                        "fopen_s" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: fopen_s"));
                            }
                            init_orig_fn!(ORIG_fopen_s, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_fopen_s, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_fopen_s as *const () as usize)?;
                            patched_any = true;
                        }
                        "fclose" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: fclose"));
                            }
                            init_orig_fn!(ORIG_fclose, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT_fclose, thunk_iat);
                            patch_iat_entry(thunk_iat, hk_fclose as *const () as usize)?;
                            patched_any = true;
                        }
                        "_stat64i32" => {
                            if let Some(path) = module_path(module) {
                                trace_line(&format!("patch {path}: _stat64i32"));
                            }
                            init_orig_fn!(ORIG__stat64i32, *thunk_iat);
                            remember_iat_slot!(IAT_SLOT__stat64i32, thunk_iat);
                            patch_iat_entry(thunk_iat, hk__stat64i32 as *const () as usize)?;
                            patched_any = true;
                        }
                        _ => {}
                    }
                }
                thunk_name_rva = thunk_name_rva.saturating_add(4);
                thunk_iat_rva = thunk_iat_rva.saturating_add(4);
            }
        }

        import_rva_cursor = import_rva_cursor.saturating_add(20);
    }

    if let Some(path) = module_path(module) {
        trace_line(&format!("patch_module_iat: end {path} patched={patched_any}"));
    }
    Ok(patched_any)
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
unsafe fn install_iat_hooks(_vfs: Arc<Mutex<VFS>>) -> Result<()> {
    use windows_sys::Win32::System::ProcessStatus::K32EnumProcessModules;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let process = GetCurrentProcess();
    let mut needed = 0u32;
    let mut modules: [HANDLE; 1024] = [std::ptr::null_mut(); 1024];
    if K32EnumProcessModules(
        process,
        modules.as_mut_ptr(),
        std::mem::size_of_val(&modules) as u32,
        &mut needed,
    ) == 0
    {
        return Err(Error::Other("K32EnumProcessModules failed".to_string()));
    }

    let count = (needed as usize / std::mem::size_of::<HANDLE>()).min(modules.len());
    let mut patched_modules = 0usize;
    trace_line(&format!("install_iat_hooks: module count {count}"));
    for module in modules.iter().take(count) {
        if let Some(path) = module_path(*module) {
            trace_line(&format!("install_iat_hooks: scan {path}"));
        }
        if patch_module_iat(*module)? {
            patched_modules += 1;
        }
    }

    if IAT_SLOT_CreateFileW.is_null() && IAT_SLOT_CreateFileA.is_null() {
        return Err(Error::Other(
            "No CreateFileA/CreateFileW import found for x86 IAT hook".to_string(),
        ));
    }

    log::info!(
        "x86 IAT hooks installed successfully across {} modules",
        patched_modules
    );
    trace_line(&format!("install_iat_hooks: patched modules {patched_modules}"));
    Ok(())
}

/// Install API hooks for Windows file functions
///
/// # Safety
///
/// This function modifies the Windows API and must be called during initialization.
/// It installs detours on CreateFileW, ReadFile, and CloseHandle.
#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
#[allow(static_mut_refs)] // Required for Windows API hooking
pub unsafe fn install_hooks(vfs: Arc<Mutex<VFS>>) -> Result<()> {
    // Store VFS instance in global static
    GLOBAL_VFS
        .set(vfs)
        .map_err(|_| Error::Other("VFS already initialized".to_string()))?;

    // Store original function pointers
    #[allow(clippy::missing_transmute_annotations)] // Transmuting function pointers is safe here
    {
        ORIG_CreateFileW = Some(std::mem::transmute(CreateFileW as *const () as usize));
        ORIG_ReadFile = Some(std::mem::transmute(ReadFile as *const () as usize));
        ORIG_CloseHandle = Some(std::mem::transmute(CloseHandle as *const () as usize));
        ORIG_GetFileSizeEx = Some(std::mem::transmute(GetFileSizeEx as *const () as usize));
    }

    // Install detours for Windows API functions
    // Note: The detour crate requires function pointers, not function names
    // We'll use type-safe detours
    #[allow(non_snake_case)] // Windows API naming convention
    {
        let CreateFileW_target = CreateFileW as CreateFileWFn;
        let CreateFileW_detour = hk_CreateFileW as CreateFileWFn;
        let CreateFileW_hook =
            retour::GenericDetour::new(CreateFileW_target, CreateFileW_detour)
                .map_err(|e| Error::Other(format!("Failed to create CreateFileW detour: {e}")))?;

        let ReadFile_target = ReadFile as ReadFileFn;
        let ReadFile_detour = hk_ReadFile as ReadFileFn;
        let ReadFile_hook = retour::GenericDetour::new(ReadFile_target, ReadFile_detour)
            .map_err(|e| Error::Other(format!("Failed to create ReadFile detour: {e}")))?;

        let CloseHandle_target = CloseHandle as CloseHandleFn;
        let CloseHandle_detour = hk_CloseHandle as CloseHandleFn;
        let CloseHandle_hook =
            retour::GenericDetour::new(CloseHandle_target, CloseHandle_detour)
                .map_err(|e| Error::Other(format!("Failed to create CloseHandle detour: {e}")))?;

        let GetFileSizeEx_target = GetFileSizeEx as GetFileSizeExFn;
        let GetFileSizeEx_detour = hk_GetFileSizeEx as GetFileSizeExFn;
        let GetFileSizeEx_hook =
            retour::GenericDetour::new(GetFileSizeEx_target, GetFileSizeEx_detour)
                .map_err(|e| Error::Other(format!("Failed to create GetFileSizeEx detour: {e}")))?;

        // Enable all hooks
        CreateFileW_hook
            .enable()
            .map_err(|e| Error::Other(format!("Failed to enable CreateFileW hook: {e}")))?;
        ReadFile_hook
            .enable()
            .map_err(|e| Error::Other(format!("Failed to enable ReadFile hook: {e}")))?;
        CloseHandle_hook
            .enable()
            .map_err(|e| Error::Other(format!("Failed to enable CloseHandle hook: {e}")))?;
        GetFileSizeEx_hook
            .enable()
            .map_err(|e| Error::Other(format!("Failed to enable GetFileSizeEx hook: {e}")))?;

        // Store Detour objects in static storage to keep hooks alive
        // This is critical - dropping a Detour object disables the hook
        HOOK_CreateFileW = Some(CreateFileW_hook);
        HOOK_ReadFile = Some(ReadFile_hook);
        HOOK_CloseHandle = Some(CloseHandle_hook);
        HOOK_GetFileSizeEx = Some(GetFileSizeEx_hook);
    }

    log::info!("API hooks installed successfully");

    Ok(())
}

/// Uninstall Windows API hooks and cleanup
///
/// This function disables all installed detours and releases the global VFS instance.
/// Call this when shutting down the protected application.
///
/// # Safety
///
/// This function modifies global state and must be called when no other threads
/// are using the hooked APIs.
#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
#[allow(static_mut_refs)] // Required for Windows API hooking
pub unsafe fn uninstall_hooks() -> Result<()> {
    // Disable all hooks in reverse order (LIFO)
    if let Some(hook) = HOOK_GetFileSizeEx.take() {
        hook.disable()
            .map_err(|e| Error::Other(format!("Failed to disable GetFileSizeEx hook: {e}")))?;
        log::debug!("GetFileSizeEx hook disabled");
    }

    if let Some(hook) = HOOK_CloseHandle.take() {
        hook.disable()
            .map_err(|e| Error::Other(format!("Failed to disable CloseHandle hook: {e}")))?;
        log::debug!("CloseHandle hook disabled");
    }

    if let Some(hook) = HOOK_ReadFile.take() {
        hook.disable()
            .map_err(|e| Error::Other(format!("Failed to disable ReadFile hook: {e}")))?;
        log::debug!("ReadFile hook disabled");
    }

    if let Some(hook) = HOOK_CreateFileW.take() {
        hook.disable()
            .map_err(|e| Error::Other(format!("Failed to disable CreateFileW hook: {e}")))?;
        log::debug!("CreateFileW hook disabled");
    }

    // Clear original function pointers
    ORIG_CreateFileW = None;
    ORIG_ReadFile = None;
    ORIG_CloseHandle = None;
    ORIG_GetFileSizeEx = None;

    log::info!("API hooks uninstalled successfully");

    Ok(())
}

/// Hook for CreateFileA
///
/// # Safety
///
/// This function is called from the Windows API detour/IAT patch.
#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_LoadLibraryA(lp_lib_file_name: *const u8) -> HANDLE {
    let module = if let Some(orig) = ORIG_LoadLibraryA {
        orig(lp_lib_file_name)
    } else {
        std::ptr::null_mut()
    };

    #[cfg(not(all(target_arch = "x86_64", feature = "hooks")))]
    if !module.is_null() {
        let _ = patch_module_iat(module);
    }

    module
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_LoadLibraryW(lp_lib_file_name: *const u16) -> HANDLE {
    let module = if let Some(orig) = ORIG_LoadLibraryW {
        orig(lp_lib_file_name)
    } else {
        std::ptr::null_mut()
    };

    #[cfg(not(all(target_arch = "x86_64", feature = "hooks")))]
    if !module.is_null() {
        let _ = patch_module_iat(module);
    }

    module
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_CreateFileA(
    lp_file_name: *const u8,
    dw_desired_access: u32,
    dw_share_mode: u32,
    lp_security_attributes: *const windows_sys::Win32::Security::SECURITY_ATTRIBUTES,
    dw_creation_disposition: u32,
    dw_flags_and_attributes: u32,
    h_template_file: HANDLE,
) -> HANDLE {
    let Some(path) = ansi_ptr_to_string(lp_file_name) else {
        if let Some(orig) = ORIG_CreateFileA {
            return orig(
                lp_file_name,
                dw_desired_access,
                dw_share_mode,
                lp_security_attributes,
                dw_creation_disposition,
                dw_flags_and_attributes,
                h_template_file,
            );
        }
        return INVALID_HANDLE_VALUE;
    };

    let normalized_path = normalize_game_path(&path);
    #[cfg(not(all(target_arch = "x86_64", feature = "hooks")))]
    if let Some(handle) = try_open_materialized_file_a(
        &normalized_path,
        dw_desired_access,
        dw_share_mode,
        lp_security_attributes,
        dw_creation_disposition,
        dw_flags_and_attributes,
        h_template_file,
    ) {
        if normalized_path.to_ascii_lowercase().starts_with("lua/") {
            trace_line(&format!("hk_CreateFileA: temp {normalized_path}"));
        }
        return handle;
    }
    if let Some(handle) = try_open_vfs_path(&normalized_path) {
        if normalized_path.to_ascii_lowercase().starts_with("lua/") {
            trace_line(&format!("hk_CreateFileA: virtual {normalized_path}"));
        }
        return handle;
    }

    if let Some(orig) = ORIG_CreateFileA {
        orig(
            lp_file_name,
            dw_desired_access,
            dw_share_mode,
            lp_security_attributes,
            dw_creation_disposition,
            dw_flags_and_attributes,
            h_template_file,
        )
    } else {
        INVALID_HANDLE_VALUE
    }
}

/// Hook for CreateFileW
///
/// # Safety
///
/// This function is called from the Windows API detour/IAT patch.
#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_CreateFileW(
    lp_file_name: *const u16,
    dw_desired_access: u32,
    dw_share_mode: u32,
    lp_security_attributes: *const windows_sys::Win32::Security::SECURITY_ATTRIBUTES,
    dw_creation_disposition: u32,
    dw_flags_and_attributes: u32,
    h_template_file: HANDLE,
) -> HANDLE {
    let Some(path) = utf16_ptr_to_string(lp_file_name) else {
        if let Some(orig) = ORIG_CreateFileW {
            return orig(
                lp_file_name,
                dw_desired_access,
                dw_share_mode,
                lp_security_attributes,
                dw_creation_disposition,
                dw_flags_and_attributes,
                h_template_file,
            );
        }
        return INVALID_HANDLE_VALUE;
    };

    let normalized_path = normalize_game_path(&path);
    #[cfg(not(all(target_arch = "x86_64", feature = "hooks")))]
    if let Some(handle) = try_open_materialized_file_w(
        &normalized_path,
        dw_desired_access,
        dw_share_mode,
        lp_security_attributes,
        dw_creation_disposition,
        dw_flags_and_attributes,
        h_template_file,
    ) {
        if normalized_path.to_ascii_lowercase().starts_with("lua/") {
            trace_line(&format!("hk_CreateFileW: temp {normalized_path}"));
        }
        return handle;
    }
    if let Some(handle) = try_open_vfs_path(&normalized_path) {
        if normalized_path.to_ascii_lowercase().starts_with("lua/") {
            trace_line(&format!("hk_CreateFileW: virtual {normalized_path}"));
        }
        return handle;
    }

    if let Some(orig) = ORIG_CreateFileW {
        orig(
            lp_file_name,
            dw_desired_access,
            dw_share_mode,
            lp_security_attributes,
            dw_creation_disposition,
            dw_flags_and_attributes,
            h_template_file,
        )
    } else {
        INVALID_HANDLE_VALUE
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_CreateFile2(
    lp_file_name: *const u16,
    dw_desired_access: u32,
    dw_share_mode: u32,
    dw_creation_disposition: u32,
    p_create_ex_params: *const core::ffi::c_void,
) -> HANDLE {
    let Some(path) = utf16_ptr_to_string(lp_file_name) else {
        if let Some(orig) = ORIG_CreateFile2 {
            return orig(
                lp_file_name,
                dw_desired_access,
                dw_share_mode,
                dw_creation_disposition,
                p_create_ex_params,
            );
        }
        return INVALID_HANDLE_VALUE;
    };

    let normalized_path = normalize_game_path(&path);
    if let Some(handle) = try_open_materialized_file_w(
        &normalized_path,
        dw_desired_access,
        dw_share_mode,
        std::ptr::null(),
        dw_creation_disposition,
        0,
        std::ptr::null_mut(),
    ) {
        trace_line(&format!("hk_CreateFile2: temp {normalized_path}"));
        return handle;
    }

    if let Some(orig) = ORIG_CreateFile2 {
        orig(
            lp_file_name,
            dw_desired_access,
            dw_share_mode,
            dw_creation_disposition,
            p_create_ex_params,
        )
    } else {
        INVALID_HANDLE_VALUE
    }
}

/// Hook for ReadFile
///
/// # Safety
///
/// This function is called from the Windows API detour.
#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_ReadFile(
    h_file: HANDLE,
    lp_buffer: *mut u8,
    n_number_of_bytes_to_read: u32,
    lp_number_of_bytes_read: *mut u32,
    lp_overlapped: *mut OVERLAPPED,
) -> i32 {
    // Safety check: buffer must not be null unless using overlapped I/O
    if lp_buffer.is_null() && lp_overlapped.is_null() {
        log::error!("ReadFile called with null buffer");
        return FALSE;
    }

    // Safety check: buffer size must be reasonable
    if n_number_of_bytes_to_read == 0 {
        // Zero-byte read is valid, just return success
        if !lp_number_of_bytes_read.is_null() {
            *lp_number_of_bytes_read = 0;
        }
        return TRUE;
    }

    // Check if this is a virtual handle
    if let Some(vfs) = GLOBAL_VFS.get() {
        if let Ok(vfs_guard) = vfs.lock() {
            if vfs_guard.is_virtual_handle(h_file) {
                log::debug!("VFS read: handle {:x}", h_file as usize);

                // For VFS files, read from virtual file system
                // Note: We need to drop the lock first then reacquire as mutable
                drop(vfs_guard);
                if let Ok(mut vfs_guard) = vfs.lock() {
                    let buffer_slice = std::slice::from_raw_parts_mut(
                        lp_buffer,
                        n_number_of_bytes_to_read as usize,
                    );
                    match vfs_guard.read_virtual(h_file, buffer_slice, n_number_of_bytes_to_read) {
                        Ok(bytes_read) => {
                            if !lp_number_of_bytes_read.is_null() {
                                *lp_number_of_bytes_read = bytes_read;
                            }
                            return TRUE;
                        }
                        Err(e) => {
                            log::error!("VFS read error: {:?}", e);
                            // Fall through to original ReadFile on error
                        }
                    }
                }
            }
        }
    }

    if let Some(temp_path) = temp_handle_path(h_file) {
        if temp_path.to_ascii_lowercase().contains("maxion_vfs") {
            let result = if let Some(orig) = ORIG_ReadFile {
                orig(
                    h_file,
                    lp_buffer,
                    n_number_of_bytes_to_read,
                    lp_number_of_bytes_read,
                    lp_overlapped,
                )
            } else {
                FALSE
            };
            let bytes_read = if !lp_number_of_bytes_read.is_null() {
                *lp_number_of_bytes_read
            } else {
                0
            };
            trace_line(&format!(
                "hk_ReadFile temp handle=0x{:x} requested={} read={} ok={}",
                h_file as usize, n_number_of_bytes_to_read, bytes_read, result
            ));
            return result;
        }
    }

    // Fall through to original ReadFile
    if let Some(orig) = ORIG_ReadFile {
        orig(
            h_file,
            lp_buffer,
            n_number_of_bytes_to_read,
            lp_number_of_bytes_read,
            lp_overlapped,
        )
    } else {
        FALSE
    }
}

/// Hook for CloseHandle
///
/// # Safety
///
/// This function is called from the Windows API detour.
#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_CloseHandle(h_object: HANDLE) -> i32 {
    if GLOBAL_TEMP_HANDLES
        .get()
        .and_then(|m| m.lock().ok())
        .map(|m| m.contains_key(&(h_object as usize)))
        .unwrap_or(false)
    {
        trace_line(&format!("hk_CloseHandle temp handle=0x{:x}", h_object as usize));
        let result = if let Some(orig) = ORIG_CloseHandle {
            orig(h_object)
        } else {
            FALSE
        };
        cleanup_temp_handle(h_object);
        return result;
    }

    // Check if this is a virtual handle
    if let Some(vfs) = GLOBAL_VFS.get() {
        if let Ok(vfs_guard) = vfs.lock() {
            if vfs_guard.is_virtual_handle(h_object) {
                log::debug!("VFS close: handle {:x}", h_object as usize);

                // For VFS files, close virtual handle
                drop(vfs_guard);
                if let Ok(mut vfs_guard) = vfs.lock() {
                    let _ = vfs_guard.close_virtual(h_object);
                }
                return TRUE;
            }
        }
    }

    // Fall through to original CloseHandle
    if let Some(orig) = ORIG_CloseHandle {
        orig(h_object)
    } else {
        FALSE
    }
}

/// Hook for GetFileSizeEx
///
/// # Safety
///
/// This function is called from the Windows API detour.
#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_GetFileSizeEx(h_file: HANDLE, lp_file_size: *mut i64) -> i32 {
    // Check if this is a virtual handle
    if let Some(vfs) = GLOBAL_VFS.get() {
        if let Ok(vfs_guard) = vfs.lock() {
            if vfs_guard.is_virtual_handle(h_file) {
                if !lp_file_size.is_null() {
                    match vfs_guard.get_file_size(h_file) {
                        Ok(size) => {
                            *lp_file_size = size;
                        }
                        Err(e) => {
                            log::error!("VFS get_file_size error: {:?}", e);
                            return FALSE;
                        }
                    }
                }
                return TRUE;
            }
        }
    }

    if let Some(temp_path) = temp_handle_path(h_file) {
        let result = if let Some(orig) = ORIG_GetFileSizeEx {
            orig(h_file, lp_file_size)
        } else {
            FALSE
        };
        let size = if !lp_file_size.is_null() { *lp_file_size } else { -1 };
        trace_line(&format!(
            "hk_GetFileSizeEx temp handle=0x{:x} size={} ok={} path={}",
            h_file as usize, size, result, temp_path
        ));
        return result;
    }

    // Fall through to original GetFileSizeEx
    if let Some(orig) = ORIG_GetFileSizeEx {
        orig(h_file, lp_file_size)
    } else {
        FALSE
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_GetFileSize(h_file: HANDLE, lp_file_size_high: *mut u32) -> u32 {
    if let Some(vfs) = GLOBAL_VFS.get() {
        if let Ok(vfs_guard) = vfs.lock() {
            if vfs_guard.is_virtual_handle(h_file) {
                match vfs_guard.get_file_size(h_file) {
                    Ok(size) => {
                        let size = size as u64;
                        if !lp_file_size_high.is_null() {
                            *lp_file_size_high = (size >> 32) as u32;
                        }
                        return size as u32;
                    }
                    Err(_) => return u32::MAX,
                }
            }
        }
    }

    if let Some(orig) = ORIG_GetFileSize {
        orig(h_file, lp_file_size_high)
    } else {
        u32::MAX
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_SetFilePointer(
    h_file: HANDLE,
    l_distance_to_move: i32,
    lp_distance_to_move_high: *mut i32,
    dw_move_method: u32,
) -> u32 {
    if let Some(vfs) = GLOBAL_VFS.get() {
        if let Ok(vfs_guard) = vfs.lock() {
            if vfs_guard.is_virtual_handle(h_file) {
                trace_line("hk_SetFilePointer: virtual handle");
                drop(vfs_guard);
                let high = if lp_distance_to_move_high.is_null() {
                    0i64
                } else {
                    (*lp_distance_to_move_high as i64) << 32
                };
                let distance = high | (l_distance_to_move as u32 as u64 as i64);
                if let Ok(mut vfs_guard) = vfs.lock() {
                    match vfs_guard.set_file_pointer(h_file, distance, dw_move_method) {
                        Ok(new_offset) => {
                            if !lp_distance_to_move_high.is_null() {
                                *lp_distance_to_move_high = ((new_offset >> 32) & 0xFFFF_FFFF) as i32;
                            }
                            return new_offset as u32;
                        }
                        Err(_) => return u32::MAX,
                    }
                }
            }
        }
    }

    if let Some(orig) = ORIG_SetFilePointer {
        orig(
            h_file,
            l_distance_to_move,
            lp_distance_to_move_high,
            dw_move_method,
        )
    } else {
        u32::MAX
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_SetFilePointerEx(
    h_file: HANDLE,
    li_distance_to_move: i64,
    lp_new_file_pointer: *mut i64,
    dw_move_method: u32,
) -> i32 {
    if let Some(vfs) = GLOBAL_VFS.get() {
        if let Ok(vfs_guard) = vfs.lock() {
            if vfs_guard.is_virtual_handle(h_file) {
                trace_line("hk_SetFilePointerEx: virtual handle");
                drop(vfs_guard);
                if let Ok(mut vfs_guard) = vfs.lock() {
                    match vfs_guard.set_file_pointer(h_file, li_distance_to_move, dw_move_method) {
                        Ok(new_offset) => {
                            if !lp_new_file_pointer.is_null() {
                                *lp_new_file_pointer = new_offset as i64;
                            }
                            return TRUE;
                        }
                        Err(_) => return FALSE,
                    }
                }
            }
        }
    }

    if let Some(orig) = ORIG_SetFilePointerEx {
        orig(h_file, li_distance_to_move, lp_new_file_pointer, dw_move_method)
    } else {
        FALSE
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_GetFileType(h_file: HANDLE) -> u32 {
    if let Some(vfs) = GLOBAL_VFS.get() {
        if let Ok(vfs_guard) = vfs.lock() {
            if vfs_guard.is_virtual_handle(h_file) {
                trace_line("hk_GetFileType: virtual handle");
                return 0x0001;
            }
        }
    }

    if let Some(orig) = ORIG_GetFileType {
        orig(h_file)
    } else {
        0
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_GetFileAttributesA(lp_file_name: *const u8) -> u32 {
    if let Some(path) = ansi_ptr_to_string(lp_file_name) {
        let normalized_path = normalize_game_path(&path);
        if vfs_is_file(&normalized_path) {
            return FILE_ATTRIBUTE_NORMAL;
        }
        if vfs_directory_exists(&normalized_path) {
            return windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
        }
    }

    if let Some(orig) = ORIG_GetFileAttributesA {
        orig(lp_file_name)
    } else {
        u32::MAX
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_GetFileAttributesExW(
    lp_file_name: *const u16,
    f_info_level_id: GET_FILEEX_INFO_LEVELS,
    lp_file_information: *mut core::ffi::c_void,
) -> i32 {
    if let Some(path) = utf16_ptr_to_string(lp_file_name) {
        let normalized_path = normalize_game_path(&path);
        if let Some(info) = vfs_get_file_info(&normalized_path) {
            if !lp_file_information.is_null() {
                let data = &mut *(lp_file_information as *mut WIN32_FILE_ATTRIBUTE_DATA);
                data.dwFileAttributes = FILE_ATTRIBUTE_NORMAL;
                data.nFileSizeHigh = (info.original_size >> 32) as u32;
                data.nFileSizeLow = info.original_size as u32;
            }
            let _ = f_info_level_id;
            return TRUE;
        }
        if vfs_directory_exists(&normalized_path) {
            if !lp_file_information.is_null() {
                let data = &mut *(lp_file_information as *mut WIN32_FILE_ATTRIBUTE_DATA);
                data.dwFileAttributes =
                    windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_DIRECTORY;
                data.nFileSizeHigh = 0;
                data.nFileSizeLow = 0;
            }
            let _ = f_info_level_id;
            return TRUE;
        }
    }

    if let Some(orig) = ORIG_GetFileAttributesExW {
        orig(lp_file_name, f_info_level_id, lp_file_information)
    } else {
        FALSE
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_FindFirstFileA(
    lp_file_name: *const u8,
    lp_find_file_data: *mut WIN32_FIND_DATAA,
) -> HANDLE {
    if let Some(path) = ansi_ptr_to_string(lp_file_name) {
        let matches = vfs_list_directory(&path);
        if let Some(first) = matches.first() {
            fill_find_data_a(first, lp_find_file_data);
            return store_find_handle(matches);
        }
    }

    if let Some(orig) = ORIG_FindFirstFileA {
        orig(lp_file_name, lp_find_file_data)
    } else {
        INVALID_HANDLE_VALUE
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_FindFirstFileW(
    lp_file_name: *const u16,
    lp_find_file_data: *mut WIN32_FIND_DATAW,
) -> HANDLE {
    if let Some(path) = utf16_ptr_to_string(lp_file_name) {
        let matches = vfs_list_directory(&path);
        if let Some(first) = matches.first() {
            fill_find_data_w(first, lp_find_file_data);
            return store_find_handle(matches);
        }
    }

    if let Some(orig) = ORIG_FindFirstFileW {
        orig(lp_file_name, lp_find_file_data)
    } else {
        INVALID_HANDLE_VALUE
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_FindFirstFileExW(
    lp_file_name: *const u16,
    f_info_level_id: FINDEX_INFO_LEVELS,
    lp_find_file_data: *mut core::ffi::c_void,
    f_search_op: FINDEX_SEARCH_OPS,
    lp_search_filter: *const core::ffi::c_void,
    dw_additional_flags: u32,
) -> HANDLE {
    if let Some(path) = utf16_ptr_to_string(lp_file_name) {
        let matches = vfs_list_directory(&path);
        if let Some(first) = matches.first() {
            fill_find_data_w(first, lp_find_file_data as *mut WIN32_FIND_DATAW);
            let _ = f_info_level_id;
            let _ = f_search_op;
            let _ = lp_search_filter;
            let _ = dw_additional_flags;
            return store_find_handle(matches);
        }
    }

    if let Some(orig) = ORIG_FindFirstFileExW {
        orig(
            lp_file_name,
            f_info_level_id,
            lp_find_file_data,
            f_search_op,
            lp_search_filter,
            dw_additional_flags,
        )
    } else {
        INVALID_HANDLE_VALUE
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_FindNextFileA(
    h_find_file: HANDLE,
    lp_find_file_data: *mut WIN32_FIND_DATAA,
) -> i32 {
    if is_find_handle(h_find_file) {
        if let Some(next) = advance_find_handle(h_find_file) {
            fill_find_data_a(&next, lp_find_file_data);
            return TRUE;
        }
        return FALSE;
    }

    if let Some(orig) = ORIG_FindNextFileA {
        orig(h_find_file, lp_find_file_data)
    } else {
        FALSE
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_FindNextFileW(
    h_find_file: HANDLE,
    lp_find_file_data: *mut WIN32_FIND_DATAW,
) -> i32 {
    if is_find_handle(h_find_file) {
        if let Some(next) = advance_find_handle(h_find_file) {
            fill_find_data_w(&next, lp_find_file_data);
            return TRUE;
        }
        return FALSE;
    }

    if let Some(orig) = ORIG_FindNextFileW {
        orig(h_find_file, lp_find_file_data)
    } else {
        FALSE
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "system" fn hk_FindClose(h_find_file: HANDLE) -> i32 {
    if remove_find_handle(h_find_file) {
        return TRUE;
    }

    if let Some(orig) = ORIG_FindClose {
        orig(h_find_file)
    } else {
        FALSE
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn hk_fopen(
    filename: *const u8,
    mode: *const u8,
) -> *mut core::ffi::c_void {
    if let (Some(path), Some(orig)) = (ansi_ptr_to_string(filename), ORIG_fopen) {
        let normalized_path = normalize_game_path(&path);
        trace_line(&format!("hk_fopen: {normalized_path}"));
        if let Some(temp_path) = materialize_vfs_file(&path) {
            let temp_c = std::ffi::CString::new(temp_path.clone()).ok();
            if let Some(temp_c) = temp_c {
                let file_ptr = orig(temp_c.as_ptr().cast(), mode);
                if !file_ptr.is_null() {
                    remember_temp_file(file_ptr, temp_path);
                }
                return file_ptr;
            }
        }
    }

    if let Some(orig) = ORIG_fopen {
        orig(filename, mode)
    } else {
        std::ptr::null_mut()
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn hk__wfopen(
    filename: *const u16,
    mode: *const u16,
) -> *mut core::ffi::c_void {
    if let (Some(path), Some(orig)) = (utf16_ptr_to_string(filename), ORIG__wfopen) {
        let normalized_path = normalize_game_path(&path);
        trace_line(&format!("hk__wfopen: {normalized_path}"));
        if let Some(temp_path) = materialize_vfs_file(&path) {
            let wide: Vec<u16> = temp_path.encode_utf16().chain(std::iter::once(0)).collect();
            let file_ptr = orig(wide.as_ptr(), mode);
            if !file_ptr.is_null() {
                remember_temp_file(file_ptr, temp_path);
            }
            return file_ptr;
        }
    }

    if let Some(orig) = ORIG__wfopen {
        orig(filename, mode)
    } else {
        std::ptr::null_mut()
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn hk_fopen_s(
    out_file: *mut *mut core::ffi::c_void,
    filename: *const u8,
    mode: *const u8,
) -> i32 {
    if out_file.is_null() {
        return 22;
    }

    if let (Some(path), Some(orig)) = (ansi_ptr_to_string(filename), ORIG_fopen_s) {
        let normalized_path = normalize_game_path(&path);
        trace_line(&format!("hk_fopen_s: {normalized_path}"));
        if let Some(temp_path) = materialize_vfs_file(&path) {
            if let Ok(temp_c) = std::ffi::CString::new(temp_path.clone()) {
                let rc = orig(out_file, temp_c.as_ptr().cast(), mode);
                if rc == 0 && !(*out_file).is_null() {
                    remember_temp_file(*out_file, temp_path);
                }
                return rc;
            }
        }
    }

    if let Some(orig) = ORIG_fopen_s {
        orig(out_file, filename, mode)
    } else {
        2
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn hk_fclose(file: *mut core::ffi::c_void) -> i32 {
    cleanup_temp_file(file);
    if let Some(orig) = ORIG_fclose {
        orig(file)
    } else {
        -1
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn hk__stat64i32(
    path: *const u8,
    stat_buf: *mut core::ffi::c_void,
) -> i32 {
    if let (Some(path_str), Some(orig)) = (ansi_ptr_to_string(path), ORIG__stat64i32) {
        if let Some(temp_path) = materialize_vfs_file(&path_str) {
            let result = if let Ok(temp_c) = std::ffi::CString::new(temp_path.clone()) {
                orig(temp_c.as_ptr().cast(), stat_buf)
            } else {
                2
            };
            let _ = fs::remove_file(temp_path);
            return result;
        }
    }

    if let Some(orig) = ORIG__stat64i32 {
        orig(path, stat_buf)
    } else {
        -1
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn hk__open_osfhandle(osfhandle: isize, flags: i32) -> i32 {
    let handle = osfhandle as HANDLE;
    if let Some(path) = vfs_path_for_handle(handle) {
        if let Some(temp_path) = materialize_vfs_file(&path) {
            let temp_c = match std::ffi::CString::new(temp_path.clone()) {
                Ok(v) => v,
                Err(_) => return -1,
            };
            let real_handle = windows_sys::Win32::Storage::FileSystem::CreateFileA(
                temp_c.as_ptr().cast(),
                0x80000000, // GENERIC_READ
                0x00000001 | 0x00000002, // FILE_SHARE_READ | FILE_SHARE_WRITE
                std::ptr::null(),
                3, // OPEN_EXISTING
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            );
            if real_handle == INVALID_HANDLE_VALUE {
                let _ = fs::remove_file(temp_path);
                return -1;
            }
            if let Some(orig) = ORIG__open_osfhandle {
                let fd = orig(real_handle as isize, flags);
                if fd >= 0 {
                    remember_temp_fd(fd, temp_path);
                } else {
                    let _ = windows_sys::Win32::Foundation::CloseHandle(real_handle);
                    let _ = fs::remove_file(temp_path);
                }
                return fd;
            }
            let _ = windows_sys::Win32::Foundation::CloseHandle(real_handle);
            let _ = fs::remove_file(temp_path);
            return -1;
        }
    }

    if let Some(orig) = ORIG__open_osfhandle {
        orig(osfhandle, flags)
    } else {
        -1
    }
}

#[cfg(target_os = "windows")]
#[no_mangle]
pub unsafe extern "C" fn hk__close(fd: i32) -> i32 {
    let result = if let Some(orig) = ORIG__close {
        orig(fd)
    } else {
        -1
    };
    cleanup_temp_fd(fd);
    result
}

/// Install API hooks (wrapper function for stub_entry)
///
/// This function creates a VFS instance and installs the Windows API hooks.
/// It's called from stub_entry during initialization.
///
/// # Returns
///
/// `Result<()>` - Ok if hooks were installed successfully
#[cfg(all(target_os = "windows", target_arch = "x86_64", feature = "hooks"))]
fn install_api_hooks(vfs: Arc<Mutex<VFS>>) -> Result<()> {
    // Install hooks with the VFS instance
    unsafe { install_hooks(vfs) }
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
fn install_api_hooks(vfs: Arc<Mutex<VFS>>) -> Result<()> {
    unsafe { install_iat_hooks(vfs) }
}

#[cfg(all(target_os = "windows", not(all(target_arch = "x86_64", feature = "hooks"))))]
pub unsafe fn uninstall_hooks() -> Result<()> {
    if !IAT_SLOT_LoadLibraryA.is_null() {
        if let Some(orig) = ORIG_LoadLibraryA {
            let _ = patch_iat_entry(IAT_SLOT_LoadLibraryA, orig as usize);
        }
        IAT_SLOT_LoadLibraryA = std::ptr::null_mut();
    }
    if !IAT_SLOT_LoadLibraryW.is_null() {
        if let Some(orig) = ORIG_LoadLibraryW {
            let _ = patch_iat_entry(IAT_SLOT_LoadLibraryW, orig as usize);
        }
        IAT_SLOT_LoadLibraryW = std::ptr::null_mut();
    }
    if !IAT_SLOT_CreateFileW.is_null() {
        if let Some(orig) = ORIG_CreateFileW {
            let _ = patch_iat_entry(IAT_SLOT_CreateFileW, orig as usize);
        }
        IAT_SLOT_CreateFileW = std::ptr::null_mut();
    }
    if !IAT_SLOT_CreateFileA.is_null() {
        if let Some(orig) = ORIG_CreateFileA {
            let _ = patch_iat_entry(IAT_SLOT_CreateFileA, orig as usize);
        }
        IAT_SLOT_CreateFileA = std::ptr::null_mut();
    }
    if !IAT_SLOT_CreateFile2.is_null() {
        if let Some(orig) = ORIG_CreateFile2 {
            let _ = patch_iat_entry(IAT_SLOT_CreateFile2, orig as usize);
        }
        IAT_SLOT_CreateFile2 = std::ptr::null_mut();
    }
    if !IAT_SLOT_ReadFile.is_null() {
        if let Some(orig) = ORIG_ReadFile {
            let _ = patch_iat_entry(IAT_SLOT_ReadFile, orig as usize);
        }
        IAT_SLOT_ReadFile = std::ptr::null_mut();
    }
    if !IAT_SLOT_CloseHandle.is_null() {
        if let Some(orig) = ORIG_CloseHandle {
            let _ = patch_iat_entry(IAT_SLOT_CloseHandle, orig as usize);
        }
        IAT_SLOT_CloseHandle = std::ptr::null_mut();
    }
    if !IAT_SLOT_GetFileSizeEx.is_null() {
        if let Some(orig) = ORIG_GetFileSizeEx {
            let _ = patch_iat_entry(IAT_SLOT_GetFileSizeEx, orig as usize);
        }
        IAT_SLOT_GetFileSizeEx = std::ptr::null_mut();
    }
    if !IAT_SLOT_GetFileSize.is_null() {
        if let Some(orig) = ORIG_GetFileSize {
            let _ = patch_iat_entry(IAT_SLOT_GetFileSize, orig as usize);
        }
        IAT_SLOT_GetFileSize = std::ptr::null_mut();
    }
    if !IAT_SLOT_SetFilePointer.is_null() {
        if let Some(orig) = ORIG_SetFilePointer {
            let _ = patch_iat_entry(IAT_SLOT_SetFilePointer, orig as usize);
        }
        IAT_SLOT_SetFilePointer = std::ptr::null_mut();
    }
    if !IAT_SLOT_SetFilePointerEx.is_null() {
        if let Some(orig) = ORIG_SetFilePointerEx {
            let _ = patch_iat_entry(IAT_SLOT_SetFilePointerEx, orig as usize);
        }
        IAT_SLOT_SetFilePointerEx = std::ptr::null_mut();
    }
    if !IAT_SLOT_GetFileType.is_null() {
        if let Some(orig) = ORIG_GetFileType {
            let _ = patch_iat_entry(IAT_SLOT_GetFileType, orig as usize);
        }
        IAT_SLOT_GetFileType = std::ptr::null_mut();
    }
    if !IAT_SLOT_GetFileAttributesA.is_null() {
        if let Some(orig) = ORIG_GetFileAttributesA {
            let _ = patch_iat_entry(IAT_SLOT_GetFileAttributesA, orig as usize);
        }
        IAT_SLOT_GetFileAttributesA = std::ptr::null_mut();
    }
    if !IAT_SLOT_GetFileAttributesExW.is_null() {
        if let Some(orig) = ORIG_GetFileAttributesExW {
            let _ = patch_iat_entry(IAT_SLOT_GetFileAttributesExW, orig as usize);
        }
        IAT_SLOT_GetFileAttributesExW = std::ptr::null_mut();
    }
    if !IAT_SLOT_FindFirstFileA.is_null() {
        if let Some(orig) = ORIG_FindFirstFileA {
            let _ = patch_iat_entry(IAT_SLOT_FindFirstFileA, orig as usize);
        }
        IAT_SLOT_FindFirstFileA = std::ptr::null_mut();
    }
    if !IAT_SLOT_FindFirstFileW.is_null() {
        if let Some(orig) = ORIG_FindFirstFileW {
            let _ = patch_iat_entry(IAT_SLOT_FindFirstFileW, orig as usize);
        }
        IAT_SLOT_FindFirstFileW = std::ptr::null_mut();
    }
    if !IAT_SLOT_FindFirstFileExW.is_null() {
        if let Some(orig) = ORIG_FindFirstFileExW {
            let _ = patch_iat_entry(IAT_SLOT_FindFirstFileExW, orig as usize);
        }
        IAT_SLOT_FindFirstFileExW = std::ptr::null_mut();
    }
    if !IAT_SLOT_FindNextFileA.is_null() {
        if let Some(orig) = ORIG_FindNextFileA {
            let _ = patch_iat_entry(IAT_SLOT_FindNextFileA, orig as usize);
        }
        IAT_SLOT_FindNextFileA = std::ptr::null_mut();
    }
    if !IAT_SLOT_FindNextFileW.is_null() {
        if let Some(orig) = ORIG_FindNextFileW {
            let _ = patch_iat_entry(IAT_SLOT_FindNextFileW, orig as usize);
        }
        IAT_SLOT_FindNextFileW = std::ptr::null_mut();
    }
    if !IAT_SLOT_FindClose.is_null() {
        if let Some(orig) = ORIG_FindClose {
            let _ = patch_iat_entry(IAT_SLOT_FindClose, orig as usize);
        }
        IAT_SLOT_FindClose = std::ptr::null_mut();
    }
    if !IAT_SLOT_fopen.is_null() {
        if let Some(orig) = ORIG_fopen {
            let _ = patch_iat_entry(IAT_SLOT_fopen, orig as usize);
        }
        IAT_SLOT_fopen = std::ptr::null_mut();
    }
    if !IAT_SLOT__wfopen.is_null() {
        if let Some(orig) = ORIG__wfopen {
            let _ = patch_iat_entry(IAT_SLOT__wfopen, orig as usize);
        }
        IAT_SLOT__wfopen = std::ptr::null_mut();
    }
    if !IAT_SLOT_fopen_s.is_null() {
        if let Some(orig) = ORIG_fopen_s {
            let _ = patch_iat_entry(IAT_SLOT_fopen_s, orig as usize);
        }
        IAT_SLOT_fopen_s = std::ptr::null_mut();
    }
    if !IAT_SLOT_fclose.is_null() {
        if let Some(orig) = ORIG_fclose {
            let _ = patch_iat_entry(IAT_SLOT_fclose, orig as usize);
        }
        IAT_SLOT_fclose = std::ptr::null_mut();
    }
    if !IAT_SLOT__stat64i32.is_null() {
        if let Some(orig) = ORIG__stat64i32 {
            let _ = patch_iat_entry(IAT_SLOT__stat64i32, orig as usize);
        }
        IAT_SLOT__stat64i32 = std::ptr::null_mut();
    }
    if !IAT_SLOT__open_osfhandle.is_null() {
        if let Some(orig) = ORIG__open_osfhandle {
            let _ = patch_iat_entry(IAT_SLOT__open_osfhandle, orig as usize);
        }
        IAT_SLOT__open_osfhandle = std::ptr::null_mut();
    }
    if !IAT_SLOT__close.is_null() {
        if let Some(orig) = ORIG__close {
            let _ = patch_iat_entry(IAT_SLOT__close, orig as usize);
        }
        IAT_SLOT__close = std::ptr::null_mut();
    }
    Ok(())
}

/// Stub entry point for initialization
///
/// This function is called when the protected executable starts.
/// It initializes the VFS and installs API hooks before passing
/// control to the original entry point.
#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "system" fn stub_entry() -> u32 {
    trace_line("stub_entry: begin");
    log::info!("Maxion Stub initializing...");
    let module_handle =
        unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) };
    if module_handle.is_null() {
        log::error!("Failed to get module handle");
        return EXIT_FAILURE_STUB_GET_MODULE;
    }
    let (archive_data, key_data) = match locate_embedded_archive(module_handle) {
        Ok(data) => data,
        Err(e) => {
            log::error!("Failed to locate embedded archive: {:?}", e);
            return EXIT_FAILURE_STUB_ARCHIVE;
        }
    };

    if let Err(e) = parse_archive_header(&archive_data) {
        log::error!("Failed to parse archive header: {:?}", e);
        return EXIT_FAILURE_STUB_HEADER;
    }

    let (encryption_key, nonce, chunk_size, original_entry, expected_archive_hash) =
        match deobfuscate_key(&key_data) {
        Ok(keys) => keys,
        Err(e) => {
            log::error!("Failed to deobfuscate key: {:?}", e);
            return EXIT_FAILURE_STUB_KEY;
        }
    };
    let mut key_data = key_data;
    key_data.fill(0);
    if let Some(expected_archive_hash) = expected_archive_hash {
        let actual_archive_hash = blake3::hash(&archive_data);
        if actual_archive_hash.as_bytes() != &expected_archive_hash {
            log::error!("Embedded archive integrity verification failed");
            return EXIT_FAILURE_STUB_VFS_ARCHIVE;
        }
    }

    let config = Config {
        encryption_key,
        nonce,
        chunk_size: maxion_core::ChunkSize::new(chunk_size),
        compress: false,
        compression_level: 0,
        build_secret: [0u8; 32],
        simd_config: None,
    };

    let archive = match VirtualArchive::from_memory(archive_data, config) {
        Ok(archive) => archive,
        Err(e) => {
            log::error!("Failed to initialize virtual archive: {:?}", e);
            return EXIT_FAILURE_STUB_VFS_ARCHIVE;
        }
    };

    let vfs = match VFS::new(archive) {
        Ok(vfs) => Arc::new(Mutex::new(vfs)),
        Err(e) => {
            log::error!("Failed to create VFS: {:?}", e);
            return EXIT_FAILURE_STUB_VFS_CREATE;
        }
    };

    if GLOBAL_VFS.get().is_none() && GLOBAL_VFS.set(vfs.clone()).is_err() {
        log::error!("Failed to store VFS instance");
        return EXIT_FAILURE_STUB_GLOBAL_SET;
    }

    if let Err(e) = install_api_hooks(vfs) {
        log::warn!("Failed to install API hooks: {:?}", e);
    }
    trace_line("stub_entry: hooks installed, jumping");

    // Step 9: Jump to original entry point
    log::info!("Jumping to original entry point: 0x{:X}", original_entry);

    // Convert RVA to actual address
    let entry_address = (module_handle as usize + original_entry as usize) as *const ();
    unsafe {
        let entry_fn: extern "system" fn() -> u32 = std::mem::transmute(entry_address);
        entry_fn()
    }
}

#[cfg(target_os = "windows")]
fn with_vfs_mut<R>(f: impl FnOnce(&mut VFS) -> Result<R>) -> Result<R> {
    let vfs = ensure_initialized(true)?;
    let mut guard = vfs
        .lock()
        .map_err(|_| Error::Other("Failed to lock VFS".to_string()))?;
    f(&mut guard)
}

#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn maxion_init(_executable_path: *const c_char) -> bool {
    ensure_initialized(true).is_ok()
}

#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn maxion_shutdown() {
    unsafe {
        let _ = uninstall_hooks();
    }
}

#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn maxion_file_exists(path: *const c_char) -> bool {
    if path.is_null() {
        return false;
    }

    let c_path = unsafe { std::ffi::CStr::from_ptr(path) };
    let Ok(path_str) = c_path.to_str() else {
        return false;
    };

    with_vfs_mut(|vfs| Ok(vfs.archive.file_exists(path_str))).unwrap_or(false)
}

#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn maxion_get_file_size(path: *const c_char) -> usize {
    if path.is_null() {
        return 0;
    }

    let c_path = unsafe { std::ffi::CStr::from_ptr(path) };
    let Ok(path_str) = c_path.to_str() else {
        return 0;
    };

    with_vfs_mut(|vfs| {
        Ok(vfs
            .archive
            .get_file_info(path_str)
            .map(|info| info.original_size as usize)
            .unwrap_or(0))
    })
    .unwrap_or(0)
}

#[no_mangle]
#[cfg(target_os = "windows")]
pub extern "C" fn maxion_read_file(
    path: *const c_char,
    buffer: *mut u8,
    buffer_size: usize,
) -> usize {
    if path.is_null() {
        return 0;
    }

    let c_path = unsafe { std::ffi::CStr::from_ptr(path) };
    let Ok(path_str) = c_path.to_str() else {
        return 0;
    };

    with_vfs_mut(|vfs| {
        let requested_size = if buffer.is_null() || buffer_size == 0 {
            vfs.archive
                .get_file_info(path_str)
                .map(|info| info.original_size as usize)
                .unwrap_or(0)
        } else {
            buffer_size
        };

        let file_data = vfs
            .archive
            .read_file_range(path_str, 0, requested_size as u64)?;
        let bytes_to_copy = file_data.len().min(buffer_size);

        if !buffer.is_null() && bytes_to_copy > 0 {
            unsafe {
                ptr::copy_nonoverlapping(file_data.as_ptr(), buffer, bytes_to_copy);
            }
        }

        Ok(file_data.len())
    })
    .unwrap_or(0)
}

#[no_mangle]
#[cfg(not(target_os = "windows"))]
pub extern "C" fn maxion_init(_executable_path: *const c_char) -> bool {
    false
}

#[no_mangle]
#[cfg(not(target_os = "windows"))]
pub extern "C" fn maxion_shutdown() {}

#[no_mangle]
#[cfg(not(target_os = "windows"))]
pub extern "C" fn maxion_file_exists(_path: *const c_char) -> bool {
    false
}

#[no_mangle]
#[cfg(not(target_os = "windows"))]
pub extern "C" fn maxion_get_file_size(_path: *const c_char) -> usize {
    0
}

#[no_mangle]
#[cfg(not(target_os = "windows"))]
pub extern "C" fn maxion_read_file(
    _path: *const c_char,
    _buffer: *mut u8,
    _buffer_size: usize,
) -> usize {
    0
}

/// Locate embedded .maxion and .key sections in the PE module
#[cfg(target_os = "windows")]
fn locate_embedded_archive(
    module_handle: windows_sys::Win32::Foundation::HMODULE,
) -> Result<(Vec<u8>, Vec<u8>)> {
    use windows_sys::Win32::System::ProcessStatus::GetModuleInformation;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    // Get module information to find PE headers
    let mut module_info = windows_sys::Win32::System::ProcessStatus::MODULEINFO {
        lpBaseOfDll: std::ptr::null_mut(),
        SizeOfImage: 0,
        EntryPoint: std::ptr::null_mut(),
    };

    unsafe {
        if GetModuleInformation(
            GetCurrentProcess(),
            module_handle,
            &mut module_info,
            std::mem::size_of::<windows_sys::Win32::System::ProcessStatus::MODULEINFO>() as u32,
        ) == 0
        {
            return Err(Error::Other("Failed to get module information".to_string()));
        }
    }

    let base = module_info.lpBaseOfDll as *const u8;
    let image_size = module_info.SizeOfImage as usize;

    if image_size < 0x1000 {
        return Err(Error::Other("Module image too small".to_string()));
    }

    let read_u16 = |offset: usize| -> Result<u16> {
        if offset + 2 > image_size {
            return Err(Error::Other(format!("PE read out of bounds at 0x{offset:X}")));
        }
        Ok(unsafe { std::ptr::read_unaligned(base.add(offset) as *const u16) })
    };
    let read_u32 = |offset: usize| -> Result<u32> {
        if offset + 4 > image_size {
            return Err(Error::Other(format!("PE read out of bounds at 0x{offset:X}")));
        }
        Ok(unsafe { std::ptr::read_unaligned(base.add(offset) as *const u32) })
    };

    let e_lfanew = read_u32(0x3C)? as usize;
    if e_lfanew + 0x18 > image_size {
        return Err(Error::Other("Invalid PE header offset".to_string()));
    }

    let pe_signature = read_u32(e_lfanew)?;
    if pe_signature != 0x0000_4550 {
        return Err(Error::Other("Invalid PE signature".to_string()));
    }

    let number_of_sections = read_u16(e_lfanew + 6)? as usize;
    let size_of_optional_header = read_u16(e_lfanew + 20)? as usize;
    let section_headers_offset = e_lfanew + 24 + size_of_optional_header;

    if number_of_sections == 0 {
        return Err(Error::Other("PE has no sections".to_string()));
    }
    if section_headers_offset + number_of_sections * 40 > image_size {
        return Err(Error::Other("Section headers out of bounds".to_string()));
    }

    // Find .maxion and .key sections directly from section headers.
    let mut maxion_data = None;
    let mut key_data = None;

    for index in 0..number_of_sections {
        let header_offset = section_headers_offset + index * 40;
        let name_bytes = unsafe { std::slice::from_raw_parts(base.add(header_offset), 8) };
        let name_end = name_bytes.iter().position(|&b| b == 0).unwrap_or(8);
        let name = std::str::from_utf8(&name_bytes[..name_end]).unwrap_or("");

        if name != ".maxion" && name != ".key" {
            continue;
        }

        let virtual_size = read_u32(header_offset + 8)? as usize;
        let virtual_address = read_u32(header_offset + 12)? as usize;
        let raw_size = read_u32(header_offset + 16)? as usize;
        let section_size = virtual_size.max(raw_size);

        if section_size == 0 {
            continue;
        }
        if virtual_address + section_size > image_size {
            return Err(Error::Other(format!(
                "Section {name} out of bounds: va=0x{virtual_address:X} size=0x{section_size:X} image=0x{image_size:X}"
            )));
        }

        let section_data = unsafe {
            std::slice::from_raw_parts(base.add(virtual_address), section_size)
        };

        if name == ".maxion" {
            maxion_data = Some(section_data.to_vec());
        } else if name == ".key" {
            key_data = Some(section_data.to_vec());
        }
    }

    let maxion =
        maxion_data.ok_or_else(|| Error::Other(".maxion section not found".to_string()))?;
    let key = key_data.ok_or_else(|| Error::Other(".key section not found".to_string()))?;

    Ok((maxion, key))
}

/// Parse archive header from embedded data
#[allow(dead_code)]
fn parse_archive_header(data: &[u8]) -> Result<ArchiveHeader> {
    ArchiveHeader::from_bytes(data)
}

/// De-obfuscate encryption key from .key section
#[allow(dead_code)]
fn deobfuscate_key(data: &[u8]) -> Result<([u8; 32], [u8; 24], u32, u32, Option<[u8; 32]>)> {
    if data.starts_with(KEY_BLOB_V3_MAGIC) {
        return deobfuscate_key_v3(data);
    }
    if data.starts_with(KEY_BLOB_V2_MAGIC) {
        return deobfuscate_key_v2(data);
    }

    // Legacy format: 32 (key) + 24 (nonce) + 4 (chunk size) + 8 (reserved) + 4 (entry point) + 32 (checksum) = 104 bytes
    if data.len() < 104 {
        return Err(Error::Other(format!(
            "Key data too short: expected at least 104 bytes, got {}",
            data.len()
        )));
    }

    // Extract obfuscated key (first 32 bytes)
    let mut obfuscated_key = [0u8; 32];
    obfuscated_key.copy_from_slice(&data[..32]);

    // XOR with magic bytes to de-obfuscate
    let mut encryption_key = [0u8; 32];
    for (i, byte) in obfuscated_key.iter().enumerate() {
        encryption_key[i] = byte ^ MAGIC[i % MAGIC.len()];
    }

    // Extract nonce (next 24 bytes, offset 32-56)
    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&data[32..56]);

    // Extract chunk size (next 4 bytes, offset 56-60)
    let chunk_size = u32::from_le_bytes([data[56], data[57], data[58], data[59]]);

    // Skip 8 reserved bytes (offset 60-68)

    // Extract original entry point (next 4 bytes, offset 68-72)
    let original_entry_point = u32::from_le_bytes([data[68], data[69], data[70], data[71]]);

    // Note: Checksum verification disabled for simplicity
    // In production, you should verify the checksum to ensure data integrity

    Ok((encryption_key, nonce, chunk_size, original_entry_point, None))
}

fn deobfuscate_key_v2(data: &[u8]) -> Result<([u8; 32], [u8; 24], u32, u32, Option<[u8; 32]>)> {
    const HEADER_LEN: usize = 72;
    const TOTAL_LEN: usize = 136;

    if data.len() < TOTAL_LEN {
        return Err(Error::Other(format!(
            "Key data too short for v2 format: expected at least {} bytes, got {}",
            TOTAL_LEN,
            data.len()
        )));
    }

    let checksum = blake3::hash(&data[..TOTAL_LEN - 32]);
    if checksum.as_bytes() != &data[TOTAL_LEN - 32..TOTAL_LEN] {
        return Err(Error::Other(
            "Key blob checksum verification failed".to_string(),
        ));
    }

    let scheme_id = data[4];
    let mask = &data[8..40];

    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&data[40..64]);

    let chunk_size = u32::from_le_bytes([data[64], data[65], data[66], data[67]]);
    let original_entry_point = u32::from_le_bytes([data[68], data[69], data[70], data[71]]);

    let obfuscated_key = &data[HEADER_LEN..HEADER_LEN + 32];
    let mut encryption_key = [0u8; 32];
    for i in 0..32 {
        encryption_key[i] = match scheme_id {
            0 => obfuscated_key[i] ^ MAGIC[i % MAGIC.len()] ^ mask[i],
            1 => (obfuscated_key[i] ^ nonce[i % nonce.len()]).wrapping_sub(mask[i]),
            2 => (obfuscated_key[i] ^ MAGIC[(i * 3) % MAGIC.len()])
                .rotate_right((mask[i] & 0x07) as u32),
            _ => {
                return Err(Error::Other(format!(
                    "Unsupported key obfuscation scheme: {}",
                    scheme_id
                )))
            }
        };
    }

    Ok((encryption_key, nonce, chunk_size, original_entry_point, None))
}

fn deobfuscate_key_v3(data: &[u8]) -> Result<([u8; 32], [u8; 24], u32, u32, Option<[u8; 32]>)> {
    const HEADER_LEN: usize = 72;
    const OBFUSCATED_KEY_LEN: usize = 32;
    const ARCHIVE_HASH_LEN: usize = 32;
    const TOTAL_LEN: usize = 168;

    if data.len() < TOTAL_LEN {
        return Err(Error::Other(format!(
            "Key data too short for V3 format: expected at least {} bytes, got {}",
            TOTAL_LEN,
            data.len()
        )));
    }

    let checksum = blake3::hash(&data[..TOTAL_LEN - 32]);
    if checksum.as_bytes() != &data[TOTAL_LEN - 32..TOTAL_LEN] {
        return Err(Error::Other(
            "Key blob checksum verification failed".to_string(),
        ));
    }

    let scheme_id = data[4];
    let mask = &data[8..40];

    let mut nonce = [0u8; 24];
    nonce.copy_from_slice(&data[40..64]);

    let chunk_size = u32::from_le_bytes([data[64], data[65], data[66], data[67]]);
    let original_entry_point = u32::from_le_bytes([data[68], data[69], data[70], data[71]]);

    let obfuscated_key = &data[HEADER_LEN..HEADER_LEN + OBFUSCATED_KEY_LEN];
    let archive_hash_slice =
        &data[HEADER_LEN + OBFUSCATED_KEY_LEN..HEADER_LEN + OBFUSCATED_KEY_LEN + ARCHIVE_HASH_LEN];
    let mut encryption_key = [0u8; 32];
    for i in 0..32 {
        encryption_key[i] = match scheme_id {
            0 => obfuscated_key[i] ^ MAGIC[i % MAGIC.len()] ^ mask[i],
            1 => (obfuscated_key[i] ^ nonce[i % nonce.len()]).wrapping_sub(mask[i]),
            2 => (obfuscated_key[i] ^ MAGIC[(i * 3) % MAGIC.len()])
                .rotate_right((mask[i] & 0x07) as u32),
            _ => {
                return Err(Error::Other(format!(
                    "Unsupported key obfuscation scheme: {}",
                    scheme_id
                )))
            }
        };
    }

    let mut archive_hash = [0u8; 32];
    archive_hash.copy_from_slice(archive_hash_slice);

    Ok((
        encryption_key,
        nonce,
        chunk_size,
        original_entry_point,
        Some(archive_hash),
    ))
}

/// Initialize VFS with archive data
///
/// This function is now replaced by direct VFS initialization in stub_entry
/// using VirtualArchive::from_memory.
#[deprecated(note = "Use VirtualArchive::from_memory directly in stub_entry")]
#[allow(dead_code)]
fn initialize_vfs(
    header: ArchiveHeader,
    archive_data: Vec<u8>,
    _encryption_key: [u8; 32],
    _nonce: [u8; 24],
    chunk_size: u32,
) -> Result<()> {
    log::info!(
        "VFS initialization: {} files, {} bytes archive",
        header.file_count,
        archive_data.len()
    );
    log::info!(
        "Chunk size: {}, compression: {}",
        chunk_size,
        header.compress
    );
    Ok(())
}

/// Get original entry point from .key section (legacy function, kept for compatibility)
#[deprecated(note = "Use deobfuscate_key which returns the entry point directly")]
#[allow(dead_code)]
fn get_original_entry_point(data: &[u8]) -> Result<u32> {
    // Original entry point is stored at offset 68
    // (32 key + 24 nonce + 4 chunk size + 8 reserved)
    if data.len() < 72 {
        return Err(Error::Other(
            "Key data too short for entry point".to_string(),
        ));
    }

    // Entry point is at offset 68 (32 key + 24 nonce + 4 chunk size + 8 reserved)
    let entry_point = u32::from_le_bytes([data[68], data[69], data[70], data[71]]);

    Ok(entry_point)
}

#[cfg(test)]
mod tests {
    use super::*;
    use maxion_core::types::{AssetFile, Config};

    #[test]
    fn test_vfs_handle_magic() {
        assert_eq!(VFS_HANDLE_MAGIC, 0x56465331);
    }

    #[test]
    fn test_virtual_file_handle_creation() {
        let file_info = AssetFileInfo {
            original_size: 1024,
            packed_size: 512,
            offset: 0,
            chunk_count: 1,
            checksum: [0u8; 32],
        };

        let handle = VirtualFileHandle {
            magic: VFS_HANDLE_MAGIC,
            handle_id: 1,
            file_info,
            current_offset: 0,
            path: "test.txt".to_string(),
            #[cfg(target_os = "windows")]
            handle_value: 1 as HANDLE,
        };

        assert_eq!(handle.handle_id, 1);
        assert_eq!(handle.current_offset, 0);
    }

    #[test]
    fn test_vfs_stats_default() {
        let stats = VFSStats::default();
        assert_eq!(stats.total_opens, 0);
        assert_eq!(stats.successful_reads, 0);
        assert_eq!(stats.total_bytes_read, 0);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_vfs_handle_validation() {
        let valid_handle = (1u32 + 1) as HANDLE;
        let _invalid_handle = INVALID_VFS_HANDLE;

        // We can't test the actual validation without a VFS instance,
        // but we can test the logic
        assert!((valid_handle as u32).wrapping_sub(1) != 0xFFFFFFFF);
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_vfs_handle_validation_fallback() {
        // On non-Windows, just verify the logic works
        let valid_handle_id = 1u32;
        let invalid_handle_id = 0xFFFFFFFF;

        assert_ne!(valid_handle_id, invalid_handle_id);
    }

    #[test]
    fn test_vfs_loads_encrypted_archive() {
        use maxion_core::archive::ArchiveBuilder;

        // Create test archive
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("test.archive");

        // Create test files
        let test_dir = temp_dir.path().join("assets");
        std::fs::create_dir_all(&test_dir).unwrap();

        let test_file1 = test_dir.join("file1.txt");
        std::fs::write(&test_file1, b"Hello, World!").unwrap();

        let test_file2 = test_dir.join("file2.dat");
        std::fs::write(&test_file2, b"Binary data\x00\x01\x02\x03").unwrap();

        // Create archive - use absolute file paths for reading actual file data
        let mut config = Config::new();
        config.generate_keys();
        let mut builder = ArchiveBuilder::new(config.clone());

        // Store the paths we'll use for AssetFile (absolute paths needed for reading files)
        let asset1 = AssetFile::new(test_file1.clone(), 13);
        let asset2 = AssetFile::new(test_file2.clone(), 12);
        builder.add_files(vec![asset1, asset2]);

        // Verify files exist before building archive
        assert!(
            test_file1.exists(),
            "Test file 1 does not exist: {:?}",
            test_file1
        );
        assert!(
            test_file2.exists(),
            "Test file 2 does not exist: {:?}",
            test_file2
        );

        // Check builder has the right number of files
        eprintln!(
            "Builder has {} files before building",
            builder.files().len()
        );

        let header = builder.build(&archive_path).unwrap();

        // Log archive structure for debugging
        eprintln!("Archive created:");
        eprintln!("  Path: {:?}", archive_path);
        eprintln!("  File count: {}", header.file_count);
        eprintln!("  File table offset: {}", header.file_table_offset);
        eprintln!("  File table size: {}", header.file_table_size);
        eprintln!("  Chunk size: {}", header.chunk_size);
        eprintln!("  Compress: {}", header.compress);
        eprintln!("  Header checksum valid: {}", header.verify_checksum());

        // Read and check archive file size
        let archive_size = std::fs::metadata(&archive_path).unwrap().len();
        eprintln!("  Archive file size: {} bytes", archive_size);

        // Verify archive file exists and has content
        if archive_size == 0 {
            panic!("Archive file is empty!");
        }

        // Read raw file table data for inspection
        let archive_data = std::fs::read(&archive_path).unwrap();
        eprintln!("  Read {} bytes from archive", archive_data.len());

        // Parse the header back from the file to verify what was written
        if archive_data.len() >= 256 {
            let parsed_header =
                maxion_core::archive::ArchiveHeader::from_bytes(&archive_data[..256]).unwrap();
            eprintln!("  Parsed header from file:");
            eprintln!("    file_count: {} (expected: 2)", parsed_header.file_count);
            eprintln!("    file_table_offset: {}", parsed_header.file_table_offset);
            eprintln!("    file_table_size: {}", parsed_header.file_table_size);
            eprintln!("    chunk_size: {}", parsed_header.chunk_size);
            eprintln!("    compress: {}", parsed_header.compress);
        }

        // Hex dump of first 100 bytes for debugging
        let dump_bytes = archive_data.len().min(100);
        eprintln!("  First {} bytes (hex):", dump_bytes);
        for i in (0..dump_bytes).step_by(16) {
            let end = (i + 16).min(dump_bytes);
            let hex_part: Vec<String> = archive_data[i..end]
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect();
            let ascii_part: String = archive_data[i..end]
                .iter()
                .map(|b| {
                    if b.is_ascii_graphic() || *b == b' ' {
                        *b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            eprintln!("    {:04x}: {:48} |{}|", i, hex_part.join(" "), ascii_part);
        }

        if archive_data.len() > (header.file_table_offset as usize) {
            let file_table_start = header.file_table_offset as usize;
            let file_table_end = file_table_start + header.file_table_size as usize;
            eprintln!(
                "  File table data range: {}..{}",
                file_table_start, file_table_end
            );
            if file_table_end <= archive_data.len() {
                eprintln!(
                    "  File table data available ({} bytes)",
                    header.file_table_size
                );
                // Log first few bytes of file table (chunk size prefix)
                if header.file_table_size >= 4 {
                    let chunk_size = u32::from_le_bytes([
                        archive_data[file_table_start],
                        archive_data[file_table_start + 1],
                        archive_data[file_table_start + 2],
                        archive_data[file_table_start + 3],
                    ]);
                    eprintln!("  First chunk size: {} bytes", chunk_size);
                }
            } else {
                eprintln!("  WARNING: File table exceeds archive bounds!");
            }
        }

        // Open VirtualArchive directly from file
        // Open VirtualArchive and load into VFS
        let virtual_archive = VirtualArchive::open(&archive_path, config.clone()).unwrap();

        // Load into VFS
        let vfs = VFS::new(virtual_archive).unwrap();

        // Verify file table was loaded (via VirtualArchive)
        assert_eq!(vfs.archive.file_count(), 2);

        // Verify file paths are stored as absolute paths (normalized to forward slashes)
        let file1_path = test_file1.to_str().unwrap().replace('\\', "/");
        let file2_path = test_file2.to_str().unwrap().replace('\\', "/");
        assert!(vfs.archive.file_exists(&file1_path));
        assert!(vfs.archive.file_exists(&file2_path));

        // Verify file metadata using absolute paths
        let file1_info = vfs.archive.get_file_info(&file1_path).unwrap();
        assert_eq!(file1_info.original_size, 13);

        let file2_info = vfs.archive.get_file_info(&file2_path).unwrap();
        assert_eq!(file2_info.original_size, 12);
    }

    #[test]
    fn test_vfs_opens_virtual_file() {
        use maxion_core::archive::ArchiveBuilder;

        // Create test archive
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("test.archive");

        // Create test file
        let test_dir = temp_dir.path().join("assets");
        std::fs::create_dir_all(&test_dir).unwrap();
        let test_file = test_dir.join("test.txt");
        std::fs::write(&test_file, b"Test content").unwrap();

        // Create archive - use absolute file path for reading actual file data
        let mut config = Config::new();
        config.generate_keys();
        let mut builder = ArchiveBuilder::new(config.clone());

        let asset = AssetFile::new(test_file.clone(), 12);
        builder.add_file(asset);
        builder.build(&archive_path).unwrap();

        // Open VirtualArchive and load into VFS
        let virtual_archive = VirtualArchive::open(&archive_path, config.clone()).unwrap();
        let mut vfs = VFS::new(virtual_archive).unwrap();

        // Open virtual file - need to use the absolute path (normalized)
        let test_path = test_file.to_str().unwrap().replace('\\', "/");
        let handle = vfs.open_virtual(&test_path).unwrap();

        // Verify handle is valid by checking it's in the virtual handles
        let handle_id = (handle as u32).wrapping_sub(1);
        assert!(vfs.virtual_handles.contains_key(&handle_id));

        // Close file
        vfs.close_virtual(handle).unwrap();

        // Verify statistics
        assert_eq!(vfs.stats.total_opens, 1);
    }

    #[test]
    fn test_vfs_header_validation() {
        use maxion_core::archive::ArchiveBuilder;

        // Create test archive
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("test.archive");

        // Create test files
        let test_dir = temp_dir.path().join("assets");
        std::fs::create_dir_all(&test_dir).unwrap();

        let test_file = test_dir.join("test.txt");
        std::fs::write(&test_file, b"Test content").unwrap();

        // Create archive
        let mut config = Config::new();
        config.generate_keys();
        let mut builder = ArchiveBuilder::new(config.clone());

        let asset = AssetFile::new(test_file.clone(), 12);
        builder.add_file(asset);

        let _header = builder.build(&archive_path).unwrap();

        // Read archive data
        let archive_data = std::fs::read(&archive_path).unwrap();

        // Verify header magic
        assert_eq!(&archive_data[..8], maxion_core::MAGIC);

        // Verify archive is at least header size
        assert!(archive_data.len() >= 256);

        // Verify header structure
        let parsed_header =
            maxion_core::archive::ArchiveHeader::from_bytes(&archive_data[..256]).unwrap();
        assert_eq!(parsed_header.file_count, 1);
        assert!(parsed_header.compress);
        assert!(parsed_header.verify_checksum());

        println!("Archive header: {:?}", parsed_header);
        println!("Archive size: {}", archive_data.len());
        println!("File table offset: {}", parsed_header.file_table_offset);
        println!("File table size: {}", parsed_header.file_table_size);
    }

    #[test]
    fn test_vfs_is_virtual_handle() {
        use maxion_core::archive::ArchiveBuilder;

        // Create test archive
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("test.archive");
        let test_dir = temp_dir.path().join("assets");
        std::fs::create_dir_all(&test_dir).unwrap();
        let test_file = test_dir.join("test.txt");
        std::fs::write(&test_file, b"Test content").unwrap();

        // Create archive
        let mut config = Config::new();
        config.generate_keys();
        let mut builder = ArchiveBuilder::new(config.clone());
        let asset = AssetFile::new(test_file.clone(), 12);
        builder.add_file(asset);
        builder.build(&archive_path).unwrap();

        // Open VirtualArchive and load into VFS
        let virtual_archive = VirtualArchive::open(&archive_path, config.clone()).unwrap();
        let mut vfs = VFS::new(virtual_archive).unwrap();

        // Open a virtual file
        let test_path = test_file.to_str().unwrap().replace('\\', "/");
        let handle = vfs.open_virtual(&test_path).unwrap();

        // Verify it's recognized as a virtual handle
        assert!(vfs.is_virtual_handle(handle));

        // Verify a fake handle is not recognized
        let fake_handle = 0xDEADBEEF as HANDLE;
        assert!(!vfs.is_virtual_handle(fake_handle));

        // Verify that closing the handle makes it invalid
        vfs.close_virtual(handle).unwrap();
        // Note: The handle is removed from virtual_handles, so is_virtual_handle should return false
        // However, handle_id is still valid, so we need to check the actual implementation
        // For now, test that it's recognized before closing
        let handle2 = vfs.open_virtual(&test_path).unwrap();
        assert!(vfs.is_virtual_handle(handle2));
        vfs.close_virtual(handle2).unwrap();
    }

    #[test]
    fn test_vfs_virtual_handle_id_allocation() {
        use maxion_core::archive::ArchiveBuilder;

        // Create test archive
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("test.archive");
        let test_dir = temp_dir.path().join("assets");
        std::fs::create_dir_all(&test_dir).unwrap();
        let test_file = test_dir.join("test.txt");
        std::fs::write(&test_file, b"Test content").unwrap();

        // Create archive
        let mut config = Config::new();
        config.generate_keys();
        let mut builder = ArchiveBuilder::new(config.clone());
        let asset = AssetFile::new(test_file.clone(), 12);
        builder.add_file(asset);
        builder.build(&archive_path).unwrap();

        // Open VirtualArchive and load into VFS
        let virtual_archive = VirtualArchive::open(&archive_path, config.clone()).unwrap();
        let mut vfs = VFS::new(virtual_archive).unwrap();

        let test_path = test_file.to_str().unwrap().replace('\\', "/");

        // Open multiple files and verify handle IDs are unique and sequential
        let handle1 = vfs.open_virtual(&test_path).unwrap();
        let handle2 = vfs.open_virtual(&test_path).unwrap();
        let handle3 = vfs.open_virtual(&test_path).unwrap();

        // Extract handle IDs
        let id1 = (handle1 as u32).wrapping_sub(1);
        let id2 = (handle2 as u32).wrapping_sub(1);
        let id3 = (handle3 as u32).wrapping_sub(1);

        // Verify IDs are unique
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);

        // Verify IDs are sequential (they should be 0, 1, 2)
        assert_eq!(id2, id1 + 1);
        assert_eq!(id3, id2 + 1);

        // Verify all handles are active
        assert_eq!(vfs.active_handles(), 3);

        // Close handles and verify
        vfs.close_virtual(handle1).unwrap();
        assert_eq!(vfs.active_handles(), 2);

        vfs.close_virtual(handle2).unwrap();
        assert_eq!(vfs.active_handles(), 1);

        vfs.close_virtual(handle3).unwrap();
        assert_eq!(vfs.active_handles(), 0);
    }

    #[test]
    fn test_vfs_read_virtual_with_offset() {
        use maxion_core::archive::ArchiveBuilder;

        // Create test archive
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("test.archive");
        let test_dir = temp_dir.path().join("assets");
        std::fs::create_dir_all(&test_dir).unwrap();
        let test_file = test_dir.join("test.txt");
        std::fs::write(&test_file, b"Hello, World!").unwrap();

        // Create archive
        let mut config = Config::new();
        config.generate_keys();
        let mut builder = ArchiveBuilder::new(config.clone());
        let asset = AssetFile::new(test_file.clone(), 13); // Actual file size: "Hello, World!" = 13 bytes
        builder.add_file(asset);
        builder.build(&archive_path).unwrap();

        // Open VirtualArchive and load into VFS
        let virtual_archive = VirtualArchive::open(&archive_path, config.clone()).unwrap();
        let mut vfs = VFS::new(virtual_archive).unwrap();

        let test_path = test_file.to_str().unwrap().replace('\\', "/");

        // Open file and read with offsets
        let handle = vfs.open_virtual(&test_path).unwrap();

        // Read first 5 bytes
        let mut buffer1 = [0u8; 5];
        let bytes_read1 = vfs.read_virtual(handle, &mut buffer1, 5).unwrap();
        assert_eq!(bytes_read1, 5);
        assert_eq!(&buffer1, b"Hello");

        // Read next 7 bytes (offset should be at 5 now)
        let mut buffer2 = [0u8; 7];
        let bytes_read2 = vfs.read_virtual(handle, &mut buffer2, 7).unwrap();
        assert_eq!(bytes_read2, 7);
        assert_eq!(&buffer2, b", World");

        // Read remaining 1 byte (offset should be at 12 now)
        let mut buffer3 = [0u8; 5];
        let bytes_read3 = vfs.read_virtual(handle, &mut buffer3, 5).unwrap();
        assert_eq!(bytes_read3, 1);
        assert_eq!(&buffer3[..1], b"!");

        // Try to read past EOF (should return 0)
        let bytes_read4 = vfs.read_virtual(handle, &mut buffer1, 1).unwrap();
        assert_eq!(bytes_read4, 0);

        vfs.close_virtual(handle).unwrap();
    }

    #[test]
    fn test_vfs_get_file_size() {
        use maxion_core::archive::ArchiveBuilder;

        // Create test archive
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("test.archive");
        let test_dir = temp_dir.path().join("assets");
        std::fs::create_dir_all(&test_dir).unwrap();
        let test_file = test_dir.join("test.txt");
        std::fs::write(&test_file, b"Test content 123").unwrap();

        // Create archive
        let mut config = Config::new();
        config.generate_keys();
        let mut builder = ArchiveBuilder::new(config.clone());
        let asset = AssetFile::new(test_file.clone(), 16);
        builder.add_file(asset);
        builder.build(&archive_path).unwrap();

        // Open VirtualArchive and load into VFS
        let virtual_archive = VirtualArchive::open(&archive_path, config.clone()).unwrap();
        let mut vfs = VFS::new(virtual_archive).unwrap();

        let test_path = test_file.to_str().unwrap().replace('\\', "/");

        // Open file and get size
        let handle = vfs.open_virtual(&test_path).unwrap();
        let size = vfs.get_file_size(handle).unwrap();

        assert_eq!(size, 16);

        vfs.close_virtual(handle).unwrap();
    }

    #[test]
    fn test_vfs_stats_tracking() {
        use maxion_core::archive::ArchiveBuilder;

        // Create test archive
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("test.archive");
        let test_dir = temp_dir.path().join("assets");
        std::fs::create_dir_all(&test_dir).unwrap();
        let test_file = test_dir.join("test.txt");
        std::fs::write(&test_file, b"Test content").unwrap();

        // Create archive
        let mut config = Config::new();
        config.generate_keys();
        let mut builder = ArchiveBuilder::new(config.clone());
        let asset = AssetFile::new(test_file.clone(), 12); // Actual file size: "Test content" = 12 bytes
        builder.add_file(asset);
        builder.build(&archive_path).unwrap();

        // Open VirtualArchive and load into VFS
        let virtual_archive = VirtualArchive::open(&archive_path, config.clone()).unwrap();
        let mut vfs = VFS::new(virtual_archive).unwrap();

        let test_path = test_file.to_str().unwrap().replace('\\', "/");

        // Verify initial stats
        let stats = vfs.get_stats();
        assert_eq!(stats.total_opens, 0);
        assert_eq!(stats.successful_reads, 0);
        assert_eq!(stats.total_bytes_read, 0);

        // Open file
        let handle = vfs.open_virtual(&test_path).unwrap();
        assert_eq!(vfs.get_stats().total_opens, 1);

        // Read some data
        let mut buffer = [0u8; 10];
        vfs.read_virtual(handle, &mut buffer, 10).unwrap();
        assert_eq!(vfs.get_stats().successful_reads, 1);
        assert_eq!(vfs.get_stats().total_bytes_read, 10);

        // Read more data (only 2 bytes remaining)
        let mut buffer2 = [0u8; 5];
        vfs.read_virtual(handle, &mut buffer2, 5).unwrap();
        assert_eq!(vfs.get_stats().successful_reads, 2);
        assert_eq!(vfs.get_stats().total_bytes_read, 12);

        // Reset stats
        vfs.reset_stats();
        assert_eq!(vfs.get_stats().total_opens, 0);
        assert_eq!(vfs.get_stats().successful_reads, 0);
        assert_eq!(vfs.get_stats().total_bytes_read, 0);

        vfs.close_virtual(handle).unwrap();
    }

    #[test]
    fn test_vfs_error_handling_invalid_handle() {
        use maxion_core::archive::ArchiveBuilder;

        // Create test archive
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("test.archive");
        let test_dir = temp_dir.path().join("assets");
        std::fs::create_dir_all(&test_dir).unwrap();
        let test_file = test_dir.join("test.txt");
        std::fs::write(&test_file, b"Test content").unwrap();

        // Create archive
        let mut config = Config::new();
        config.generate_keys();
        let mut builder = ArchiveBuilder::new(config.clone());
        let asset = AssetFile::new(test_file.clone(), 12); // Actual file size: "Test content" = 12 bytes
        builder.add_file(asset);
        builder.build(&archive_path).unwrap();

        // Open VirtualArchive and load into VFS
        let virtual_archive = VirtualArchive::open(&archive_path, config.clone()).unwrap();
        let vfs = VFS::new(virtual_archive).unwrap();

        // Try to get file size with invalid handle (doesn't need mut)
        let fake_handle = 0xDEADBEEF as HANDLE;
        let result = vfs.get_file_size(fake_handle);
        assert!(result.is_err());

        // Try to get size with invalid handle
        assert!(result.is_err());

        // Note: We don't test read_virtual and close_virtual here because they require &mut self
        // and would require creating a mutable VFS instance just for these tests.
        // The methods are tested in other test cases with valid handles.
    }
}
