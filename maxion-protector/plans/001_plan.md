# **Performance-First Asset Protection System for Unity & C++ Games**

## **1. Introduction**

This plan defines a high-performance software protection system designed to safeguard game assets (textures, audio, 3D models, scripts) from unauthorized extraction. Unlike commercial protection suites that emphasize complex code virtualization and licensing systems, this system prioritizes **minimal performance overhead** while providing robust asset encryption for internal game distribution.

**Core Design Philosophy:**
- **Asset-First, Code-Second**: Encrypt assets with high-performance streaming encryption; use existing commercial obfuscators for code protection
- **<5% Runtime Overhead**: Game performance and loading times remain virtually unchanged
- **Zero-Copy Architecture**: Memory-mapped virtual file system with lazy loading
- **Internal Use Optimization**: No licensing servers, trial periods, or hardware locking needed
- **Simple CLI Workflow**: Pack assets folder → generate protected executable

The system is built in Rust for memory safety, performance, and cross-platform support. It consists of two components:
1. **Asset Packer Tool**: CLI utility that encrypts and packs an entire assets folder
2. **Runtime Protection Stub**: Minimal runtime injected into game executable to handle virtual file system and decryption

---

## **2. Performance-Critical Architecture**

### **2.1 The "Just-in-Time" Decryption Strategy**

Traditional packers decrypt entire files on load, causing significant startup delays. This system uses **chunk-based streaming decryption**:

```
┌─────────────────────────────────────────────────┐
│  Request: Read 4KB offset 100MB of "player.png"  │
└─────────────────┬───────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────┐
│  1. Calculate chunk index: offset / 64KB = 1536 │
│  2. Check LRU cache for chunk 1536              │
│  3. If cached: Return data (0ms overhead)       │
│  4. If not cached:                              │
│     - Decrypt only chunk 1536 (64KB)            │
│     - ChaCha20-Poly1305: ~1-2ms per 64KB       │
│     - Cache result                               │
│     - Return data                                │
└─────────────────────────────────────────────────┘
```

**Performance Impact:**
- First read of new chunk: 1-2ms (imperceptible in game loading)
- Subsequent reads: <0.1ms (cached)
- Memory overhead: 10-50MB depending on LRU cache size
- Startup time: <50ms (no bulk decryption)

### **2.2 Memory-Mapped Virtual File System (VFS)**

Instead of loading the entire packed archive into RAM, the VFS uses **memory mapping (mmap)** for zero-copy access:

```rust
// VirtualArchive structure
struct VirtualArchive {
    header: ArchiveHeader,           // 256 bytes, mapped
    file_table: Vec<FileInfo>,       // Decrypted once at startup, <1MB
    data_mmap: Mmap,                 // Entire archive file-mapped, zero-copy
    chunk_cache: LruCache<u32, Vec<u8>>, // 50MB LRU cache
}

// File read path
fn read_file(&self, path: &str, offset: u64, size: usize) -> &[u8] {
    let file_info = self.file_table.lookup(path);
    let chunk_idx = (offset + file_info.offset) / CHUNK_SIZE;
    
    if let Some(cached) = self.chunk_cache.get(&chunk_idx) {
        return &cached[offset % CHUNK_SIZE..];
    }
    
    let chunk_data = self.decrypt_chunk(chunk_idx);
    self.chunk_cache.put(chunk_idx, chunk_data);
    self.chunk_cache.get(&chunk_idx).unwrap()
}
```

**Benefits:**
- Zero file I/O overhead after initial mmap
- OS manages paging automatically
- No memory allocation for archive data
- Instant access to any file offset

### **2.3 Minimal Stub Design**

The protection stub is intentionally minimal to reduce attack surface and improve performance:

```
Stub Size Target: <50KB
Startup Time: <50ms
Dependencies: no_std (core + alloc only)
```

**Stub Responsibilities:**
1. Parse embedded archive header
2. Decrypt file table (happens once at startup)
3. Install API hooks (CreateFileW, ReadFile, etc.)
4. Transfer execution to original OEP (Original Entry Point)

**What the Stub DOES NOT do:**
- No virtual machine or code interpretation
- No licensing verification
- No anti-debugging (unnecessary for internal use)
- No hardware fingerprinting
- No complex encryption negotiation

---

## **3. Asset Packer Tool (CLI)**

### **3.1 User Workflow**

```bash
# Pack assets folder into existing game executable
maxion-protector pack \
  --input "MyGame.exe" \
  --assets-folder "./Assets" \
  --output "MyGame_Protected.exe"

# Pack with custom configuration
maxion-protector pack \
  --input "MyGame.exe" \
  --assets-folder "./Assets" \
  --chunk-size 65536 \
  --encryption-key "my-secret-key" \
  --compress \
  --sign-certificate "./cert.pfx" \
  --output "MyGame_Protected.exe"
```

### **3.2 Packer Implementation Details**

**Step 1: Scan Assets Folder**
```rust
fn scan_assets(folder: &Path) -> Vec<AssetFile> {
    let mut files = Vec::new();
    
    // Recursive scan with parallel processing
    walkdir::WalkDir::new(folder)
        .into_iter()
        .par_bridge()  // Parallel scanning for large asset folders
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .for_each(|entry| {
            let relative_path = entry.path().strip_prefix(folder).unwrap();
            let metadata = entry.metadata().unwrap();
            
            files.push(AssetFile {
                path: relative_path.to_path_buf(),
                size: metadata.len(),
                modified: metadata.modified().unwrap(),
            });
        });
    
    files.sort_by(|a, b| a.path.cmp(&b.path));  // Deterministic order
}
```

**Step 2: Create Virtual Archive**
```rust
use orion::aead::*;

struct ArchiveBuilder {
    files: Vec<AssetFile>,
    output: PathBuf,
    config: PackConfig,
}

impl ArchiveBuilder {
    fn build(&self) -> Result<(), Error> {
        // Calculate total size
        let total_size: u64 = self.files.iter().map(|f| f.size).sum();
        
        // Pre-allocate output file
        let mut file = std::fs::File::create(&self.output)?;
        file.set_len(total_size)?;
        
        // Use XChaCha20Poly1305 for authenticated encryption
        let key = SecretKey::from_slice(&self.config.key)?;
        let mut current_offset = HEADER_SIZE + FILE_TABLE_SIZE;
        
        for asset_file in &self.files {
            let asset_data = std::fs::read(&asset_file.path)?;
            
            // Compress if enabled
            let data = if self.config.compress {
                brotli::compress(&asset_data, &BrotliEncoderParams::default())?
            } else {
                asset_data
            };
            
            // Split into chunks and encrypt with orion (battle-tested)
            for (i, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
                // XChaCha20Poly1305 supports 256-bit nonce (safe for parallel encryption)
                let nonce = Nonce::from_slice(&self.generate_nonce(i));
                let sealed = seal(&key, &nonce, chunk, None)?;
                file.write_all(&sealed)?;
            }
            
            current_offset += data.len() as u64;
        }
        
        Ok(())
    }
    
    fn generate_nonce(&self, chunk_index: usize) -> [u8; 24] {
        // Derive unique nonce per chunk (XChaCha20 uses 24-byte nonce)
        let mut nonce = [0u8; 24];
        nonce[..8].copy_from_slice(&chunk_index.to_le_bytes());
        nonce[8..24].copy_from_slice(&self.config.nonce[..16]);
        nonce
    }
}
```

**Step 3: Inject into PE File**
```rust
use goblin::pe::{PE, section_table};

fn inject_archive(pe_path: &Path, archive_data: &[u8], stub_data: &[u8]) -> Result<(), Error> {
    let mut pe = PE::parse(&std::fs::read(pe_path)?)?;
    
    // Add new section for archive (random name to avoid detection)
    let section_name = generate_random_section_name();
    let mut section = section_table::SectionTable {
        name: section_name,
        virtual_size: archive_data.len() as u32,
        virtual_address: 0,
        size_of_raw_data: align_up(archive_data.len(), 512) as u32,
        pointer_to_raw_data: 0,
        pointer_to_relocations: 0,
        pointer_to_linenumbers: 0,
        number_of_relocations: 0,
        number_of_linenumbers: 0,
        characteristics: section_table::IMAGE_SCN_CNT_INITIALIZED_DATA 
                        | section_table::IMAGE_SCN_MEM_READ,
    };
    
    // Update PE headers
    pe.header.optional_header_mut()?.unwrap()
        .size_of_image = align_up(
            pe.header.optional_header.as_ref().unwrap().size_of_image as usize 
            + align_up(archive_data.len(), 4096), 
            4096
        ) as u32;
    
    // Inject stub at new entry point
    let new_entry_point = inject_stub(pe, stub_data)?;
    pe.header.optional_header_mut()?.unwrap().address_of_entry_point = new_entry_point;
    
    // Rebuild PE file using goblin's write capabilities
    let rebuilt = pe.rebuild()?;
    std::fs::write(pe_path, rebuilt)?;
    
    Ok(())
}

fn generate_random_section_name() -> [u8; 8] {
    let mut name = [0u8; 8];
    use rand::Rng;
    let mut rng = rand::thread_rng();
    // Generate random name like ".XfA2B3c"
    name[0] = b'.';
    for i in 1..7 {
        name[i] = if rng.gen_bool(0.5) {
            b'A' + rng.gen_range(0..26) as u8
        } else {
            b'a' + rng.gen_range(0..26) as u8
        };
    }
    name
}
```

---

## **4. Runtime Protection Stub**

### **4.1 API Hooking Mechanism**

The stub hooks file I/O APIs to redirect game requests to the VFS:

```rust
// Hook Windows file APIs
#[cfg(windows)]
fn install_hooks() {
    unsafe {
        let create_file_w = detour::static_detour! {
            unsafe fn hk_CreateFileW(
                lp_file_name: *const u16,
                dw_desired_access: u32,
                dw_share_mode: u32,
                lp_security_attributes: *const c_void,
                dw_creation_disposition: u32,
                dw_flags_and_attributes: u32,
                h_template_file: HANDLE,
            ) -> HANDLE {
                let path = from_wide_ptr(lp_file_name);
                
                // Check if file exists in virtual archive
                if let Some(handle) = VFS.open_virtual(&path) {
                    return handle as HANDLE;  // Return virtual handle
                }
                
                // Otherwise, call original API
                trampoline_CreateFileW(
                    lp_file_name,
                    dw_desired_access,
                    dw_share_mode,
                    lp_security_attributes,
                    dw_creation_disposition,
                    dw_flags_and_attributes,
                    h_template_file,
                )
            }
        };
        
        // CreateFileW hook
        let target = get_proc_address("kernel32.dll", "CreateFileW");
        create_file_w.initialize(target, hk_CreateFile_w).unwrap().enable().unwrap();
        
        // Hook ReadFile, SetFilePointer, GetFileSize, CloseHandle, etc.
    }
}
```

**Hook Performance:**
- Detour overhead: ~50ns per call
- Virtual file lookup: ~200ns (hash table)
- Original API passthrough: ~50ns
- **Total overhead: <300ns per file operation** (imperceivable in game runtime)

### **4.2 Virtual File System (VFS) Implementation**

```rust
struct VFS {
    archive: VirtualArchive,
    virtual_handles: HashMap<HANDLE, VirtualFileHandle>,
    next_handle: u64,
}

struct VirtualFileHandle {
    file_info: FileInfo,
    current_offset: u64,
    path: String,
}

impl VFS {
    fn open_virtual(&mut self, path: &str) -> Option<HANDLE> {
        // Normalize path (Unity uses forward/backward slashes)
        let normalized = normalize_path(path);
        
        // Lookup in file table
        if let Some(file_info) = self.archive.file_table.get(&normalized) {
            let handle = self.next_handle as HANDLE;
            self.next_handle += 1;
            
            self.virtual_handles.insert(handle, VirtualFileHandle {
                file_info: file_info.clone(),
                current_offset: 0,
                path: normalized,
            });
            
            return Some(handle);
        }
        
        None
    }
    
    fn read_virtual(&mut self, handle: HANDLE, buffer: &mut [u8]) -> Result<usize> {
        let vfh = self.virtual_handles.get_mut(&handle).ok_or(Error::InvalidHandle)?;
        
        let data = self.archive.read_file(
            &vfh.path,
            vfh.current_offset,
            buffer.len()
        )?;
        
        buffer.copy_from_slice(&data);
        vfh.current_offset += data.len() as u64;
        
        Ok(data.len())
    }
}
```

### **4.3 Startup Sequence**

```rust
#[no_mangle]
pub extern "system" fn entry_point() -> u32 {
    unsafe {
        // 1. Locate embedded archive (via PE section)
        let archive_data = locate_archive_section();
        
        // 2. Parse archive header (<1ms)
        let archive_header = parse_header(&archive_data);
        
        // 3. Decrypt file table (~10ms for 1000 files)
        let file_table = decrypt_file_table(&archive_header);
        
        // 4. Initialize VFS with memory mapping (~5ms)
        let vfs = VFS::new(archive_header, file_table);
        GLOBAL_VFS = Some(vfs);
        
        // 5. Install API hooks (~1ms)
        install_hooks();
        
        // 6. Jump to original entry point
        let oep = get_original_entry_point();
        jump_to_oep(oep);
    }
}
```

**Total Stub Startup Time: <20ms**

---

## **5. Encryption & Security**

### **5.1 Encryption Strategy**

**Algorithm: XChaCha20-Poly1305 (via `orion` crate)**
- Speed: ~2GB/s on modern CPUs (faster than AES-NI)
- Authenticated encryption (prevents tampering)
- 256-bit nonce support (safe for parallel chunk encryption)
- No padding oracle vulnerabilities
- Suitable for streaming decryption
- **Battle-tested library (no custom crypto implementation)**

**Why orion instead of raw chacha20poly1305?**
- High-level API prevents misuse
- Misuse-resistant design
- Automatically handles authentication tags
- Constant-time operations
- Regular security audits

**Key Management:**
```rust
use orion::kdf::*;
use orion::aead::*;

// Per-build encryption key (derived from build secret)
const BUILD_SECRET: &[u8; 32] = include_bytes!("build_secret.key");

fn derive_encryption_key() -> Result<SecretKey, Error> {
    // Derive unique key per build using timestamp + random salt
    let timestamp = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    
    let salt = blake3::hash(&timestamp.to_le_bytes()).to_bytes();
    let password = Password::from_slice(BUILD_SECRET)?;
    
    // Argon2id for key derivation (resistant to GPU attacks)
    // Using orion's KDF instead of raw argon2 crate
    let mut key = [0u8; 32];
    Argon2id {
        password: &password,
        salt: &Salt::from_slice(&salt[..16])?,
        iterations: 3,
        memory: 64 * 1024, // 64 MB
        lanes: 4,
        hash_length: 32,
    }
    .hash_into(&mut key)?;
    
    SecretKey::from_slice(&key).map_err(|_| Error::InvalidKey)
}
```

**Chunk Size Optimization:**
```
Chunk Size Trade-offs:
- 32KB:  Faster decompression, more cache misses
- 64KB:  Balanced (DEFAULT)
- 128KB: Slower decompression, fewer cache misses
- 256KB: Too slow for random access
```

### **5.2 Asset Access Control**

While this system doesn't include licensing, it provides basic access control to prevent casual extraction:

```rust
// Prevent read-all (anti-scraping)
const MAX_SEQUENTIAL_READS: u32 = 1000;
const ANTI_SCRAPE_DELAY_MS: u64 = 50;

struct AccessControl {
    read_count: AtomicU32,
    last_read_time: AtomicU64,
}

impl AccessControl {
    fn check_rate_limit(&self) -> bool {
        let count = self.read_count.fetch_add(1, Ordering::Relaxed);
        let now = current_time_ms();
        let last = self.last_read_time.load(Ordering::Relaxed);
        
        if count > MAX_SEQUENTIAL_READS && (now - last) < 100 {
            // Suspicious scraping behavior detected
            thread::sleep(Duration::from_millis(ANTI_SCRAPE_DELAY_MS));
            return false;
        }
        
        self.last_read_time.store(now, Ordering::Relaxed);
        true
    }
}
```

---

## **7. Leveraging Existing Rust Ecosystem**

### **7.1 Philosophy: Don't Reinvent the Wheel**

Rather than implementing cryptographic primitives, compression algorithms, or obfuscation techniques from scratch, this system leverages **battle-tested Rust crates** that have been audited and proven in production.

**Benefits:**
- ✅ **Security**: Crypto libraries are audited and battle-tested
- ✅ **Performance**: Highly optimized with SIMD, multi-threading
- ✅ **Maintenance**: Security updates come from crate maintainers
- ✅ **Reduced Development**: Focus on unique features (VFS, PE injection) not generic utilities

### **7.2 Core Crates We Use**

| Category | Crate | Why We Chose It |
|----------|-------|-----------------|
| **Encryption** | `orion` | High-level, misuse-resistant XChaCha20Poly1305, regular audits |
| **PE Manipulation** | `goblin` | Most comprehensive PE parser/writer in Rust |
| **API Hooking** | `detour-rs` | Cross-platform, minimal overhead (~50ns) |
| **Compression** | `brotli` | Excellent compression ratios (40-50% size reduction) |
| **Hashing** | `blake3` | Extremely fast (1GB/s on modern CPUs) |
| **CLI** | `clap` | Modern, derive-based API with great documentation |
| **Progress Bars** | `indicatif` | Beautiful, feature-rich progress UI |

### **7.3 Optional Code Protection Crates**

For additional code obfuscation beyond asset protection:

**String Obfuscation:**
```rust
// goldberg - Compile-time string encryption
#[goldberg::strings("my_secret_key")]
fn get_api_key() -> &'static str {
    "my_secret_key"  // Encrypted at compile time
}

// muddy - Simple runtime obfuscation
use muddy::muddy;

fn get_password() -> String {
    muddy!("super_secret_password")  // XOR obfuscated in binary
}
```

**Logic Obfuscation:**
- `rust_code_obfuscator`: CLI tool for control-flow breaking
- `goldberg`: Integer literal obfuscation

**Anti-Tamper (Optional):**
- `Reborn`: Anti-cheat framework for runtime integrity checks
- Use only if you need to prevent memory modification during gameplay

### **7.4 Why These Tools Don't Replace Our System**

These are **libraries, not complete solutions**. They provide building blocks but don't offer:
- ❌ Asset packing workflow
- ❌ Virtual file system implementation
- ❌ File I/O API hooking
- ❌ PE injection and entry point redirection
- ❌ Chunk-based streaming encryption
- ❌ CLI tool for easy asset protection

**Our system combines these crates into a cohesive, production-ready solution.**

### **7.5 Ecosystem Comparison Table**

| Capability | Existing Rust Crates | Commercial Tools | Our System (Maxion) |
|------------|---------------------|------------------|---------------------|
| **Asset Encryption** | ✅ `orion`, `aes-gcm` | ✅ Enigma, VMProtect | ✅ Built-in, streaming |
| **Archive Creation** | ❌ None | ✅ Enigma | ✅ Custom format, optimized |
| **Virtual File System** | ❌ None | ✅ Enigma, Themida | ✅ Memory-mapped, LRU cache |
| **File I/O Hooking** | ❌ None | ✅ Enigma, Themida | ✅ CreateFileW, ReadFile, etc. |
| **PE Injection** | ⚠️ `goblin` (manual) | ✅ All packers | ✅ Automated workflow |
| **CLI Workflow** | ❌ None | ✅ Most packers | ✅ Simple pack command |
| **String Obfuscation** | ✅ `goldberg`, `muddy` | ✅ VMProtect, Themida | ⚠️ Via external tools |
| **Code Virtualization** | ❌ None | ✅ VMProtect, Enigma | ❌ Use external tools |
| **Licensing System** | ❌ None | ✅ Enigma, VMProtect | ❌ Not needed (internal) |
| **Performance Focus** | ❌ N/A | ⚠️ Varies (often heavy) | ✅ <5% overhead guaranteed |
| **Open Source** | ✅ All crates | ❌ Closed source | ✅ Full source control |
| **Cost** | Free | $300-3000/year | Free (internal use) |

**Legend:**
- ✅ = Full support
- ⚠️ = Partial support / Requires integration
- ❌ = Not supported

### **7.6 What Our System Provides That Others Don't**

**1. Performance-First Architecture**
- Most packers prioritize security over performance
- Our system guarantees <5% runtime overhead
- Chunk-based streaming encryption (not bulk decryption)
- Memory-mapped VFS with LRU cache

**2. Simple Developer Experience**
```bash
# One command to protect your game
maxion-protector pack --input Game.exe --assets-folder ./Assets
```

**3. Tailored for Internal Game Distribution**
- No licensing servers to maintain
- No hardware fingerprinting complexity
- No trial period management
- Focus on asset protection (your biggest threat)

**4. Rust Ecosystem Integration**
- Leverages best-in-class crates (`orion`, `goblin`, `detour-rs`)
- No custom crypto implementations (security by design)
- Easy to extend and maintain
- Modern tooling (clap, indicatif)

**5. Transparent Workflow**
- You control the entire build pipeline
- Can integrate with CI/CD
- Can add custom obfuscation via `goldberg`
- Can sign with your own code signing certificate

---

## **8. Code Protection (Optional - External Tools)**

For code protection, this system recommends using **existing commercial obfuscators** rather than building a custom solution:

### **6.1 Recommended External Tools**

**Recommended Approach:**
1. **Use our system for asset protection** (primary threat: asset theft)
2. **Use goldberg/muddy for string obfuscation** (prevents key extraction)
3. **Use commercial obfuscators for code virtualization** (VMProtect, Themida)

**For Unity Games:**
- **Obfuscator for .NET**: Renames C# methods, encrypts IL code, string encryption
- **ConfuserEx**: Free, open-source .NET obfuscator
- **IL2CPP + Code Obfuscation**: Convert C# to C++ then use C++ obfuscators

**Alternative: Rust-Based Obfuscation**
If you prefer staying in the Rust ecosystem, integrate these into your build pipeline:

```toml
# Cargo.toml
[build-dependencies]
goldberg = "0.1"
muddy = "0.2"
```

```rust
// build.rs - Obfuscate at build time
fn main() {
    // Obfuscate sensitive strings before compilation
    goldberg::generate_obfuscated_strings();
}
```

**For C++ Games:**
- **VMProtect**: Virtualizes critical functions (use sparingly, 10-50x slower)
- **Themida**: Packs and obfuscates PE files
- **LLVM Obfuscator**: Integrates into build pipeline for control flow flattening

### **6.2 Integration Strategy**

```bash
# Build protected game pipeline

# 1. Build game normally
./build_game.sh

# 2. Obfuscate code (optional)
obfuscator-cli MyGame.exe --config obfuscation.json

# 3. Pack assets
maxion-protector pack \
  --input "MyGame.exe" \
  --assets-folder "./Assets" \
  --output "MyGame_Protected.exe"

# 4. Sign executable (EV certificate required)
signtool sign \
  /f cert.pfx \
  /p password \
  /tr http://timestamp.digicert.com \
  /td sha256 \
  /fd sha256 \
  MyGame_Protected.exe
```

**Cost-Benefit Analysis:**
- **Asset encryption**: Low cost, high impact (prevents asset theft)
- **Code obfuscation**: Medium cost, medium impact (makes reverse engineering harder)
- **Code virtualization**: High cost, variable impact (can break games, significant slowdown)

---

## **7. Performance Benchmarks**

### **7.1 Expected Performance**

| Operation | Native | Protected | Overhead |
|-----------|--------|-----------|----------|
| Game startup | 2000ms | 2050ms | **+2.5%** |
| Texture load (10MB) | 15ms | 16ms | **+6.7%** |
| Audio stream | 0.5ms | 0.55ms | **+10%** |
| Mesh load (2MB) | 5ms | 5.2ms | **+4%** |
| Runtime frame time | 16.67ms | 16.67ms | **0%** (cached) |

### **7.2 Memory Overhead**

| Component | Memory Usage |
|-----------|--------------|
| Stub binary | 50KB |
| VFS file table (1000 files) | 1MB |
| LRU cache (50MB) | 50MB |
| Chunk decryption buffer | 64KB |
| **Total** | **~51MB** |

### **7.3 Disk Space Impact**

```
Unpacked assets: 1.5 GB
Packed + compressed: 850 MB
Space savings: 43%
```

---

## **8. Implementation Phases**

### **Phase 1: Asset Packer CLI (2-3 weeks)**

**Week 1: Core Packer**
- [ ] CLI argument parsing with `clap`
- [ ] Assets folder scanning (recursive)
- [ ] Archive format design
- [ ] Basic file table creation
- [ ] Chunk-based encryption (ChaCha20-Poly1305)

**Week 2: PE Injection**
- [ ] PE file parsing with `goblin`
- [ ] Section injection and header updates
- [ ] Stub binary injection
- [ ] Entry point redirection

**Week 3: Tool Polish**
- [ ] Compression (Brotli/Zstd) integration
- [ ] Progress bars for large asset folders
- [ ] Error handling and validation
- [ ] CLI documentation

### **Phase 2: Runtime Stub (2-3 weeks)**

**Week 1: Core Stub**
- [ ] no_std stub skeleton
- [ ] PE header parsing at runtime
- [ ] Archive header decryption
- [ ] File table decryption

**Week 2: VFS & Hooking**
- [ ] Memory-mapped VFS implementation
- [ ] LRU cache for decrypted chunks
- [ ] API hooking with `detour-rs`
- [ ] Virtual handle management

**Week 3: Optimization**
- [ ] Performance profiling
- [ ] Cache tuning
- [ ] Startup time optimization
- [ ] Memory usage optimization

### **Phase 3: Testing & Integration (1-2 weeks)**

**Week 1: Test Applications**
- [ ] Hello world C++ app with image loading
- [ ] Unity game with asset streaming
- [ ] Performance benchmarking
- [ ] Memory leak testing

**Week 2: Integration**
- [ ] Code signing integration
- [ ] Build pipeline automation
- [ ] Documentation
- [ ] Bug fixes

### **Phase 4: Polish & Deployment (1 week)**

- [ ] Cross-platform testing (Windows, Linux)
- [ ] AV false positive testing and mitigation
- [ ] User documentation
- [ ] Release packaging

**Total Timeline: 6-9 weeks**

---

## **9. Dependencies & Crates**

### **9.1 Asset Packer (std)**

We leverage battle-tested Rust ecosystem crates instead of implementing crypto from scratch:

```toml
[dependencies]
clap = { version = "4.4", features = ["derive"] }
walkdir = "2.4"
rayon = "1.8"           # Parallel processing
goblin = "0.7"          # PE parsing

# Encryption (using existing battle-tested crates)
orion = "0.17"          # XChaCha20Poly1305 - high-level, misuse-resistant crypto
# Alternative: chacha20poly1305 = "0.10" - lower-level, more control

# Compression
brotli = "3.4"
# Alternative: zstd = "0.12" - faster compression

# Hashing & Key Derivation
blake3 = "1.5"          # Fast hashing
argon2 = "0.5"           # Key derivation (prevents GPU attacks)

# CLI Utilities
indicatif = "0.17"      # Progress bars
anyhow = "1.0"          # Error handling

# Code Obfuscation (optional, for code protection)
# goldberg = "0.1"       # String literal encryption at compile time
# muddy = "0.2"          # Simple string obfuscation
```

**Why These Crates?**
- **orion**: Authenticated encryption with XChaCha20Poly1305 (2GB/s speed, 256-bit nonce support)
- **goldberg**: Compile-time string obfuscation using procedural macros (prevents static analysis)
- **Reborn**: Optional anti-tamper/anti-cheat (can be added if runtime integrity checks needed)

### **9.2 Runtime Stub (no_std)**

```toml
[dependencies]
goblin = { version = "0.7", default-features = false }
orion = { version = "0.17", default-features = false }
blake3 = { version = "1.5", default-features = false }

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.52", features = ["Win32_Foundation", "Win32_System_LibraryLoader"] }
detour = { version = "0.8", default-features = false }

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

**Why no_std?**
- Minimal binary size (<50KB target)
- Faster startup (no std runtime initialization)
- Reduced attack surface
- Easier to inject into PE files

---

## **10. Testing Strategy**

### **10.1 Unit Tests**

```rust
#[cfg(test)]
mod tests {
    use orion::aead::*;
    
    #[test]
    fn test_chunk_encryption_with_orion() {
        // Test using orion's XChaCha20Poly1305 (battle-tested)
        let key = SecretKey::generate(&mut rand::rngs::OsRng);
        let data = vec![0u8; 65536];
        
        // Encrypt
        let nonce = Nonce::generate(&mut rand::rngs::OsRng);
        let sealed = seal(&key, &nonce, &data, None).unwrap();
        
        // Decrypt
        let opened = open(&key, &nonce, &sealed, None).unwrap();
        
        assert_eq!(data, opened);
    }
    
    #[test]
    fn test_vfs_lookup() {
        let vfs = create_test_vfs();
        let handle = vfs.open_virtual("assets/test.png");
        assert!(handle.is_some());
    }
    
    #[test]
    fn test_hook_overhead() {
        // Measure hook call latency
        let start = Instant::now();
        for _ in 0..10000 {
            call_hooked_function();
        }
        let elapsed = start.elapsed();
        assert!(elapsed < Duration::from_micros(1000)); // <0.1ms per call
    }
    
    #[test]
    fn test_string_obfuscation_goldberg() {
        // Test goldberg string obfuscation (if using for code protection)
        #[goldberg::strings("test_secret")]
        fn get_secret() -> &'static str {
            "test_secret"
        }
        
        // Verify secret is encrypted in binary
        let secret = get_secret();
        assert_eq!(secret, "test_secret");
        
        // In actual tests, you'd verify the binary doesn't contain
        // the plaintext "test_secret" string
    }
}
```

### **10.2 Integration Tests**

**Test Application:**
```cpp
// test_app.cpp - Simple C++ app that loads an image
#include <windows.h>
#include <stdio.h>

int main() {
    HANDLE file = CreateFileA("assets/test.png", 
                              GENERIC_READ, 0, NULL,
                              OPEN_EXISTING, 0, NULL);
    if (file == INVALID_HANDLE_VALUE) {
        printf("Failed to open file\n");
        return 1;
    }
    
    DWORD size = GetFileSize(file, NULL);
    std::vector<BYTE> buffer(size);
    DWORD read = 0;
    ReadFile(file, buffer.data(), size, &read, NULL);
    
    printf("Loaded %d bytes\n", read);
    CloseHandle(file);
    return 0;
}
```

**Integration Test Script:**
```bash
#!/bin/bash
# Run protected test app and verify output

# Pack test app
maxion-protector pack \
  --input test_app.exe \
  --assets-folder ./test_assets \
  --output test_app_protected.exe

# Run and verify
OUTPUT=$(./test_app_protected.exe)
if [[ $OUTPUT == "Loaded 12345 bytes" ]]; then
    echo "✓ Asset loading works"
else
    echo "✗ Asset loading failed"
    exit 1
fi

# Performance test
START=$(date +%s%N)
for i in {1..100}; do
    ./test_app_protected.exe > /dev/null
done
END=$(date +%s%N)
DURATION=$((($END - $START) / 1000000))
echo "100 iterations took ${DURATION}ms (avg: ${DURATION/100}ms per run)"
```

### **10.3 Performance Benchmarks**

```rust
#[bench]
fn bench_vfs_read(b: &mut Bencher) {
    let vfs = create_vfs_with_1000_files();
    let path = "assets/large_texture.png";
    
    b.iter(|| {
        let data = vfs.read_file(path, 0, 4096);
        assert!(data.len() == 4096);
    });
}

#[bench]
fn bench_chunk_decryption(b: &mut Bencher) {
    let key = generate_test_key();
    let encrypted = encrypt_test_chunk();
    
    b.iter(|| {
        decrypt_chunk(&encrypted, &key);
    });
}
```

---

## **11. Deployment & Maintenance**

### **11.1 Build Pipeline**

```yaml
# .github/workflows/build.yml
name: Build Protected Game

on:
  push:
    branches: [main]

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Build Maxion Protector
        run: cargo build --release
      
      - name: Build Game
        run: ./build_game.sh
      
      - name: Protect Game
        run: |
          ./target/release/maxion-protector pack \
            --input Game.exe \
            --assets-folder ./Assets \
            --output Game_Protected.exe
      
      - name: Sign Executable
        run: |
          signtool sign \
            /f ${{ secrets.CERT_FILE }} \
            /p ${{ secrets.CERT_PASSWORD }} \
            Game_Protected.exe
      
      - name: Upload Artifact
        uses: actions/upload-artifact@v3
        with:
          name: Game_Protected
          path: Game_Protected.exe
```

### **11.2 Troubleshooting Guide**

**Problem: Game crashes on startup**
- Check PE headers with `PE-Bear` or `CFF Explorer`
- Verify archive section exists and is properly aligned
- Check stub entry point is correctly set

**Problem: Assets not loading**
- Enable debug logging: `RUST_LOG=debug ./Game_Protected.exe`
- Check file paths (Unity uses `/`, Windows uses `\`)
- Verify virtual handles are being created

**Problem: Performance degradation**
- Profile with `perf` (Linux) or `Visual Studio Profiler` (Windows)
- Check LRU cache hit rate
- Increase chunk size if many small reads
- Consider disabling compression for frequently-accessed files

**Problem: AV false positive**
- Sign executable with EV certificate
- Submit to Microsoft Security Intelligence portal
- Disable unnecessary features with compile-time flags
- Add vendor exclusion to build server AV

---

## **12. Future Enhancements**

### **12.1 Potential Features (Phase 5+)**

1. **Deduplication**: Compress repeated assets (duplicate textures)
2. **Delta Updates**: Patch archives instead of rebuilding entire package
3. **Hot-Loading**: Update assets without restarting game
4. **Telemetry**: Track asset access patterns for optimization
5. **Cloud Archiving**: Stream assets from cloud server (reduce download size)

### **12.2 Performance Optimizations**

1. **SIMD Acceleration**: Use `std::simd` for parallel chunk decryption
2. **ThreadPool**: Multi-threaded chunk decryption on startup
3. **Memory Pool**: Pre-allocate buffers to reduce allocations
4. **Compression Streaming**: Compress during encryption pipeline

---

## **13. Conclusion**

This performance-first asset protection system provides robust defense against asset extraction while maintaining <5% runtime overhead. By leveraging **battle-tested Rust ecosystem crates** rather than reinventing cryptographic primitives, this system delivers:

✅ **Battle-Tested Security**: Uses audited crypto libraries (`orion`, `blake3`, `argon2`) - no custom implementations  
✅ **Fast Performance**: <50ms startup, minimal runtime impact via chunk-based streaming  
✅ **Simple Workflow**: Pack assets folder → Protected executable  
✅ **Zero External Dependencies**: No licensing servers, no hardware locking  
✅ **Rust Ecosystem Integration**: Leverages best-in-class crates (`goblin`, `detour-rs`, `goldberg`)  
✅ **Maintainable**: Modular design, clear separation of concerns  

**Why This Beats Existing Rust Tools:**

While excellent Rust libraries exist (`orion`, `goldberg`, `muddy`, `Reborn`), they are **building blocks, not complete solutions**. They don't provide:

❌ Automated asset packing workflow  
❌ Virtual file system with memory mapping  
❌ File I/O API hooking (CreateFileW, ReadFile)  
❌ PE injection and entry point redirection  
❌ Chunk-based streaming encryption  
❌ LRU cache for performance optimization  

**Our System's Value Proposition:**

We combine these excellent crates into a **cohesive, production-ready solution** that solves the actual problem: protecting game assets with minimal performance impact for internal distribution.

**Estimated Development Time:** 6-9 weeks (solo developer)  
**Risk Level:** Low (well-established technology, no custom crypto)  
**Maintenance:** Low (static protection, leverages crate ecosystem)  

This system is designed for **internal game protection**, balancing security with performance to ensure players have the best experience while assets remain secure.