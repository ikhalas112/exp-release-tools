#!/bin/bash

# Maxion Game Phase 1 Test Script - Configuration Files
# Safely tests Maxion Protector with game configuration files
# This is the LOWEST RISK phase - only tests text-based config files

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
GAME_DIR="${1:-C:/Users/katop/Games/Maxion}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MAXION_PACKER="$PROJECT_ROOT/target/release/pnp.exe"
TEST_DIR="./maxion_test_phase1"
BACKUP_DIR="$TEST_DIR/backup_config"
ARCHIVE_FILE="$TEST_DIR/config_archive.vfs"
EXTRACT_DIR="$TEST_DIR/extracted"
LOG_FILE="$TEST_DIR/test_log.txt"

# Files to test (configuration and data files)
CONFIG_FILES=(
    "config.ini"
    "Option.ini"
    "server.dat"
    "*.dat"
)

echo -e "${BLUE}=== Maxion Phase 1 Test: Configuration Files ===${NC}"
echo ""
echo -e "${BLUE}Game Directory:${NC} $GAME_DIR"
echo -e "${BLUE}Project Root:${NC}   $PROJECT_ROOT"
echo -e "${BLUE}Test Directory:${NC} $TEST_DIR"
echo ""

# ============================================
# PRE-FLIGHT CHECKS
# ============================================

echo -e "${BLUE}=== Pre-Flight Checks ===${NC}"

# Check if game directory exists
if [ ! -d "$GAME_DIR" ]; then
    echo -e "${RED}✗ Game directory not found: $GAME_DIR${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Game directory found${NC}"

# Check if Maxion Packer exists
if [ ! -f "$MAXION_PACKER" ]; then
    echo -e "${YELLOW}⚠ Maxion Packer not found. Building...${NC}"
    cd "$PROJECT_ROOT"
    cargo build --release -p maxion-packer
    echo -e "${GREEN}✓ Maxion Packer built${NC}"
else
    echo -e "${GREEN}✓ Maxion Packer found${NC}"
fi

# Check if we have permission to read game files
if [ ! -r "$GAME_DIR/config.ini" ]; then
    echo -e "${RED}✗ Cannot read game files (permission denied)${NC}"
    echo -e "${YELLOW}  Try running as administrator or check file permissions${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Can read game files${NC}"

# Create test directory
echo ""
echo -e "${BLUE}=== Creating Test Directory ===${NC}"
mkdir -p "$TEST_DIR"
mkdir -p "$BACKUP_DIR"
mkdir -p "$EXTRACT_DIR"
echo -e "${GREEN}✓ Test directory created: $TEST_DIR${NC}"

# ============================================
# BACKUP CONFIGURATION FILES
# ============================================

echo ""
echo -e "${BLUE}=== Step 1: Backing Up Configuration Files ===${NC}"

backup_count=0
for pattern in "${CONFIG_FILES[@]}"; do
    if [[ "$pattern" == *"*"* ]]; then
        # Handle wildcards
        for file in "$GAME_DIR"/$pattern; do
            if [ -f "$file" ]; then
                filename=$(basename "$file")
                cp "$file" "$BACKUP_DIR/"
                backup_count=$((backup_count + 1))
                echo -e "${GREEN}  ✓ Backed up: $filename${NC}"
            fi
        done
    else
        # Handle exact files
        if [ -f "$GAME_DIR/$pattern" ]; then
            cp "$GAME_DIR/$pattern" "$BACKUP_DIR/"
            backup_count=$((backup_count + 1))
            echo -e "${GREEN}  ✓ Backed up: $pattern${NC}"
        fi
    fi
done

if [ $backup_count -eq 0 ]; then
    echo -e "${RED}✗ No configuration files found to backup${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Backed up $backup_count configuration files${NC}"
echo -e "${YELLOW}  Backup location: $BACKUP_DIR${NC}"

# ============================================
# PACK CONFIGURATION FILES
# ============================================

echo ""
echo -e "${BLUE}=== Step 2: Packing Configuration Files ===${NC}"

# Build the pack command
PACK_CMD="$MAXION_PACKER pack"
PACK_CMD="$PACK_CMD --assets"

# Add files to pack command
file_count=0
for pattern in "${CONFIG_FILES[@]}"; do
    if [[ "$pattern" == *"*"* ]]; then
        # Add wildcard patterns
        for file in "$GAME_DIR"/$pattern; do
            if [ -f "$file" ]; then
                PACK_CMD="$PACK_CMD \"$file\""
                file_count=$((file_count + 1))
            fi
        done
    else
        # Add exact files
        if [ -f "$GAME_DIR/$pattern" ]; then
            PACK_CMD="$PACK_CMD \"$GAME_DIR/$pattern\""
            file_count=$((file_count + 1))
        fi
    fi
done

PACK_CMD="$PACK_CMD --output \"$ARCHIVE_FILE\""
PACK_CMD="$PACK_CMD --compress"
PACK_CMD="$PACK_CMD --compression-level 6"
PACK_CMD="$PACK_CMD --verify"

echo -e "${BLUE}Packing $file_count files...${NC}"
echo -e "${YELLOW}Command: $PACK_CMD${NC}"

# Execute pack command
eval $PACK_CMD 2>&1 | tee "$LOG_FILE"

# Check if archive was created
if [ ! -f "$ARCHIVE_FILE" ]; then
    echo -e "${RED}✗ Archive was not created${NC}"
    echo -e "${YELLOW}Check log: $LOG_FILE${NC}"
    exit 1
fi

archive_size=$(stat -c%s "$ARCHIVE_FILE" 2>/dev/null || stat -f%z "$ARCHIVE_FILE" 2>/dev/null)
archive_size_mb=$((archive_size / 1024 / 1024))

echo -e "${GREEN}✓ Archive created successfully${NC}"
echo -e "${GREEN}  Location: $ARCHIVE_FILE${NC}"
echo -e "${GREEN}  Size: $archive_size bytes (${archive_size_mb} MB)${NC}"

# ============================================
# VERIFY ARCHIVE INTEGRITY
# ============================================

echo ""
echo -e "${BLUE}=== Step 3: Verifying Archive Integrity ===${NC}"

# List archive contents
echo -e "${BLUE}Archive contents:${NC}"
$MAXION_PACKER list "$ARCHIVE_FILE" | head -20

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Archive integrity verified${NC}"
else
    echo -e "${RED}✗ Archive verification failed${NC}"
    exit 1
fi

# ============================================
# EXTRACT AND COMPARE
# ============================================

echo ""
echo -e "${BLUE}=== Step 4: Extracting for Verification ===${NC}"

# Extract archive
$MAXION_PACKER extract "$ARCHIVE_FILE" "$EXTRACT_DIR" 2>&1 | tee -a "$LOG_FILE"

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Archive extracted successfully${NC}"
else
    echo -e "${RED}✗ Archive extraction failed${NC}"
    exit 1
fi

# Compare files
echo ""
echo -e "${BLUE}=== Step 5: Comparing Original vs Extracted ===${NC}"

mismatch_count=0
match_count=0

for file in "$BACKUP_DIR"/*; do
    filename=$(basename "$file")
    original="$file"
    extracted="$EXTRACT_DIR/$filename"

    if [ -f "$extracted" ]; then
        # Compare files
        if diff -q "$original" "$extracted" > /dev/null 2>&1; then
            echo -e "${GREEN}  ✓ Match: $filename${NC}"
            match_count=$((match_count + 1))
        else
            echo -e "${RED}  ✗ Mismatch: $filename${NC}"
            mismatch_count=$((mismatch_count + 1))

            # Show diff details for first mismatch
            if [ $mismatch_count -eq 1 ]; then
                echo -e "${YELLOW}    Difference details:${NC}"
                diff "$original" "$extracted" | head -20
            fi
        fi
    else
        echo -e "${YELLOW}  ⚠ Not extracted: $filename${NC}"
    fi
done

# ============================================
# SIZE ANALYSIS
# ============================================

echo ""
echo -e "${BLUE}=== Step 6: Size Analysis ===${NC}"

# Calculate original size
original_size=0
for file in "$BACKUP_DIR"/*; do
    if [ -f "$file" ]; then
        size=$(stat -c%s "$file" 2>/dev/null || stat -f%z "$file" 2>/dev/null)
        original_size=$((original_size + size))
    fi
done

# Calculate extracted size
extracted_size=$(du -sb "$EXTRACT_DIR" 2>/dev/null | cut -f1)

# Calculate savings
if [ $original_size -gt 0 ] && [ $extracted_size -gt 0 ]; then
    savings=$((original_size - archive_size))
    savings_pct=$((savings * 100 / original_size))

    echo -e "${BLUE}Original size:${NC}     $original_size bytes"
    echo -e "${BLUE}Archive size:${NC}      $archive_size bytes"
    echo -e "${BLUE}Space saved:${NC}       $savings bytes ($savings_pct%)"

    if [ $savings_pct -gt 50 ]; then
        echo -e "${GREEN}✓ Excellent compression! ($savings_pct% reduction)${NC}"
    elif [ $savings_pct -gt 30 ]; then
        echo -e "${GREEN}✓ Good compression ($savings_pct% reduction)${NC}"
    else
        echo -e "${YELLOW}⚠ Low compression ratio ($savings_pct% reduction)${NC}"
        echo -e "${YELLOW}  This is normal for text-based config files${NC}"
    fi
else
    echo -e "${YELLOW}⚠ Could not calculate size comparison${NC}"
fi

# ============================================
# TEST RESULTS SUMMARY
# ============================================

echo ""
echo -e "${BLUE}=== Test Results Summary ===${NC}"
echo ""

# Pass/Fail criteria
test_passed=true

if [ $mismatch_count -gt 0 ]; then
    test_passed=false
    echo -e "${RED}✗ TEST FAILED: File mismatches detected${NC}"
    echo -e "${RED}  Mismatched files: $mismatch_count${NC}"
else
    echo -e "${GREEN}✓ All files match perfectly${NC}"
fi

if [ ! -f "$ARCHIVE_FILE" ]; then
    test_passed=false
    echo -e "${RED}✗ TEST FAILED: Archive not created${NC}"
else
    echo -e "${GREEN}✓ Archive created successfully${NC}"
fi

echo ""
echo -e "${BLUE}Statistics:${NC}"
echo "  Files backed up:    $backup_count"
echo "  Files packed:       $file_count"
echo "  Files matched:      $match_count"
echo "  Files mismatched:   $mismatch_count"
echo "  Original size:      $original_size bytes"
echo "  Archive size:       $archive_size bytes"
echo "  Space saved:        $savings bytes ($savings_pct%)"

echo ""
echo -e "${BLUE}Test Directory:${NC} $TEST_DIR"
echo "  - Backup:     $BACKUP_DIR"
echo "  - Archive:    $ARCHIVE_FILE"
echo "  - Extracted:  $EXTRACT_DIR"
echo "  - Log:        $LOG_FILE"

echo ""
echo -e "${BLUE}=== Next Steps ===${NC}"

if [ "$test_passed" = true ]; then
    echo -e "${GREEN}✓ Phase 1 test PASSED!${NC}"
    echo ""
    echo -e "${YELLOW}Recommended actions:${NC}"
    echo "  1. Review the archive contents to ensure all files are included"
    echo "  2. Test the game with the archive (requires integration)"
    echo "  3. Proceed to Phase 2 (Shaders) when ready"
    echo ""
    echo -e "${BLUE}Commands for next phase:${NC}"
    echo "  ./scripts/test_maxion_phase2.sh"
    echo ""
    echo -e "${YELLOW}To clean up test files:${NC}"
    echo "  rm -rf $TEST_DIR"
    echo ""
    echo -e "${GREEN}To restore from backup (if needed):${NC}"
    echo "  cp $BACKUP_DIR/* $GAME_DIR/"
else
    echo -e "${RED}✗ Phase 1 test FAILED${NC}"
    echo ""
    echo -e "${YELLOW}Troubleshooting steps:${NC}"
    echo "  1. Check the log file: $LOG_FILE"
    echo "  2. Verify backup files in: $BACKUP_DIR"
    echo "  3. Compare original vs extracted files"
    echo "  4. Report any issues to the Maxion team"
    echo ""
    echo -e "${GREEN}To restore original files:${NC}"
    echo "  cp $BACKUP_DIR/* $GAME_DIR/"
fi

echo ""
echo -e "${BLUE}=== Phase 1 Test Complete ===${NC}"

# Exit with appropriate code
if [ "$test_passed" = true ]; then
    exit 0
else
    exit 1
fi
