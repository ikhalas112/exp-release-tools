/**
 * Maxion Loader Stub (C version)
 * 
 * This is a tiny loader stub that gets injected into protected executables.
 * It loads maxion_stub.dll from the same directory and jumps to stub_entry.
 * 
 * Design Goals:
 * - Minimal code size
 * - Position-independent
 * - No C library dependencies (Windows API only)
 * - Simple error handling via ExitProcess
 */

#include <windows.h>

// Constants
#define STUB_DLL_NAME "maxion_stub.dll"
#define STUB_ENTRY_NAME "stub_entry"
#define MAX_PATH 260

// Exit codes for debugging
#define EXIT_SUCCESS 0
#define EXIT_FAILURE_PATH 100
#define EXIT_FAILURE_DLL 101
#define EXIT_FAILURE_EXPORT 102

/**
 * Main entry point - injected into target .exe
 * 
 * This function is called when the protected executable starts.
 * It performs the following:
 * 1. Gets the current executable path
 * 2. Extracts the directory
 * 3. Loads maxion_stub.dll from that directory
 * 4. Gets the stub_entry function address
 * 5. Jumps to stub_entry (never returns here)
 */
__declspec(dllexport) void __stdcall loader_entry(void) {
    // Step 1: Get current executable path
    char exe_path[MAX_PATH];
    DWORD path_len = GetModuleFileNameA(NULL, exe_path, MAX_PATH);
    
    if (path_len == 0 || path_len >= MAX_PATH) {
        ExitProcess(EXIT_FAILURE_PATH);
    }
    
    // Step 2: Find the last backslash to get the directory
    char* last_slash = NULL;
    for (DWORD i = path_len; i > 0; i--) {
        if (exe_path[i - 1] == '\\') {
            last_slash = &exe_path[i - 1];
            break;
        }
    }
    
    // Build DLL path
    char dll_path[MAX_PATH + sizeof(STUB_DLL_NAME)];
    if (last_slash) {
        // Copy directory part
        DWORD dir_len = (DWORD)(last_slash - exe_path + 1);
        memcpy(dll_path, exe_path, dir_len);
        // Append DLL name
        strcpy(dll_path + dir_len, STUB_DLL_NAME);
    } else {
        // No directory, just use DLL name
        strcpy(dll_path, STUB_DLL_NAME);
    }
    // Ensure null termination
    dll_path[sizeof(dll_path) - 1] = '\0';
    
    // Step 3: Load the stub DLL
    HMODULE dll_handle = LoadLibraryA(dll_path);
    if (dll_handle == NULL) {
        ExitProcess(EXIT_FAILURE_DLL);
    }
    
    // Step 4: Get stub_entry function address
    FARPROC stub_entry_ptr = GetProcAddress(dll_handle, STUB_ENTRY_NAME);
    if (stub_entry_ptr == NULL) {
        ExitProcess(EXIT_FAILURE_EXPORT);
    }
    
    // Step 5: Cast to function pointer and call it
    typedef DWORD (__stdcall *StubEntryFunc)(void);
    StubEntryFunc stub_entry = (StubEntryFunc)stub_entry_ptr;
    
    // Call stub_entry (never returns here)
    DWORD result = stub_entry();
    
    // If we somehow return, exit with the result
    ExitProcess(result);
}