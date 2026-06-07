# Troubleshooting Guide

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-24 |
| Version | 3.0.0 |
| Complexity | Beginner to Intermediate |
| Time to Read | 10 minutes |
| Audience | Users, Developers, DevOps |

---

## Overview

This troubleshooting guide helps you diagnose and resolve common issues with Maxion Protector. It covers installation problems, protection failures, runtime issues, and performance concerns.

### Quick Diagnosis Flow

```
Issue Type
    │
    ├─> Installation/Build
    │   ├─> Rust not installed?
    │   ├─> Dependency conflicts?
    │   └─> Compilation errors?
    │
    ├─> Protection/Packing
    │   ├─> Cannot find input file?
    │   ├─> Assets directory issue?
    │   ├─> Out of memory?
    │   └─> PE injection failure?
    │
    ├─> Runtime Execution
    │   ├─> Protected exe won't run?
    │   ├─> Asset loading failed?
    │   ├─> Decryption error?
    │   └─> DLL load failure?
    │
    └─> Performance
        ├─> Slow startup?
        ├─> Slow asset loading?
        ├─> High memory usage?
        └─> Frame drops?
```

---

## Quick Diagnostics

### Run Diagnostics

```bash
# Check Maxion Protector version
pnp --version

# Validate PE structure
./target/debug/validate_pe.exe protected_exe.exe

# Check test coverage
cargo test -- --test-threads=1
```

### Enable Debug Logging

```bash
# Set environment variable
export RUST_LOG=debug

# Run packer with debug output
RUST_LOG=debug pnp protect \
    --input my_game.exe \
    --assets assets/ \
    --output protected.exe
```

### Check System Requirements

```bash
# Check Rust version
rustc --version
# Should be 1.70 or later

# Check available memory
# Windows: System Information
# Linux: free -h
# macOS: system_profiler SPHardwareDataType
```

---

## Archive Creation Issues

### Issue: "Failed to open input directory"

**Symptom**: `pnp` cannot find the assets directory

**Diagnosis**:
```bash
# Check if directory exists
ls -la assets/

# Check permissions
ls -ld assets/
```

**Solutions**:

1. **Use Absolute Path**
```bash
pnp protect \
    --input "C:\path\to\game.exe" \
    --assets "C:\path\to\assets\" \
    --output protected.exe
```

2. **Verify Directory Structure**
```bash
# Should see assets and subdirectories
ls -la assets/
# Output: textures/ models/ audio/ scripts/
```

3. **Check File Permissions**
```bash
# Ensure read access on all files
chmod -R +r assets/

# On Windows, ensure no access restrictions
icacls assets\ /grant Users:F
```

---

### Issue: "Out of memory during archive creation"

**Symptom**: Process crashes during compression/encryption

**Diagnosis**:
```bash
# Check system memory
free -h  # Linux
system_profiler SPHardwareDataType | grep Memory  # macOS

# Check largest files in assets
du -ah assets/ | sort -rh | head -10
```

**Solutions**:

1. **Reduce Chunk Size**
```bash
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --chunk-size 16384  # 16KB instead of 64KB
```

2. **Exclude Large Files**
```bash
# Create configuration file
cat > maxion_config.toml << EOF
[filters]
exclude = ["*.bin", "*.cache", "*.tmp"]
EOF

# Use config file
pnp protect --config maxion_config.toml
```

3. **Process Large Files Separately**
```bash
# Process in batches
pnp protect \
    --input game.exe \
    --assets assets/textures/ \
    --output game_textures.exe \
    --chunk-size 32768

pnp protect \
    --input game_textures.exe \
    --assets assets/models/ \
    --output game_full.exe
```

---

### Issue: "Compression failed: Input too large for compression"

**Symptom**: Brotli compression fails on large files

**Diagnosis**:
```bash
# Check file sizes
find assets/ -type f -exec ls -lh {} \; | awk '{print $5, $9}'

# Identify files >100MB
find assets/ -type f -size +100M
```

**Solutions**:

1. **Use Lower Compression Level**
```bash
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --compression-level 0  # No compression
```

2. **Exclude Very Large Files**
```bash
cat > maxion_config.toml << EOF
[filters]
exclude = ["*.bin", "large_assets/*"]
EOF
```

3. **Compress Manually**
```bash
# Compress large file with external tool
brotli -6 assets/large_file.bin -o assets/large_file.bin.br

# Then protect
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --exclude "*.bin.br"
```

---

## Runtime Integration Issues

### Issue: "Failed to open archive at runtime"

**Symptom**: Application cannot load protected archive

**Diagnosis**:
```c
// Add debug logging to your game
#include <stdio.h>

void maxion_debug_log(const char* message) {
    FILE* log = fopen("maxion_debug.log", "a");
    if (log) {
        fprintf(log, "[DEBUG] %s\n", message);
        fclose(log);
    }
}

// Before loading assets
maxion_debug_log("About to initialize Maxion");
bool success = maxion_init("my_protected_game.exe");
maxion_debug_log(success ? "Initialized successfully" : "Initialization failed");
```

**Solutions**:

1. **Check Executable Path**
```c
// Use correct executable path
const char* exe_path = "my_protected_game.exe";  // Relative path
// or
const char* exe_path = "C:\\path\\to\\my_protected_game.exe";  // Absolute path

bool success = maxion_init(exe_path);
if (!success) {
    fprintf(stderr, "Failed to initialize Maxion with path: %s\n", exe_path);
    return 1;
}
```

2. **Check Working Directory**
```c
// Ensure working directory is correct
char cwd[1024];
if (getcwd(cwd, sizeof(cwd)) != NULL) {
    printf("Current working directory: %s\n", cwd);
}

// Set working directory if needed
chdir("C:\\path\\to\\game\\directory");
```

3. **Verify Archive Integrity**
```bash
# Validate PE structure
./target/debug/validate_pe.exe my_protected_game.exe

# Check file size
ls -lh my_protected_game.exe
```

---

### Issue: "Decryption failed: Invalid key"

**Symptom**: Assets cannot be decrypted

**Diagnosis**:
```bash
# Check if key file exists
ls -la encryption_key.bin

# Verify key generation
pnp generate-key --output test_key.bin
```

**Solutions**:

1. **Use Same Key for Protection and Runtime**
```bash
# Generate key
pnp generate-key --output my_key.bin

# Protect with this key
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --key-file my_key.bin

# Runtime will use embedded key automatically
# No need to specify key file at runtime
```

2. **Verify Key is Embedded**
```bash
# Check protected executable has .key section
./target/debug/validate_pe.exe protected.exe
# Should show .key section present
```

3. **Re-protect with New Key**
```bash
# If key was corrupted, regenerate
rm my_key.bin
pnp generate-key --output my_key.bin

# Re-protect
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --key-file my_key.bin
```

---

### Issue: "Asset not found in archive"

**Symptom**: `maxion_file_exists()` returns false for valid file

**Diagnosis**:
```c
// Check virtual path
const char* path = "textures/player.png";

if (maxion_file_exists(path)) {
    printf("File exists: %s\n", path);
} else {
    printf("File NOT found: %s\n", path);
    printf("Current working directory: %s\n", cwd);
}
```

**Solutions**:

1. **Check Virtual Path Format**
```c
// Correct: Use forward slashes
const char* correct_path = "textures/player.png";

// Incorrect: Use backslashes (Windows)
const char* incorrect_path = "textures\\player.png";

// Incorrect: Use absolute path
const char* incorrect_path2 = "C:\\game\\assets\\textures\\player.png";

bool success = maxion_file_exists(correct_path);
```

2. **Check Case Sensitivity**
```c
// Paths are case-sensitive
const char* path = "Textures/Player.png";  // Wrong case
const char* path2 = "textures/player.png";  // Correct case
```

3. **List Available Files**
```c
// Debug: Try to load common paths
const char* test_paths[] = {
    "textures/player.png",
    "Textures/Player.png",
    "assets/textures/player.png",
    NULL
};

for (int i = 0; test_paths[i] != NULL; i++) {
    if (maxion_file_exists(test_paths[i])) {
        printf("Found at: %s\n", test_paths[i]);
        break;
    }
}
```

---

## Performance Issues

### Issue: "Slow asset loading"

**Symptom**: Asset loading takes longer than expected

**Diagnosis**:
```bash
# Enable profiling
export RUST_LOG=info

# Run with profiler
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe

# Check metrics
cat metrics.json | jq .
```

**Solutions**:

1. **Increase Cache Size**
```c
MaxionConfig config;
config.archive_path = "my_protected_game.exe";
config.cache_size = 536870912;  // 512MB (larger cache)

bool success = maxion_init_with_config(&config);
```

2. **Preload Frequently Used Assets**
```c
// Preload assets at startup
const char* preload_files[] = {
    "textures/player.png",
    "textures/ui/button.png",
    "models/player.obj",
    NULL
};

maxion_preload(preload_files, 3);
```

3. **Use Smaller Chunk Size for Frequent Access**
```bash
# For many small assets
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --chunk-size 16384  # 16KB chunks (faster for small files)
```

---

### Issue: "High memory usage"

**Symptom**: Application uses more memory than expected

**Diagnosis**:
```c
// Check cache statistics
CacheStats stats;
maxion_get_cache_stats(&stats);

printf("Cache size: %zu bytes\n", stats.size);
printf("Cache entries: %zu\n", stats.entries);
printf("Cache hits: %zu\n", stats.hits);
printf("Cache misses: %zu\n", stats.misses);
double hit_rate = (double)stats.hits / (stats.hits + stats.misses) * 100.0;
printf("Hit rate: %.2f%%\n", hit_rate);
```

**Solutions**:

1. **Reduce Cache Size**
```c
MaxionConfig config;
config.archive_path = "my_protected_game.exe";
config.cache_size = 134217728;  // 128MB (smaller cache)

bool success = maxion_init_with_config(&config);
```

2. **Clear Cache Periodically**
```c
// Clear cache after level load
maxion_clear_cache();

// Or clear specific assets
maxion_preload(new_level_assets, 10);
```

3. **Reduce Chunk Size**
```bash
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --chunk-size 32768  # 32KB chunks (less memory per chunk)
```

---

### Issue: "Frame drops during asset streaming"

**Symptom**: Game stutters when streaming large assets

**Diagnosis**:
```bash
# Profile asset loading
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --chunk-size 65536

# Check streaming performance
RUST_LOG=info ./target/release/hello-world
```

**Solutions**:

1. **Load Assets Asynchronously**
```c
// Load large assets in background thread
void* load_asset_async(const char* path) {
    // Start thread to load asset
    // Load in background while game continues
    // Signal when ready
}

// In game loop
if (asset_ready("large_asset.png")) {
    use_asset("large_asset.png");
}
```

2. **Increase Chunk Size for Large Files**
```bash
# For streaming large assets
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --chunk-size 262144  # 256KB chunks (better for streaming)
```

3. **Preload Critical Assets**
```c
// Preload assets before they're needed
void pre_load_level_assets(int level) {
    const char* assets[] = {
        get_level_assets(level),
        NULL
    };
    maxion_preload(assets, count);
}
```

---

## Security Issues

### Issue: "Security warning: Weak encryption detected"

**Symptom**: System reports weak encryption settings

**Diagnosis**:
```bash
# Check encryption settings
pnp info protected.exe

# Check key length
hexdump -C protected.exe | grep -A 20 "MAXION"
```

**Solutions**:

1. **Use Strong Encryption Key**
```bash
# Generate new key with secure random
pnp generate-key --output secure_key.bin

# Use this key
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --key-file secure_key.bin
```

2. **Verify Encryption Algorithm**
```bash
# Should be ChaCha20-Poly1305
pnp info protected.exe | grep Encryption
# Output: Encryption: ChaCha20-Poly1305 (256-bit)
```

3. **Check for Deprecated Features**
```bash
# Avoid Phase 1 (stub loader) if possible
# Use Phase 2 (DLL embedding) instead

pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --phase2  # Explicitly use Phase 2
```

---

### Issue: "Potential asset extraction detected"

**Symptom**: System detects suspicious access patterns

**Diagnosis**:
```bash
# Check access logs
cat maxion_access.log | tail -50

# Check for rapid sequential reads
grep "sequential" maxion_access.log
```

**Solutions**:

1. **Enable Access Control**
```bash
# Create config with access control
cat > maxion_config.toml << EOF
[advanced]
max_sequential_reads = 100
anti_scrape_delay_ms = 50
EOF

pnp protect --config maxion_config.toml
```

2. **Monitor Access Patterns**
```c
// Enable access logging
MaxionConfig config;
config.archive_path = "my_protected_game.exe";
config.enable_logging = true;
config.log_file = "maxion_access.log";

bool success = maxion_init_with_config(&config);
```

3. **Implement Rate Limiting**
```c
// Check access patterns
if (detect_suspicious_access()) {
    // Add delay
    Sleep(100);  // 100ms delay
    // Or block access
    fprintf(stderr, "Access denied: Too many requests\n");
    return false;
}
```

---

## Platform-Specific Issues

### Windows Issues

#### Issue: "DLL load failed"

**Symptom**: Protected executable fails to load DLLs

**Diagnosis**:
```powershell
# Check DLL dependencies
dumpbin /dependents my_protected_game.exe

# Check DLL paths
where kernel32.dll
```

**Solutions**:

1. **Use Phase 2 (No External Dependencies)**
```bash
# Phase 2 embeds all DLLs
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --phase2
```

2. **Check Windows Version**
```powershell
# Check Windows version
[Environment]::OSVersion.Version

# Maxion requires Windows 7 or later (64-bit)
# Windows 10/11 recommended
```

3. **Run as Administrator**
```powershell
# Some operations require admin privileges
Run-AsAdministrator powershell
.\my_protected_game.exe
```

#### Issue: "Access denied when creating archive"

**Symptom**: Cannot write to output directory

**Diagnosis**:
```powershell
# Check permissions
icacls .\
icacls output_directory\
```

**Solutions**:

1. **Run as Administrator**
```powershell
Run-AsAdministrator powershell

pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe
```

2. **Use Different Output Directory**
```powershell
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output $env:USERPROFILE\protected.exe
```

3. **Check Directory Permissions**
```powershell
# Grant write access
icacls .\ /grant Users:(OI)(CI)F

# Or use specific directory
mkdir protected_output
icacls protected_output\ /grant Users:F
```

---

### Linux Issues

#### Issue: "Permission denied when executing"

**Symptom**: Cannot execute protected executable

**Diagnosis**:
```bash
# Check file permissions
ls -l protected.exe

# Check executable bit
file protected.exe
```

**Solutions**:

1. **Make Executable**
```bash
chmod +x protected.exe

# Try running
./protected.exe
```

2. **Check Wine Compatibility**
```bash
# Windows executables need Wine on Linux
wine --version

# Run with Wine
wine protected.exe
```

3. **Cross-Compile for Linux**
```bash
# Compile for Linux target
rustup target add x86_64-unknown-linux-gnu

# Build for Linux
cargo build --release --target x86_64-unknown-linux-gnu
```

---

### macOS Issues

#### Issue: "macOS cannot verify developer"

**Symptom**: Gatekeeper blocks protected executable

**Diagnosis**:
```bash
# Check quarantine attribute
xattr -l protected.exe

# Check code signature
codesign -dv protected.exe
```

**Solutions**:

1. **Bypass Gatekeeper (Development Only)**
```bash
# Remove quarantine attribute
xattr -d com.apple.quarantine protected.exe

# Allow execution
sudo spctl --master-disable
./protected.exe

# Re-enable Gatekeeper after testing
sudo spctl --master-enable
```

2. **Code Sign the Executable**
```bash
# Sign with developer certificate
codesign --force --deep --sign "Developer ID" protected.exe

# Verify signature
codesign -dv protected.exe
```

3. **Use Windows VM**
```bash
# Run protected executable in Windows VM
# macOS support planned for future releases
```

---

## Debugging Tools

### Built-in Debug Commands

```bash
# Validate PE structure
./target/debug/validate_pe.exe protected.exe

# Show archive info
pnp info protected.exe

# Generate encryption key
pnp generate-key --output my_key.bin

# Check version
pnp --version
```

### Runtime Debugging

```c
// Enable profiling
void maxion_enable_profiling(bool enable);

// Get performance report
const char* report = maxion_get_performance_report();
printf("%s\n", report);

// Get cache statistics
CacheStats stats;
maxion_get_cache_stats(&stats);

// Log access patterns
MaxionConfig config;
config.enable_logging = true;
config.log_file = "maxion_debug.log";
```

### Log Analysis

```bash
# View recent errors
grep "ERROR" maxion_debug.log | tail -20

# View asset access patterns
grep "File accessed" maxion_access.log | wc -l

# Check for suspicious activity
grep "Sequential read" maxion_access.log | tail -50
```

---

## Getting Help

### Before Requesting Help

1. **Search Documentation**
   - [Troubleshooting Guide](../07_troubleshooting/README.md)
   - [Common Issues](01_common_issues.md)
   - [Performance Guide](03_performance.md)

2. **Check Known Issues**
   - [ISSUES.md](../../ISSUES.md)
   - [GitHub Issues](https://github.com/maxion-game/maxion-protector/issues)

3. **Gather Information**
   - Maxion Protector version
   - Operating system and version
   - Rust version (if building from source)
   - Error messages
   - Steps to reproduce

### Information to Include

When requesting help, include:

```markdown
**Environment:**
- Maxion Protector: 0.1.0
- OS: Windows 11 64-bit
- Rust: 1.75.0

**Issue Description:**
Protected executable won't run. Shows error: "Cannot initialize Maxion runtime"

**Steps to Reproduce:**
1. Created protected executable: `pnp protect --input game.exe --assets assets/ --output protected.exe`
2. Ran: `protected.exe`
3. Error appeared

**Error Messages:**
```
Error: Cannot initialize Maxion runtime
Code: 0x1234
```

**Debug Output:**
[Include RUST_LOG=debug output]

**Files:**
- game.exe (original)
- protected.exe (protected)
- assets/ directory structure
```

### Environment Variables

```bash
# Enable debug logging
export RUST_LOG=debug
export RUST_LOG=maxion=trace

# Set library path
export LD_LIBRARY_PATH=/path/to/maxion/libraries  # Linux
export DYLD_LIBRARY_PATH=/path/to/maxion/libraries  # macOS

# Set cache directory
export MAXION_CACHE_DIR=/tmp/maxion_cache
```

### Configuration File Template

```toml
# maxion_config.toml
[archive]
output = "protected.exe"
compression_type = "brotli"
compression_level = 6
chunk_size = 65536

[encryption]
generate_key = true
# Or use: key_file = "my_key.bin"

[filters]
include = ["textures/**", "models/**", "audio/**"]
exclude = ["*.tmp", "*.log", "*.cache"]

[advanced]
cache_size = 268435456  # 256MB
max_sequential_reads = 100
anti_scrape_delay_ms = 50
enable_logging = true
log_file = "maxion_debug.log"
```

---

## Common Workflows

### Debug Protection Failure

```bash
# 1. Enable debug logging
export RUST_LOG=debug

# 2. Run protection with verbose output
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output protected.exe \
    --verbose

# 3. Validate output
./target/debug/validate_pe.exe protected.exe

# 4. Check file size
ls -lh game.exe protected.exe
```

### Debug Runtime Failure

```c
// 1. Enable debug logging
MaxionConfig config;
config.archive_path = "my_protected_game.exe";
config.enable_logging = true;
config.log_file = "maxion_runtime_debug.log";

// 2. Initialize
bool success = maxion_init_with_config(&config);
if (!success) {
    fprintf(stderr, "Failed to initialize\n");
    return 1;
}

// 3. Try to load asset
size_t size = maxion_file_size("textures/player.png");
if (size == 0) {
    fprintf(stderr, "Failed to get file size\n");
    return 1;
}

// 4. Load asset
char* buffer = malloc(size);
size_t bytes_read = maxion_read_file("textures/player.png", buffer, size);
if (bytes_read != size) {
    fprintf(stderr, "Failed to read file\n");
    return 1;
}

// 5. Check log file
// Check maxion_runtime_debug.log for errors
```

### Debug Performance Issue

```c
// 1. Enable profiling
maxion_enable_profiling(true);

// 2. Load assets
load_all_assets();

// 3. Get performance report
const char* report = maxion_get_performance_report();
printf("Performance Report:\n%s\n", report);

// 4. Check cache statistics
CacheStats stats;
maxion_get_cache_stats(&stats);
printf("Cache size: %zu bytes\n", stats.size);
printf("Cache entries: %zu\n", stats.entries);
printf("Cache hits: %zu\n", stats.hits);
printf("Cache misses: %zu\n", stats.misses);
double hit_rate = (double)stats.hits / (stats.hits + stats.misses) * 100.0;
printf("Hit rate: %.2f%%\n", hit_rate);
```

---

## Related Documentation

- [Common Issues](01_common_issues.md) - Frequently encountered problems
- [Debug Guide](02_debug_guide.md) - Debugging techniques and tools
- [Performance Issues](03_performance.md) - Performance troubleshooting
- [User Guide](../00_overview/02_user_guide.md) - Comprehensive user documentation
- [Security Documentation](../06_security/README.md) - Security issues and considerations

---

## See Also

- [ISSUES.md](../../ISSUES.md) - Current issues and status
- [Architecture Overview](../01_architecture/README.md) - System architecture
- [Implementation Status](../00_overview/03_implementation_status.md) - Current development status
- [GitHub Issues](https://github.com/maxion-game/maxion-protector/issues) - Report bugs

---

**Document Version**: 3.0.0  
**Last Updated**: 2025-01-24  
**Maintained By**: Maxion Protector Support Team