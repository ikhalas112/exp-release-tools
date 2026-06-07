//! Import-Free Loader Stub for PE Injection
//!
//! This stub resolves Windows APIs dynamically via PEB walking,
//! eliminating the need for an import table. This allows the stub
//! to be injected as a raw .text section without .idata.
//!
//! Design Goals:
//! - No imports (PEB-based API resolution)
//! - Position-independent code
//! - Minimal size (< 1KB)
//! - Windows x86 / x86_64

#![no_std]
#![allow(clippy::upper_case_acronyms)]

#[cfg(target_os = "windows")]
use core::arch::asm;
#[cfg(target_os = "windows")]
use core::mem;

// Type definitions for Windows API
type HMODULE = *mut u8;
#[allow(dead_code)]
type LPCSTR = *const u8;
#[allow(dead_code)]
type LPSTR = *mut u8;
#[allow(dead_code)]
type DWORD = u32;
type FARPROC = *mut u8;
#[allow(dead_code)] // Reserved for future use
type BOOL = i32;

/// Exit codes for debugging
#[allow(dead_code)] // Reserved for future use
const EXIT_SUCCESS: u32 = 0;
#[allow(dead_code)] // Reserved for future use
const EXIT_FAILURE_KERNEL32: u32 = 102;
#[allow(dead_code)] // Reserved for future use
const EXIT_FAILURE_LOADLIBRARY: u32 = 104;
#[allow(dead_code)]
const EXIT_FAILURE_PEB: u32 = 101;
#[allow(dead_code)]
const EXIT_FAILURE_GETPROCADDR: u32 = 103;
#[allow(dead_code)]
const EXIT_FAILURE_DLL: u32 = 105;
#[allow(dead_code)]
const EXIT_FAILURE_EXPORT: u32 = 106;

/// Maximum path length for Windows
#[allow(dead_code)]
const MAX_PATH: usize = 260;

/// PEB (Process Environment Block) structure offsets
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const PEB_LDR_OFFSET: usize = 0x18;
#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
const LDR_IN_LOAD_ORDER_OFFSET: usize = 0x10;

#[cfg(all(target_os = "windows", target_arch = "x86"))]
const PEB_LDR_OFFSET: usize = 0x0C;
#[cfg(all(target_os = "windows", target_arch = "x86"))]
const LDR_IN_LOAD_ORDER_OFFSET: usize = 0x0C;

/// LIST_ENTRY structure
#[cfg(target_os = "windows")]
#[repr(C)]
struct ListEntry {
    flink: *mut ListEntry,
    blink: *mut ListEntry,
}

/// LDR_DATA_TABLE_ENTRY structure (partial)
#[cfg(target_os = "windows")]
#[repr(C)]
struct LdrDataTableEntry {
    in_load_order_links: ListEntry,
    in_memory_order_links: ListEntry,
    in_initialization_order_links: ListEntry,
    dll_base: HMODULE,
    entry_point: FARPROC,
    size_of_image: usize,
    full_dll_name: usize, // Actually a UNICODE_STRING, we only need the offset
    base_dll_name: usize, // UNICODE_STRING
}

/// Function pointer types for dynamically resolved APIs
#[allow(dead_code)]
type FnGetModuleFileNameA = unsafe extern "system" fn(HMODULE, LPSTR, DWORD) -> DWORD;
#[allow(dead_code)]
type FnLoadLibraryA = unsafe extern "system" fn(LPCSTR) -> HMODULE;
#[allow(dead_code)]
type FnGetProcAddress = unsafe extern "system" fn(HMODULE, LPCSTR) -> FARPROC;
#[allow(dead_code)]
type FnExitProcess = unsafe extern "system" fn(u32) -> !;

const HASH_KERNEL32_DLL: u32 = 0x7040ee75;
const HASH_EXIT_PROCESS: u32 = 0x024773de;
const HASH_GET_MODULE_FILE_NAME_A: u32 = 0xd13bcded;
const HASH_LOAD_LIBRARY_A: u32 = 0x0666395b;
const HASH_GET_PROC_ADDRESS: u32 = 0x82172f7f;

/// Simple DJB2 hash for function name comparison
#[allow(dead_code)] // Reserved for future use
fn hash_name(name: &[u8]) -> u32 {
    let mut hash: u32 = 5381;
    let mut i = 0;
    while i < name.len() && name[i] != 0 {
        let c = if name[i] >= b'A' && name[i] <= b'Z' {
            name[i] + 32
        } else {
            name[i]
        };
        hash = ((hash << 5).wrapping_add(hash)).wrapping_add(c as u32);
        i += 1;
    }
    hash
}

/// Read u32 from memory safely
#[cfg(target_os = "windows")]
fn read_u32(ptr: *const u8) -> u32 {
    unsafe { ptr.cast::<u32>().read_unaligned() }
}

/// Read u16 from memory safely
#[cfg(target_os = "windows")]
fn read_u16(ptr: *const u8) -> u16 {
    unsafe { ptr.cast::<u16>().read_unaligned() }
}

#[cfg(target_os = "windows")]
unsafe fn write_u32_le(dst: *mut u8, value: u32) {
    dst.cast::<u32>().write_unaligned(value);
}

#[cfg(target_os = "windows")]
unsafe fn write_u16_le(dst: *mut u8, value: u16) {
    dst.cast::<u16>().write_unaligned(value);
}

#[cfg(target_os = "windows")]
unsafe fn build_stub_dll_name(buf: *mut u8) {
    write_u32_le(buf.add(0), 0x6978616D);
    write_u32_le(buf.add(4), 0x735F6E6F);
    write_u32_le(buf.add(8), 0x2E627574);
    write_u32_le(buf.add(12), 0x006C6C64);
}

#[cfg(target_os = "windows")]
unsafe fn build_stub_entry_name(buf: *mut u8) {
    write_u32_le(buf.add(0), 0x62757473);
    write_u32_le(buf.add(4), 0x746E655F);
    write_u16_le(buf.add(8), 0x7972);
    *buf.add(10) = 0;
}

/// Resolve a function from a module by name (case-insensitive)
#[cfg(target_os = "windows")]
#[inline(never)]
unsafe fn get_export_by_hash(module_base: HMODULE, target_hash: u32) -> FARPROC {
    // DOS header
    let dos_header = module_base;
    let e_lfanew = read_u32(dos_header.add(0x3C)) as usize;

    // PE header
    let pe_header = dos_header.add(e_lfanew);
    let machine = read_u16(pe_header.add(0x4));
    #[cfg(target_arch = "x86_64")]
    if machine != 0x8664 {
        return core::ptr::null_mut();
    }
    #[cfg(target_arch = "x86")]
    if machine != 0x014C {
        return core::ptr::null_mut();
    }

    // Optional header
    let optional_header = pe_header.add(0x18);
    #[cfg(target_arch = "x86_64")]
    let export_dir_rva = read_u32(optional_header.add(0x70)) as usize;
    #[cfg(target_arch = "x86")]
    let export_dir_rva = read_u32(optional_header.add(0x60)) as usize;

    if export_dir_rva == 0 {
        return core::ptr::null_mut();
    }

    // Export directory
    let export_dir = module_base.add(export_dir_rva);
    let _number_of_functions = read_u32(export_dir.add(0x14)) as usize;
    let number_of_names = read_u32(export_dir.add(0x18)) as usize;
    let address_of_functions = read_u32(export_dir.add(0x1C)) as usize;
    let address_of_names = read_u32(export_dir.add(0x20)) as usize;
    let address_of_name_ordinals = read_u32(export_dir.add(0x24)) as usize;

    // Export names are sorted lexicographically, not by our hash. Walk linearly.
    let mut index: usize = 0;
    while index < number_of_names {
        let name_rva = read_u32(module_base.add(address_of_names + index * 4)) as usize;
        let func_name_ptr = module_base.add(name_rva);

        // Read function name (null-terminated)
        let mut func_name = [0u8; 64];
        let mut i = 0;
        while i < 63 {
            let c = *func_name_ptr.add(i);
            if c == 0 {
                break;
            }
            func_name[i] = c;
            i += 1;
        }

        if hash_name(&func_name[..i]) == target_hash {
            // Found! Get ordinal and function address
            let ordinal =
                read_u16(module_base.add(address_of_name_ordinals + index * 2)) as usize;
            let func_rva = read_u32(module_base.add(address_of_functions + ordinal * 4)) as usize;
            return module_base.add(func_rva);
        }
        index += 1;
    }

    core::ptr::null_mut()
}

/// Get PEB address from GS segment register
#[cfg(target_os = "windows")]
#[inline(never)]
unsafe fn get_peb() -> *mut u8 {
    let peb_ptr: *mut u8;
    #[cfg(target_arch = "x86_64")]
    asm!(
        "mov {}, gs:[60h]",
        out(reg) peb_ptr,
        options(nomem, nostack, pure)
    );
    #[cfg(target_arch = "x86")]
    asm!(
        "mov {}, fs:[30h]",
        out(reg) peb_ptr,
        options(nomem, nostack, pure)
    );
    peb_ptr
}

/// Find kernel32.dll base address by walking the module list
#[cfg(target_os = "windows")]
#[inline(never)]
unsafe fn find_kernel32() -> HMODULE {
    let peb = get_peb();

    // PEB->Ldr
    let ldr = *(peb.add(PEB_LDR_OFFSET) as *const *mut u8);

    // Ldr->InLoadOrderModuleList
    let module_list = *(ldr.add(LDR_IN_LOAD_ORDER_OFFSET) as *const *mut ListEntry);
    let mut current = module_list;

    // Walk the module list (3 modules ahead should get us past the exe)
    for _ in 0..64 {
        current = (*current).flink;

        // Check if we've wrapped around
        if current == module_list {
            break;
        }

        // Get LDR_DATA_TABLE_ENTRY
        let entry_bytes = current as *mut u8;
        let entry = entry_bytes.cast::<LdrDataTableEntry>();

        #[cfg(target_arch = "x86_64")]
        let (base_name_length_offset, base_name_buffer_offset) = (0x58usize, 0x60usize);
        #[cfg(target_arch = "x86")]
        let (base_name_length_offset, base_name_buffer_offset) = (0x2Cusize, 0x30usize);

        let base_name_offset = read_u16(entry_bytes.add(base_name_length_offset)) as usize;
        let base_name_buffer = *(entry_bytes.add(base_name_buffer_offset) as *const *const u16);

        // Convert UTF-16 to ASCII for comparison
        let mut name_bytes = [0u8; 32];
        let mut i = 0;
        while i < 32 && i * 2 < base_name_offset {
            let c = *base_name_buffer.add(i) as u8;
            if c == 0 {
                break;
            }
            name_bytes[i] = c;
            i += 1;
        }

        if hash_name(&name_bytes[..i]) == HASH_KERNEL32_DLL {
            return (*entry).dll_base;
        }
    }

    core::ptr::null_mut()
}

/// Panic handler
#[cfg(all(not(test), target_os = "windows"))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        // Try to get PEB and find ExitProcess
        let _peb = get_peb();
        let kernel32 = find_kernel32();

        if !kernel32.is_null() {
            let exit_process = get_export_by_hash(kernel32, HASH_EXIT_PROCESS);
            if !exit_process.is_null() {
                let exit_proc: FnExitProcess = core::mem::transmute(exit_process);
                exit_proc(99);
            }
        }

        // Infinite loop if we can't exit
        loop {}
    }
}

/// Entry point for injected stub
#[inline(never)]
#[no_mangle]
#[cfg(target_os = "windows")]
#[allow(non_snake_case)] // Windows API naming convention
#[allow(unused_unsafe)] // Required for Windows API calls
#[allow(useless_ptr_null_checks)] // Needed for checking raw function pointers
pub extern "C" fn stub_entry() -> u32 {
    unsafe {
        // Step 1: Get PEB and find kernel32.dll
        let kernel32 = find_kernel32();

        if kernel32.is_null() {
            // Can't exit cleanly, just hang
            #[allow(clippy::empty_loop)] // Intentional hang when unrecoverable
            loop {}
        }

        // Step 2: Resolve required APIs from kernel32
        let get_module_file_name_a =
            get_export_by_hash(kernel32, HASH_GET_MODULE_FILE_NAME_A);
        let load_library_a = get_export_by_hash(kernel32, HASH_LOAD_LIBRARY_A);
        let get_proc_address = get_export_by_hash(kernel32, HASH_GET_PROC_ADDRESS);
        let exit_process = get_export_by_hash(kernel32, HASH_EXIT_PROCESS);

        if exit_process.is_null() {
            #[allow(clippy::empty_loop)]
            loop {}
        }

        let ExitProcess: FnExitProcess = mem::transmute(exit_process);

        if get_module_file_name_a.is_null()
            || load_library_a.is_null()
            || get_proc_address.is_null()
        {
            ExitProcess(EXIT_FAILURE_GETPROCADDR);
        }

        let GetModuleFileNameA: FnGetModuleFileNameA = mem::transmute(get_module_file_name_a);
        let LoadLibraryA: FnLoadLibraryA = mem::transmute(load_library_a);
        let GetProcAddress: FnGetProcAddress = mem::transmute(get_proc_address);

        // Step 3: Get current executable path
        let mut exe_path: [u8; MAX_PATH] = [0; MAX_PATH];
        let path_len = GetModuleFileNameA(
            core::ptr::null_mut(),
            exe_path.as_mut_ptr(),
            MAX_PATH as u32,
        );

        if path_len == 0 || path_len >= MAX_PATH as u32 {
            ExitProcess(EXIT_FAILURE_PEB);
        }

        // Step 4: Find the last backslash to get the directory
        let mut last_slash_pos: usize = 0;
        let mut i: usize = path_len as usize;
        while i > 0 {
            i -= 1;
            if exe_path[i] == b'\\' {
                last_slash_pos = i;
                break;
            }
        }

        // Step 5: Build DLL path
        let mut dll_path: [u8; MAX_PATH + 32] = [0; MAX_PATH + 32];
        let mut dll_path_pos: usize = 0;

        // Copy directory part
        i = 0;
        while i <= last_slash_pos {
            dll_path[dll_path_pos] = exe_path[i];
            dll_path_pos += 1;
            i += 1;
        }

        // Append DLL name
        build_stub_dll_name(dll_path.as_mut_ptr().add(dll_path_pos));

        // Step 6: Load the stub DLL
        let dll_handle = LoadLibraryA(dll_path.as_ptr());

        if dll_handle.is_null() {
            ExitProcess(EXIT_FAILURE_DLL);
        }

        // Step 7: Get stub_entry function address
        let mut stub_entry_name = [0u8; 11];
        build_stub_entry_name(stub_entry_name.as_mut_ptr());
        let stub_entry_ptr = GetProcAddress(dll_handle, stub_entry_name.as_ptr());

        if stub_entry_ptr.is_null() {
            ExitProcess(EXIT_FAILURE_EXPORT);
        }

        // Step 8: Cast to function pointer and call it
        let stub_entry_fn: extern "C" fn() -> u32 = mem::transmute(stub_entry_ptr);

        // Call stub_entry (never returns here)
        let result = stub_entry_fn();

        // If we somehow return, exit with the result
        ExitProcess(result);
    }
}

// Prevent linker from removing stub_entry
#[cfg(target_os = "windows")]
#[used]
static _FORCE_EXPORT: extern "C" fn() -> u32 = stub_entry;
