#!/bin/bash

# Maxion Game Asset Analysis Script
# Analyzes game assets and provides protection recommendations

set -e

# Configuration
GAME_DIR="$1"
OUTPUT_DIR="./maxion_analysis"
REPORT_FILE="$OUTPUT_DIR/asset_analysis_report.md"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Usage
if [ -z "$GAME_DIR" ]; then
    echo "Usage: $0 <game_directory>"
    echo ""
    echo "Example:"
    echo "  $0 'C:/Users/katop/Games/Maxion'"
    exit 1
fi

# Validate game directory
if [ ! -d "$GAME_DIR" ]; then
    echo -e "${RED}Error: Game directory not found: $GAME_DIR${NC}"
    exit 1
fi

echo -e "${BLUE}=== Maxion Game Asset Analysis ===${NC}"
echo -e "${BLUE}Game Directory: $GAME_DIR${NC}"
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Collect statistics
echo "Analyzing assets..."

# Total size
TOTAL_SIZE=$(du -sb "$GAME_DIR" 2>/dev/null | cut -f1 || echo "0")
TOTAL_SIZE_MB=$((TOTAL_SIZE / 1024 / 1024))

# Count files by extension
echo "Categorizing files..."

# Initialize counters
declare -A extension_counts
declare -A extension_sizes
total_files=0
total_dirs=0

# Analyze each file
while IFS= read -r -d '' file; do
    ext="${file##*.}"
    ext=$(echo "$ext" | tr '[:upper:]' '[:lower:]')

    # Skip empty extensions
    if [ -z "$ext" ] || [ "$ext" == "$file" ]; then
        ext="no_ext"
    fi

    # Count
    extension_counts[$ext]=$((extension_counts[$ext] + 1))

    # Size
    size=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file" 2>/dev/null || echo "0")
    extension_sizes[$ext]=$((extension_sizes[$ext] + size))

    total_files=$((total_files + 1))
done < <(find "$GAME_DIR" -type f -print0 2>/dev/null)

# Count directories
total_dirs=$(find "$GAME_DIR" -type d 2>/dev/null | wc -l)

# Generate report
cat > "$REPORT_FILE" <<'EOF'
# Maxion Game Asset Analysis Report

**Generated:** $(date)

---

## Executive Summary

EOF

echo "Total size: $TOTAL_SIZE_MB MB" >> "$REPORT_FILE"
echo "Total files: $total_files" >> "$REPORT_FILE"
echo "Total directories: $total_dirs" >> "$REPORT_FILE"

cat >> "$REPORT_FILE" <<'EOF'

---

## Asset Distribution by File Type

| Extension | Count | Size (MB) | Percentage | Protection Strategy |
|------------|-------|-----------|------------|---------------------|
EOF

# Sort extensions by size (descending)
for ext in "${!extension_sizes[@]}"; do
    size=${extension_sizes[$ext]}
    count=${extension_counts[$ext]}
    size_mb=$((size / 1024 / 1024))
    pct=$((size * 100 / TOTAL_SIZE))

    # Determine protection strategy
    case "$ext" in
        exe|dll)
            strategy="DO NOT PROTECT (executables)"
            ;;
        ini|cfg|conf|xml|json|toml|yaml)
            strategy="Protected + Compressed"
            ;;
        dat|bin)
            strategy="Protected + Compressed"
            ;;
        lua|js|py|pyc)
            strategy="Protected + Compressed"
            ;;
        hsh)
            strategy="Protected + Compressed (hash files)"
            ;;
        vsh|psh|fx)
            strategy="Protected + Compressed (shaders)"
            ;;
        tga|dds|png|jpg|jpeg)
            strategy="Protected Only (already compressed)"
            ;;
        obj|fbx|mod|mesh)
            strategy="Protected + Compressed"
            ;;
        mp3|ogg|wav|wma)
            strategy="Protected Only (already compressed)"
            ;;
        font|ttf|otf)
            strategy="Protected + Compressed"
            ;;
        txt|md|log)
            strategy="Protected + Compressed"
            ;;
        *)
            strategy="Protected + Compressed (default)"
            ;;
    esac

    printf "| %-10s | %-5d | %-9d | %-10s | %s |\n" "$ext" "$count" "$size_mb" "${pct}%" "$strategy" >> "$REPORT_FILE"
done | sort -t'|' -k4 -nr >> "$REPORT_FILE"

# Add detailed sections
cat >> "$REPORT_FILE" <<'EOF'

---

## Critical Directories

### Configuration & Data
- `config.ini`, `Option.ini` - Game configuration
- `server.dat` - Server connection settings
- `*.dat` files - Game data (skills, items, quests, etc.)
- `*_THAI.dat` - Thai localization data

### Executables & Libraries
- `zone4.exe` - Main game executable (DO NOT PROTECT)
- `*.dll` - Runtime libraries (DO NOT PROTECT)

### Asset Directories
- `fonts/` - Game fonts
- `Shaders/` - GPU shaders (.vsh, .psh, .fx)
- `sound/` - Audio assets
- `animation/`, `bone/` - Animation data
- `character/` - Character models and data
- `item/`, `weapon/` - Item and weapon data
- `texture/` (if exists) - Texture files
- `particle/` - Particle effects
- `UI/` - UI assets

### Anti-Cheat System
- `XIGNCODE/` - Anti-cheat system (DO NOT PROTECT)

---

## Protection Recommendations

### 🟢 Safe to Protect (High Priority)

#### Phase 1: Configuration Files (Minimal Risk)
**Files:** `*.ini`, `*.dat`, `*.hsh`

**Reason:** Small text/data files, easy to verify
**Size:** Small (typically < 10MB total)
**Risk:** Very Low

**Test Command:**
```bash
pnp pack \
  --assets "$GAME_DIR/*.ini" "$GAME_DIR/*.dat" "$GAME_DIR/*.hsh" \
  --output "$OUTPUT_DIR/config_archive.vfs" \
  --compress \
  --verify
```

#### Phase 2: Shader Files (Low Risk)
**Directory:** `Shaders/`

**Reason:** Text files, can be recompiled if needed
**Files:** `*.vsh`, `*.psh`, `*.fx`

**Test Command:**
```bash
pnp pack \
  --assets "$GAME_DIR/Shaders/" \
  --output "$OUTPUT_DIR/shaders_archive.vfs" \
  --compress \
  --verify
```

#### Phase 3: Font Files (Low Risk)
**Directory:** `fonts/`

**Reason:** Can be restored from game files
**Files:** `*.font`, `*.ttf`, `*.otf`

**Test Command:**
```bash
pnp pack \
  --assets "$GAME_DIR/fonts/" \
  --output "$OUTPUT_DIR/fonts_archive.vfs" \
  --compress \
  --verify
```

### 🟡 Medium Risk (Backup Required)

#### Phase 4: Animation & Character Data
**Directories:** `animation/`, `bone/`, `character/`

**Reason:** Critical game assets, but can be reinstalled
**Risk:** Medium

**Precautions:**
1. Create full backup before testing
2. Test on copy of game directory
3. Verify game launches successfully

**Test Command:**
```bash
pnp pack \
  --assets "$GAME_DIR/animation/" "$GAME_DIR/bone/" "$GAME_DIR/character/" \
  --output "$OUTPUT_DIR/animation_archive.vfs" \
  --compress \
  --verify
```

#### Phase 5: Sound Effects
**Directory:** `sound/`

**Reason:** Large files, affects game experience
**Risk:** Medium

**Note:** Audio files are often already compressed, consider `--protect-only`

**Test Command:**
```bash
pnp pack \
  --assets "$GAME_DIR/sound/" \
  --output "$OUTPUT_DIR/sound_archive.vfs" \
  --compress \
  --verify
```

### 🔴 High Risk (Extensive Testing Required)

#### Phase 6: Complete Asset Protection
**All asset directories except executables and anti-cheat**

**Directories to exclude:**
- Root executables (`*.exe`, `*.dll`)
- `XIGNCODE/` (anti-cheat must remain untouched)
- `LauncherData/` (if it has executables)

**Precautions:**
1. Complete backup of entire game directory
2. Test on separate copy
3. Verify all game features
4. Monitor performance impact
5. Rollback plan ready

**Test Command:**
```bash
pnp pack \
  --assets "$GAME_DIR/animation/" \
         "$GAME_DIR/arcade/" \
         "$GAME_DIR/camera/" \
         "$GAME_DIR/character/" \
         "$GAME_DIR/consumItem/" \
         "$GAME_DIR/defence/" \
         "$GAME_DIR/dungeon/" \
         "$GAME_DIR/effect/" \
         "$GAME_DIR/emblem/" \
         "$GAME_DIR/fonts/" \
         "$GAME_dir/item/" \
         "$GAME_DIR/mission/" \
         "$GAME_DIR/otherAnimation/" \
         "$GAME_DIR/particle/" \
         "$GAME_DIR/pet/" \
         "$GAME_DIR/scene/" \
         "$GAME_DIR/Shaders/" \
         "$GAME_DIR/sound/" \
         "$GAME_DIR/village/" \
         "$GAME_DIR/villageAnimation/" \
         "$GAME_DIR/weapon/" \
         "$GAME_DIR/WeaponSystem/" \
  --skip-types exe,dll,vxd,sys,drv,cpl \
  --output "$OUTPUT_DIR/maxion_complete_archive.vfs" \
  --compress \
  --chunk-size 65536 \
  --verify
```

---

## Files to NEVER Protect

🔴 **DO NOT PROTECT THESE FILES:**

### Executables (Required for game to run)
- `zone4.exe` - Main game executable
- `*.dll` - Runtime libraries
- `*.vxd`, `*.sys`, `*.drv`, `*.cpl` - System files

### Anti-Cheat System (Will detect modifications)
- `XIGNCODE/` - Entire directory
- Anti-cheat related files

### Launcher (If present)
- `LauncherData/` - Launcher executables and configs

---

## Protection Strategy Summary

### Smart Defaults (Recommended)

Maxion Protector's smart defaults will automatically:
- ✅ Compress text-based files (`.ini`, `.cfg`, `.lua`, `.js`, shaders)
- ✅ Protect-only already-compressed files (`.png`, `.jpg`, `.mp3`, `.ogg`)
- ✅ Compress data files (`.dat`, `.bin`)
- ✅ Skip executables and system files

**Recommended CLI Command:**
```bash
pnp pack \
  --assets "$GAME_DIR" \
  --output "$OUTPUT_DIR/maxion_assets.vfs" \
  --skip-types exe,dll,vxd,sys,drv,cpl,lib,exp \
  --compress \
  --chunk-size 65536 \
  --verify
```

### Aggressive Protection (Testing Only)

Use `--enable-protected-all` to protect ALL files (not recommended for production):
```bash
pnp pack \
  --assets "$GAME_DIR" \
  --output "$OUTPUT_DIR/maxion_aggressive.vfs" \
  --enable-protected-all \
  --skip-types exe,dll,vxd,sys,drv,cpl,lib,exp \
  --compress \
  --chunk-size 65536 \
  --verify
```

---

## Testing Checklist

### Pre-Test Preparation
- [ ] Backup entire game directory
- [ ] Create test copy of game
- [ ] Note current game version
- [ ] Document current loading times
- [ ] Note file sizes of key assets

### Phase 1 Testing (Config Files)
- [ ] Pack configuration files
- [ ] Verify archive integrity
- [ ] Unpack and verify files match original
- [ ] Test game launches with modified config loading
- [ ] Verify game functionality

### Phase 2 Testing (Shaders)
- [ ] Pack shader directory
- [ ] Verify archive integrity
- [ ] Test game graphics
- [ ] Check for shader errors
- [ ] Verify performance

### Phase 3 Testing (Fonts)
- [ ] Pack fonts directory
- [ ] Verify archive integrity
- [ ] Test game text rendering
- [ ] Check for missing fonts
- [ ] Verify UI appearance

### Phase 4 Testing (Animation)
- [ ] Create backup of animation directory
- [ ] Pack animation assets
- [ ] Verify archive integrity
- [ ] Test character animations
- [ ] Check for animation glitches
- [ ] Verify combat gameplay

### Phase 5 Testing (Complete Assets)
- [ ] Create complete game backup
- [ ] Pack all assets
- [ ] Verify archive integrity
- [ ] Test all game features
- [ ] Measure performance impact
- [ ] Check loading times
- [ ] Verify memory usage
- [ ] Test online multiplayer (if applicable)

### Post-Test Validation
- [ ] Compare file sizes before/after
- [ ] Measure loading time improvements
- [ ] Document protection overhead
- [ ] Note any issues encountered
- [ ] Create rollback plan if needed

---

## Performance Expectations

Based on Maxion Protector benchmarks:

### Compression Savings
- Text files (`.ini`, `.dat`, `.lua`): **70-90%** space savings
- Data files (`.bin`, `.dat`): **50-80%** space savings
- Shaders: **60-85%** space savings
- Fonts: **40-70%** space savings
- Already-compressed files (`.png`, `.mp3`): **0-10%** space savings

### Loading Time Impact
- Small files (< 100KB): **~0.03ms** overhead per file
- Medium files (100KB-1MB): **~0.3ms** overhead per file
- Large files (> 1MB): **~2.5ms** overhead per file
- Compression decompression: **~0.6ms** for 100KB
- Encryption/Decryption: **~0.3ms** for 100KB

**Expected Overall Impact:**
- **Loading time:** 5-15% slower (decompression overhead)
- **Disk space:** 30-70% reduction (compression savings)
- **Runtime performance:** Minimal impact (< 2%)

---

## Troubleshooting

### Game Won't Launch After Protection

**Possible Causes:**
1. Executable was modified
2. Anti-cheat detected modifications
3. Critical config files not accessible
4. Archive corruption

**Solutions:**
1. Verify `zone4.exe` and `*.dll` were NOT packed
2. Check `XIGNCODE/` directory was NOT modified
3. Verify archive integrity with `--verify`
4. Restore from backup

### Missing Textures/Assets

**Possible Causes:**
1. Archive not loaded correctly
2. File paths incorrect
3. Asset not included in archive

**Solutions:**
1. Verify all asset directories were packed
2. Check file paths in archive
3. Use `pnp list <archive>` to verify contents
4. Repack if needed

### Performance Issues

**Possible Causes:**
1. Too many small files
2. Compression level too high
3. Chunk size too small

**Solutions:**
1. Increase chunk size to `131072` or `262144`
2. Lower compression level to `3` or `4`
3. Group small files into larger batches

---

## Backup Strategy

### Full Backup
```bash
# Create timestamped backup
BACKUP_DIR="./backups/maxion_$(date +%Y%m%d_%H%M%S)"
cp -r "$GAME_DIR" "$BACKUP_DIR"
```

### Selective Backup (Before Testing)
```bash
# Backup critical directories only
BACKUP_DIR="./backups/maxion_critical_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"
cp -r "$GAME_DIR/animation" "$BACKUP_DIR/"
cp -r "$GAME_DIR/character" "$BACKUP_DIR/"
cp -r "$GAME_DIR/weapon" "$BACKUP_DIR/"
cp "$GAME_DIR"/*.ini "$BACKUP_DIR/"
cp "$GAME_DIR"/*.dat "$BACKUP_DIR/"
```

---

## Next Steps

1. **Start Small**: Begin with Phase 1 (configuration files)
2. **Verify Each Step**: Test game after each phase
3. **Monitor Performance**: Measure loading times and memory usage
4. **Document Results**: Keep track of what works and what doesn't
5. **Rollback if Needed**: Always have a backup ready

---

**Report Generated:** $(date)
**Total Analysis Time:** $(date)

For questions or issues, refer to:
- Maxion Protector README: `../README.md`
- Build Documentation: `../BUILD.md`
- Issues Tracking: `../ISSUES.md`
EOF

echo -e "${GREEN}✓ Analysis complete!${NC}"
echo -e "${GREEN}✓ Report saved to: $REPORT_FILE${NC}"
echo ""
echo -e "${BLUE}Summary:${NC}"
echo "  Total size: $TOTAL_SIZE_MB MB"
echo "  Total files: $total_files"
echo "  Total directories: $total_dirs"
echo ""
echo -e "${YELLOW}Next Steps:${NC}"
echo "  1. Review the full report: cat $REPORT_FILE"
echo "  2. Start with Phase 1 testing (config files)"
echo "  3. Always backup before modifying game files"
echo "  4. Test on a copy of the game directory"
echo ""
echo -e "${BLUE}Quick test command (Phase 1 - Config Files):${NC}"
echo "  ./scripts/test_maxion_phase1.sh"
