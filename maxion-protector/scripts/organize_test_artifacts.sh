#!/bin/bash

# Maxion Protector Test Artifact Organizer
# Automatically organizes .maxion files from project root to test_artifacts directory

# Default values
SOURCE_DIR=""
TARGET_DIR=""
DRY_RUN=false
FORCE=false
INTERACTIVE=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --source-dir)
            SOURCE_DIR="$2"
            shift 2
            ;;
        --target-dir)
            TARGET_DIR="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --force)
            FORCE=true
            shift
            ;;
        --interactive|-i)
            INTERACTIVE=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  --source-dir DIR    Source directory (default: project root)"
            echo "  --target-dir DIR    Target directory (default: test_artifacts)"
            echo "  --dry-run           Show what would be done without making changes"
            echo "  --force             Overwrite existing files"
            echo "  --interactive, -i   Prompt before moving files"
            echo "  --help, -h          Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0                    # Organize all .maxion files"
            echo "  $0 --dry-run          # Preview changes"
            echo "  $0 --interactive      # Prompt for confirmation"
            echo "  $0 --source-dir ./test --target-dir ./artifacts"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Set default directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

if [ -z "$SOURCE_DIR" ]; then
    SOURCE_DIR="$PROJECT_ROOT"
fi

if [ -z "$TARGET_DIR" ]; then
    TARGET_DIR="$PROJECT_ROOT/test_artifacts"
fi

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
GRAY='\033[0;90m'
NC='\033[0m' # No Color

echo -e "${CYAN}=== Maxion Protector Test Artifact Organizer ===${NC}"
echo ""

# Create target directory if it doesn't exist
if [ ! -d "$TARGET_DIR" ]; then
    echo -e "${YELLOW}Creating target directory: $TARGET_DIR${NC}"
    if [ "$DRY_RUN" = false ]; then
        mkdir -p "$TARGET_DIR"
    fi
fi

# Find all .maxion files in source directory
echo -e "${GRAY}Scanning for .maxion files in: $SOURCE_DIR${NC}"
MAXION_FILES=($(find "$SOURCE_DIR" -maxdepth 1 -name "*.maxion" -type f 2>/dev/null))

if [ ${#MAXION_FILES[@]} -eq 0 ]; then
    echo -e "${YELLOW}No .maxion files found in source directory${NC}"
    exit 0
fi

echo -e "${WHITE}Found ${#MAXION_FILES[@]} .maxion file(s)${NC}"
echo ""

# Function to categorize a file
function get_file_category() {
    local filename
    filename=$(basename "$1")

    if [[ "$filename" =~ ^test\.maxion$ ]]; then
        echo "basic"
    elif [[ "$filename" =~ ^test_archive\d*(_(on|off))?\.maxion$ ]]; then
        echo "archive"
    elif [[ "$filename" =~ ^test(_archive)?_on\.maxion$ ]]; then
        echo "feature_on"
    elif [[ "$filename" =~ ^test(_archive)?_off\.maxion$ ]]; then
        echo "feature_off"
    elif [[ "$filename" =~ ^test_final\.maxion$ ]]; then
        echo "final"
    else
        echo "unknown"
    fi
}

# Function to get category range
function get_category_range() {
    local category="$1"

    case "$category" in
        "basic")
            echo "1:99:basic_test"
            ;;
        "archive")
            echo "10:99:archive"
            ;;
        "feature_on")
            echo "20:29:feature_enabled"
            ;;
        "feature_off")
            echo "30:39:feature_disabled"
            ;;
        "final")
            echo "999:999:final"
            ;;
        *)
            echo "100:199:misc"
            ;;
    esac
}

# Function to get next available number
function get_next_number() {
    local range_start="$1"
    local range_end="$2"
    local target_dir="$3"

    for ((i = range_start; i <= range_end; i++)); do
        padded_num=$(printf "%03d" "$i")
        existing_file=$(find "$target_dir" -maxdepth 1 -name "${padded_num}_*.maxion" 2>/dev/null)
        if [ -z "$existing_file" ]; then
            echo "$i"
            return
        fi
    done

    echo ""
}

# Function to generate new filename
function get_new_filename() {
    local filename="$1"
    local category="$2"

    local base_name
    base_name=$(basename "$filename" .maxion)
    local old_name="${base_name#test_}"

    case "$category" in
        "archive")
            if [[ "$old_name" =~ ^archive([0-9]*)$ ]]; then
                local version="${BASH_REMATCH[1]}"
                if [ -z "$version" ]; then
                    version="1"
                fi
                echo "archive_v${version}.maxion"
            elif [[ "$old_name" =~ ^archive_(on|off)$ ]]; then
                local state="${BASH_REMATCH[1]}"
                if [ "$state" = "on" ]; then
                    state="enabled"
                else
                    state="disabled"
                fi
                echo "archive_feature_${state}.maxion"
            else
                echo "archive.maxion"
            fi
            ;;
        "feature_on")
            if [ "$old_name" = "archive_on" ]; then
                echo "archive_feature_enabled.maxion"
            else
                echo "feature_enabled.maxion"
            fi
            ;;
        "feature_off")
            if [ "$old_name" = "archive_off" ]; then
                echo "archive_feature_disabled.maxion"
            else
                echo "feature_disabled.maxion"
            fi
            ;;
        "final")
            echo "final.maxion"
            ;;
        "basic"|*)
            echo "basic_test.maxion"
            ;;
    esac
}

# Process files
MOVES=()
declare -A MOVES_SOURCE
declare -A MOVES_DEST
declare -A MOVES_CATEGORY
declare -A MOVES_ORIG_NAME
declare -A MOVES_NEW_NAME

for file in "${MAXION_FILES[@]}"; do
    filename=$(basename "$file")
    category=$(get_file_category "$file")

    if [ "$category" != "unknown" ]; then
        range_info=$(get_category_range "$category")
        IFS=':' read -r range_start range_end prefix <<< "$range_info"

        if [ "$category" = "final" ]; then
            number=999
        else
            number=$(get_next_number "$range_start" "$range_end" "$TARGET_DIR")
        fi

        if [ -n "$number" ]; then
            padded_num=$(printf "%03d" "$number")
            new_base=$(get_new_filename "$filename" "$category")
            new_filename="${padded_num}_${new_base}"
            new_filepath="$TARGET_DIR/$new_filename"

            idx=${#MOVES[@]}
            MOVES+=("$idx")
            MOVES_SOURCE["$idx"]="$file"
            MOVES_DEST["$idx"]="$new_filepath"
            MOVES_CATEGORY["$idx"]="$category"
            MOVES_ORIG_NAME["$idx"]="$filename"
            MOVES_NEW_NAME["$idx"]="$new_filename"
        else
            echo -e "${YELLOW}Warning: Could not find available number for $filename${NC}"
        fi
    else
        echo -e "${YELLOW}Skipping unmatched file: $filename${NC}"
    fi
done

# Display proposed moves
if [ ${#MOVES[@]} -gt 0 ]; then
    echo -e "${CYAN}=== Proposed File Organization ===${NC}"
    echo ""

    for idx in "${MOVES[@]}"; do
        category="${MOVES_CATEGORY[$idx]}"
        orig_name="${MOVES_ORIG_NAME[$idx]}"
        new_name="${MOVES_NEW_NAME[$idx]}"
        echo -e "${GRAY}$category${NC}"
        echo -e "  ${WHITE}$orig_name${NC} → ${GREEN}$new_name${NC}"
    done

    echo ""

    # Interactive confirmation
    if [ "$INTERACTIVE" = true ] && [ "$DRY_RUN" = false ]; then
        read -p "Proceed with these moves? (y/n): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${YELLOW}Cancelled${NC}"
            exit 0
        fi
    fi

    # Execute moves
    echo -e "${CYAN}=== Organizing Files ===${NC}"
    echo ""

    success_count=0
    fail_count=0

    for idx in "${MOVES[@]}"; do
        source="${MOVES_SOURCE[$idx]}"
        dest="${MOVES_DEST[$idx]}"
        orig_name="${MOVES_ORIG_NAME[$idx]}"
        new_name="${MOVES_NEW_NAME[$idx]}"

        if [ "$DRY_RUN" = true ]; then
            echo -e "[DRY RUN] Would move: $orig_name → $new_name"
            ((success_count++))
        else
            if [ "$FORCE" = true ] || [ ! -f "$dest" ]; then
                if mv -f "$source" "$dest" 2>/dev/null; then
                    echo -e "${GREEN}✓ Moved${NC}: ${WHITE}$orig_name${NC} → ${GREEN}$new_name${NC}"
                    ((success_count++))
                else
                    echo -e "${RED}✗ Failed${NC}: $orig_name - Could not move file"
                    ((fail_count++))
                fi
            else
                echo -e "${YELLOW}⊘ Skipped${NC}: $new_name already exists (use --force to overwrite)"
            fi
        fi
    done

    echo ""
    if [ "$DRY_RUN" = false ]; then
        echo -e "${GREEN}✓ Organization complete!${NC}"
        echo -e "  Success: $success_count"
        if [ $fail_count -gt 0 ]; then
            echo -e "  Failed:  $fail_count"
        fi
    fi
else
    echo -e "${YELLOW}No files to organize${NC}"
fi

echo ""
echo -e "${CYAN}Target directory:${NC} $TARGET_DIR"
echo ""

exit 0
