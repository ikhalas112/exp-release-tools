maxion-core/src/
├── lib.rs                    # Main module exports
├── access_control.rs         # Rate limiting and anti-scraping
├── archive.rs                # Archive format definition
├── archive_simple.rs         # Simplified archive operations
├── cache/
│   └── mod.rs               # LRU cache implementation
├── compression.rs            # Brotli compression wrapper
├── compression_parallel.rs   # Parallel compression with rayon
├── context/
│   └── mod.rs               # Encryption context and chunk cipher
├── crypto.rs                 # ChaCha20-Poly1305 implementation
├── debug/
│   └── mod.rs               # Debugging utilities
├── error.rs                  # Comprehensive error types
├── io/
│   └── mod.rs               # File I/O utilities
├── simd.rs                   # SIMD detection and configuration
├── types.rs                  # Core type definitions
└── virtual_archive.rs       # Virtual file system trait
```

### Key Types and APIs

#### ChunkCipher

**Purpose**: Stateless encryption cipher for chunk-based AEAD encryption

**Definition**:
```rust
pub struct ChunkCipher {
    secret_key: [u8; 32],      // 256-bit ChaCha20 key
    base_nonce: [u8; 24],      // 24-byte XChaCha20 nonce
    chunk_size: ChunkSize,     // Size of each chunk (default 64KB)
}
```

**Methods**:
- `new(key: &[u8; 32], nonce: &[u8; 24], chunk_size: ChunkSize)` - Create new cipher
- `encrypt_single(plaintext: &[u8], nonce: &Nonce) -> Result<Vec<u8>>` - Encrypt single chunk
- `decrypt_single(ciphertext: &[u8], nonce: &Nonce) -> Result<Vec<u8>>` - Decrypt single chunk
- `encrypt_all(data: &[u8]) -> Result<Vec<Vec<u8>>>` - Encrypt all chunks
- `decrypt_all(chunks: &[Vec<u8>]) -> Result<Vec<u8>>` - Decrypt all chunks
- `chunk_size(&self) -> ChunkSize` - Get chunk size

**Encryption Algorithm**: ChaCha20-Poly1305 AEAD
- **ChaCha20**: Stream cipher, 64-byte blocks, 20 rounds
- **Poly1305**: Message authentication code, 16-byte tag
- **Nonce Derivation**: XChaCha20 construction for per-chunk nonces
- **Security**: 256-bit key, authenticated encryption, no padding oracle attacks

#### EncryptionContext Trait

**Purpose**: Trait for context-aware encryption with state and access control

**Definition**:
```rust
pub trait EncryptionContext: Send + Sync {
    fn encrypt_chunk(&self, plaintext: &[u8], chunk_index: u32) -> Result<Vec<u8>>;
    fn decrypt_chunk(&self, ciphertext: &[u8], chunk_index: u32) -> Result<Vec<u8>>;
    fn chunk_size(&self) -> ChunkSize;
    fn access_control(&self) -> &AccessControl;
    fn access_control_mut(&mut self) -> &mut AccessControl;
    fn check_access(&mut self) -> Result<()>;
    fn record_access(&mut self);
}
```

**Implementations**:
- `ChunkCipherContext` - Default implementation with access control

#### ChunkCipherContext

**Purpose**: Stateful encryption context with access control integration

**Definition**:
```rust
pub struct ChunkCipherContext {
    cipher: Arc<ChunkCipher>,           // Shared underlying cipher
    access_control: AccessControl,       // Rate limiting
    base_nonce: [u8; 24],               // Base nonce for derivation
}
```

**Methods**:
- `from_keys(key: &[u8; 32], nonce: &[u8; 24], chunk_size: ChunkSize) -> Self` - Create from keys
- `from_keys_with_limits(key: &[u8; 32], nonce: &[u8; 24], chunk_size: ChunkSize, max_reads: u32, delay_ms: u64) -> Self` - Create with custom limits
- `derive_nonce(&self, chunk_index: u32) -> Nonce` - Derive nonce for chunk
- `encrypt_range_with_access(&mut self, data: &[u8], start_chunk: u32) -> Result<Vec<Vec<u8>>>` - Encrypt with rate limiting
- `decrypt_range_with_access(&mut self, encrypted_chunks: &[Vec<u8>], start_chunk: u32) -> Result<Vec<u8>>` - Decrypt with rate limiting
- `reset_access_control(&mut self)` - Reset rate limit state
- `access_stats(&self) -> (u32, Option<Duration>)` - Get access statistics

#### ArchiveHeader

**Purpose**: Archive metadata and structure definition

**Definition**:
```rust
pub struct ArchiveHeader {
    pub magic: [u8; 8],              // b"MAXION\x01\x00"
    pub version: u32,                // Archive format version (currently 1)
    pub file_count: u32,             // Number of files in archive
    pub file_table_offset: u64,      // Offset to file table
    pub file_table_size: u64,        // Size of file table
    pub header_checksum: [u8; 32],   // BLAKE3 checksum of header
    pub chunk_size: u32,             // Chunk size used for encryption
    pub compress: u32,                // Compression flag (0/1)
}
```

**Constants**:
- `MAGIC: &[u8; 8] = b"MAXION\x01\x00"` - Magic number for Maxion archives
- `ARCHIVE_VERSION: u32 = 1` - Current archive format version

**Methods**:
- `new(file_count: u32, chunk_size: ChunkSize, compress: bool) -> Self` - Create new header
- `calculate_checksum(&self) -> [u8; 32]` - Calculate BLAKE3 checksum
- `to_bytes(&self) -> Vec<u8>` - Serialize to bytes
- `from_bytes(data: &[u8]) -> Result<Self>` - Deserialize from bytes
- `verify_checksum(&self) -> bool` - Verify header integrity

#### ArchiveBuilder

**Purpose**: Build encrypted archives from asset files

**Definition**:
```rust
pub struct ArchiveBuilder {
    config: Config,
    files: Vec<AssetFile>,
    base_dir: PathBuf,
}
```

**Methods**:
- `new(config: Config) -> Self` - Create new builder
- `with_base_dir(mut self, dir: PathBuf) -> Self` - Set base directory
- `add_file(&mut self, path: PathBuf, data: &[u8])` - Add single file
- `add_files(&mut self, files: Vec<(PathBuf, Vec<u8>)>)` - Add multiple files
- `files(&self) -> &[AssetFile]` - Get file list
- `build(&self) -> Result<Vec<u8>>` - Build archive (deprecated)
- `build_with_base_dir(&self, assets_dir: &Path) -> Result<Vec<u8>>` - Build from directory

**Build Process**:
1. Scan asset directory recursively
2. For each file:
   - Read file data
   - Compress with Brotli (if enabled)
   - Encrypt with ChaCha20-Poly1305
   - Store chunk metadata
   - Calculate BLAKE3 checksum
3. Build file table with metadata
4. Serialize archive header
5. Concatenate: header + file table + encrypted data

#### Compression

**Purpose**: Brotli compression wrapper with configurable levels

**Constants**:
- `DEFAULT_COMPRESSION_LEVEL: u32 = 6` - Balanced speed/size
- `MIN_COMPRESSION_LEVEL: u32 = 0` - Fastest, lowest compression
- `MAX_COMPRESSION_LEVEL: u32 = 11` - Slowest, highest compression

**Functions**:
- `compress(data: &[u8], level: u32) -> Result<Vec<u8>>` - Compress single buffer
- `decompress(data: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>>` - Decompress single buffer
- `decompress_into(data: &[u8], output: &mut Vec<u8>) -> Result<()>` - Decompress into pre-allocated buffer
- `compress_stream<R: Read, W: Write>(reader: &mut R, writer: &mut W, level: u32) -> Result<()>` - Stream compression
- `decompress_stream<R: Read, W: Write>(reader: &mut R, writer: &mut W) -> Result<()>` - Stream decompression
- `estimate_compressed_size(original_size: u64, level: u32) -> u64` - Estimate compressed size

**Compression Stats**:
```rust
pub struct CompressionStats {
    pub original_size: u64,           // Total original size
    pub compressed_size: u64,         // Total compressed size
    pub compression_time_ms: u64,     // Compression time
    pub level: u32,                   // Compression level used
}
```

#### Parallel Compression

**Purpose**: Multi-threaded compression using rayon

**Configuration**:
```rust
pub struct ParallelCompressionConfig {
    pub level: u32,                   // Brotli level (0-11)
    pub num_threads: Option<usize>,   // Number of threads (None = auto)
    pub chunk_size: usize,            // Chunk size for parallelization
}
```

**Functions**:
- `compress_parallel(data: &[u8], config: &ParallelCompressionConfig) -> Result<ParallelCompressionResult>` - Compress in parallel
- `compress_parallel_with_config(data: &[u8], level: u32) -> Result<Vec<u8>>` - Compress with default config
- `decompress_parallel(data: &[u8], chunk_size: usize) -> Result<Vec<u8>>` - Decompress in parallel

**Result**:
```rust
pub struct ParallelCompressionResult {
    pub compressed_data: Vec<u8>,     // Compressed output
    pub stats: CompressionStats,      // Compression statistics
}
```

**Why Parallel Compression**:
- Utilizes multi-core CPUs
- Reduces compression time by 4-8x on typical workstations
- Rayon's work-stealing scheduler automatically balances load
- Minimal overhead for large files (>1MB)

#### Access Control

**Purpose**: Rate limiting and anti-scraping mechanisms

**Constants**:
- `ANTI_SCRAPE_DELAY_MS: u64 = 100` - Minimum delay between reads (100ms)
- `MAX_SEQUENTIAL_READS: u32 = 10` - Maximum sequential reads before delay

**AccessControl Struct**:
```rust
pub struct AccessControl {
    read_count: u32,                  // Number of sequential reads
    last_read_time: Option<Instant>,  // Timestamp of last read
    max_reads: u32,                   // Maximum sequential reads
    delay_ms: u64,                    // Minimum delay between reads
}
```

**Methods**:
- `new() -> Self` - Create with default limits
- `with_limits(max_reads: u32, delay_ms: u64) -> Self` - Create with custom limits
- `check_rate_limit(&mut self) -> Result<()>` - Check if rate limit exceeded
- `record_read(&mut self)` - Record a read operation
- `reset(&mut self)` - Reset rate limit state
- `is_rate_limited(&self) -> bool` - Check if currently rate limited
- `read_count(&self) -> u32` - Get current read count
- `delay_ms(&self) -> u64` - Get delay setting
- `max_reads(&self) -> u32` - Get max reads setting
- `time_since_last_read(&self) -> Option<Duration>` - Get time since last read

**Rate Limiting Algorithm**:
1. On each read, increment `read_count`
2. If `read_count > max_reads`, check time since last read
3. If elapsed time < `delay_ms`, return `RateLimitExceeded` error
4. If elapsed time >= `delay_ms`, reset `read_count` to 0
5. Update `last_read_time` to current time

**Why Access Control**:
- Prevents brute-force extraction of encrypted assets
- Adds delay to make scraping impractical
- Configurable limits for different security requirements
- Minimal performance impact for normal usage

#### LRU Cache

**Purpose**: In-memory caching of decrypted assets with least-recently-used eviction

**LruCache Struct**:
```rust
pub struct LruCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    capacity: usize,                  // Maximum cache size
    store: HashMap<K, NodePtr<K, V>>,  // Hash map for O(1) lookup
    head: *mut Node<K, V>,           // Head of doubly-linked list
    tail: *mut Node<K, V>,           // Tail of doubly-linked list
    size: usize,                      // Current cache size
}
```

**Methods**:
- `new(capacity: usize) -> Self` - Create new LRU cache
- `get(&mut self, key: &K) -> Option<&V>` - Get value by key (moves to front)
- `put(&mut self, key: K, value: V)` - Insert or update value
- `remove(&mut self, key: &K) -> Option<V>` - Remove value by key
- `clear(&mut self)` - Clear all entries
- `len(&self) -> usize` - Get current size
- `is_empty(&self) -> bool` - Check if empty
- `capacity(&self) -> usize` - Get capacity

**LRU Algorithm**:
1. Hash map provides O(1) key lookup
2. Doubly-linked list tracks access order (most recent at head)
3. On `get()`: Move accessed item to head
4. On `put()`: Insert at head, evict tail if over capacity
5. O(1) operations for get/put

**Why LRU Cache**:
- Reduces repeated decryption/decompression overhead
- Common access patterns (assets reused frequently)
- Simple and effective algorithm
- Thread-safe when wrapped in Arc<Mutex<>>

#### SIMD Detection

**Purpose**: Detect CPU SIMD capabilities and configure acceleration

**SimdLevel Enum**:
```rust
pub enum SimdLevel {
    None,          // No SIMD support
    Sse2,          // SSE2 (128-bit, x86)
    Avx2,          // AVX2 (256-bit, x86)
    Avx512,        // AVX-512 (512-bit, x86)
    Neon,          // NEON (128-bit, ARM)
}
```

**SimdConfig Struct**:
```rust
pub struct SimdConfig {
    level: SimdLevel,                // Detected SIMD level
    enabled: bool,                   // Whether SIMD is enabled
}
```

**Methods**:
- `auto() -> Self` - Auto-detect SIMD support
- `enabled() -> Self` - Force enable SIMD
- `disabled() -> Self` - Force disable SIMD
- `detect_simd_level() -> SimdLevel` - Detect CPU SIMD capabilities
- `level(&self) -> SimdLevel` - Get current SIMD level
- `is_enabled(&self) -> bool` - Check if SIMD enabled

**Detection Method**:
1. Compile-time CPUID detection on x86
2. System calls on ARM
3. Default to None if detection fails
4. Respect user override (enabled/disabled)

**Why SIMD**:
- 4-8x speedup for cryptographic operations
- 2-4x speedup for compression
- Automatic fallback for unsupported CPUs
- No unsafe code required (libraries handle internally)

#### Virtual Archive

**Purpose**: Trait-based virtual file system for runtime asset access

**VirtualArchive Trait**:
```rust
pub trait VirtualArchive: Send + Sync {
    fn read_file(&self, path: &str, buffer: &mut [u8]) -> Result<usize>;
    fn get_file_size(&self, path: &str) -> Result<u64>;
    fn file_exists(&self, path: &str) -> bool;
    fn list_files(&self) -> Result<Vec<String>>;
}
```

**DefaultVirtualArchive Implementation**:
```rust
pub struct DefaultVirtualArchive<E>
where
    E: EncryptionContext,
{
    encryption: E,                   // Encryption context
    cache: Arc<Mutex<LruCache<String, Vec<u8>>>>, // LRU cache
    file_table: HashMap<String, AssetFileInfo>,  // File metadata
    archive_data: Vec<u8>,           // Encrypted archive data
}
```

**Methods**:
- `new(archive_data: Vec<u8>, file_table: HashMap<String, AssetFileInfo>, encryption: E) -> Self` - Create new VFS
- `read_file(&self, path: &str, buffer: &mut [u8]) -> Result<usize>` - Read file into buffer
- `get_file_size(&self, path: &str) -> Result<u64>` - Get file size
- `file_exists(&self, path: &str) -> bool` - Check if file exists
- `list_files(&self) -> Result<Vec<String>>` - List all files
- `preload(&self, paths: &[String])` - Preload files into cache
- `clear_cache(&self)` - Clear cache

**Read Flow**:
1. Check cache (LRU hit?) → return cached data
2. Check access control (rate limit?)
3. Translate path to encrypted chunk range
4. Read encrypted chunks from archive
5. Decrypt chunks using encryption context
6. Decompress chunks (if compressed)
7. Store in cache
8. Return data

**Why Virtual File System**:
- Seamless integration with game engines
- Transparent encryption/decryption
- Cache for performance optimization
- Configurable via trait for different implementations

#### Error Types

**Purpose**: Comprehensive error handling with context

**Error Enum**:
```rust
pub enum Error {
    Io { source: std::io::Error, context: String },
    Compression { source: String, context: String },
    Encryption { source: CryptoError, context: String },
    Archive { source: String, context: String },
    Pe { source: String, context: String },
    AccessControl { source: String, context: String },
    RateLimitExceeded { delay_ms: u64, reads: u32 },
    InvalidInput { field: String, reason: String },
}
```

**CryptoError**:
```rust
pub enum CryptoError {
    KeyGenerationFailed { reason: String },
    KeyDerivationFailed { reason: String },
    EncryptionFailed { reason: String },
    DecryptionFailed { reason: String },
    InvalidKeyLength { expected: usize, actual: usize },
    InvalidNonceLength { expected: usize, actual: usize },
    AuthenticationFailed,  // Poly1305 tag mismatch
}
```

**Result Type**:
```rust
pub type Result<T> = std::result::Result<T, Error>;
```

**Error Handling**:
- `anyhow::Context` for adding context to errors
- `thiserror` for automatic Display/Error implementations
- `#[source]` attribute for error chaining
- Descriptive error messages for debugging

---

## 2. maxion-injector

**Purpose**: PE file parsing and manipulation for protected executable generation. Handles DLL embedding with relocations, import resolution, and IAT patching (Phase 2).

**Location**: `crates/maxion-injector/src/`

**Version**: 0.1.0

**Rust Edition**: 2021

**Minimum Rust Version**: 1.75

**License**: MIT OR Apache-2.0

**Features**:
- `default` - No features enabled
- `phase2` - Enable Phase 2 DLL embedding (production-ready)

### Dependencies

| Dependency | Version | Purpose | Why This Choice |
|------------|---------|---------|-----------------|
| anyhow | 1.0 (workspace) | Error handling with context | Contextual errors, minimal boilerplate |
| log | 0.4 (workspace) | Logging facade | Flexible backend, multiple log levels |
| maxion-core | path: ../maxion-core | Shared types and utilities | Consistent types across crates |
| goblin | 0.10.4 (workspace) | PE file parsing | Pure Rust, handles edge cases, PE32 support |
| memmap2 | 0.9 (workspace) | Memory-mapped I/O | Zero-copy operations, efficient for large PE files |
| blake3 | 1.8 (workspace) | Key hashing | Fast, secure, preferred over SHA1/SHA256 |
| windows-sys | 0.61 (target: windows) | Windows API bindings | Minimal overhead, latest APIs, feature-gated |
| tempfile | 3.24 (workspace) | Testing utilities | Temporary files for testing, auto-cleanup |

### Module Structure

```
maxion-injector/src/
├── lib.rs                    # Main module exports, PE injection logic
└── dll_loader/
    ├── mod.rs               # DLL loader module
    ├── import.rs            # Import resolution and IAT patching
    └── loader.rs            # DLL loading and relocation application
```

### Key Types and APIs

#### PeInjector

**Purpose**: Main PE injector for creating protected executables

**Definition**:
```rust
pub struct PeInjector {
    pe_path: PathBuf,                      // Path to original PE file
    protected_path: PathBuf,              // Path for output protected PE
    archive_data: Vec<u8>,                 // Encrypted archive data
    encryption_key: [u8; 32],              // Encryption key
    nonce: [u8; 24],                      // Nonce for encryption
    chunk_size: ChunkSize,                // Chunk size used
    stub_loader: Option<StubLoader>,      // Stub loader data (Phase 1)
    stub_dll_path: Option<PathBuf>,       // Path to stub DLL (Phase 2)
    dll_structure: Option<DllStructure>,   // Parsed DLL structure (Phase 2)
}
```

**Methods**:
- `new(pe_path: PathBuf, protected_path: PathBuf, archive_data: Vec<u8>, encryption_key: [u8; 32], nonce: [u8; 24]) -> Self` - Create new injector
- `with_stub_loader(mut self, stub_loader: StubLoader) -> Self` - Set stub loader (Phase 1)
- `with_dll_loader(mut self, dll_path: PathBuf) -> Self` - Set stub DLL (Phase 2)
- `with_dll(mut self, dll_structure: DllStructure) -> Self` - Set parsed DLL structure (Phase 2)
- `with_embedded_stub(mut self, stub_data: Vec<u8>) -> Self` - Set embedded stub (Phase 2)
- `inject(&mut self) -> Result<()>` - Inject with Phase 1 stub loader
- `inject_with_dll(&mut self) -> Result<()>` - Inject with external DLL (Phase 2)
- `inject_full_dll(&mut self) -> Result<()>` - Inject with fully embedded DLL (Phase 2)

**Injection Process (Phase 2)**:
1. **Parse PE**: Load original PE file using goblin
   - Read DOS header
   - Read PE headers
   - Parse section headers
   - Validate PE structure

2. **Parse DLL**: Parse runtime DLL structure
   - Parse DLL PE headers
   - Identify all sections (.text, .data, .idata, .reloc)
   - Parse import table
   - Parse relocation table
   - Calculate required memory layout

3. **Create Sections**: Create new PE sections
   - `.maxion` - Encrypted archive data
   - `.dll_text` - Embedded DLL code section
   - `.dll_data` - Embedded DLL data section
   - `.dll_idata` - Resolved imports
   - `.dll_reloc` - Applied relocations
   - `.key` - Obfuscated encryption key

4. **Apply Relocations**: Map DLL to new addresses
   - Calculate delta between original and new addresses
   - Apply base relocations to code/data sections
   - Update section addresses
   - Fix import references

5. **Resolve Imports**: Resolve DLL dependencies
   - Parse original PE import table
   - Resolve DLL imports using Windows API
   - Create IAT for embedded DLL
   - Patch import references
   - Verify all imports resolved

6. **Update Headers**: Update PE headers
   - Update section count
   - Update SizeOfImage
   - Update entry point to stub initialization
   - Recalculate checksum
   - Write protected executable

#### PE Constants

```rust
const PE_SECTION_ALIGNMENT: u32 = 4096;        // Section alignment in memory
const PE_FILE_ALIGNMENT: u32 = 512;           // Section alignment on disk
const SECTION_HEADER_SIZE: usize = 40;         // Size of IMAGE_SECTION_HEADER
const MAX_STUB_SIZE: usize = 256 * 1024;       // Maximum stub size (256KB)
const MAX_KEY_SIZE: usize = 1024;              // Maximum key size (1KB)
```

**Why These Constants**:
- **PE_SECTION_ALIGNMENT**: 4KB is standard Windows page size
- **PE_FILE_ALIGNMENT**: 512 bytes is minimum for alignment on disk
- **SECTION_HEADER_SIZE**: Fixed size defined by PE format (40 bytes)
- **MAX_STUB_SIZE**: Prevents oversized stubs from breaking PE structure
- **MAX_KEY_SIZE**: Ensures key section is reasonably sized

#### Section Info

**Purpose**: Represents a PE section with all required metadata

**Definition**:
```rust
struct SectionInfo {
    name: [u8; 8],                  // Section name (8 bytes, null-padded)
    virtual_size: u32,              // Size in memory
    virtual_address: u32,           // RVA (Relative Virtual Address)
    size_of_raw_data: u32,          // Size on disk (must be multiple of file alignment)
    pointer_to_raw_data: u32,       // File offset to section data
    characteristics: u32,           // Section flags (readable, writable, executable)
}
```

**Section Flags (Characteristics)**:
```rust
pub mod section_flags {
    pub const IMAGE_SCN_CNT_CODE: u32 = 0x00000020;          // Contains code
    pub const IMAGE_SCN_CNT_INITIALIZED_DATA: u32 = 0x00000040; // Contains initialized data
    pub const IMAGE_SCN_MEM_READ: u32 = 0x40000000;          // Readable
    pub const IMAGE_SCN_MEM_WRITE: u32 = 0x80000000;         // Writable
    pub const IMAGE_SCN_MEM_EXECUTE: u32 = 0x20000000;        // Executable
    
    // Combinations
    pub const DATA: u32 = IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ;
    pub const DATA_READ: u32 = DATA;
    pub const DATA_WRITE: u32 = DATA | IMAGE_SCN_MEM_WRITE;
    pub const CODE: u32 = IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_EXECUTE;
}
```

**Methods**:
- `new(name: &str, virtual_size: u32, characteristics: u32) -> Self` - Create new section
- `to_bytes(&self) -> Vec<u8>` - Serialize to section header format

**Section Creation**:
1. Calculate virtual size (round up to section alignment)
2. Calculate raw size (round up to file alignment)
3. Assign virtual address (next available RVA)
4. Assign file offset (next available file offset)
5. Set characteristics (read/write/execute flags)
6. Serialize to 40-byte section header

#### Section Layout

**Purpose**: Tracks layout of all new sections

**Definition**:
```rust
struct SectionLayout {
    maxion_va: u32,                  // Virtual address of .maxion section
    maxion_offset: u32,              // File offset of .maxion section
    stub_va: u32,                    // Virtual address of .stub or .dll_text section
    stub_offset: u32,                // File offset of .stub or .dll_text section
    stub_size: u32,                 // Size of stub/DLL code
    key_va: u32,                    // Virtual address of .key section
    key_offset: u32,                // File offset of .key section
    new_entry_point: u32,            // New entry point RVA
}
```

**Layout Calculation**:
1. Start after last existing section
2. Align to section alignment (4KB)
3. Assign virtual addresses sequentially
4. Assign file offsets sequentially
5. Calculate new entry point (stub initialization)
6. Ensure no overlaps

**Why Careful Layout**:
- PE loader maps sections at specific RVAs
- Overlaps cause crashes or corruption
- Alignment requirements are strict
- Entry point must be valid executable code

#### Key Obfuscation

**Purpose**: Hide encryption key in protected executable

**Method**: XOR with random data stored in stub

**Algorithm**:
```rust
fn obfuscate_key(key: &[u8; 32], stub: &[u8]) -> Vec<u8> {
    let mut obfuscated = key.to_vec();
    for (i, byte) in obfuscated.iter_mut().enumerate() {
        *byte ^= stub[i % stub.len()];
    }
    obfuscated
}
```

**Why Obfuscation**:
- Prevents easy extraction of encryption key
- XOR is simple and reversible
- Requires stub to be present to recover key
- Adds layer of security through obscurity

#### DLL Loader (Phase 2)

**Purpose**: Load and embed DLL with relocations and imports

**DllStructure**:
```rust
struct DllStructure {
    pe: goblin::pe::PE,             // Parsed PE structure
    sections: Vec<SectionInfo>,     // All DLL sections
    imports: Vec<ImportEntry>,      // Import entries
    relocations: Vec<RelocationEntry>, // Base relocations
    entry_point: u32,               // DLL entry point RVA
    image_base: u64,                // Preferred load address
}
```

**Import Entry**:
```rust
struct ImportEntry {
    dll_name: String,               // DLL name (e.g., "kernel32.dll")
    functions: Vec<ImportFunction>, // Imported functions
}

struct ImportFunction {
    name: String,                   // Function name (e.g., "GetProcAddress")
    ordinal: u16,                   // Ordinal number (if imported by ordinal)
    hint: u16,                      // Hint (index in export table)
}
```

**Relocation Entry**:
```rust
struct RelocationEntry {
    page_rva: u32,                  // Page RVA (4KB-aligned)
    block_size: u32,                // Block size in bytes
    entries: Vec<RelocationType>,   // Relocation types
}

enum RelocationType {
    Dir64(u32),                     // 64-bit absolute address
    HighLow(u32),                   // 32-bit absolute address
    Absolute,                       // Padding
}
```

**Import Resolution**:
1. Use `LoadLibraryA` to load dependency DLLs
2. Use `GetProcAddress` to get function addresses
3. Fill IAT (Import Address Table) with addresses
4. Patch references in code sections
5. Verify all imports resolved

**Base Relocations**:
1. Calculate delta: `delta = new_base - original_base`
2. For each relocation entry:
   - Get target address at `page_rva + offset`
   - Apply delta: `new_address = old_address + delta`
   - Write back to memory
3. Handle different relocation types (64-bit, 32-bit, etc.)

**Why Full DLL Embedding (Phase 2)**:
- No external dependencies (self-contained)
- Standard PE linking practices
- Proper error handling and debugging
- More robust than Phase 1 stub loader approach
- Production-ready implementation

---

## 3. maxion-loader-stub

**Purpose**: Minimal C loader stub for PE entry point injection. Uses raw Windows API bindings and PEB walking for API resolution. No dependencies, pure C implementation.

**Location**: `crates/maxion-loader-stub/`

**Version**: 0.1.0

**Rust Edition**: 2021

**Minimum Rust Version**: 1.75

**License**: MIT OR Apache-2.0

**Crate Type**: `cdylib` (C dynamic library)

**Build Script**: `build.rs` - Compiles C source with appropriate flags

### Dependencies

None. Uses raw Windows API bindings and PEB walking.

### Module Structure

```
maxion-loader-stub/
├── Cargo.toml             # Crate manifest (no dependencies)
├── build.rs               # Build script for C compilation
├── c/
│   └── stub.c             # Pure C stub implementation
└── src/
    └── lib.rs             # Rust stub wrapper (if needed)
```

### Implementation Details

**Pure C Implementation**:

The stub is written in pure C to achieve:
- **Minimal Size**: ~2KB binary (no Rust runtime or std library)
- **Early Execution**: Runs before any Rust code, minimal initialization
- **Simplicity**: Direct Windows API calls, no abstraction overhead
- **No Dependencies**: Self-contained, no external DLLs required

**PEB Walking**:

The Process Environment Block (PEB) contains:
- Process information
- Loaded module list
- Heap information
- Environment variables

The stub traverses the PEB to:
1. Get list of loaded modules (DLLs)
2. Find `kernel32.dll` base address
3. Parse kernel32's export table
4. Find `LoadLibraryA` and `GetProcAddress` addresses
5. Use these functions to resolve additional APIs

**Why PEB Walking**:
- No need for `windows-sys` or other bindings
- Works in early initialization stages
- Minimal code footprint
- Demonstrates low-level Windows knowledge

**Loader Flow**:
1. PE entry point calls stub initialization
2. Stub walks PEB to get `LoadLibraryA` and `GetProcAddress`
3. Stub loads `maxion_stub.dll` using `LoadLibraryA`
4. Stub calls `maxion_init()` from loaded DLL
5. DLL initializes runtime (encryption, VFS, cache)
6. Control returns to application

**Why Pure C Stub**:
- Phase 1 legacy approach (Phase 2 is preferred)
- Demonstrates feasibility of minimal loader
- Educational value for understanding PE loading
- ~2KB size demonstrates optimization

---

## 4. maxion-packer

**Purpose**: Command-line interface for asset protection. Provides CLI tool (pnp binary) for encrypting, compressing, and injecting assets into Windows executables. Supports batch processing, configuration files, and progress reporting.

**Location**: `crates/maxion-packer/src/`

**Version**: 0.1.0

**Rust Edition**: 2021

**Minimum Rust Version**: 1.75

**License**: MIT OR Apache-2.0

**Binary Name**: `pnp` (Maxion Packer)

**Features**:
- `default` - Phase 2 enabled
- `phase2` - Enable Phase 2 DLL embedding

### Dependencies

| Dependency | Version | Purpose | Why This Choice |
|------------|---------|---------|-----------------|
| maxion-core | path: ../maxion-core | Shared encryption, compression, archive | Consistent types and functionality |
| maxion-injector | path: ../maxion-injector | PE injection and DLL embedding | Reuse injection logic |
| clap | 4.5 (workspace) | CLI argument parsing | Derive API, type-safe, automatic help |
| indicatif | 0.18 (workspace) | Progress bars and ETA | Pretty progress bars, accurate ETA |
| anyhow | 1.0 (workspace) | Error handling with context | Contextual errors, minimal boilerplate |
| thiserror | 2.0 (workspace) | Error derivation | Automatic Display/Error impls |
| rayon | 1.10 (workspace) | Parallel compression | Work-stealing scheduler, auto balancing |
| walkdir | 2.5 (workspace) | Directory traversal | Efficient walking, filters |
| goblin | 0.10.4 (workspace) | PE manipulation | PE parsing and validation |
| hex | 0.4 (workspace) | Hex encoding/decoding | Simple API, error handling |
| rand | 0.8 (workspace) | Random key generation | Cryptographically secure, thread_rng() |
| tempfile | 3.24 (workspace) | Temporary files | Temporary files for testing |
| env_logger | 0.11 (workspace) | Logging | RUST_LOG support, colored output |

### Module Structure

```
maxion-packer/src/
└── main.rs                    # CLI entry point and main logic
```

### CLI Interface

**Binary Name**: `pnp`

**Usage**:
```bash
pnp [OPTIONS] --input <EXE_PATH> --assets <ASSETS_DIR> --output <PROTECTED_EXE>

Options:
  -i, --input <EXE_PATH>        Path to input executable (PE file)
  -a, --assets <ASSETS_DIR>     Path to assets directory
  -o, --output <PROTECTED_EXE>  Path to output protected executable
  -c, --config <CONFIG_FILE>    Path to configuration file (TOML)
  -k, --key <HEX_KEY>           Encryption key (32 bytes, hex encoded)
  -n, --nonce <HEX_NONCE>       Encryption nonce (24 bytes, hex encoded)
  -l, --level <LEVEL>           Compression level (0-11, default: 6)
  --chunk-size <SIZE>           Chunk size in bytes (default: 65536)
  --no-compression              Disable compression
  --phase2                      Use Phase 2 DLL embedding (default)
  --verbose, -v                 Verbose output
  --quiet, -q                   Quiet output
  -h, --help                    Print help
  -V, --version                 Print version
```

### Configuration File

**Format**: TOML

**Example**:
```toml
# Configuration file for Maxion Packer

# Input/Output
input = "game.exe"
assets = "assets"
output = "game_protected.exe"

# Encryption
key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
nonce = "0123456789abcdef0123456789abcdef012345"

# Compression
compress = true
level = 6

# Chunking
chunk_size = 65536

# Phase 2
phase2 = true

# DLL path (optional, auto-generated if not specified)
# dll_path = "maxion_stub.dll"

# Logging
verbose = false
quiet = false
```

**Config Loading**:
1. Parse CLI arguments
2. If `--config` specified, load TOML file
3. Override config with CLI arguments (CLI takes precedence)
4. Validate all settings
5. Generate random keys if not specified

### Protection Workflow

**Step 1: Load Configuration**
```rust
fn load_config(cli: CliArgs, toml: Option<TomlConfig>) -> Config {
    let mut config = if let Some(toml) = toml {
        toml.into_config()
    } else {
        Config::new()
    };

    // Override with CLI arguments
    if let Some(key) = cli.key {
        config.encryption_key = decode_hex_key(&key)?;
    }
    if let Some(nonce) = cli.nonce {
        config.nonce = decode_hex_nonce(&nonce)?;
    }
    if let Some(level) = cli.level {
        config.compression_level = level;
    }
    if let Some(chunk_size) = cli.chunk_size {
        config.chunk_size = ChunkSize::new(chunk_size);
    }

    // Generate random keys if not specified
    if config.encryption_key == [0u8; 32] {
        config.generate_keys();
    }

    Ok(config)
}
```

**Step 2: Scan Asset Directory**
```rust
fn scan_assets(assets_dir: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut assets = Vec::new();

    for entry in WalkDir::new(assets_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        let relative_path = path.strip_prefix(assets_dir)?;
        let data = read_file(path)?;

        // Filter by extension (optional)
        if should_include(relative_path) {
            assets.push((relative_path.to_path_buf(), data));
        }
    }

    Ok(assets)
}
```

**Step 3: Create Archive**
```rust
fn create_archive(assets: Vec<(PathBuf, Vec<u8>)>, config: &Config) -> Result<Vec<u8>> {
    let mut builder = ArchiveBuilder::new(config.clone());

    for (path, data) in assets {
        builder.add_file(path, &data);
    }

    let archive_data = builder.build()?;
    Ok(archive_data)
}
```

**Step 4: Inject into PE**
```rust
fn inject_archive(
    exe_path: &Path,
    archive_data: Vec<u8>,
    config: &Config,
) -> Result<()> {
    let mut injector = PeInjector::new(
        exe_path.to_path_buf(),
        config.output_path.clone(),
        archive_data,
        config.encryption_key,
        config.nonce,
    );

    #[cfg(feature = "phase2")]
    {
        // Load DLL structure
        let dll_structure = parse_stub_dll(&config.dll_path)?;
        injector = injector.with_dll(dll_structure);

        // Inject with full DLL embedding
        injector.inject_full_dll()?;
    }

    #[cfg(not(feature = "phase2"))]
    {
        // Phase 1: Use stub loader
        let stub_loader = load_stub_loader(&config.stub_path)?;
        injector = injector.with_stub_loader(stub_loader);
        injector.inject()?;
    }

    Ok(())
}
```

**Step 5: Validate Output**
```rust
fn validate_output(output_path: &Path) -> Result<()> {
    // Check file size
    let metadata = std::fs::metadata(output_path)?;
    println!("Protected executable size: {} bytes", metadata.len());

    // Validate PE structure
    let data = read_file(output_path)?;
    let pe = goblin::pe::PE::parse(&data)?;
    println!("PE structure: Valid");
    println!("Number of sections: {}", pe.sections.len());

    // Check for Maxion sections
    let has_maxion_section = pe.sections.iter().any(|s| {
        s.name().unwrap_or("") == ".maxion"
    });
    if has_maxion_section {
        println!("✓ Maxion sections found");
    } else {
        return Err(Error::InvalidInput {
            field: "output",
            reason: "Maxion sections not found".to_string(),
        });
    }

    Ok(())
}
```

### Progress Reporting

**Using indicatif**:
```rust
use indicatif::{ProgressBar, ProgressStyle};

let total_assets = assets.len();
let progress = ProgressBar::new(total_assets as u64);
progress.set_style(
    ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
        .progress_chars("##-")
);

for (i, (path, data)) in assets.iter().enumerate() {
    progress.set_message(format!("Processing: {}", path.display()));
    
    // Compress and encrypt
    let compressed = compress(data, config.compression_level)?;
    let encrypted = encrypt(&compressed, &config.encryption_key, &config.nonce)?;
    
    progress.inc(1);
}

progress.finish_with_message("Protection complete!");
```

### Batch Processing

**Multiple Executables**:
```bash
# Protect multiple executables with same assets
pnp --input game.exe --assets assets/ --output game_protected.exe
pnp --input editor.exe --assets assets/ --output editor_protected.exe

# Or use configuration file
pnp --config config.toml
```

**Multiple Asset Directories**:
```bash
# Run multiple times with different asset directories
pnp --input game.exe --assets assets_v1/ --output game_v1.exe
pnp --input game.exe --assets assets_v2/ --output game_v2.exe
```

### Error Handling

**Comprehensive Error Messages**:
```rust
#[derive(Debug, thiserror::Error)]
enum PackerError {
    #[error("Failed to read input executable: {source}")]
    ReadInput { source: std::io::Error },

    #[error("Failed to scan assets directory: {path}")]
    ScanAssets { path: PathBuf },

    #[error("Failed to create archive: {source}")]
    CreateArchive { source: maxion_core::Error },

    #[error("Failed to inject into PE: {source}")]
    InjectPe { source: maxion_injector::Error },

    #[error("Invalid encryption key: must be 32 hex-encoded bytes")]
    InvalidKey,

    #[error("Invalid nonce: must be 24 hex-encoded bytes")]
    InvalidNonce,

    #[error("Failed to write output: {source}")]
    WriteOutput { source: std::io::Error },
}
```

---

## 5. maxion-profiler

**Purpose**: Performance measurement and benchmarking library. Provides high-precision timing (nanosecond resolution), metric collection, and JSON report generation for performance analysis.

**Location**: `crates/maxion-profiler/src/`

**Version**: 0.1.0

**Rust Edition**: 2021

**Minimum Rust Version**: 1.75

**License**: MIT OR Apache-2.0

### Dependencies

| Dependency | Version | Purpose | Why This Choice |
|------------|---------|---------|-----------------|
| serde | 1.0 (workspace) | Serialization | De facto standard, derive macros |
| serde_json | 1.0 | JSON generation | Fast JSON, pretty-printing, familiar format |
| log | 0.4 (workspace) | Logging | Flexible backend, multiple log levels |
| anyhow | 1.0 (workspace) | Error handling | Contextual errors, minimal boilerplate |
| tempfile | 3.24 (workspace) | Testing utilities | Temporary files for testing |

### Module Structure

```
maxion-profiler/src/
└── lib.rs                    # Profiling API, Timer, MetricsCollector
```

### Key Types and APIs

#### Timer

**Purpose**: RAII-style timer with automatic recording on drop

**Definition**:
```rust
pub struct Timer {
    start: Option<Instant>,     // Start time
    label: String,              // Metric label
}
```

**Methods**:
- `start(label: &str) -> Self` - Start a new timer
- `stop(self) -> Duration` - Stop timer and record duration
- `stop_with_label(mut self, label: &str) -> Duration` - Stop with custom label
- `duration(&self) -> Duration` - Get elapsed duration without stopping
- `elapsed_ms(&self) -> u128` - Get elapsed duration in milliseconds
- `elapsed_us(&self) -> u128` - Get elapsed duration in microseconds
- `elapsed_ns(&self) -> u128` - Get elapsed duration in nanoseconds

**Auto-Recording on Drop**:
```rust
impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(start) = self.start.take() {
            let duration = start.elapsed();
            if let Ok(mut metrics) = get_metrics().lock() {
                metrics.record_timing(&self.label, duration);
            }
        }
    }
}
```

**Usage Examples**:
```rust
// Simple automatic timing
{
    let _timer = maxion_profiler::Timer::start("asset_load");
    load_asset();  // Automatically recorded when timer goes out of scope
}

// Manual stopping with custom label
let timer = maxion_profiler::Timer::start("operation");
do_work();
let duration = timer.stop_with_label("custom_operation");

// Get duration without recording
let timer = maxion_profiler::Timer::start("measure_only");
do_work();
let elapsed = timer.elapsed_ms();
println!("Took {} ms", elapsed);
```

#### MetricsCollector

**Purpose**: Collect and store timing, counter, and file load metrics

**Definition**:
```rust
pub struct MetricsCollector {
    output_path: String,                          // JSON output file path
    timings: HashMap<String, Vec<Duration>>,      // Timing measurements
    counters: HashMap<String, u64>,              // Counter values
    file_loads: Vec<FileLoadMetric>,             // File load metrics
}
```

**Methods**:
- `new(output_path: &str) -> Self` - Create new collector
- `record_timing(&mut self, label: &str, duration: Duration)` - Record timing
- `record_counter(&mut self, label: &str, value: u64)` - Record counter
- `record_file_load(&mut self, metric: FileLoadMetric)` - Record file load
- `get_timings_ms(&self, label: &str) -> Vec<u128>` - Get timings in milliseconds
- `get_average_ms(&self, label: &str) -> Option<f64>` - Get average timing
- `get_min_ms(&self, label: &str) -> Option<u128>` - Get minimum timing
- `get_max_ms(&self, label: &str) -> Option<u128>` - Get maximum timing
- `flush(&self) -> anyhow::Result<()>` - Flush metrics to JSON file

**Statistics Calculation**:
```rust
fn generate_summary(&self) -> MetricsSummary {
    let mut timing_summary = HashMap::new();

    for (label, durations) in &self.timings {
        let count = durations.len();
        let total: Duration = durations.iter().sum();
        let avg = total / count as u32;
        let min = durations.iter().min().copied().unwrap_or(Duration::ZERO);
        let max = durations.iter().max().copied().unwrap_or(Duration::ZERO);

        timing_summary.insert(
            label.clone(),
            TimingStats {
                count,
                avg_ms: avg.as_millis(),
                min_ms: min.as_millis(),
                max_ms: max.as_millis(),
                total_ms: total.as_millis(),
            },
        );
    }

    MetricsSummary {
        timings: timing_summary,
        counters: self.counters.clone(),
    }
}
```

#### FileLoadMetric

**Purpose**: Track individual file load operations

**Definition**:
```rust
pub struct FileLoadMetric {
    pub file_path: String,     // File path
    pub file_size: u64,         // File size in bytes
    pub load_time_ms: u128,     // Load time in milliseconds
    pub method: LoadMethod,     // Load method used
}
```

**LoadMethod**:
```rust
pub enum LoadMethod {
    Direct,    // Direct file system read
    Vfs,       // VFS read from packed executable
    Stream,    // Streaming read
}
```

#### Global Metrics

**Initialization**:
```rust
use maxion_profiler::*;

// Initialize metrics collector (must be called first)
init_metrics("performance_metrics.json");

// All subsequent Timer and metric calls will be recorded
```

**Flushing Metrics**:
```rust
// Flush all metrics to JSON file
flush_metrics()?;
```

**JSON Output Format**:
```json
{
  "timings": {
    "asset_load": [12, 15, 11, 13, 14],
    "decryption": [5, 6, 5, 7, 5],
    "decompression": [8, 9, 7, 8, 9]
  },
  "counters": {
    "files_loaded": 42,
    "cache_hits": 38,
    "cache_misses": 4
  },
  "file_loads": [
    {
      "file_path": "assets/texture.png",
      "file_size": 1048576,
      "load_time_ms": 15,
      "method": "Vfs"
    }
  ],
  "summary": {
    "timings": {
      "asset_load": {
        "count": 5,
        "avg_ms": 13,
        "min_ms": 11,
        "max_ms": 15,
        "total_ms": 65
      }
    },
    "counters": {
      "files_loaded": 42,
      "cache_hits": 38,
      "cache_misses": 4
    }
  }
}
```

#### Benchmark Utilities

**benchmark()** - Run benchmark with multiple iterations:
```rust
use maxion_profiler::*;

let avg_duration = benchmark(
    "compression_test",
    100,  // iterations
    || {
        let data = vec![0u8; 1024 * 1024];
        compress(&data, 6)
    }
)?;

println!("Average compression time: {:?}", avg_duration);
```

**benchmark_auto()** - Automatic timing:
```rust
let result = benchmark_auto("decompression_test", || {
    decompress(&compressed_data, Some(1024 * 1024))
})?;
```

**benchmark_compare()** - Compare two operations:
```rust
let (time1, time2) = benchmark_compare(
    "method_a",
    || perform_method_a(),
    "method_b",
    || perform_method_b()
)?;

println!("Method A: {:?}", time1);
println!("Method B: {:?}", time2);
```

#### High Precision Timing

**Precision**: Nanosecond resolution via `std::time::Instant`

**Accuracy**:
- Monotonic clock (guaranteed to never go backwards)
- Not affected by system time changes
- High resolution on all platforms

**Platform Details**:
- **Windows**: QueryPerformanceCounter (QPC)
- **Linux/macOS**: clock_gettime(CLOCK_MONOTONIC)
- **WASM**: performance.now()

**Why High Precision**:
- Accurate measurement of fast operations (<1ms)
- Detection of performance regressions
- Identification of bottlenecks
- Benchmarking with statistical significance

---

## 6. maxion-stub

**Purpose**: Runtime DLL library for game engine integration. Provides C API for asset loading, virtual file system, decryption, decompression, caching, and performance profiling. Thread-safe operations with Windows API bindings.

**Location**: `crates/maxion-stub/src/`

**Version**: 0.1.0

**Rust Edition**: 2021

**Minimum Rust Version**: 1.75

**License**: MIT OR Apache-2.0

**Crate Type**: `cdylib` (C dynamic library)

**Features**:
- `default` - std + hooks enabled
- `std` - Standard library support
- `hooks` - Function hooking support
- `profiling` - Performance profiling integration

### Dependencies

| Dependency | Version | Purpose | Why This Choice |
|------------|---------|---------|-----------------|
| maxion-core | path: ../maxion-core | Shared encryption, compression, archive | Consistent types and functionality |
| goblin | 0.10.4 (workspace) | PE parsing for self-awareness | PE structure validation |
| blake3 | 1.8 (workspace) | Integrity verification | Fast, secure hash |
| windows-sys | 0.61 (target: windows) | Windows API bindings | Comprehensive Windows API coverage |
| retour | 0.3 | Function hooking | Stable API, hot-patching support |
| maxion-profiler | path: ../maxion-profiler (optional) | Performance profiling | Optional profiling support |
| anyhow | 1.0 (workspace) | Error handling | Contextual errors |
| log | 0.4 (workspace) | Logging | Flexible backend |
| serde | 1.0 (workspace) | Serialization (alloc feature) | Serialization for no_std |
| bincode | 1.3 (workspace) | Binary serialization | Compact format |
| tempfile | 3.24 (workspace) | Testing utilities | Temporary files for testing |

### Windows API Bindings

**windows-sys 0.61 Features**:
```toml
[dependencies.windows-sys]
version = "0.61"
features = [
    "Win32_Foundation",              # Basic types and constants
    "Win32_Security",               # Security functions
    "Win32_Storage_FileSystem",     # File I/O operations
    "Win32_System_Diagnostics_ToolHelp",  # Process information
    "Win32_System_IO",              # I/O completion ports
    "Win32_System_LibraryLoader",  # DLL loading
    "Win32_System_Memory",          # Memory management
    "Win32_System_ProcessStatus",   # Process status
    "Win32_System_Threading",       # Thread management
]
```

**Why windows-sys**:
- Minimal overhead (no wrappers, direct bindings)
- Latest Windows APIs (always up to date)
- Feature-gated (only include what you use)
- Microsoft-maintained
- Compatible with Rust's async ecosystem

### C API

**Initialization**:
```c
#include "maxion_stub.h"

// Initialize with default configuration
bool maxion_init(const char* executable_path);

// Initialize with custom configuration
bool maxion_init_with_config(const MaxionConfig* config);

// Shutdown and cleanup
void maxion_shutdown(void);
```

**File Operations**:
```c
// Read file into buffer
size_t maxion_read_file(
    const char* path,      // Virtual file path
    void* buffer,          // Output buffer
    size_t size            // Buffer size
);

// Check if file exists
bool maxion_file_exists(const char* path);

// Get file size
size_t maxion_get_file_size(const char* path);

// Read file into allocated buffer (caller must free)
void* maxion_read_file_alloc(const char* path, size_t* out_size);
```

**Cache Management**:
```c
// Preload files into cache
void maxion_preload(
    const char** paths,    // Array of paths
    size_t count           // Number of paths
);

// Clear cache
void maxion_clear_cache(void);

// Get cache statistics
void maxion_get_cache_stats(CacheStats* stats);
```

**Profiling**:
```c
// Enable/disable profiling
void maxion_enable_profiling(bool enable);

// Get performance report (JSON string, caller must free)
const char* maxion_get_performance_report(void);
```

**Configuration**:
```c
typedef struct {
    uint32_t chunk_size;          // Chunk size (default: 65536)
    bool compress;                // Enable compression (default: true)
    uint32_t compression_level;   // Compression level (0-11, default: 6)
    uint32_t cache_size;          // Cache size in entries (default: 100)
    uint32_t max_reads;           // Max sequential reads (default: 10)
    uint64_t delay_ms;            // Delay between reads (default: 100)
    bool enable_profiling;        // Enable profiling (default: false)
} MaxionConfig;

typedef struct {
    uint32_t hits;               // Cache hits
    uint32_t misses;             // Cache misses
    uint32_t evictions;          // Cache evictions
    double hit_rate;             // Hit rate (0.0 - 1.0)
    uint32_t current_size;        // Current cache size
} CacheStats;
```

### Runtime Flow

**1. DLL Entry Point (DllMain)**:
```rust
#[no_mangle]
pub extern "system" fn DllMain(
    hinst_dll: HINSTANCE,
    fdw_reason: u32,
    lpv_reserved: *mut c_void,
) -> BOOL {
    match fdw_reason {
        DLL_PROCESS_ATTACH => {
            // Initialize logging
            env_logger::init();
            
            // Load embedded sections
            load_embedded_sections();
            
            // Apply relocations
            apply_relocations();
            
            // Resolve imports
            resolve_imports();
            
            log::info!("Maxion Stub initialized");
        }
        DLL_PROCESS_DETACH => {
            // Cleanup
            cleanup();
            log::info!("Maxion Stub shutdown");
        }
        _ => {}
    }
    TRUE
}
```

**2. Load Embedded Sections**:
```rust
fn load_embedded_sections() {
    // Find .maxion section (encrypted archive)
    let maxion_section = find_section(".maxion")?;
    let archive_data = read_section_data(maxion_section)?;
    
    // Find .key section (obfuscated encryption key)
    let key_section = find_section(".key")?;
    let obfuscated_key = read_section_data(key_section)?;
    let key = deobfuscate_key(obfuscated_key, &archive_data[..256]);
    
    // Parse archive header
    let header = ArchiveHeader::from_bytes(&archive_data[..HEADER_SIZE])?;
    
    // Parse file table
    let file_table_offset = header.file_table_offset as usize;
    let file_table_size = header.file_table_size as usize;
    let file_table_data = &archive_data[file_table_offset..file_table_offset + file_table_size];
    let file_table = deserialize_file_table(file_table_data)?;
    
    // Store in global state
    *ARCHIVE_DATA.lock() = Some(archive_data);
    *FILE_TABLE.lock() = Some(file_table);
    *ENCRYPTION_KEY.lock() = Some(key);
}
```

**3. Apply Relocations**:
```rust
fn apply_relocations() {
    // Find .dll_reloc section
    let reloc_section = find_section(".dll_reloc")?;
    let reloc_data = read_section_data(reloc_section)?;
    
    // Get image base from PE header
    let image_base = get_image_base();
    
    // Apply relocations
    for entry in parse_relocations(reloc_data) {
        let target_address = image_base + entry.offset;
        let current_value = unsafe { *(target_address as *const u64) };
        let new_value = current_value + entry.delta;
        unsafe {
            *(target_address as *mut u64) = new_value;
        }
    }
}
```

**4. Resolve Imports**:
```rust
fn resolve_imports() {
    // Find .dll_idata section (already resolved by injector)
    // Imports are already patched, just verify
    
    // Get import table
    let idata_section = find_section(".dll_idata")?;
    let idata_data = read_section_data(idata_section)?;
    
    // Verify imports are resolved
    for import in parse_imports(idata_data) {
        if import.address == 0 {
            log::error!("Import not resolved: {}", import.name);
            panic!("Unresolved import: {}", import.name);
        }
    }
}
```

**5. Initialize Virtual Archive**:
```rust
fn init_virtual_archive() {
    let archive_data = ARCHIVE_DATA.lock().unwrap().clone().unwrap();
    let file_table = FILE_TABLE.lock().unwrap().clone().unwrap();
    let encryption_key = ENCRYPTION_KEY.lock().unwrap().clone().unwrap();
    
    let nonce = derive_nonce_from_stub(&archive_data[..24]);
    
    let context = ChunkCipherContext::from_keys(
        &encryption_key,
        &nonce,
        ChunkSize::new(65536),
    );
    
    let vfs = DefaultVirtualArchive::new(
        archive_data,
        file_table,
        context,
    );
    
    *VFS.lock() = Some(Arc::new(Mutex::new(vfs)));
}
```

**6. Asset Request from Game**:
```rust
#[no_mangle]
pub extern "C" fn maxion_read_file(
    path: *const c_char,
    buffer: *mut c_void,
    size: size_t,
) -> size_t {
    let path_str = unsafe { CStr::from_ptr(path).to_str().unwrap() };
    let buffer_slice = unsafe { slice_from_raw_parts_mut(buffer as *mut u8, size) };
    
    let vfs = VFS.lock().unwrap();
    let vfs = vfs.as_ref().unwrap().lock().unwrap();
    
    match vfs.read_file(path_str, buffer_slice) {
        Ok(bytes_read) => bytes_read,
        Err(e) => {
            log::error!("Failed to read file '{}': {}", path_str, e);
            0
        }
    }
}
```

### Function Hooking (retour)

**Purpose**: Intercept and modify function calls at runtime

**Usage**:
```rust
use retour::static_detour;

// Original function type
type CreateFileWFn = unsafe extern "system" fn(
    *const u16,
    u32,
    u32,
    *mut c_void,
    u32,
    u32,
    windows_sys::Win32::Foundation::HANDLE,
) -> windows_sys::Win32::Foundation::HANDLE;

static_detour! {
    static CreateFileWHook: unsafe extern "system" fn(
        *const u16, u32, u32, *mut c_void, u32, u32, HANDLE
    ) -> HANDLE;
}

// Hook function
unsafe extern "system" fn hooked_create_file_w(
    file_name: *const u16,
    desired_access: u32,
    share_mode: u32,
    security_attributes: *mut c_void,
    creation_disposition: u32,
    flags_and_attributes: u32,
    template_file: HANDLE,
) -> HANDLE {
    let path = PathBuf::from(&*U16CStr::from_ptr_str(file_name).to_string_lossy());
    
    // Check if file is protected asset
    if is_protected_asset(&path) {
        log::info!("Intercepted CreateFileW for protected asset: {:?}", path);
        return handle_vfs_open(&path);
    }
    
    // Call original function
    CreateFileWHook.call(
        file_name,
        desired_access,
        share_mode,
        security_attributes,
        creation_disposition,
        flags_and_attributes,
        template_file,
    )
}

// Install hook
fn install_hooks() {
    unsafe {
        let create_file_w: CreateFileWFn = std::mem::transmute(
            GetProcAddress(
                GetModuleHandleA("kernel32.dll\0".as_ptr() as *const i8),
                b"CreateFileW\0".as_ptr() as *const i8,
            )
        );
        
        CreateFileWHook
            .initialize(create_file_w, hooked_create_file_w)
            .unwrap()
            .enable()
            .unwrap();
    }
}
```

**Why Function Hooking**:
- Transparent interception of file I/O
- No need to modify game engine code
- Works with existing file APIs
- Can redirect to VFS seamlessly

### Thread Safety

**Arc<Mutex<>> Pattern**:
```rust
static VFS: OnceLock<Arc<Mutex<DefaultVirtualArchive<ChunkCipherContext>>>> = OnceLock::new();
static CACHE: OnceLock<Arc<Mutex<LruCache<String, Vec<u8>>>>> = OnceLock::new();

// Initialize
fn init_thread_safe() {
    let vfs = DefaultVirtualArchive::new(...);
    VFS.set(Arc::new(Mutex::new(vfs))).unwrap();
    
    let cache = LruCache::new(100);
    CACHE.set(Arc::new(Mutex::new(cache))).unwrap();
}

// Access from multiple threads
fn read_file_safe(path: &str) -> Result<Vec<u8>> {
    let vfs = VFS.get().unwrap();
    let vfs = vfs.lock().unwrap();
    vfs.read_file(path, &mut buffer)
}
```

**Why Thread-Safe**:
- Game engines use multiple threads
- Concurrent asset access is common
- Prevents data races and corruption
- Ensures cache consistency

### no_std Compatibility

**Conditional Compilation**:
```rust
#[cfg(feature = "std")]
use std::collections::HashMap;

#[cfg(not(feature = "std"))]
use alloc::collections::BTreeMap;
```

**Alloc Feature**:
```toml
[dependencies]
serde = { version = "1.0", default-features = false, features = ["alloc", "derive"] }
```

**Why no_std**:
- Enables use in embedded systems
- Can run in constrained environments
- No standard library overhead
- Smaller binary size

---

# Technology Stack

## Cryptography

### ChaCha20-Poly1305 AEAD (via orion 0.17)

**What**: Authenticated Encryption with Associated Data (AEAD) combining ChaCha20 stream cipher and Poly1305 message authentication code.

**Why**:
- **256-bit Security**: Provides strong encryption with a 256-bit key
- **No Padding Oracle Attacks**: Stream cipher is resistant to padding attacks
- **Constant-Time**: Implementation is timing-attack resistant
- **RFC 7539 Standard**: Well-vetted, widely adopted
- **Performance**: Fast in software, SIMD-accelerated implementations available
- **Pure Rust**: orion crate has no unsafe code, memory-safe implementation

**Parameters**:
- **Key Size**: 32 bytes (256 bits)
- **Nonce Size**: 24 bytes (192 bits) for XChaCha20 variant
- **Tag Size**: 16 bytes (128 bits) for Poly1305 authentication
- **Block Size**: 64 bytes for ChaCha20

**Usage in Maxion**:
- Encrypt asset data in chunks (default 64KB)
- Per-chunk nonces derived via XChaCha20 construction
- Poly1305 tag per chunk for integrity verification
- Detects tampering (authentication failure)

**Implementation**:
```rust
use orion::aead::*;

pub fn encrypt_chunk(key: &[u8; 32], nonce: &[u8; 24], plaintext: &[u8]) -> Result<Vec<u8>> {
    let secret_key = SecretKey::from_slice(key)?;
    let nonce = Nonce::from_slice(nonce)?;
    
    let seal = Sealer::new(&secret_key, &nonce)?;
    let ciphertext = seal.seal(plaintext, None)?;  // No associated data
    
    Ok(ciphertext)
}

pub fn decrypt_chunk(key: &[u8; 32], nonce: &[u8; 24], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let secret_key = SecretKey::from_slice(key)?;
    let nonce = Nonce::from_slice(nonce)?;
    
    let opener = Opener::new(&secret_key, &nonce)?;
    let plaintext = opener.open(ciphertext, None)?;
    
    Ok(plaintext)
}
```

**Nonce Derivation**:
```rust
pub fn derive_chunk_nonce(chunk_index: u32, base_nonce: &[u8; 24]) -> Nonce {
    let mut nonce = [0u8; 24];
    
    // Combine chunk index with base nonce
    nonce[..4].copy_from_slice(&chunk_index.to_le_bytes());
    nonce[4..24].copy_from_slice(&base_nonce[..20]);
    
    Nonce::from_bytes(&nonce)
}
```

### Argon2id KDF (via argon2 0.5)

**What**: Password-based key derivation function (KDF) that is memory-hard and resistant to GPU/ASIC attacks.

**Why**:
- **Memory-Hard**: Requires significant memory to compute, thwarting GPU/ASIC attacks
- **Argon2id Variant**: Hybrid of Argon2i (data-independent) and Argon2d (data-dependent)
- **RFC 9106 Standard**: Password hashing competition winner (2015)
- **Configurable**: Tunable parameters for security vs. performance tradeoff
- **Salted**: Uses unique salt for each key derivation to prevent rainbow table attacks

**Parameters**:
- **Time Cost**: Number of iterations (default: 3)
- **Memory Cost**: Memory in KB (default: 65536 = 64MB)
- **Parallelism**: Number of threads (default: 4)
- **Salt Length**: 16 bytes (128 bits)
- **Output Length**: 32 bytes (256 bits) for encryption key

**Usage in Maxion**:
- Derive encryption key from build secret or password
- Provides password-based protection option
- Memory-hard properties resist hardware attacks

**Implementation**:
```rust
use argon2::{self, Config, ThreadMode, Variant, Version};

pub fn derive_key(password: &[u8], salt: &[u8; 16]) -> Result<[u8; 32]> {
    let config = Config {
        variant: Variant::Argon2id,
        version: Version::Version13,
        mem_cost: 65536,          // 64 MB
        time_cost: 3,             // 3 iterations
        lanes: 4,                 // 4 threads
        thread_mode: ThreadMode::Parallel,
        secret: &[],              // No secret
        ad: &[],                   // No associated data
        hash_length: 32,          // 256-bit output
    };
    
    let mut key = [0u8; 32];
    argon2::hash_raw(
        password,
        salt,
        &config,
        &mut key,
    )?;
    
    Ok(key)
}
```

**Parameters Rationale**:
- **Memory Cost 64MB**: High enough to thwart GPUs, low enough for desktop systems
- **Time Cost 3**: Balanced for user experience vs. security
- **Parallelism 4**: Utilizes typical quad-core CPUs
- **Salt 16 bytes**: Prevents rainbow table attacks

### BLAKE3 Hash (via blake3 1.8)

**What**: Fast cryptographic hash function with parallelizable tree structure and XOF (extendable output function) capabilities.

**Why**:
- **Performance**: ~10 GB/s on modern CPUs with SIMD, much faster than SHA-2/SHA-3
- **Parallelizable**: Merkle tree structure enables parallel processing
- **XOF Support**: Can produce output of any length (useful for deriving keys)
- **BLAKE2 Successor**: Improved over BLAKE2 with better performance and security
- **CC0 + Apache 2.0 License**: Permissive, royalty-free
- **Simple API**: Easy to use, no complex state management

**Parameters**:
- **Output Size**: 32 bytes (256 bits) for default hash
- **Block Size**: 64 bytes
- **Tree Structure**: 1024-byte chunks, 1024-leaf fanout

**Usage in Maxion**:
- Archive header checksum (integrity verification)
- Asset file checksums (detect corruption)
- Key derivation (XOF for key material)
- Fast hash for integrity checks

**Implementation**:
```rust
use blake3::{Hasher, OutputReader};

// Simple hash
pub fn hash_data(data: &[u8]) -> [u8; 32] {
    blake3::hash(data).into()
}

// Streaming hash
pub fn hash_stream(reader: &mut impl Read) -> Result<[u8; 32]> {
    let mut hasher = Hasher::new();
    std::io::copy(reader, &mut hasher)?;
    Ok(hasher.finalize().into())
}

// XOF (derive multiple values from one hash)
pub fn derive_keys(master_key: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let mut hasher = Hasher::new();
    hasher.update(master_key);
    let mut reader = hasher.finalize_xof();
    
    let mut key1 = [0u8; 32];
    reader.fill(&mut key1);
    
    let mut key2 = [0u8; 32];
    reader.fill(&mut key2);
    
    (key1, key2)
}
```

**Comparison with SHA-2/SHA-3**:
- **BLAKE3**: ~10 GB/s (AVX2), ~6 GB/s (SSE2)
- **SHA-256**: ~700 MB/s (AVX2), ~200 MB/s (SSE2)
- **SHA-3**: ~500 MB/s (AVX2), ~150 MB/s (SSE2)
- **BLAKE3 is 10-20x faster** than SHA-2/SHA-3

**Why BLAKE3 over SHA-256**:
- 10-20x faster performance
- Parallelizable for multi-core CPUs
- XOF for key derivation
- Stronger security margin than SHA-256
- Simpler implementation, fewer edge cases
- Modern design (2019 vs 2001 for SHA-256)

---

## Compression

### Brotli Compression (via brotli 8.0)

**What**: General-purpose compression algorithm developed by Google, combining LZ77, Huffman coding, and context modeling.

**Why**:
- **Better Compression**: 15-25% better than gzip/deflate
- **Fast Decompression**: Optimized for fast decompression speed
- **RFC 7932 Standard**: Well-vetted, widely adopted
- **Configurable**: 12 compression levels (0-11) for speed/size tradeoff
- **Open Source**: Permissive license, freely available
- **Hardware Support**: SIMD-accelerated implementations

**Parameters**:
- **Levels**: 0-11 (0 = fastest/lowest, 11 = slowest/highest)
- **Default**: Level 6 (good balance)
- **Dictionary**: Optional dictionary for improved compression on similar data

**Compression Performance**:
```
Level | Compression Speed | Decompression Speed | Ratio
------|-------------------|---------------------|--------
  0   | 400 MB/s          | 600 MB/s            | 1.8x
  6   | 25 MB/s           | 500 MB/s            | 3.5x
  11  | 2 MB/s            | 450 MB/s            | 5.0x
```

**Space Savings**:
- Text files: 60-80%
- Code/Scripts: 50-70%
- Images (PNG/JPG): 5-15% (already compressed)
- Audio (MP3/OGG): 0-5% (already compressed)
- Mixed assets: 40-60% average

**Usage in Maxion**:
- Compress asset files before encryption
- Reduces encrypted archive size
- Faster decompression than LZMA
- Configurable for different use cases

**Implementation**:
```rust
use brotli::{CompressorWriter, Decompressor};

// Compress
pub fn compress(data: &[u8], level: u32) -> Result<Vec<u8>> {
    let mut compressed = Vec::new();
    {
        let mut writer = CompressorWriter::new(&mut compressed, 4096, level, 22);
        writer.write_all(data)?;
    }
    Ok(compressed)
}

// Decompress
pub fn decompress(data: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    let mut decompressed = Vec::with_capacity(expected_size);
    {
        let mut reader = Decompressor::new(data, 4096);
        std::io::copy(&mut reader, &mut decompressed)?;
    }
    decompressed.truncate(expected_size);  // Ensure exact size
    Ok(decompressed)
}
```

**Level Selection Guide**:
- **Level 0-1**: Fast builds, large downloads (CI/CD, development)
- **Level 4-6**: Balanced (default recommendation)
- **Level 9-11**: Maximum compression, slow builds (release distribution)

---

## PE Manipulation

### PE File Format (via goblin 0.10.4)

**What**: Portable Executable (PE) format is the file format for executables, object code, and DLLs used in 32-bit and 64-bit versions of Windows.

**Structure**:
```
[ DOS Header ]
    e_magic: "MZ" (2 bytes)
    e_lfanew: Offset to PE header (4 bytes)

[ DOS Stub ]
    Simple DOS program to print "This program cannot be run in DOS mode"

[ PE Signature ]
    "PE\0\0" (4 bytes)

[ File Header (IMAGE_FILE_HEADER) ]
    Machine: CPU type (2 bytes)
    NumberOfSections: Number of sections (2 bytes)
    TimeDateStamp: Compilation timestamp (4 bytes)
    PointerToSymbolTable: (4 bytes)
    NumberOfSymbols: (4 bytes)
    SizeOfOptionalHeader: (2 bytes)
    Characteristics: File attributes (2 bytes)

[ Optional Header (IMAGE_OPTIONAL_HEADER) ]
    Magic: PE32 or PE32+ (2 bytes)
    AddressOfEntryPoint: Entry point RVA (4 bytes)
    ImageBase: Preferred load address (8 bytes)
    SectionAlignment: Section alignment in memory (4 bytes)
    FileAlignment: Section alignment on disk (4 bytes)
    SizeOfImage: Total image size (4 bytes)
    SizeOfHeaders: Headers size (4 bytes)
    DataDirectory[16]: Array of directory entries

[ Section Headers ]
    Name[8]: Section name (8 bytes, null-padded)
    VirtualSize: Size in memory (4 bytes)
    VirtualAddress: RVA (4 bytes)
    SizeOfRawData: Size on disk (4 bytes)
    PointerToRawData: File offset (4 bytes)
    Characteristics: Section flags (4 bytes)
    ... (repeated for each section)

[ Section Data ]
    .text: Executable code
    .data: Initialized data
    .idata: Import table
    .reloc: Base relocations
    ... (other sections)
```

**Key Constants**:
```rust
const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D;     // "MZ"
const IMAGE_NT_SIGNATURE: u32 = 0x00004550; // "PE\0\0"
const IMAGE_NT_OPTIONAL_HDR32_MAGIC: u16 = 0x10b;  // PE32
const IMAGE_NT_OPTIONAL_HDR64_MAGIC: u16 = 0x20b;  // PE32+
const IMAGE_SECTION_ALIGNMENT: u32 = 4096;   // 4KB page size
const IMAGE_FILE_ALIGNMENT: u32 = 512;      // Disk alignment
const SECTION_HEADER_SIZE: usize = 40;      // Fixed size
```

**Why goblin**:
- **Pure Rust**: No C dependencies, memory-safe
- **Handles Edge Cases**: Robust parsing of malformed files
- **PE32 Support**: Both 32-bit and 64-bit PE files
- **Well-Tested**: Used in production by many projects
- **Active Maintenance**: Regular updates and bug fixes

**Usage in Maxion**:
- Parse original executable PE structure
- Validate PE format and integrity
- Calculate section layout for new sections
- Parse embedded DLL structure (Phase 2)
- Update PE headers after injection

**Implementation**:
```rust
use goblin::pe::PE;

fn parse_pe(data: &[u8]) -> Result<PE> {
    let pe = PE::parse(data)?;
    
    // Validate PE structure
    if pe.is_lib {
        return Err(Error::InvalidInput {
            field: "pe_file",
            reason: "Input is a library, not an executable".to_string(),
        });
    }
    
    // Check for supported architecture
    match pe.header.coff_header.machine {
        goblin::pe::header::IMAGE_FILE_MACHINE_AMD64 => {
            log::info!("64-bit PE file detected");
        }
        goblin::pe::header::IMAGE_FILE_MACHINE_I386 => {
            log::info!("32-bit PE file detected");
        }
        _ => {
            return Err(Error::InvalidInput {
                field: "pe_file",
                reason: format!("Unsupported machine type: {:x}", pe.header.coff_header.machine),
            });
        }
    }
    
    Ok(pe)
}
```

### Base Relocations

**What**: Base relocations are data in a PE file that allow the Windows loader to fix up absolute addresses if the DLL/EXE is loaded at a different address than its preferred base address.

**Why**:
- **ASLR**: Address Space Layout Randomization requires relocations
- **Conflict Resolution**: Prevents address conflicts between modules
- **DLL Embedding**: Embedded DLL must be relocated to new address space

**Relocation Types**:
```rust
pub enum RelocationType {
    IMAGE_REL_BASED_ABSOLUTE(u32),   // Padding, no action needed
    IMAGE_REL_BASED_HIGH(u32),      // High 16 bits of 32-bit address
    IMAGE_REL_BASED_LOW(u32),       // Low 16 bits of 32-bit address
    IMAGE_REL_BASED_HIGHLOW(u32),   // 32-bit address (most common)
    IMAGE_REL_BASED_DIR64(u32),     // 64-bit address (x64)
}
```

**Relocation Block Format**:
```
Page RVA: 4 bytes (4KB-aligned address of page to apply relocations to)
Block Size: 4 bytes (total size of block, including these fields)
Type Offset entries: 2 bytes each
    - High 4 bits: relocation type
    - Low 12 bits: offset from Page RVA
```

**Application Algorithm**:
```rust
pub fn apply_relocations(
    image_base: u64,
    preferred_base: u64,
    relocation_data: &[u8],
    image_data: &mut [u8],
) -> Result<()> {
    let delta = image_base.wrapping_sub(preferred_base) as i64;
    
    let mut offset = 0;
    while offset < relocation_data.len() {
        // Read relocation block header
        let page_rva = u32::from_le_bytes([
            relocation_data[offset],
            relocation_data[offset + 1],
            relocation_data[offset + 2],
            relocation_data[offset + 3],
        ]);
        let block_size = u32::from_le_bytes([
            relocation_data[offset + 4],
            relocation_data[offset + 5],
            relocation_data[offset + 6],
            relocation_data[offset + 7],
        ]) as usize;
        
        offset += 8;
        
        // Process relocation entries
        let entry_count = (block_size - 8) / 2;
        for i in 0..entry_count {
            let entry = u16::from_le_bytes([
                relocation_data[offset + i * 2],
                relocation_data[offset + i * 2 + 1],
            ]);
            
            let reloc_type = (entry >> 12) as u32;
            let reloc_offset = (entry & 0xFFF) as u64;
            
            match reloc_type {
                0 => { /* IMAGE_REL_BASED_ABSOLUTE, skip */ }
                10 => { // IMAGE_REL_BASED_DIR64
                    let target_addr = page_rva as u64 + reloc_offset;
                    let current_value = u64::from_le_bytes([
                        image_data[target_addr as usize],
                        image_data[target_addr as usize + 1],
                        image_data[target_addr as usize + 2],
                        image_data[target_addr as usize + 3],
                        image_data[target_addr as usize + 4],
                        image_data[target_addr as usize + 5],
                        image_data[target_addr as usize + 6],
                        image_data[target_addr as usize + 7],
                    ]);
                    let new_value = (current_value as i64 + delta) as u64;
                    image_data[target_addr as usize..target_addr as usize + 8]
                        .copy_from_slice(&new_value.to_le_bytes());
                }
                3 => { // IMAGE_REL_BASED_HIGHLOW
                    let target_addr = page_rva as u64 + reloc_offset;
                    let current_value = u32::from_le_bytes([
                        image_data[target_addr as usize],
                        image_data[target_addr as usize + 1],
                        image_data[target_addr as usize + 2],
                        image_data[target_addr as usize + 3],
                    ]);
                    let new_value = (current_value as i64 + delta) as u32;
                    image_data[target_addr as usize..target_addr as usize + 4]
                        .copy_from_slice(&new_value.to_le_bytes());
                }
                _ => {
                    return Err(Error::Pe {
                        source: format!("Unsupported relocation type: {}", reloc_type),
                        context: "apply_relocations".to_string(),
                    });
                }
            }
        }
        
        offset += entry_count * 2;
    }
    
    Ok(())
}
```

**Why Base Relocations**:
- Required for DLL embedding (DLL loaded at different address)
- Enables ASLR (randomized load addresses)
- Prevents address conflicts between modules
- Standard PE mechanism, well-understood

### Import Address Table (IAT)

**What**: The Import Address Table (IAT) is an array of function pointers that the Windows loader fills in with the actual addresses of imported functions at load time.

**Structure**:
```
[ Import Directory Entry ]
    OriginalFirstThunk: RVA to import name table (INT)
    TimeDateStamp: Timestamp (0 = bound)
    ForwarderChain: (0 = no forwarding)
    Name: RVA to DLL name (e.g., "kernel32.dll\0")
    FirstThunk: RVA to IAT (filled by loader)

[ Import Name Table (INT) / Hint Name Table ]
    Hint: 2 bytes (index into export table)
    Name: Variable length (function name, null-terminated)

[ IAT (filled by loader) ]
    Function addresses (4 bytes for x86, 8 bytes for x64)
```

**Import Resolution Process**:
```rust
use windows_sys::Win32::System_LibraryLoader::{LoadLibraryA, GetProcAddress};

pub fn resolve_imports(
    import_directory: &[u8],
    image_data: &mut [u8],
    image_base: u64,
) -> Result<()> {
    let mut offset = 0;
    
    // Process each import directory entry
    loop {
        // Read import directory entry
        let original_first_thunk = u32::from_le_bytes([
            import_directory[offset],
            import_directory[offset + 1],
            import_directory[offset + 2],
            import_directory[offset + 3],
        ]);
        
        if original_first_thunk == 0 {
            break;  // End of import directory
        }
        
        let name_rva = u32::from_le_bytes([
            import_directory[offset + 12],
            import_directory[offset + 13],
            import_directory[offset + 14],
            import_directory[offset + 15],
        ]);
        
        let first_thunk = u32::from_le_bytes([
            import_directory[offset + 16],
            import_directory[offset + 17],
            import_directory[offset + 18],
            import_directory[offset + 19],
        ]);
        
        // Read DLL name
        let dll_name_ptr = (image_base + name_rva as u64) as *const i8;
        let dll_name = unsafe { std::ffi::CStr::from_ptr(dll_name_ptr) };
        log::debug!("Loading DLL: {:?}", dll_name);
        
        // Load DLL
        let dll_handle = unsafe { LoadLibraryA(dll_name.as_ptr() as *const u8) };
        if dll_handle == 0 {
            return Err(Error::Pe {
                source: format!("Failed to load DLL: {:?}", dll_name),
                context: "resolve_imports".to_string(),
            });
        }
        
        // Resolve imports and fill IAT
        let mut int_offset = original_first_thunk as usize;
        let mut iat_offset = first_thunk as usize;
        
        loop {
            // Read import name table entry (64-bit)
            let int_entry = u64::from_le_bytes([
                image_data[int_offset],
                image_data[int_offset + 1],
                image_data[int_offset + 2],
                image_data[int_offset + 3],
                image_data[int_offset + 4],
                image_data[int_offset + 5],
                image_data[int_offset + 6],
                image_data[int_offset + 7],
            ]);
            
            if int_entry == 0 {
                break;  // End of import list
            }
            
            // Check if imported by ordinal (high bit set)
            let is_ordinal = (int_entry & 0x8000000000000000) != 0;
            
            if is_ordinal {
                let ordinal = (int_entry & 0xFFFF) as u16;
                let func_addr = unsafe { GetProcAddress(dll_handle, ordinal as *const i8) };
                let func_addr = func_addr as u64;
                
                // Write to IAT
                image_data[iat_offset..iat_offset + 8].copy_from_slice(&func_addr.to_le_bytes());
            } else {
                // Import by name
                let hint_name_rva = (int_entry & 0x7FFFFFFFFFFFFFFF) as u32;
                let hint_name_ptr = (image_base + hint_name_rva as u64) as *const u16;
                
                // Skip hint (2 bytes)
                let name_ptr = unsafe { hint_name_ptr.add(1) };
                let func_name = unsafe { U16CStr::from_ptr_str(name_ptr) };
                
                log::trace!("  Resolving: {}", func_name.to_string_lossy());
                
                let func_addr = unsafe {
                    GetProcAddress(
                        dll_handle,
                        func_name.as_ptr() as *const i8
                    )
                };
                
                if func_addr == 0 {
                    return Err(Error::Pe {
                        source: format!(
                            "Failed to resolve function: {}",
                            func_name.to_string_lossy()
                        ),
                        context: "resolve_imports".to_string(),
                    });
                }
                
                // Write to IAT
                let func_addr = func_addr as u64;
                image_data[iat_offset..iat_offset + 8].copy_from_slice(&func_addr.to_le_bytes());
            }
            
            int_offset += 8;
            iat_offset += 8;
        }
        
        offset += 20;  // Size of import directory entry
    }
    
    Ok(())
}
```

**Why Import Resolution**:
- Required for DLL embedding (resolve DLL dependencies)
- Enables dynamic linking to Windows APIs
- Standard PE mechanism, required for executables
- Phase 2: Pre-resolve imports to avoid runtime resolution

---

## Performance

### SIMD Acceleration (via simd module)

**What**: SIMD (Single Instruction, Multiple Data) enables processing multiple data points in parallel with a single instruction, providing significant speedup for cryptographic and compression operations.

**SIMD Levels**:
```rust
pub enum SimdLevel {
    None,      // No SIMD support
    Sse2,      // SSE2 (128-bit, x86, requires SSE2)
    Avx2,      // AVX2 (256-bit, x86, requires AVX2)
    Avx512,    // AVX-512 (512-bit, x86, requires AVX-512F)
    Neon,      // NEON (128-bit, ARM, requires ARMv7/A64)
}
```

**Detection**:
```rust
#[cfg(target_arch = "x86_64")]
pub fn detect_simd_level() -> SimdLevel {
    use std::arch::x86_64::{__cpuid, CpuidResult};
    
    let CpuidResult { eax, ebx, ecx, edx } = unsafe { __cpuid(1) };
    
    // Check for SSE2
    let has_sse2 = (edx & (1 << 26)) != 0;
    
    // Check for AVX
    let has_avx = (ecx & (1 << 28)) != 0 && has_xsave_feature();
    
    // Check for AVX2
    let has_avx2 = has_avx && (ebx & (1 << 5)) != 0;
    
    // Check for AVX-512F
    let CpuidResult { ebx: ebx7, .. } = unsafe { __cpuid(7) };
    let has_avx512 = has_avx && (ebx7 & (1 << 16)) != 0;
    
    if has_avx512 {
        SimdLevel::Avx512
    } else if has_avx2 {
        SimdLevel::Avx2
    } else if has_sse2 {
        SimdLevel::Sse2
    } else {
        SimdLevel::None
    }
}

#[cfg(target_arch = "x86_64")]
fn has_xsave_feature() -> bool {
    let CpuidResult { ecx, .. } = unsafe { __cpuid(1) };
    (ecx & (1 << 27)) != 0
}

#[cfg(target_arch = "aarch64")]
pub fn detect_simd_level() -> SimdLevel {
    // ARM64 always has NEON
    SimdLevel::Neon
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn detect_simd_level() -> SimdLevel {
    SimdLevel::None
}
```

**Configuration**:
```rust
pub struct SimdConfig {
    level: SimdLevel,
    enabled: bool,
}

impl SimdConfig {
    pub fn auto() -> Self {
        let level = detect_simd_level();
        Self {
            level,
            enabled: true,
        }
    }
    
    pub fn enabled() -> Self {
        Self {
            level: detect_simd_level(),
            enabled: true,
        }
    }
    
    pub fn disabled() -> Self {
        Self {
            level: SimdLevel::None,
            enabled: false,
        }
    }
}
```

**Performance Impact**:
```
Operation          | None  | SSE2  | AVX2  | AVX-512
-------------------|-------|-------|-------|---------
ChaCha20 Encrypt   | 1x    | 2.5x  | 5x    | 8x
Poly1305 Auth      | 1x    | 3x    | 6x    | 10x
BLAKE3 Hash        | 1x    | 3x    | 7x    | 12x
Brotli Compress    | 1x    | 1.5x  | 2x    | 3x
Brotli Decompress  | 1x    | 1.3x  | 1.7x  | 2.2x
```

**Why SIMD**:
- 4-12x speedup for cryptographic operations
- 1.3-3x speedup for compression
- Automatic fallback for unsupported CPUs
- No unsafe code required (libraries handle internally)
- Standard in modern CPUs (SSE2 since 2001, AVX2 since 2013)

### LRU Cache

**What**: Least Recently Used (LRU) cache evicts the least recently accessed items when capacity is reached, providing efficient caching with O(1) operations.

**Structure**:
```rust
pub struct LruCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    capacity: usize,
    store: HashMap<K, NodePtr<K, V>>,
    head: *mut Node<K, V>,
    tail: *mut Node<K, V>,
    size: usize,
}

struct Node<K, V> {
    key: Option<K>,
    value: Option<V>,
    prev: *mut Node<K, V>,
    next: *mut Node<K, V>,
}

type NodePtr<K, V> = *mut Node<K, V>;
```

**Operations**:
- `get(&mut self, key: &K) -> Option<&V>` - O(1): Move to front
- `put(&mut self, key: K, value: V)` - O(1): Insert/Update
- `remove(&mut self, key: &K) -> Option<V>` - O(1): Remove by key
- `clear(&mut self)` - O(n): Clear all entries

**Why LRU**:
- O(1) operations (hash map + doubly-linked list)
- Simple and effective algorithm
- Common access patterns work well (assets reused frequently)
- Thread-safe when wrapped in Arc<Mutex<>>
- Reduces repeated decryption/decompression overhead

**Thread-Safe Wrapper**:
```rust
use std::sync::{Arc, Mutex};

type ThreadSafeLruCache<K, V> = Arc<Mutex<LruCache<K, V>>>;

fn init_thread_safe_cache<K, V>(capacity: usize) -> ThreadSafeLruCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    Arc::new(Mutex::new(LruCache::new(capacity)))
}

fn get_from_cache<K, V>(
    cache: &ThreadSafeLruCache<K, V>,
    key: &K,
) -> Option<V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    cache.lock().unwrap().get(key).cloned()
}
```

### Memory Mapping (via memmap2)

**What**: Memory mapping (mmap) maps a file into the process's virtual address space, allowing the operating system to handle paging and file I/O automatically.

**Why**:
- **Zero-Copy**: No intermediate buffer copies
- **Efficient**: OS handles paging, lazy loading
- **Fast**: Direct memory access to file data
- **Automatic**: OS handles cache management
- **Cross-Platform**: Works on Windows, Linux, macOS

**Usage**:
```rust
use memmap2::Mmap;

pub fn read_file_zero_copy(path: &Path) -> Result<Mmap> {
    let file = std::fs::File::open(path)?;
    let mmap = unsafe { Mmap::map(&file)? };
    Ok(mmap)
}

// Use memory-mapped data
fn process_large_file(path: &Path) -> Result<()> {
    let mmap = read_file_zero_copy(path)?;
    
    // Directly access file data as slice
    let header = &mmap[..64];
    let data = &mmap[64..];
    
    // No explicit read() calls needed
    // OS handles paging automatically
    
    Ok(())
}
```

**Performance Impact**:
- **Large Files**: 10-100x faster than read/write for files >100MB
- **Random Access**: O(1) access to any offset
- **Lazy Loading**: Only loads accessed pages
- **OS Cache**: Leverages OS page cache

**Why memmap2**:
- Pure Rust implementation
- Cross-platform (Windows, Linux, macOS)
- No unsafe code required for basic usage
- Well-maintained, active development
- Handles edge cases (file growth, locking, etc.)

---

## Data Structures

### Archive Format

**Purpose**: Efficient storage and retrieval of encrypted asset files

**Structure**:
```
[ Archive Header ]  (variable, ~128 bytes)
    Magic: 8 bytes (b"MAXION\x01\x00")
    Version: 4 bytes (u32, currently 1)
    File Count: 4 bytes (u32)
    File Table Offset: 8 bytes (u64)
    File Table Size: 8 bytes (u64)
    Header Checksum: 32 bytes (BLAKE3)
    Chunk Size: 4 bytes (u32)
    Compress: 4 bytes (u32, 0/1)

[ File Table ]  (variable, depends on file count)
    For each file:
        Path Length: 4 bytes (u32)
        Path: N bytes (UTF-8 string)
        Original Size: 8 bytes (u64)
        Packed Size: 8 bytes (u64)
        Offset: 8 bytes (u64)
        Chunk Count: 4 bytes (u32)
        Modified: 8 bytes (u64)
        Checksum: 32 bytes (BLAKE3)

[ Encrypted Data ]  (variable, depends on total asset size)
    Chunk 0: Encrypted + Authenticated (variable)
    Chunk 1: Encrypted + Authenticated (variable)
    ...
    Chunk N: Encrypted + Authenticated (variable)
```

**Header Constants**:
```rust
const MAGIC: &[u8; 8] = b"MAXION\x01\x00";
const ARCHIVE_VERSION: u32 = 1;
const HEADER_SIZE: usize = 128;  // Approximate size
```

**File Table Entry**:
```rust
pub struct AssetFile {
    pub path: PathBuf,           // Relative path (e.g., "assets/texture.png")
    pub original_size: u64,      // Original file size (before compression)
    pub packed_size: u64,        // Encrypted size (after compression + encryption)
    pub offset: u64,             // Offset in encrypted data section
    pub chunk_count: u32,        // Number of chunks
    pub modified: u64,           // Modification time (Unix timestamp)
    pub checksum: [u8; 32],      // BLAKE3 checksum
}
```

**Why This Format**:
- **Efficient Metadata**: O(1) file lookup via hash map
- **Random Access**: Read any file without decrypting entire archive
- **Integrity**: Per-file checksums detect corruption
- **Chunking**: Enables parallel decryption and caching
- **Compressible**: Reduces storage overhead

### Configuration

**Purpose**: Type-safe, validated configuration for packer and runtime

**Structure**:
```rust
pub struct Config {
    pub chunk_size: ChunkSize,           // Chunk size (default: 64KB)
    pub compress: bool,                  // Enable compression (default: true)
    pub compression_level: u32,          // Compression level (default: 6)
    pub build_secret: [u8; 32],          // Build secret (for key derivation)
    pub nonce: [u8; 24],                // Nonce for encryption
    pub encryption_key: [u8; 32],       // Encryption key (or derived)
    pub simd_config: Option<SimdConfig>, // SIMD configuration
}
```

**Builder Pattern**:
```rust
impl Config {
    pub fn new() -> Self { /* ... */ }
    
    pub fn with_compression(mut self, enabled: bool, level: u32) -> Self {
        self.compress = enabled;
        self.compression_level = level.min(11);
        self
    }
    
    pub fn with_chunk_size(mut self, size: u32) -> Self {
        self.chunk_size = ChunkSize::new(size);
        self
    }
    
    pub fn with_simd_auto(mut self) -> Self {
        self.simd_config = Some(SimdConfig::auto());
        self
    }
    
    pub fn with_simd_enabled(mut self) -> Self {
        self.simd_config = Some(SimdConfig::enabled());
        self
    }
    
    pub fn with_simd_disabled(mut self) -> Self {
        self.simd_config = Some(SimdConfig::disabled());
        self
    }
}
```

**Usage**:
```rust
let config = Config::new()
    .with_compression(true, 6)
    .with_chunk_size(65536)
    .with_simd_auto();

// Generate random keys
let mut config = Config::new();
config.generate_keys();

// Or derive key from password
let mut config = Config::new();
config.build_secret.copy_from_slice(password.as_bytes());
config.derive_key()?;
```

**Serialization** (via serde):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub chunk_size: u32,
    pub compress: bool,
    pub compression_level: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub build_secret: [u8; 32],
    #[serde(skip_serializing_if = "is_zero")]
    pub nonce: [u8; 24],
    #[serde(skip_serializing_if = "is_zero")]
    pub encryption_key: [u8; 32],
}

fn is_zero<T: AsRef<[u8]>>(data: &T) -> bool {
    data.as_ref().iter().all(|&b| b == 0)
}
```

**Why Builder Pattern**:
- Type-safe configuration
- Fluent API for readability
- Validation on construction
- Extensible (add new methods easily)
- Compatible with serde serialization

---

# Jargon Glossary

## Cryptography

- **AEAD (Authenticated Encryption with Associated Data)**: Encryption scheme that provides both confidentiality and integrity/authentication. Combines encryption with authentication tag.
- **ChaCha20**: Stream cipher designed by Daniel J. Bernstein. 256-bit key, 64-byte blocks, 20 rounds. Faster than AES in software.
- **Poly1305**: Message authentication code (MAC) designed by Daniel J. Bernstein. 16-byte tag, used with ChaCha20 for AEAD.
- **Nonce (Number Used Once)**: Unique value used in encryption to ensure that same plaintext encrypted with same key produces different ciphertext. Critical for security.
- **XChaCha20**: Extended nonce variant of ChaCha20 that accepts 24-byte nonces instead of 12-byte. Safer for random nonce generation.
- **Argon2**: Password hashing algorithm (Argon2i, Argon2d, Argon2id). Memory-hard KDF resistant to GPU/ASIC attacks.
- **KDF (Key Derivation Function)**: Function that derives cryptographic keys from a secret (password, master key).
- **BLAKE3**: Fast cryptographic hash function, successor to BLAKE2. 256-bit output, parallelizable, Merkle tree structure.
- **Hash Function**: One-way function that maps data of arbitrary size to fixed-size output. Used for integrity verification and key derivation.

## Compression

- **Brotli**: General-purpose compression algorithm developed by Google. Combines LZ77, Huffman coding, and context modeling. RFC 7932.
- **LZ77**: Sliding window compression algorithm. Replaces repeated patterns with references to previous occurrences.
- **Huffman Coding**: Lossless data compression algorithm. Assigns shorter codes to more frequent symbols.
- **Compression Ratio**: Ratio of compressed size to original size. Lower ratio = better compression.
- **Entropy**: Measure of randomness or unpredictability in data. Higher entropy = harder to compress.

## PE (Portable Executable)

- **PE File**: Windows executable file format (.exe, .dll). Contains DOS header, PE headers, section headers, and section data.
- **RVA (Relative Virtual Address)**: Offset from image base address. Used for addressing within PE file.
- **VA (Virtual Address)**: Absolute address in process memory space.
- **Image Base**: Preferred load address for PE file. Usually 0x00400000 for 32-bit, 0x0000000100000000 for 64-bit.
- **Section**: Contiguous block of code or data in PE file (.text, .data, .rdata, etc.).
- **Entry Point**: Address where execution begins after PE file is loaded.
- **Base Relocations**: Data that allows loader to fix up absolute addresses if loaded at different address.
- **Import Table**: List of DLLs and functions that PE file depends on.
- **IAT (Import Address Table)**: Array of function pointers filled by Windows loader with actual addresses of imported functions.
- **Export Table**: List of functions that DLL makes available for other modules to import.
- **ASLR (Address Space Layout Randomization)**: Security feature that randomizes memory addresses to prevent exploit predictability.

## Performance

- **SIMD (Single Instruction, Multiple Data)**: Parallel processing technique where single instruction operates on multiple data points simultaneously.
- **SSE2 (Streaming SIMD Extensions 2)**: 128-bit SIMD instruction set for x86 processors. Introduced in Pentium 4 (2001).
- **AVX2 (Advanced Vector Extensions)**: 256-bit SIMD instruction set for x86 processors. Introduced in Haswell (2013).
- **AVX-512**: 512-bit SIMD instruction set for x86 processors. Introduced in Xeon Skylake (2016).
- **NEON**: SIMD instruction set for ARM processors. 128-bit registers.
- **CPUID**: CPU instruction that returns information about processor features (including SIMD support).
- **Memory Mapping (mmap)**: Technique that maps file into virtual memory address space. OS handles paging automatically.
- **Zero-Copy**: Data transfer technique that avoids copying data between buffers. Reduces CPU overhead.
- **LRU Cache**: Least Recently Used cache. Evicts least recently accessed items when capacity reached.

## Rust

- **no_std**: Rust compilation mode that doesn't link to standard library. Used for embedded systems or constrained environments.
- **alloc**: Rust crate that provides heap allocation without full std library. Used in no_std environments.
- **Arc (Atomically Reference Counted)**: Thread-safe reference counting pointer. Enables shared ownership across threads.
- **Mutex**: Mutual exclusion primitive. Ensures only one thread can access data at a time.
- **RwLock (Read-Write Lock)**: Lock that allows multiple readers or single writer.
- **Send**: Trait that indicates type can be safely transferred between threads.
- **Sync**: Trait that indicates type can be safely shared between threads.
- **Trait**: Rust's way of defining shared behavior. Similar to interfaces in other languages.
- **Generics**: Rust's way of writing code that works with multiple types. Type-safe and zero-cost.
- **Result<T, E>**: Enum type for error handling. Either Ok(T) or Err(E).
- **Option**: Enum type for optional values. Either Some(T) or None.

## Windows

- **PEB (Process Environment Block)**: Data structure in Windows process memory that contains process information.
- **TEB (Thread Environment Block)**: Data structure in Windows thread memory that contains thread information.
- **DLL (Dynamic-Link Library)**: Windows library format that contains code and data that can be used by multiple programs.
- **API Hooking**: Technique that intercepts function calls and redirects them to custom implementation.
- **Hot Patching**: Technique that patches code at runtime without restarting process.
- **LoadLibrary**: Windows API that loads a DLL into process memory.
- **GetProcAddress**: Windows API that returns address of exported function from loaded DLL.
- **QueryPerformanceCounter**: Windows API that returns high-resolution timer value. Used for precise timing.
- **RAII (Resource Acquisition Is Initialization)**: Programming idiom where resource acquisition and release tied to object lifetime.

---

# Architecture Patterns

## Trait-Based Design

**Purpose**: Enable polymorphism and extensibility while maintaining type safety.

**Example - EncryptionContext**:
```rust
pub trait EncryptionContext: Send + Sync {
    fn encrypt_chunk(&self, plaintext: &[u8], chunk_index: u32) -> Result<Vec<u8>>;
    fn decrypt_chunk(&self, ciphertext: &[u8], chunk_index: u32) -> Result<Vec<u8>>;
    fn chunk_size(&self) -> ChunkSize;
}

// Multiple implementations possible
pub struct ChunkCipherContext { /* ... */ }
pub struct AesGcmCipherContext { /* ... */ }  // Future implementation

impl EncryptionContext for ChunkCipherContext { /* ... */ }
impl EncryptionContext for AesGcmCipherContext { /* ... */ }

// Generic over trait
pub struct VirtualArchive<E: EncryptionContext> {
    encryption: E,
    // ...
}
```

**Benefits**:
- **Extensibility**: Add new implementations without changing existing code
- **Testability**: Mock implementations for testing
- **Polymorphism**: Runtime or compile-time polymorphism
- **Type Safety**: Compiler ensures all methods implemented

## Builder Pattern

**Purpose**: Construct complex objects step-by-step with validation.

**Example - Config**:
```rust
let config = Config::new()
    .with_compression(true, 6)
    .with_chunk_size(65536)
    .with_simd_auto();
```

**Benefits**:
- **Readability**: Fluent API clearly shows configuration
- **Validation**: Validate at construction time, not at use time
- **Optional Parameters**: Default values for common cases
- **Immutability**: Build once, use many times

## RAII (Resource Acquisition Is Initialization)

**Purpose**: Tie resource lifetime to object scope for automatic cleanup.

**Example - Timer**:
```rust
{
    let _timer = Timer::start("operation");
    do_work();
}  // Timer drops automatically, records timing
```

**Benefits**:
- **Automatic Cleanup**: Resources freed when object goes out of scope
- **Exception Safety**: Resources cleaned up even if error occurs
- **No Memory Leaks**: Drop trait ensures cleanup
- **Simplicity**: No explicit cleanup code needed

## Singleton Pattern (OnceLock)

**Purpose**: Ensure only one instance of global resource exists.

**Example - Global Metrics**:
```rust
static METRICS: OnceLock<Mutex<MetricsCollector>> = OnceLock::new();

pub fn init_metrics(output_path: &str) {
    let collector = MetricsCollector::new(output_path);
    METRICS.set(Mutex::new(collector))
        .expect("Metrics collector already initialized");
}

pub fn record_metric(label: &str, duration: Duration) {
    let metrics = METRICS.get().unwrap();
    metrics.lock().unwrap().record_timing(label, duration);
}
```

**Benefits**:
- **Global Access**: Single point of access to resource
- **Lazy Initialization**: Created only when first needed
- **Thread-Safe**: OnceLock ensures safe initialization
- **No Race Conditions**: Single initialization guaranteed

## Thread-Safe Shared State (Arc<Mutex<T>>)

**Purpose**: Share mutable state across multiple threads safely.

**Example - Thread-Safe Cache**:
```rust
type ThreadSafeCache<K, V> = Arc<Mutex<LruCache<K, V>>>;

let cache: ThreadSafeCache<String, Vec<u8>> = 
    Arc::new(Mutex::new(LruCache::new(100)));

// Thread 1
let cache1 = Arc::clone(&cache);
thread::spawn(move || {
    cache1.lock().unwrap().put("key1".to_string(), vec![1, 2, 3]);
});

// Thread 2
let cache2 = Arc::clone(&cache);
thread::spawn(move || {
    let value = cache2.lock().unwrap().get("key1");
    // ...
});
```

**Benefits**:
- **Shared Ownership**: Multiple references to same data
- **Thread Safety**: Mutex ensures only one thread accesses at a time
- **Interior Mutability**: Modify data even through shared reference
- **Reference Counting**: Automatically cleaned up when all references dropped

---

# API Reference

## maxion-core

### EncryptionContext Trait

```rust
pub trait EncryptionContext: Send + Sync {
    /// Encrypt a chunk of data
    fn encrypt_chunk(&self, plaintext: &[u8], chunk_index: u32) -> Result<Vec<u8>>;
    
    /// Decrypt a chunk of data
    fn decrypt_chunk(&self, ciphertext: &[u8], chunk_index: u32) -> Result<Vec<u8>>;
    
    /// Get the chunk size
    fn chunk_size(&self) -> ChunkSize;
    
    /// Get access control reference
    fn access_control(&self) -> &AccessControl;
    
    /// Get mutable access control reference
    fn access_control_mut(&mut self) -> &mut AccessControl;
}
```

### ChunkCipher

```rust
impl ChunkCipher {
    /// Create new chunk cipher
    pub fn new(key: &[u8; 32], nonce: &[u8; 24], chunk_size: ChunkSize) -> Self;
    
    /// Encrypt single chunk
    pub fn encrypt_single(&self, plaintext: &[u8], nonce: &Nonce) -> Result<Vec<u8>>;
    
    /// Decrypt single chunk
    pub fn decrypt_single(&self, ciphertext: &[u8], nonce: &Nonce) -> Result<Vec<u8>>;
    
    /// Encrypt all chunks
    pub fn encrypt_all(&self, data: &[u8]) -> Result<Vec<Vec<u8>>>;
    
    /// Decrypt all chunks
    pub fn decrypt_all(&self, chunks: &[Vec<u8>]) -> Result<Vec<u8>>;
}
```

### Compression

```rust
/// Compress data with Brotli
pub fn compress(data: &[u8], level: u32) -> Result<Vec<u8>>;

/// Decompress data
pub fn decompress(data: &[u8], expected_size: Option<usize>) -> Result<Vec<u8>>;

/// Compress in parallel
pub fn compress_parallel(data: &[u8], config: &ParallelCompressionConfig) -> Result<ParallelCompressionResult>;

/// Decompress in parallel
pub fn decompress_parallel(data: &[u8], chunk_size: usize) -> Result<Vec<u8>>;
```

### VirtualArchive Trait

```rust
pub trait VirtualArchive: Send + Sync {
    /// Read file into buffer, returns bytes read
    fn read_file(&self, path: &str, buffer: &mut [u8]) -> Result<usize>;
    
    /// Get file size
    fn get_file_size(&self, path: &str) -> Result<u64>;
    
    /// Check if file exists
    fn file_exists(&self, path: &str) -> bool;
    
    /// List all files
    fn list_files(&self) -> Result<Vec<String>>;
}
```

### Protected&lt;T&gt;

**Purpose**: Honeypot-protected value with automatic tamper detection

```rust
impl<T: Protectable> Protected<T> {
    /// Create new protected value with initial value
    pub fn new(val: T) -> Self;
    
    /// Get value (performs tamper check)
    /// 
    /// This method:
    /// - Decrypts the real value using current key
    /// - Reads the trap value (volatile, unoptimized)
    /// - Compares both values
    /// - Returns real value if match, triggers detection if mismatch
    pub fn get(&self) -> T;
    
    /// Set new value (rotates encryption key)
    /// 
    /// This method:
    /// - Generates new random encryption key
    /// - Encrypts new value with new key
    /// - Updates both trap and real values
    pub fn set(&self, val: T);
    
    /// Get value without tamper check (unsafe, testing only)
    pub unsafe fn get_unchecked(&self) -> T;
    
    /// Set only real value (unsafe, testing only)
    pub unsafe fn set_real_only(&self, val: T);
}
```

**Supported Types**:
- `i32`, `i64`, `u32`, `u64`
- `f32`
- `(f32, f32, f32)` - For 3D coordinates

**Performance**: ~78x overhead compared to regular values

### ProtectedSync&lt;T&gt;

**Purpose**: Thread-safe version of Protected&lt;T&gt; using Mutex

```rust
impl<T: Protectable + Send + 'static> ProtectedSync<T> {
    /// Create new thread-safe protected value
    pub fn new(val: T) -> Self;
    
    /// Get value (thread-safe with tamper check)
    pub fn get(&self) -> T;
    
    /// Set new value (thread-safe with key rotation)
    pub fn set(&self, val: T);
}
```

**Traits Implemented**:
- `Clone` - Creates new instance with same value

### CheatDetector

**Purpose**: Handles cheat detection actions and tracking

```rust
impl CheatDetector {
    /// Create new cheat detector with default settings
    pub const fn new() -> Self;
    
    /// Initialize with custom settings
    pub fn init(action: CheatAction, max_detections: u32);
    
    /// Report cheat detection (triggers configured action)
    pub fn report_cheat(&self);
    
    /// Get current detection count
    pub fn detection_count(&self) -> u32;
    
    /// Reset detection count (testing only)
    #[cfg(test)]
    pub fn reset(&mut self);
}
```

### CheatAction

**Purpose**: Actions to take when cheat is detected

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheatAction {
    /// Panic immediately (development/testing)
    Panic,
    
    /// Log the detection (production, default)
    #[default]
    Log,
    
    /// Crash randomly to confuse cheaters
    RandomCrash,
    
    /// Flag account for review (multiplayer)
    FlagAccount,
}
```

### Trap Configuration

```rust
/// Enable or disable trap checking globally
pub fn set_trap_enabled(enabled: bool);

/// Check if trap checking is enabled
pub fn is_trap_enabled() -> bool;
```

## maxion-injector

### PeInjector

```rust
impl PeInjector {
    /// Create new injector
    pub fn new(
        pe_path: PathBuf,
        protected_path: PathBuf,
        archive_data: Vec<u8>,
        encryption_key: [u8; 32],
        nonce: [u8; 24],
    ) -> Self;
    
    /// Inject with Phase 1 stub loader
    pub fn inject(&mut self) -> Result<()>;
    
    /// Inject with Phase 2 DLL embedding
    pub fn inject_with_dll(&mut self) -> Result<()>;
    
    /// Inject with full DLL embedding (recommended)
    pub fn inject_full_dll(&mut self) -> Result<()>;
}
```

## maxion-profiler

### Timer

```rust
impl Timer {
    /// Start new timer
    pub fn start(label: &str) -> Self;
    
    /// Stop timer and record duration
    pub fn stop(self) -> Duration;
    
    /// Stop with custom label
    pub fn stop_with_label(mut self, label: &str) -> Duration;
    
    /// Get elapsed without stopping
    pub fn duration(&self) -> Duration;
}
```

### MetricsCollector

```rust
impl MetricsCollector {
    /// Create new collector
    pub fn new(output_path: &str) -> Self;
    
    /// Record timing
    pub fn record_timing(&mut self, label: &str, duration: Duration);
    
    /// Record counter
    pub fn record_counter(&mut self, label: &str, value: u64);
    
    /// Record file load metric
    pub fn record_file_load(&mut self, metric: FileLoadMetric);
    
    /// Flush to JSON file
    pub fn flush(&self) -> anyhow::Result<()>;
}
```

## maxion-stub (C API)

### Initialization

```c
// Initialize with executable path
bool maxion_init(const char* executable_path);

// Initialize with custom configuration
bool maxion_init_with_config(const MaxionConfig* config);

// Shutdown and cleanup
void maxion_shutdown(void);
```

### File Operations

```c
// Read file into buffer
size_t maxion_read_file(const char* path, void* buffer, size_t size);

// Check if file exists
bool maxion_file_exists(const char* path);

// Get file size
size_t maxion_get_file_size(const char* path);

// Read file into allocated buffer (caller must free)
void* maxion_read_file_alloc(const char* path, size_t* out_size);
```

### Cache Management

```c
// Preload files into cache
void maxion_preload(const char** paths, size_t count);

// Clear cache
void maxion_clear_cache(void);

// Get cache statistics
void maxion_get_cache_stats(CacheStats* stats);
```

### Profiling

```c
// Enable/disable profiling
void maxion_enable_profiling(bool enable);

// Get performance report (JSON string, caller must free)
const char* maxion_get_performance_report(void);
```

---

# Performance Optimization

## Compression Optimization

**Use Parallel Compression for Large Files**:
```rust
// For files > 1MB, use parallel compression
if data.len() > 1_000_000 {
    let config = ParallelCompressionConfig {
        level: 6,
        num_threads: None,  // Auto-detect
        chunk_size: 256_000,  // 256KB chunks
    };
    compress_parallel(&data, &config)?
} else {
    compress(&data, 6)?
}
```

**Tune Compression Level**:
```rust
// Development builds: Faster compression
let level = if cfg!(debug_assertions) { 1 } else { 6 };

// Release builds: Better compression
let level = 6;

// Distribution builds: Maximum compression
let level = 11;
```

**Why**: Parallel compression reduces compression time by 4-8x on multi-core systems for large files.

## Encryption Optimization

**Use SIMD-Accelerated Crypto**:
```rust
// Auto-detect and enable SIMD
let config = Config::new().with_simd_auto();
```

**Why**: SIMD provides 4-12x speedup for ChaCha20-Poly1305 operations.

**Chunk Size Tuning**:
```rust
// Default: 64KB - good balance
let chunk_size = ChunkSize::new(64 * 1024);

// For large files: 256KB - fewer chunks, less overhead
let chunk_size = ChunkSize::new(256 * 1024);

// For small files: 16KB - better cache utilization
let chunk_size = ChunkSize::new(16 * 1024);
```

**Why**: Larger chunks reduce overhead but reduce granularity. 64KB is optimal for most use cases.

## Caching Optimization

**Preload Common Assets**:
```c
// Preload frequently accessed assets
const char* preload_paths[] = {
    "assets/main_menu.png",
    "assets/ui_font.ttf",
    "assets/sounds/button_click.wav",
    NULL
};
maxion_preload(preload_paths, 3);
```

**Why**: Preloading reduces first-access latency and improves user experience.

**Tune Cache Size**:
```rust
// Default: 100 entries
let cache = LruCache::new(100);

// For memory-constrained systems: 50 entries
let cache = LruCache::new(50);

// For high-performance systems: 500 entries
let cache = LruCache::new(500);
```

**Why**: Larger cache reduces misses but uses more memory. Tune based on available RAM.

## I/O Optimization

**Use Memory Mapping for Large Files**:
```rust
// For files > 10MB, use memory mapping
let data = if file_size > 10_000_000 {
    read_file_zero_copy(path)?  // Mmap
} else {
    read_file(path)?  // Regular read
};
```

**Why**: Memory mapping is 10-100x faster for large files due to zero-copy and OS caching.

**Optimal Buffer Size**:
```rust
// Default: 4KB (matches page size)
let buffer_size = 4096;

// For SSDs: 64KB - matches SSD block size
let buffer_size = 65536;

// For HDDs: 1MB - sequential reads
let buffer_size = 1_048_576;
```

**Why**: Buffer size should match device block size for optimal performance.

---

## Document Information

**Last Updated**: 2025-01-24  
**Version**: 1.0.0  
**Maintained By**: Maxion Protector Team  
**Complexity**: Advanced  
**Target Audience**: System architects, senior developers, security engineers

---

## Related Documentation

- [Architecture Overview](../README.md) - High-level system architecture
- [Security Documentation](../../06_security/README.md) - Security architecture and threat model
- [Performance Benchmarks](../../05_benchmark/README.md) - Detailed performance analysis
- [Implementation Details](../../02_implementation/README.md) - Technical implementation guide

---

**End of Technical Reference**