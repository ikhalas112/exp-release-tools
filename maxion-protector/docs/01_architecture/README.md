# Architecture Overview

## Document Metadata

| Field | Value |
|-------|-------|
| Last Updated | 2025-01-24 |
| Version | 3.0.0 |
| Complexity | Intermediate |
| Time to Read | 15 minutes |
| Audience | Developers, Architects, Technical Leads |

---

## Introduction

Maxion Protector is designed with a modular, layered architecture that provides strong security guarantees while maintaining high performance and ease of integration. The system consists of several core components that work together to protect game assets through encryption, compression, and PE injection.

### Architecture Goals

1. **Security First**: Military-grade encryption with integrity protection
2. **Performance**: Minimal runtime overhead (<12.5% for typical use cases)
3. **Self-Contained**: Single-file deployment with no external dependencies
4. **Modular Design**: Clear separation of concerns for maintainability
5. **Cross-Platform**: Build on any platform, target Windows executables

---

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Application Layer                        │
│  (Unity / C++ / Custom Game Engine)                          │
│                                                               │
│  - Game logic                                                │
│  - Asset requests                                            │
│  - Rendering, audio, physics                                  │
└────────────────────┬────────────────────────────────────────┘
                     │ maxion_init(), maxion_read_file()
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              Virtual File System (maxion-stub)              │
│                                                               │
│  - Path translation (virtual → encrypted)                   │
│  - File metadata access                                     │
│  - Cache management (LRU)                                    │
│  - Thread-safe operations                                   │
└────────────────────┬────────────────────────────────────────┘
                     │ maxion_load_chunk()
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                Access Control Layer                          │
│                                                               │
│  - Rate limiting (max sequential reads)                     │
│  - Anti-scraping delays                                      │
│  - Request validation                                        │
│  - Pattern detection                                         │
└────────────────────┬────────────────────────────────────────┘
                     │ decrypt_chunk()
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                 Decryption Layer                             │
│                                                               │
│  - ChaCha20-Poly1305 AEAD encryption                         │
│  - Per-chunk authentication (Poly1305 tag)                  │
│  - Nonce derivation (unique per chunk)                      │
│  - Replay attack prevention                                  │
└────────────────────┬────────────────────────────────────────┘
                     │ decompress_chunk()
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                Compression Layer                             │
│                                                               │
│  - Brotli compression (levels 0-11)                          │
│  - Configurable chunk sizes (default 64KB)                   │
│  - Optimized for game assets                                 │
│  - Streaming support                                         │
└────────────────────┬────────────────────────────────────────┘
                     │ (embedded in protected executable)
                     ▼
┌─────────────────────────────────────────────────────────────┐
│              Protected Executable (maxion-injector)          │
│                                                               │
│  ┌─────────────┐ Original Sections (.text, .data, .rsrc)   │
│  ├─────────────┤ .maxion    (encrypted, compressed archive) │
│  ├─────────────┤ .dll_text  (embedded runtime DLL code)    │
│  ├─────────────┤ .dll_data  (embedded runtime DLL data)    │
│  ├─────────────┤ .dll_idata (resolved import address table)│
│  └─────────────┘ .key       (obfuscated encryption key)    │
└─────────────────────────────────────────────────────────────┘
```

---

## Core Components

### 1. maxion-core

**Purpose**: Shared functionality between packer and runtime

**Location**: `crates/maxion-core/src/`

**Modules**:
- `access_control` - Rate limiting and anti-scraping mechanisms
- `archive` - Archive format definition and structure
- `archive_simple` - Simplified archive operations
- `cache` - LRU cache implementation for performance
- `compression` - Brotli compression wrapper
- `context` - Encryption context and chunk cipher operations
- `crypto` - ChaCha20-Poly1305 encryption implementation
- `error` - Comprehensive error types and handling
- `protected` - Honeypot anti-cheat system for memory tampering detection
- `types` - Core type definitions (Config, AssetFile, etc.)
- `virtual_archive` - Virtual file system implementation

**Key Responsibilities**:
- Define archive format and structure
- Provide encryption and compression utilities
- Implement caching for performance
- Define core types and error handling
- Implement virtual file system for runtime
- Provide memory tampering detection with Protected<T> and trap values

**Design Principles**:
- Zero-copy operations where possible
- Thread-safe for concurrent access
- Minimal dependencies (only cryptographic and compression libraries)
- Clear API boundaries between modules

---

### 2. maxion-injector

**Purpose**: PE file parsing and protected executable generation

**Location**: `crates/maxion-injector/src/`

**Key Features**:
- PE file parsing and validation
- Section creation and manipulation
- DLL embedding with relocations (Phase 2)
- Import resolution and IAT patching
- Entry point modification
- Protected executable generation

**Injection Process**:
```
1. Parse original PE file
   ├─ Read DOS header
   ├─ Read PE headers
   ├─ Parse section headers
   └─ Validate PE structure

2. Parse DLL structure (Phase 2)
   ├─ Parse DLL PE headers
   ├─ Identify all sections (.text, .data, .idata, .reloc)
   ├─ Parse import table
   ├─ Parse relocation table
   └─ Calculate required memory layout

3. Create new sections
   ├─ .maxion (encrypted archive)
   ├─ .dll_text (embedded DLL code)
   ├─ .dll_data (embedded DLL data)
   ├─ .dll_idata (resolved imports)
   ├─ .dll_reloc (applied relocations)
   └─ .key (obfuscated encryption key)

4. Map DLL to new addresses
   ├─ Apply base relocations
   ├─ Update section addresses
   ├─ Fix import references
   └─ Calculate delta between original and new addresses

5. Resolve imports
   ├─ Parse original PE import table
   ├─ Resolve DLL imports
   ├─ Create IAT for embedded DLL
   ├─ Patch import references
   └─ Verify all imports resolved

6. Inject stub code
   ├─ Load compiled stub binary
   ├─ Write to .stub or .dll_text section
   ├─ Validate stub integrity
   └─ Set entry point to stub initialization

7. Update PE headers
   ├─ Update section count
   ├─ Update SizeOfImage
   ├─ Update entry point
   ├─ Recalculate checksum
   └─ Write protected executable
```

**Phase 1 vs Phase 2**:

**Phase 1 (Legacy)**:
- Embeds loader stub in `.stub` section
- External dependency on `maxion_stub.dll`
- Complex API resolution via PEB walking
- Not recommended for production

**Phase 2 (Production)**:
- Fully embeds DLL as multiple sections
- No external dependencies
- Standard PE linking practices
- Proper error handling and debugging
- Recommended for production

---

### 3. maxion-packer

**Purpose**: Command-line interface for asset protection

**Location**: `crates/maxion-packer/src/`

**Key Features**:
- Asset encryption and compression
- Configuration file support (TOML)
- Batch processing
- Custom encryption keys
- Configurable compression levels
- Progress reporting

**Protection Process**:
```
1. Load configuration
   ├─ Parse command-line arguments
   ├─ Read configuration file (if specified)
   ├─ Validate settings
   └─ Set up logging

2. Scan asset directory
   ├─ Recursively scan directory
   ├─ Apply include/exclude filters
   ├─ Collect file metadata
   └─ Calculate total size

3. Create archive
   ├─ For each asset file:
   │  ├─ Read file data
   │  ├─ Compress with Brotli
   │  ├─ Encrypt with ChaCha20-Poly1305
   │  ├─ Store chunk metadata
   │  └─ Add to archive
   ├─ Write archive header
   └─ Write file table

4. Inject into PE
   ├─ Parse original executable
   ├─ Parse runtime DLL (Phase 2)
   ├─ Create new sections
   ├─ Apply relocations and resolve imports
   ├─ Inject archive into .maxion section
   ├─ Inject key into .key section
   ├─ Update PE headers
   └─ Write protected executable

5. Validate output
   ├─ Check file size
   ├─ Validate PE structure
   ├─ Verify archive integrity
   └─ Report statistics
```

---

### 4. maxion-stub

**Purpose**: Runtime library for asset loading

**Location**: `crates/maxion-stub/src/`

**Key Features**:
- C API for game engine integration
- Virtual file system
- Asset decryption and decompression
- LRU cache management
- Performance profiling
- Thread-safe operations

**Runtime Flow**:
```
1. Initialization (DLL entry point)
   ├─ Load embedded DLL sections
   ├─ Apply relocations
   ├─ Resolve imports
   ├─ Initialize encryption context
   ├─ Setup virtual file system
   └─ Setup cache

2. Asset Request (from game)
   ├─ maxion_read_file(path, buffer, size)
   │  ├─ Check cache (LRU hit?)
   │  │  ├─ Yes: Return cached data
   │  │  └─ No: Continue
   │  ├─ Translate virtual path to encrypted chunk
   │  ├─ Apply access control (rate limiting)
   │  ├─ Load encrypted chunk
   │  ├─ Decrypt chunk (ChaCha20-Poly1305)
   │  ├─ Decompress chunk (Brotli)
   │  ├─ Store in cache
   │  └─ Return data
   └─ Update cache statistics

3. Cleanup
   ├─ Flush cache
   ├─ Free resources
   └─ Shutdown
```

**C API**:
```c
// Initialization
bool maxion_init(const char* executable_path);
bool maxion_init_with_config(const MaxionConfig* config);
void maxion_shutdown();

// File operations
size_t maxion_read_file(const char* path, void* buffer, size_t size);
bool maxion_file_exists(const char* path);
size_t maxion_get_file_size(const char* path);

// Cache management
void maxion_preload(const char** paths, size_t count);
void maxion_clear_cache();
void maxion_get_cache_stats(CacheStats* stats);

// Profiling
void maxion_enable_profiling(bool enable);
const char* maxion_get_performance_report();
```

---

### 5. maxion-profiler

**Purpose**: Performance measurement and benchmarking

**Location**: `crates/maxion-profiler/src/`

**Key Features**:
- High-precision timing (nanosecond resolution)
- Metric collection (timings, counters, file loads)
- JSON report generation
- Statistical analysis
- Benchmark utilities

**Usage**:
```rust
// Initialize metrics
maxion_profiler::init_metrics("metrics.json");

// Time an operation
{
    let _timer = maxion_profiler::Timer::start("operation_name");
    // Do work...
} // Timer drops automatically, records timing

// Record custom metrics
maxion_profiler::metrics::record_counter("files_loaded", 1);
maxion_profiler::metrics::record_timing("load_time", duration);

// Flush metrics to file
maxion_profiler::flush_metrics();
```

---

## Data Flow

### Protection Flow (Packer)

```
Original Assets
    │
    ├─> [Scanner] → Collect file metadata
    │
    ├─> [Compressor] → Brotli compression (40-80% reduction)
    │   └─> Compressed chunks
    │
    ├─> [Encryptor] → ChaCha20-Poly1305 encryption
    │   ├─> Generate random key
    │   ├─> Encrypt each chunk with unique nonce
    │   ├─> Add Poly1305 authentication tag
    │   └─> Encrypted chunks with integrity protection
    │
    ├─> [Archive Builder] → Create archive structure
    │   ├─> Write header (magic, version, metadata)
    │   ├─> Write file table (paths, sizes, checksums)
    │   └─> Write encrypted data chunks
    │
    ├─> [PE Injector] → Inject into executable
    │   ├─> Parse original PE
    │   ├─> Create new sections (.maxion, .dll_*, .key)
    │   ├─> Embed runtime DLL (Phase 2)
    │   ├─> Inject encrypted archive
    │   ├─> Obfuscate and inject encryption key
    │   ├─> Update entry point
    │   └─> Write protected PE
    │
    └─> Protected Executable
```

### Runtime Flow (Protected Executable)

```
Game Execution
    │
    ├─> [OS Loader] → Load protected executable
    │
    ├─> [Embedded DLL Entry] → Runtime initialization
    │   ├─> Load embedded DLL sections
    │   ├─> Apply relocations
    │   ├─> Resolve imports
    │   ├─> Initialize encryption context
    │   ├─> Setup virtual file system
    │   ├─> Initialize cache (LRU)
    │   └─> Jump to original entry point
    │
    ├─> [Game Code] → Request asset (e.g., "textures/player.png")
    │
    ├─> [maxion-stub API] → maxion_read_file()
    │   ├─> [Virtual FS] → Translate path to encrypted chunk location
    │   ├─> [Access Control] → Check rate limits, apply delays
    │   ├─> [Cache] → Check if chunk already in memory
    │   │   ├─> Hit → Return cached data
    │   │   └─> Miss → Continue
    │   ├─> [Decryptor] → ChaCha20-Poly1305 decryption
    │   │   ├─> Derive nonce (chunk index + base nonce)
    │   │   ├─> Decrypt chunk
    │   │   └─> Verify Poly1305 tag (integrity check)
    │   ├─> [Decompressor] → Brotli decompression
    │   │   └─> Decompress to original data
    │   ├─> [Cache] → Store decompressed data
    │   └─> Return asset data to game
    │
    └─> [Game] → Use asset (render, play, process)
```

---

## Security Architecture

### Encryption

**Algorithm**: ChaCha20-Poly1305 AEAD (RFC 7539)

**Properties**:
- **Confidentiality**: ChaCha20 stream cipher
- **Integrity**: Poly1305 authentication tag (128-bit)
- **Authenticity**: AEAD ensures data hasn't been tampered with
- **Nonce Management**: Unique nonce per chunk (derived from chunk index + base nonce)

**Key Management**:
- **Generation**: Cryptographically secure random number generator (CSPRNG)
- **Size**: 256 bits (32 bytes)
- **Storage**: Obfuscated in `.key` section of protected executable
- **Delivery**: Developer-managed (file, environment, or server-delivered)

### Integrity Protection

**Per-Chunk Authentication**:
- Each encrypted chunk includes a 128-bit Poly1305 tag
- Tag is verified during decryption
- Tampering detected immediately
- Prevents chunk substitution attacks

**Archive Integrity**:
- Header checksum validation
- File table integrity verification
- Version compatibility checks

### Access Control

**Rate Limiting**:
- `MAX_SEQUENTIAL_READS`: Limit rapid sequential reads
- Prevents automated bulk extraction
- Configurable per-deployment

**Anti-Scraping Delays**:
- `ANTI_SCRAPE_DELAY_MS`: Delay between suspicious reads
- Pattern detection for scraping behavior
- Configurable per-deployment

**Key-Based Authorization**:
- Assets only decryptable with correct key
- Server-delivered keys for online games
- Key revocation support

### Memory Protection

**Honeypot Anti-Cheat System**:
- Dual-value storage: encrypted real value + plaintext trap value
- Automatic detection of memory tampering by Cheat Engine and similar tools
- Key rotation on each write prevents value freezing attacks
- Thread-safe implementation via ProtectedSync<T>

**Detection Mechanisms**:
- Memory scanner detection: Compares trap vs real value on each read
- Value freeze detection: Key rotation makes frozen values invalid
- Configurable detection actions (panic, log, flag account, random crash)

**Protected<T> API**:
- `new(val: T)` - Create protected value
- `get(&self) -> T` - Read with tamper check
- `set(&self, val: T)` - Write with key rotation
- Support for i32, f32, i64, u64, and tuples
- ~78x overhead compared to regular values (use sparingly)

**Best Practices**:
- Protect only critical values (health, ammo, score, currency)
- Avoid in tight loops (batch updates instead)
- Use regular Protected<T> for single-threaded, ProtectedSync<T> for multi-threaded
- Combine with server-side validation for multiplayer games

**Limitations**:
- Requires knowledge of Protected<T> API to bypass (harder but possible)
- Not a complete solution - use with other security measures
- Performance impact makes it unsuitable for all game state

---

## Performance Architecture

### Compression

**Algorithm**: Brotli

**Levels**: 0-11 (default 6)
- Level 0: Fastest, no compression
- Level 6: Good balance (recommended)
- Level 11: Best compression, slowest

**Performance**:
- **Compression**: ~100 MB/s at level 6
- **Ratio**: 40-80% for game assets
- **Decompression**: ~200-500 MB/s
- **Streaming**: Support for large files with chunks

### Caching

**Strategy**: LRU (Least Recently Used)

**Benefits**:
- Reduces decryption/decompression overhead
- Improves access time for frequently used assets
- Configurable cache size (default 256MB)
- Thread-safe implementation

**Cache Flow**:
```
Asset Request
    │
    ├─> Check Cache
    │   ├─> Hit → Return cached data (fast path)
    │   └─> Miss → Continue
    │
    ├─> Load from Archive
    │   ├─> Decrypt chunk
    │   ├─> Decompress chunk
    │   └─> Store in cache
    │
    └─> Return data
```

### Memory Management

**Chunk-Based Design**:
- Default chunk size: 64KB
- Balances memory usage and efficiency
- Enables streaming of large assets
- Reduces memory fragmentation

**Memory Overhead**:
- Embedded DLL: ~2MB
- Cache: Configurable (default 256MB)
- Runtime: Minimal (<10MB additional)

**Performance Targets**:
- Game startup: <2.5% overhead
- Texture load (10MB): <6.7% overhead
- Audio stream: <10% overhead
- Mesh load (2MB): <4% overhead
- Small assets: <12.5% overhead

---

## Component Interactions

### Build Time

```
maxion-packer (CLI)
    │
    ├─> maxion-core
    │   ├─> Compression (Brotli)
    │   ├─> Encryption (ChaCha20-Poly1305)
    │   ├─> Archive format
    │   └─> Error handling
    │
    ├─> maxion-injector
    │   ├─> PE parsing (goblin)
    │   ├─> DLL embedding (Phase 2)
    │   ├─> Relocations
    │   ├─> Import resolution
    │   └─> Protected executable generation
    │
    └─> maxion-profiler (optional)
        └─> Build performance metrics
```

### Runtime

```
Game Application
    │
    ├─> maxion-stub (Runtime Library)
    │   │
    │   ├─> Virtual File System
    │   │   └─> maxion-core::virtual_archive
    │   │
    │   ├─> Cache Management
    │   │   └─> maxion-core::cache
    │   │
    │   ├─> Decryption
    │   │   └─> maxion-core::crypto
    │   │
    │   ├─> Decompression
    │   │   └─> maxion-core::compression
    │   │
    │   ├─> Access Control
    │   │   └─> maxion-core::access_control
    │   │
    │   └─> Profiling (optional)
    │       └─> maxion-profiler
    │
    └─> Protected Executable
        └─> Embedded sections (.maxion, .dll_*, .key)
```

---

## Design Principles

### 1. Security First

- Military-grade encryption (ChaCha20-Poly1305)
- Per-chunk integrity verification
- Key-based access control
- No hardcoded secrets

### 2. Performance

- Minimal runtime overhead (<12.5%)
- Efficient caching (LRU)
- Zero-copy operations where possible
- Configurable compression levels

### 3. Self-Contained

- Single-file deployment (Phase 2)
- No external dependencies at runtime
- Embedded runtime library
- Cross-platform build support

### 4. Modular Design

- Clear separation of concerns
- Well-defined module boundaries
- Reusable components
- Easy to test and maintain

### 5. Type Safety

- Strong Rust types
- Compile-time error checking
- No runtime type errors
- Safe memory management

---

## Technology Stack

### Crate Architecture

The system is organized into 6 modular crates, each with a specific responsibility:

#### 1. maxion-core
**Purpose**: Shared functionality between packer and runtime stub

**Dependencies**:
- `orion 0.17` - Pure Rust cryptographic library, provides ChaCha20-Poly1305 AEAD encryption
- `argon2 0.5` - Password-based key derivation (KDF), memory-hard algorithm
- `blake3 1.8` - Fast cryptographic hash function for integrity verification
- `brotli 8.0` - General-purpose compression algorithm with levels 0-11
- `goblin 0.10.4` - PE file parsing and manipulation (PE32 support)
- `walkdir 2.5` - Directory traversal for asset scanning
- `rayon 1.10` - Data parallelism for parallel compression
- `rand 0.8` - Cryptographically secure random number generation
- `serde 1.0` - Serialization framework for configuration and data
- `bincode 1.3` - Binary serialization format for compact storage
- `memmap2 0.9` - Memory-mapped I/O for zero-copy file operations

**Key Modules**:
- `access_control` - Rate limiting with `AccessControl` trait, `ANTI_SCRAPE_DELAY_MS`, `MAX_SEQUENTIAL_READS`
- `archive` - `ArchiveHeader`, `ArchiveBuilder`, `ArchiveReader` for archive format
- `compression` - `compress()`, `decompress()`, `CompressionStats` for Brotli operations
- `compression_parallel` - `compress_parallel()`, `ParallelCompressionConfig` for multi-threaded compression
- `context` - `EncryptionContext` trait, `ChunkCipherContext` for stateful encryption
- `crypto` - `ChunkCipher` for ChaCha20-Poly1305 encryption with per-chunk nonces
- `protected` - `Protected<T>`, `ProtectedSync<T>`, `CheatDetector` for honeypot anti-cheat system
- `simd` - `SimdConfig`, `detect_simd_level()` for SIMD acceleration
- `types` - `Config`, `ChunkSize`, `AssetFile`, `EncryptionKey`, `Nonce`
- `virtual_archive` - `VirtualArchive` trait for runtime VFS abstraction

**Why These Technologies**:
- **orion**: Battle-tested, no unsafe code, constant-time implementations, MIT license
- **argon2**: Memory-hard KDF resistant to GPU/ASIC attacks, Argon2id variant recommended by RFC 9106
- **blake3**: Faster than SHA-2/SHA-3, parallelizable, Merkle tree for streaming, XOF support
- **brotli**: Better compression than gzip (15-25% better), faster than LZMA, adjustable speed/size tradeoff

---

#### 2. maxion-injector
**Purpose**: PE file manipulation and DLL embedding for protected executable generation

**Dependencies**:
- `goblin 0.10.4` - PE file parsing and validation (PE32 support)
- `memmap2 0.9` - Memory-mapped I/O for efficient PE file modification
- `blake3 1.8` - Key hashing and integrity verification
- `windows-sys 0.61` - Windows API bindings for import resolution (Win32_Foundation, Win32_System_LibraryLoader)
- `anyhow 1.0` - Error handling with context
- `maxion-core` - Shared types and utilities

**Key Features**:
- **Phase 1 (Legacy)**: Stub loader approach, external `maxion_stub.dll` dependency
- **Phase 2 (Production)**: Full DLL embedding with relocations and IAT patching
- **Section Creation**: `.maxion`, `.dll_text`, `.dll_data`, `.dll_idata`, `.dll_reloc`, `.key`, `.stub`
- **PE Constants**: `PE_SECTION_ALIGNMENT` (4096), `PE_FILE_ALIGNMENT` (512), `SECTION_HEADER_SIZE` (40)
- **Base Relocations**: Delta calculation between original and embedded DLL addresses
- **Import Resolution**: IAT patching, import table parsing, DLL dependency resolution

**Injection Process**:
1. Parse original PE file (DOS header, PE headers, section headers)
2. Parse DLL structure (all sections, import table, relocation table)
3. Create new sections with proper alignment and characteristics
4. Apply base relocations to map DLL to new addresses
5. Resolve imports and patch IAT entries
6. Inject stub code and set entry point
7. Update PE headers (SizeOfImage, entry point, checksum)

**Why These Technologies**:
- **goblin**: Pure Rust PE parser, handles edge cases, no C dependencies
- **memmap2**: Zero-copy operations, efficient for large PE files, cross-platform
- **windows-sys**: Minimal overhead, latest Windows APIs, feature-gated modules

---

#### 3. maxion-loader-stub
**Purpose**: Minimal C loader stub for PE entry point injection

**Dependencies**: None (uses raw Windows API bindings)

**Key Features**:
- **Minimal C Implementation**: No Rust dependencies, pure Windows API
- **PEB Walking**: Process Environment Block traversal for API resolution
- **Loader Entry Point**: Finds `maxion_stub.dll` and initializes runtime
- **No Dependencies**: Self-contained, ~2KB binary size

**Why Pure C**:
- **Minimal Size**: No Rust runtime or std library
- **Early Execution**: Runs before any Rust code, minimal initialization
- **Simplicity**: Direct Windows API calls, no abstraction overhead

---

#### 4. maxion-packer
**Purpose**: Command-line interface for asset protection (pnp binary)

**Dependencies**:
- `maxion-core` - Shared encryption, compression, and archive functionality
- `maxion-injector` - PE injection and DLL embedding (Phase 2)
- `clap 4.5` - Command-line argument parsing with derive API
- `indicatif 0.18` - Progress bars and ETA calculation
- `rayon 1.10` - Parallel compression for large asset sets
- `walkdir 2.5` - Directory traversal with include/exclude filters
- `env_logger 0.11` - Logging with RUST_LOG support

**CLI Features**:
- Configuration file support (TOML)
- Batch processing of asset directories
- Custom encryption keys and nonces
- Configurable compression levels (0-11)
- Progress reporting with ETA
- Verbose and quiet modes

**Protection Workflow**:
1. Load configuration and validate settings
2. Scan asset directory with filters
3. Create encrypted archive (compress → encrypt → chunk)
4. Inject into PE (Phase 2: embed DLL with relocations)
5. Validate output (PE structure, archive integrity)

**Why These Technologies**:
- **clap**: Derive API for type-safe CLI, automatic help generation, subcommands
- **indicatif**: Pretty progress bars, accurate ETA calculation, spinner support
- **rayon**: Work-stealing scheduler, automatic load balancing for compression

---

#### 5. maxion-profiler
**Purpose**: Performance measurement and benchmarking with nanosecond precision

**Dependencies**:
- `serde 1.0` - Serialization for JSON report export
- `serde_json 1.0` - JSON generation for performance reports
- `anyhow 1.0` - Error handling

**Key Features**:
- **Timer**: RAII-style timer with automatic drop recording
- **MetricsCollector**: Collects timings, counters, file load metrics
- **Benchmark Utilities**: `benchmark()`, `benchmark_compare()`, `benchmark_auto()`
- **JSON Export**: Structured reports with statistics (avg, min, max, count)
- **High Precision**: Nanosecond resolution via `std::time::Instant`

**Usage**:
```rust
// Initialize metrics collector
maxion_profiler::init_metrics("metrics.json");

// Time an operation automatically
let _timer = maxion_profiler::Timer::start("asset_load");
// Work happens here...
// Timer drops and records timing

// Record custom metrics
maxion_profiler::record_counter("files_loaded", 42);

// Flush to JSON file
maxion_profiler::flush_metrics();
```

**Why These Technologies**:
- **serde**: De facto standard for Rust serialization, derive macros, zero-cost
- **serde_json**: Fast JSON generation, pretty-printing support, familiar format

---

#### 6. maxion-stub
**Purpose**: Runtime DLL library for game engine integration

**Dependencies**:
- `maxion-core` - Shared encryption, compression, and archive functionality
- `goblin 0.10.4` - PE parsing for self-awareness
- `blake3 1.8` - Integrity verification
- `windows-sys 0.61` - Windows API bindings (Win32_Foundation, Win32_Security, Win32_Storage_FileSystem, Win32_System_Diagnostics_ToolHelp, Win32_System_IO, Win32_System_LibraryLoader, Win32_System_Memory, Win32_System_ProcessStatus, Win32_System_Threading)
- `retour 0.3` - Function hooking for API interception
- `maxion-profiler` (optional) - Performance profiling integration

**Key Features**:
- **C API**: `maxion_init()`, `maxion_read_file()`, `maxion_shutdown()`, `maxion_get_file_size()`
- **Virtual File System**: `VirtualArchive` trait for seamless asset access
- **LRU Cache**: In-memory caching of decrypted assets, thread-safe operations
- **Access Control**: Rate limiting, anti-scraping detection
- **SIMD Acceleration**: Auto-detection for optimized crypto/compression
- **Function Hooking**: API interception via retour
- **Thread-Safe**: `Arc<Mutex<T>>` for concurrent access
- **no_std Compatible**: Works in no_std environments with alloc feature

**Runtime Flow**:
1. DLL entry point (DllMain) → initialization
2. Load embedded sections, apply relocations, resolve imports
3. Initialize `ChunkCipherContext` with encryption keys
4. Setup `VirtualArchive` for asset access
5. Initialize `LruCache` for performance
6. Game engine calls C API → asset request
7. Check cache (LRU hit?), apply access control, decrypt/decompress, return data

**Why These Technologies**:
- **windows-sys**: Comprehensive Windows API coverage, minimal overhead, feature-gated
- **retour**: Stable hooking library, supports hot-patching, cross-version compatibility
- **no_std**: Enables use in embedded or constrained environments

---

### Core Technologies Explained

#### Cryptography

**ChaCha20-Poly1305 AEAD (via orion)**:
- **ChaCha20**: Stream cipher, 256-bit key, 24-byte nonce, 64-byte block
- **Poly1305**: Message authentication code, 16-byte tag
- **AEAD**: Authenticated Encryption with Associated Data, ensures confidentiality and integrity
- **Why**: 256-bit security, no padding oracle attacks, constant-time, suitable for network/disk
- **Per-Chunk Nonces**: Each 64KB chunk gets unique nonce derived via XChaCha20 construction

**Argon2id KDF (via argon2)**:
- **Argon2**: Password hashing competition winner (2015), memory-hard algorithm
- **id Variant**: Hybrid of Argon2i (data-independent) and Argon2d (data-dependent)
- **Parameters**: Time cost (iterations), memory cost (KB), parallelism (threads), salt
- **Why**: Resistant to GPU/ASIC attacks, tunable security, RFC 9106 standard

**BLAKE3 Hash (via blake3)**:
- **BLAKE3**: Fast hash function, 256-bit output
- **Features**: Parallelizable, Merkle tree, XOF (extendable output function)
- **Performance**: ~10 GB/s on modern CPUs with SIMD
- **Why**: Faster than SHA-2/SHA-3, simple API, tree structure for streaming, license is permissive (CC0 + Apache 2.0)

**Chunk-Based Encryption**:
- **ChunkSize Type**: Wrapper for validated chunk sizes (must be power of 2, min 4KB, max 16MB)
- **Default**: 64KB chunks (balances memory, cache efficiency, overhead)
- **Nonce Derivation**: `Nonce::from_chunk_index(index, base_nonce)` - combines chunk index with base nonce
- **Poly1305 Tags**: 16-byte authentication tag per chunk detects tampering
- **Why**: Random access to encrypted assets, parallel decryption, partial reads, recovery from corruption

#### Compression

**Brotli Compression (via brotli)**:
- **Algorithm**: Combination of LZ77, Huffman coding, and context modeling
- **Levels**: 0-11 (0 = fastest/lowest, 11 = slowest/highest)
- **Default**: Level 6 (good balance between speed and compression)
- **Space Savings**: 40-80% reduction in asset size
- **Parallel Compression**: `compress_parallel()` uses rayon for multi-threaded compression
- **Why**: Better than gzip/deflate, faster than LZMA, adjustable tradeoff, open standard (RFC 7932)

**Parallel Compression (via rayon)**:
- **Work-Stealing Scheduler**: Automatically balances work across threads
- **Data Parallelism**: `par_iter()` for parallel iteration over files/chunks
- **Thread Pool**: Global thread pool, auto-scaling based on CPU cores
- **Why**: Utilizes multi-core CPUs, simple API (just change `iter()` to `par_iter()`), no manual thread management

#### PE Manipulation

**PE File Format (via goblin)**:
- **PE (Portable Executable)**: Windows executable format
- **Structure**: DOS header → PE header → optional header → section headers → sections
- **Sections**: Code (.text), data (.data), imports (.idata), resources (.rdata), relocations (.reloc)
- **Alignment**: Section alignment (4KB in memory), file alignment (512 on disk)
- **Why**: Windows standard format, well-documented, goblin handles edge cases

**Base Relocations**:
- **Delta**: Difference between preferred load address and actual load address
- **Types**: IMAGE_REL_BASED_DIR64 (64-bit), IMAGE_REL_BASED_HIGHLOW (32-bit), IMAGE_REL_BASED_ABSOLUTE (pad)
- **Application**: Add delta to absolute addresses in code/data sections
- **Why**: DLLs can load at different addresses, need to fix absolute references

**Import Address Table (IAT)**:
- **IAT**: Array of function pointers for imported functions
- **Patching**: Replace addresses with actual function addresses after loading
- **Resolution**: LoadLibrary/GetProcAddress to find function addresses
- **Why**: DLL dependencies, dynamic linking, Windows standard

#### Performance

**SIMD Acceleration (via simd module)**:
- **SIMD**: Single Instruction, Multiple Data - processes multiple data points in parallel
- **Levels**: AVX2 (256-bit), SSE2 (128-bit), AVX-512 (512-bit), NEON (ARM)
- **Detection**: `detect_simd_level()` at runtime via CPUID
- **Config**: `SimdConfig::auto()`, `SimdConfig::enabled()`, `SimdConfig::disabled()`
- **Why**: 4-8x speedup for crypto/compression, automatic fallback, no unsafe code

**LRU Cache (via cache module)**:
- **LRU**: Least Recently Used - evicts least recently accessed items
- **Implementation**: Hash map + doubly-linked list
- **Thread-Safe**: `Arc<Mutex<LruCache>>` for concurrent access
- **Benefits**: Avoid repeated decryption/decompression, reduce I/O, improve response time
- **Why**: Common access patterns (assets reused frequently), simple and effective

**Memory Mapping (via memmap2)**:
- **mmap**: Maps file into memory address space
- **Zero-Copy**: No intermediate buffer copies
- **Efficient**: OS handles paging, lazy loading
- **Why**: Faster than read/write for large files, works with virtual memory, cross-platform

#### Data Structures

**Archive Format**:
- **ArchiveHeader**: Magic (8 bytes), version (4 bytes), file count (4 bytes), offsets, checksum (32 bytes), chunk size (4 bytes), compression flag (4 bytes)
- **File Table**: Array of `AssetFile` entries with paths, sizes, offsets, checksums
- **Data Section**: Encrypted and compressed asset data
- **Magic**: `b"MAXION\x01\x00"` identifies Maxion archives
- **Why**: Efficient metadata, random access to files, integrity verification

**Configuration (Config struct)**:
- **Fields**: chunk_size, compress flag, compression_level, build_secret, nonce, encryption_key, simd_config
- **Builder Pattern**: `with_compression()`, `with_chunk_size()`, `with_simd_auto()`
- **Serialization**: `serde` for TOML/JSON config files
- **Why**: Type-safe, validated, extensible, familiar pattern

**Error Handling (Error enum)**:
- **Variants**: Io, Compression, Encryption, Archive, Pe, AccessControl, RateLimitExceeded
- **Context**: `anyhow::Context` for additional error context
- **Thiserror**: Derive macros for `Display` and `Error` traits
- **Why**: Type-safe errors, helpful messages, no panics, easy debugging

---

### Build System

**Cargo Workspace**:
- **Structure**: `[workspace]` in root Cargo.toml with `[workspace.package]` and `[workspace.dependencies]`
- **Members**: All 6 crates, shared version and dependencies
- **Version 0.1.0**: Current release version
- **Edition 2021**: Modern Rust features
- **Rust Version 1.75+**: Minimum compiler version required

**Build Profiles**:
- **[profile.release]**: `opt-level="z"` (optimize for size), `lto=false` (link-time optimization disabled for faster builds), `codegen-units=1`, `strip=true`, `panic="abort"`
- **[profile.stub]**: Inherits release, `strip="symbols"` (remove debug symbols), for minimal stub binary
- **[profile.dev]**: `panic="abort"` for faster development builds

**Features**:
- **phase2**: Full DLL embedding (maxion-injector, maxion-packer)
- **std**: Standard library support (maxion-stub)
- **hooks**: Function hooking support (maxion-stub)
- **profiling**: Performance profiling integration (maxion-stub)

**Why These Choices**:
- **Workspace**: Shared dependencies, consistent versions, easier maintenance
- **Opt-level=z**: Smaller binary size, faster loading
- **LTO=false**: Faster build times, adequate performance for our use case
- **Panic=abort**: No unwind tables, smaller binaries, no runtime panic overhead

---

### Testing

**Built-in Rust Testing**:
- **Unit Tests**: `#[test]` attributes in each module
- **Integration Tests**: `tests/` directory with 25/25 tests passing for Phase 2
- **E2E Tests**: `examples/` directory for real-world scenarios
- **Benchmark Tests**: `05_benchmark/` with performance validation

**Test Coverage**:
- **Crypto**: encryption/decryption, key derivation, nonce generation
- **Compression**: compress/decompress, level testing, corrupted data
- **Archive**: header serialization, file table, invalid data
- **PE Injection**: section creation, IAT patching, relocations
- **Access Control**: rate limiting, anti-scraping

**Validation Tools**:
- **PE Structure**: goblin for parsing and validation
- **Archive Integrity**: BLAKE3 checksum verification
- **Asset Decryption**: Decrypt and verify against original

---

## Reference Documentation

For detailed technical documentation on all crates, technologies, and implementation details, see:

- **[Technical Reference](00_reference/README.md)** - Comprehensive reference for:
  - All 6 crates with detailed module documentation
  - Complete technology stack with version numbers and rationale
  - Architecture patterns and design decisions
  - API documentation for public interfaces
  - Performance optimization techniques

---

## Comparison with Alternatives

### Phase 1 vs Phase 2

| Aspect | Phase 1 (Stub Loader) | Phase 2 (DLL Embedding) |
|--------|----------------------|-------------------------|
| External Dependencies | ✅ maxion_stub.dll | ❌ None |
| Self-Contained | ❌ No | ✅ Yes |
| API Resolution | Complex PEB walking | Standard PE linking |
| Debugging | Difficult | Easy |
| Error Handling | Minimal | Comprehensive |
| Production Ready | ❌ No | ✅ Yes |
| Maintenance | High | Low |
| File Size Overhead | ~500KB | ~2.5MB |

### Maxion vs. Alternatives

| Aspect | Maxion Protector | Commercial Packagers | Custom Solutions |
|--------|------------------|---------------------|------------------|
| Open Source | ✅ Yes | ❌ No | Variable |
| Encryption | ChaCha20-Poly1305 | Proprietary | Variable |
| Self-Contained | ✅ Yes | ✅ Yes | ❌ Usually |
| Performance | <12.5% overhead | Variable | Variable |
| Cost | Free | Expensive | High development cost |
| Maintenance | Community-driven | Vendor support | Self-maintained |
| Transparency | ✅ Full source | ❌ Black box | Variable |

---

## Extensibility

### Adding New Compression Algorithms

```rust
// Implement Compression trait in maxion-core::compression
pub trait Compression {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>>;
}

// Add to config
pub enum CompressionType {
    Brotli,
    Zstd,    // New
    LZMA,    // New
    Custom,  // New
}
```

### Adding New Encryption Algorithms

```rust
// Implement EncryptionContext trait in maxion-core::context
pub trait EncryptionContext: Send + Sync {
    fn encrypt_chunk(&self, plaintext: &[u8], chunk_index: u32) -> Result<Vec<u8>>;
    fn decrypt_chunk(&self, ciphertext: &[u8], chunk_index: u32) -> Result<Vec<u8>>;
}

// Add new implementation
pub struct AesGcmCipher {
    // AES-GCM implementation
}

impl EncryptionContext for AesGcmCipher {
    // Implementation
}
```

### Adding New Access Control Strategies

```rust
// Extend access_control module
pub trait AccessControl: Send + Sync {
    fn check_access(&self, path: &str) -> Result<()>;
    fn record_access(&self, path: &str);
}

// Example: Token-based access
pub struct TokenAccessControl {
    // Token validation
}
```

---

## Known Limitations

1. **Windows Only**: Currently only targets Windows executables (Linux/macOS support planned)
2. **64-bit Only**: Only x86_64 executables supported (32-bit support planned)
3. **PE Injection**: Relies on PE file format (not applicable to ELF/Mach-O)
4. **Key Management**: Developer-managed (no built-in key distribution system)
5. **Asset Size**: Very large files (>4GB) require special handling

---

## Future Enhancements

1. **Linux Support**: Target Linux ELF executables
2. **macOS Support**: Target macOS Mach-O executables
3. **Hardware Acceleration**: Use CPU SIMD for encryption/compression
4. **Server-Side Protection**: Cloud-based asset protection service
5. **Hardware Key Protection**: TPM integration for key storage
6. **Obfuscation**: Code obfuscation techniques
7. **Anti-Debugging**: Runtime anti-debugging features
8. **GUI Tool**: Visual protection tool for non-technical users
9. **Unity Plugin**: Native Unity editor integration
10. **Real-Time Protection**: Protect assets during development

---

## Related Documentation

- [Core Components](01_components.md) - Detailed component documentation
- [PE Injection](02_pe_injection.md) - PE injection process details
- [Encryption System](03_encryption.md) - Encryption design and algorithms
- [Phase Comparison](04_phase_comparison.md) - Phase 1 vs Phase 2 comparison

---

## See Also

- [Implementation Overview](../02_implementation/README.md) - Technical implementation details
- [Security Architecture](../06_security/01_architecture.md) - Security design and guarantees
- [Performance Benchmarks](../05_benchmark/02_results.md) - Performance metrics and analysis
- [Source Code](../../crates/) - Implementation source code

---

**Document Version**: 3.0.0  
**Last Updated**: 2025-01-24  
**Maintained By**: Maxion Protector Team