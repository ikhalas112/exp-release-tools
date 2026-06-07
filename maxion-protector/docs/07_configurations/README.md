# Smart Defaults for File Protection

**Version**: 1.0.0  
**Status**: Stable  
**Last Updated**: 2025-01-15

---

## Overview

Maxion Packer's **Smart Defaults** feature automatically determines the optimal protection and compression strategy for each file based on its type and size, following recommendations from extensive benchmark analysis.

This feature ensures:

- ✅ **Maximum performance**: Files are compressed only when beneficial
- ✅ **Maximum security**: All protected files are encrypted
- ✅ **Transparent operation**: No manual configuration needed for typical workloads
- ✅ **Flexible overrides**: Customize behavior when needed

---

## How It Works

### Three Protection Strategies

Smart defaults categorizes each file into one of three protection strategies:

#### 1. **Protect + Compress** (📦)
- **What**: Encrypt and compress the file
- **When to use**: Compressible formats (configs, scripts, uncompressed assets)
- **Benefit**: 88-99.9% space savings, faster loading

#### 2. **Protect Only** (🔒)
- **What**: Encrypt without compression
- **When to use**: Already-compressed formats or very large files
- **Benefit**: Avoids wasting CPU time on incompressible data

#### 3. **Skip** (⏭️)
- **What**: Exclude file from protection entirely
- **When to use**: Files that shouldn't be protected (logs, temporary files)
- **Benefit**: Faster build times for non-essential files

---

## File Type Categorization

### Always Compress (Protect + Compress)

These formats achieve **88-99.9% space savings** and load **2.5-4x faster**:

#### Configuration Files
- `json`, `xml`, `toml`, `yaml`, `yml`, `ini`, `cfg`, `conf`, `properties`
- **Why**: Text-based, highly compressible
- **Benchmark**: 88% space savings, 2.5x faster

#### Script Files
- `js`, `ts`, `py`, `lua`, `cs`, `c`, `cpp`, `h`, `hpp`, `rs`, `go`, `java`, `kt`, `swift`, `rb`, `php`, `sh`, `bat`, `ps1`
- **Why**: Plain text, compressible
- **Benchmark**: 90-99% space savings

#### Uncompressed Textures
- `bmp`, `tga`, `dds`, `psd`, `tif`, `tiff`, `pnm`, `pbm`, `pgm`, `ppm`
- **Why**: Uncompressed pixel data
- **Benchmark**: 99.9% space savings, 3.8x faster

#### Uncompressed Audio
- `wav`, `aiff`, `au`, `raw`, `pcm`
- **Why**: Raw audio data
- **Benchmark**: 90-95% space savings

#### 3D Models & Assets
- `obj`, `fbx`, `gltf`, `glb`, `dae`, `blend`, `ma`, `mb`, `max`
- **Why**: Text-based or uncompressed data
- **Benchmark**: 95-99% space savings

#### Text & Data Files
- `txt`, `md`, `csv`, `log`, `data`, `bin`, `dat`, `db`, `sqlite`, `sql`
- **Why**: Compressible data formats
- **Benchmark**: 80-99% space savings

### Protect Only (No Compression)

These formats are already compressed or don't benefit from it:

#### Compressed Images
- `png`, `jpg`, `jpeg`, `webp`, `avif`, `heic`, `heif`
- **Why**: Already compressed
- **Result**: 0-5% additional savings possible

#### Video Files
- `mp4`, `m4v`, `mov`, `avi`, `wmv`, `mkv`
- **Why**: Already heavily compressed
- **Result**: <1% additional savings possible

#### Compressed Audio
- `ogg`, `oga`, `mp3`, `m4a`, `flac`
- **Why**: Already compressed
- **Result**: 10-20% additional savings possible

#### Archives
- `zip`, `rar`, `7z`, `gz`, `bz2`, `xz`, `lzma`, `zst`
- **Why**: Already compressed
- **Result**: Incompressible

#### Large Files (>100MB)
- Any file > 100MB
- **Why**: Compression time dominates
- **Result**: Better to skip compression

---

## CLI Usage

### Basic Usage (Smart Defaults Enabled)

Smart defaults are **enabled by default**:

```bash
# Pack with smart defaults
maxion-packer pack \
  --assets ./assets \
  --output game.archive
```

Output:
```
Maxion Packer v0.1.0

Protection Mode: Smart Defaults (file type-based)
Chunk size: 65536 bytes
SIMD Configuration: auto

Scanning assets...
Found 150 files in 0.02s
Total uncompressed size: 250 MB

Processing assets...
✓ Packing complete!

📊 Final Statistics:
Total compressed size: 5 MB
Compression ratio: 98.00%
Space saved: 245 MB
  ✅ Compressed: 120 files
  ⚠️  Protected only: 30 files
```

### Verification Mode

Use `--verify` to see what will be protected/compressed before processing:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --verify
```

Output:
```
📋 File Protection Verification
================================================================================

📊 Summary:
  Total files: 150
  Total size: 250.00 MB

  ✅ Protect + Compress: 120 files (200.00 MB, 80.0%)
  ⚠️  Protect Only: 30 files (50.00 MB, 20.0%)
  ⏭️  Skipped: 0 files (0.00 MB, 0.0%)

📦 Files to be Protected and Compressed:
  ✅ Protect + Compress config/settings.json (5.00 KB)
  ✅ Protect + Compress scripts/game.js (25.00 KB)
  ✅ Protect + Compress textures/hero.dds (512.00 KB)
  ...

🔒 Files to be Protected Only (no compression):
  ⚠️  Protect Only textures/background.png (2.50 MB)
  ⚠️  Protect Only video/intro.mp4 (25.00 MB)
  ...

Press Enter to continue or Ctrl+C to cancel...
```

### Override Flags

#### Compress All Files

Force compression for everything:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --compress-all
```

**Use when**: You want maximum compression regardless of file type

#### Protect Only (No Compression)

Disable compression entirely:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --compress-none
```

**Use when**: You want encryption only (fastest build, larger size)

#### Compress Specific Extensions

Compress only certain file types:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --compress-types "json,xml,lua,dds"
```

**Use when**: You know exactly which formats benefit from compression

#### Exclude Extensions from Compression

Exclude certain types from compression:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --no-compress-types "png,jpg,mp4"
```

**Use when**: Smart defaults incorrectly categorizes a file type

#### Protect Only Specific Types

Protect only certain extensions, skip everything else:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --protect-only-types "json,xml,lua,dds,wav"
```

**Use when**: Only want to protect specific file types (e.g., just configs)

#### Skip Specific Extensions

Skip certain files entirely:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --skip-types "log,tmp,bak"
```

**Use when**: Don't want to protect temporary files or logs

### Disable Smart Defaults

Disable smart defaults and use manual configuration:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --no-smart-defaults \
  --compress-types "json,xml" \
  --no-compress-types "png,mp4"
```

---

## Use Cases

### Typical Game Development (Recommended)

Use smart defaults for most projects:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --verify
```

**Result**: Optimal balance of compression and performance

### Large Game with Mostly Compressed Assets

If your game uses mostly PNG/JPEG textures and MP4 videos:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --compress-types "json,xml,lua" \
  --no-compress-types "png,jpg,mp4,ogg"
```

**Result**: Only compress configs and scripts, protect everything else

### Debug Build (Fastest)

For fast iteration during development:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --compress-none
```

**Result**: No compression, encryption only, fastest build

### Production Build (Smallest)

For final release (longer build time):

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --compress-all
```

**Result**: Maximum compression, smallest download size

### Custom File Types

If you have custom file extensions:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --compress-types "myformat,custom1" \
  --no-compress-types "largearchive"
```

**Result**: Full control over which formats are compressed

---

## Performance Impact

### Benchmarks

Based on real-world game asset benchmarks:

| Strategy | Build Time | Archive Size | Load Time |
|----------|------------|--------------|------------|
| Smart Defaults | Baseline | 2% of original | 1.65x faster |
| Compress All | +50% | 1.5% of original | 1.4x faster |
| Protect Only | -60% | 100% of original | 1.0x (same) |
| No Protection | -70% | 100% of original | 1.0x (same) |

### Recommendations

#### Use Smart Defaults When:
- ✅ Developing typical games
- ✅ Mixed file types (configs, textures, audio, models)
- ✅ Want good balance of size and performance
- ✅ Don't want to manually configure each file type

#### Use Compress All When:
- ✅ Maximum download size reduction is critical
- ✅ Build time is not a concern
- ✅ All assets are compressible (no PNG/JPEG/MP4)

#### Use Protect Only When:
- ✅ Build speed is critical (debug builds)
- ✅ Most assets are already compressed
- ✅ Don't care about archive size

#### Use No Protection When:
- ✅ Development/testing (need quick iteration)
- ✅ Assets don't need protection
- ✅ Profiling and debugging

---

## API Reference

### CLI Flags

| Flag | Type | Default | Description |
|------|------|----------|-------------|
| `--smart-defaults` | bool | `true` | Enable smart defaults (file type-based) |
| `--compress-all` | bool | `false` | Compress all files (overrides smart defaults) |
| `--compress-none` | bool | `false` | Disable compression (overrides smart defaults) |
| `--compress-types` | string | none | Compress only these extensions (comma-separated) |
| `--no-compress-types` | string | none | Don't compress these extensions (comma-separated) |
| `--protect-only-types` | string | none | Protect only these extensions (comma-separated) |
| `--skip-types` | string | none | Skip these extensions (comma-separated) |
| `--verify` | bool | `false` | Show what will be protected before processing |

### Flag Priority

Flags are evaluated in this order:

1. **`--skip-types`**: Skip matching files entirely
2. **`--protect-only-types`**: Only protect matching files, skip others
3. **`--compress-types`**: Force compression for matching files
4. **`--no-compress-types`**: Exclude matching files from compression
5. **Smart defaults**: Use file type-based recommendations

**Example**: If a file is in both `--compress-types` and `--no-compress-types`:
- `--compress-types` takes priority (higher in list)

---

## Best Practices

### 1. Always Use `--verify` First Time

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --verify
```

Review the list and adjust flags as needed before committing to the build.

### 2. Profile Your Workload

Run with smart defaults, then with `--compress-all` and compare:

```bash
# Test smart defaults
maxion-packer pack --assets ./assets --output test1.archive

# Test compress all
maxion-packer pack --assets ./assets --output test2.archive --compress-all

# Compare sizes
ls -lh test*.archive
```

### 3. Use Extension Lists for Custom Formats

If you have proprietary formats, add them explicitly:

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --compress-types "myformat1,myformat2" \
  --no-compress-types "archiveformat"
```

### 4. Separate Debug and Release Configurations

For development (fast builds):
```bash
# debug_build.sh
maxion-packer pack \
  --assets ./assets \
  --output game_debug.archive \
  --compress-none
```

For release (smallest size):
```bash
# release_build.sh
maxion-packer pack \
  --assets ./assets \
  --output game_release.archive \
  --smart-defaults \
  --compression-level 11
```

### 5. Keep Logs Out of Archive

```bash
maxion-packer pack \
  --assets ./assets \
  --output game.archive \
  --skip-types "log,tmp,bak"
```

Logs and temporary files don't need protection and slow down builds.

---

## Troubleshooting

### Issue: File Not Compressing When Expected

**Symptom**: File shows as "Protect Only" instead of "Protect + Compress"

**Solution**: Check file extension:
```bash
# Verify extension
file mytexture.texture

# Force compress if needed
maxion-packer pack \
  --compress-types "texture" \
  --assets ./assets \
  --output game.archive
```

### Issue: Build Too Slow

**Symptom**: Smart defaults taking too long

**Solution**: Reduce compression or disable it:
```bash
# Lower compression level
maxion-packer pack \
  --compression-level 3 \
  --assets ./assets \
  --output game.archive

# Or disable compression
maxion-packer pack \
  --compress-none \
  --assets ./assets \
  --output game.archive
```

### Issue: Archive Too Large

**Symptom**: Archive size not much smaller than original

**Solution**: Check what's being compressed:
```bash
maxion-packer pack \
  --verify \
  --assets ./assets \
  --output game.archive
```

Look for files marked "Protect Only" - they may already be compressed (PNG, JPEG, MP4).

### Issue: Wrong File Categorization

**Symptom**: File type incorrectly categorized

**Solution**: Use explicit flags to override:
```bash
# Force compression
maxion-packer pack \
  --compress-types "mytype" \
  --assets ./assets \
  --output game.archive

# Or exclude from compression
maxion-packer pack \
  --no-compress-types "mytype" \
  --assets ./assets \
  --output game.archive
```

---

## Summary

Smart defaults provide:

✅ **Optimal performance**: Files compressed only when beneficial  
✅ **Maximum security**: All protected files are encrypted  
✅ **Transparent operation**: Works out-of-the-box for most games  
✅ **Flexible control**: Override behavior when needed  
✅ **Verified results**: Check what will be protected before building  

**Recommended for**: Most game development workflows

**Default behavior**: Enabled automatically

**Learn more**: See [benchmark results](../05_benchmark/01_protected_vs_unprotected.md) for detailed performance analysis.

---

## Quick Reference

```bash
# Basic usage (smart defaults enabled)
maxion-packer pack --assets ./assets --output game.archive

# Verify before processing
maxion-packer pack --assets ./assets --output game.archive --verify

# Compress everything
maxion-packer pack --assets ./assets --output game.archive --compress-all

# No compression (fastest build)
maxion-packer pack --assets ./assets --output game.archive --compress-none

# Compress only specific types
maxion-packer pack --assets ./assets --output game.archive --compress-types "json,xml,lua"

# Exclude from compression
maxion-packer pack --assets ./assets --output game.archive --no-compress-types "png,jpg,mp4"

# Protect only specific types
maxion-packer pack --assets ./assets --output game.archive --protect-only-types "json,xml"

# Skip specific files
maxion-packer pack --assets ./assets --output game.archive --skip-types "log,tmp,bak"

# Disable smart defaults
maxion-packer pack --assets ./assets --output game.archive --no-smart-defaults
```

---

**Next Steps**:
- [Run benchmarks](../05_benchmark/) to verify performance with your assets
- [View CLI reference](../03_cli_reference/) for all available options
- [Examples](../02_examples/) for common use cases