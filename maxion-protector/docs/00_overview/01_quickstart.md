# Quick Start Guide

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-24 |
| Version | 3.0.0 |
| Complexity | Beginner |
| Time to Read | 5 minutes |
| Audience | New Users |

---

## Introduction

Get started with Maxion Protector in 5 minutes. This guide will walk you through installing the tool and protecting your first Windows executable with encrypted game assets.

### What You'll Learn

1. How to install Maxion Protector
2. How to encrypt and protect game assets
3. How to integrate protected assets into your game
4. How to test the protected executable

### Prerequisites

- **For Building**: Rust 1.70 or later
- **For Using**: Windows 7+ (64-bit) or cross-compilation capability
- **For Testing**: A sample Windows executable to protect

---

## Windows Setup (Required for Windows Users)

On Windows, you need to install MSYS2/MinGW-w64 to build the project. This provides the `dlltool.exe` required for some Rust dependencies.

### Step 1: Install MSYS2

Download MSYS2 from [https://www.msys2.org/](https://www.msys2.org/) and run the installer.

### Step 2: Install MinGW-w64 Toolchain

Open the **MSYS2 UCRT64 terminal** (not the regular MSYS2 terminal) and run:

```bash
pacman -S --needed base-devel mingw-w64-ucrt-x86_64-toolchain
```

### Step 3: Update Windows PATH

Add the MSYS2 bin directory to your Windows System Environment Variables:

1. Press `Win + R`, type `sysdm.cpl`, press Enter
2. Go to **Advanced** tab → **Environment Variables...**
3. Under **System variables**, find **Path**, click **Edit...**
4. Click **New**, add: `C:\msys64\ucrt64\bin`
5. Click **OK** on all dialogs

### Step 4: Restart Terminals

**Close all terminals** (PowerShell, cmd, MSYS2 bash) and open a new one. The PATH changes won't take effect until you restart.

### Step 5: Verify Installation

Open a new **PowerShell** or **cmd** (not MSYS2 bash):

```powershell
# Verify dlltool is available
dlltool --version

# You should see:
# GNU C:\msys64\ucrt64\bin\dlltool.exe (GNU Binutils) 2.45.1
```

**Note:** Always run `cargo` commands from PowerShell or cmd, not from MSYS2 bash. MSYS2's PATH translation can confuse Rust's build system.

---

## Installation

### Option 1: Install from Source (Recommended)

```bash
# Clone the repository
git clone https://github.com/maxion-game/maxion-protector.git
cd maxion-protector

# Build the packer tool
cargo build --release

# The binary will be at: target/release/pnp.exe
```

### Option 2: Install via Cargo

```bash
# Install directly from crates.io (when published)
cargo install maxion-packer

# Or install from git repository
cargo install --git https://github.com/maxion-game/maxion-protector maxion-packer
```

### Verify Installation

```bash
# Check that the tool is installed
pnp --version

# You should see: maxion-packer 0.1.0
```

---

## Protect Your First Executable

### Step 1: Prepare Your Assets

Create a directory with your game assets:

```bash
# Create assets directory
mkdir my_game_assets

# Copy your game assets (example)
cp textures/*.png my_game_assets/
cp models/*.obj my_game_assets/
cp audio/*.wav my_game_assets/
cp scripts/*.lua my_game_assets/
```

### Step 2: Protect the Executable

```bash
# Protect your game executable
pnp protect \
    --input my_game.exe \
    --assets my_game_assets/ \
    --output my_game_protected.exe \
    --compression-level 6
```

**Command Options:**
- `--input`: Original unprotected executable
- `--assets`: Directory containing game assets to protect
- `--output`: Protected executable output path
- `--compression-level`: Brotli compression level (0-11, default 6)

**What Happens:**
1. Reads and validates the original executable
2. Compresses all assets with Brotli
3. Encrypts compressed assets with ChaCha20-Poly1305
4. Embeds encrypted archive into executable
5. Injects runtime loader (Phase 2 DLL embedding)
6. Generates protected, self-contained executable

### Step 3: Verify Protection

```bash
# Check file size increase (typical: +10-30% depending on assets)
ls -lh my_game.exe my_game_protected.exe

# You should see my_game_protected.exe is larger
# The increase includes the encrypted assets and runtime loader
```

---

## Testing the Protected Executable

### Option 1: Using the Runtime Library (C++ Example)

```cpp
// my_game.cpp
#include <windows.h>
#include <maxion.h>  // Runtime library header

int main() {
    // Initialize Maxion runtime
    if (!maxion_init("my_game_protected.exe")) {
        fprintf(stderr, "Failed to initialize Maxion runtime\n");
        return 1;
    }
    
    // Load a protected asset
    char texture_data[1024 * 1024];  // 1MB buffer
    size_t bytes_read = maxion_read_file(
        "textures/player.png", 
        texture_data, 
        sizeof(texture_data)
    );
    
    if (bytes_read == 0) {
        fprintf(stderr, "Failed to load asset\n");
        return 1;
    }
    
    // Use the asset as normal (it's now decrypted in memory)
    load_texture(texture_data, bytes_read);
    
    // Cleanup
    maxion_shutdown();
    
    return 0;
}
```

### Option 2: Using the Example Application

```bash
# Build the example hello-world application
cd examples/hello-world
cargo build --release

# The executable demonstrates asset loading
# It loads sirref.png from protected assets
.\target\release\hello-world.exe
```

### Option 3: Manual Testing

```bash
# Run the protected executable
.\my_game_protected.exe

# If your game loads assets normally, protection is working!
# Assets are automatically decrypted on-the-fly when accessed
```

---

## Configuration Options

### Basic Protection

```bash
# Minimum protection (no compression)
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output game_protected.exe \
    --compression-level 0
```

### Maximum Compression

```bash
# Best compression (slower packing, smallest output)
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output game_protected.exe \
    --compression-level 11
```

### Custom Chunk Size

```bash
# Larger chunks for streaming large assets (better performance)
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output game_protected.exe \
    --chunk-size 262144  # 256KB chunks
```

### Using Configuration File

Create `maxion_config.toml`:

```toml
[archive]
output = "game_protected.exe"
compression_type = "brotli"
compression_level = 6
chunk_size = 65536  # 64KB

[encryption]
generate_key = true
# Or use: key_file = "my_key.bin"

[filters]
include = ["textures/**", "models/**", "audio/**"]
exclude = ["*.tmp", "*.log"]

[advanced]
cache_size = 268435456  # 256MB cache
max_sequential_reads = 100
anti_scrape_delay_ms = 50
```

Then run:

```bash
pnp protect --config maxion_config.toml
```

---

## Common Use Cases

### Use Case 1: Protecting Textures Only

```bash
# Only protect texture assets
pnp protect \
    --input game.exe \
    --assets textures/ \
    --output game_protected.exe
```

### Use Case 2: Protecting All Game Assets

```bash
# Protect all assets recursively
pnp protect \
    --input game.exe \
    --assets game_data/ \
    --output game_protected.exe \
    --recursive
```

### Use Case 3: Batch Processing Multiple Executables

```powershell
# PowerShell example - protect all executables in a directory
Get-ChildItem -Path .\unprotected\*.exe | ForEach-Object {
    $baseName = [System.IO.Path]::GetFileNameWithoutExtension($_.Name)
    & pnp protect `
        --input $_.FullName `
        --assets .\assets\$baseName\ `
        --output .\protected\$baseName.exe
}
```

---

## Troubleshooting

### Issue: "Failed to open input file"

**Solution**: Ensure the executable path is correct and the file exists.

```bash
# Check file exists
ls my_game.exe

# Use absolute path if needed
pnp protect \
    --input "C:\path\to\my_game.exe" \
    --assets assets/ \
    --output my_game_protected.exe
```

### Issue: "Failed to read assets directory"

**Solution**: Verify the assets directory exists and is accessible.

```bash
# Check directory exists
ls -la my_game_assets/

# Try with absolute path
pnp protect \
    --input game.exe \
    --assets "C:\path\to\my_game_assets\" \
    --output game_protected.exe
```

### Issue: "Out of memory during protection"

**Solution**: Use smaller chunk size or exclude large files.

```bash
# Reduce chunk size to use less memory
pnp protect \
    --input game.exe \
    --assets assets/ \
    --output game_protected.exe \
    --chunk-size 16384  # 16KB instead of 64KB
```

### Issue: "Protected executable doesn't run"

**Solution**: 
1. Verify the executable is valid with `validate_pe` tool
2. Check Windows Event Viewer for errors
3. Try running as Administrator
4. Review [Troubleshooting Guide](../07_troubleshooting/README.md)

### Issue: "dlltool.exe not found" (Windows Only)

**Solution**: Install MSYS2/MinGW-w64 and add it to Windows PATH.

**Step 1: Install MSYS2**
```bash
# Download from: https://www.msys2.org/
# Run the installer and follow the prompts
```

**Step 2: Install MinGW-w64 Toolchain**
```bash
# Open MSYS2 UCRT64 terminal (not regular MSYS2 terminal)
pacman -S --needed base-devel mingw-w64-ucrt-x86_64-toolchain
```

**Step 3: Update Windows PATH**
1. Press `Win + R`, type `sysdm.cpl`, press Enter
2. Go to **Advanced** → **Environment Variables...**
3. Under **System variables**, find **Path**, click **Edit...**
4. Click **New**, add: `C:\msys64\ucrt64\bin`
5. Click **OK** on all dialogs

**Step 4: Restart Terminals**
⚠️ **Close all terminals** (PowerShell, cmd, MSYS2 bash) and open new ones. PATH changes won't take effect until you restart.

**Step 5: Verify Installation**
```powershell
# In new PowerShell or cmd (NOT MSYS2 bash)
dlltool --version

# Should output:
# GNU C:\msys64\ucrt64\bin\dlltool.exe (GNU Binutils) 2.45.1
```

**Important:** Always run `cargo` from PowerShell or cmd, not from MSYS2 bash. MSYS2's PATH translation can confuse Rust's build system.

---

## Next Steps

### Learn More

- **[User Guide](02_user_guide.md)** - Comprehensive documentation for all features
- **[Architecture Overview](../01_architecture/README.md)** - Understand how the system works
- **[Security Documentation](../06_security/README.md)** - Learn about security guarantees

### Advanced Topics

- **[Custom Encryption Keys](../01_architecture/03_encryption.md#key-management)** - Use your own encryption keys
- **[Performance Tuning](../05_benchmark/README.md)** - Optimize for your use case
- **[Deployment Guide](../03_deployment/README.md)** - CI/CD integration and automation

### Integration

- **[Unity Integration](02_user_guide.md#unity-integration)** - Use with Unity games
- **[C++ Integration](02_user_guide.md#cpp-integration)** - Use with C++ games
- **[Custom Integration](02_user_guide.md#custom-integration)** - Use with any engine

---

## Verification Checklist

After following this guide, verify:

- [ ] Maxion Protector is installed and running (`pnp --version`)
- [ ] Your executable is protected (`ls -lh my_game_protected.exe`)
- [ ] File size has increased (contains encrypted assets)
- [ ] Protected executable runs without errors
- [ ] Assets load correctly in your game
- [ ] Performance impact is acceptable (<12.5% overhead)

If all checks pass, you've successfully protected your first executable!

---

## Summary

You've learned:

✅ How to install Maxion Protector  
✅ How to encrypt and protect game assets  
✅ How to integrate protected assets into your game  
✅ How to configure protection options  
✅ How to troubleshoot common issues  

**You're now ready to use Maxion Protector in your game development workflow!**

---

## Need Help?

- **[Troubleshooting Guide](../07_troubleshooting/README.md)** - Common issues and solutions
- **[User Guide](02_user_guide.md)** - Detailed documentation
- **[GitHub Issues](../../ISSUES.md)** - Report bugs and request features
- **[Examples](../../examples/)** - Code examples and demos

---

**See Also:**
- [User Guide](02_user_guide.md) - Comprehensive user documentation
- [Architecture](../01_architecture/README.md) - System design and components
- [Troubleshooting](../07_troubleshooting/README.md) - Common issues and solutions
- [ISSUES.md](../../ISSUES.md) - Current issues and development status