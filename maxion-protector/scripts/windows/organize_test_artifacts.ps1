# Maxion Protector Test Artifact Organizer
# Automatically organizes .maxion files from project root to test_artifacts directory

param(
    [Parameter(Mandatory=$false)]
    [string]$SourceDir = "",

    [Parameter(Mandatory=$false)]
    [string]$TargetDir = "",

    [Parameter(Mandatory=$false)]
    [switch]$DryRun,

    [Parameter(Mandatory=$false)]
    [switch]$Force,

    [Parameter(Mandatory=$false)]
    [switch]$Interactive
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Get script directory and project root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

# Set default directories
if ([string]::IsNullOrEmpty($SourceDir)) {
    $SourceDir = $ProjectRoot
}

if ([string]::IsNullOrEmpty($TargetDir)) {
    $TargetDir = Join-Path $ProjectRoot "test_artifacts"
}

# Colors for output
$Cyan = "`e[36m"
$Green = "`e[32m"
$Yellow = "`e[33m"
$Red = "`e[31m"
$White = "`e[37m"
$Gray = "`e[90m"
$NC = "`e[0m"

Write-Host "${Cyan}=== Maxion Protector Test Artifact Organizer ===${NC}"
Write-Host ""

# Create target directory if it doesn't exist
if (-not (Test-Path $TargetDir)) {
    Write-Host "Creating target directory: $TargetDir" -ForegroundColor Yellow
    if (-not $DryRun) {
        New-Item -ItemType Directory -Force -Path $TargetDir | Out-Null
    }
}

# Find all .maxion files in source directory
Write-Host "Scanning for .maxion files in: $SourceDir" -ForegroundColor Gray
$MaxionFiles = Get-ChildItem -Path $SourceDir -Filter "*.maxion" -File -ErrorAction SilentlyContinue

if ($MaxionFiles.Count -eq 0) {
    Write-Host "${Yellow}No .maxion files found in source directory${NC}"
    exit 0
}

Write-Host "Found $($MaxionFiles.Count) .maxion file(s)" -ForegroundColor White
Write-Host ""

# Category definitions
$Categories = @{
    "basic" = @{
        "Pattern" = "^test\.maxion$"
        "RangeStart" = 1
        "RangeEnd" = 99
        "Name" = "Basic Tests"
        "Prefix" = "basic_test"
    }
    "archive" = @{
        "Pattern" = "^test_archive\d*(_(on|off))?\.(maxion)$"
        "RangeStart" = 10
        "RangeEnd" = 99
        "Name" = "Archive Tests"
        "Prefix" = "archive"
    }
    "feature_on" = @{
        "Pattern" = "^test(_archive)?_on\.maxion$"
        "RangeStart" = 20
        "RangeEnd" = 29
        "Name" = "Feature Enabled"
        "Prefix" = "feature_enabled"
    }
    "feature_off" = @{
        "Pattern" = "^test(_archive)?_off\.maxion$"
        "RangeStart" = 30
        "RangeEnd" = 39
        "Name" = "Feature Disabled"
        "Prefix" = "feature_disabled"
    }
    "final" = @{
        "Pattern" = "^test_final\.maxion$"
        "Number" = 999
        "Name" = "Milestone"
        "Prefix" = "final"
    }
}

# Function to categorize a file
function Get-FileCategory {
    param(
        [string]$FileName
    )

    foreach ($Category in $Categories.Values) {
        if ($FileName -match $Category.Pattern) {
            return $Category
        }
    }

    return $null
}

# Function to get next available number
function Get-NextNumber {
    param(
        [int]$RangeStart,
        [int]$RangeEnd,
        [string]$TargetDir
    )

    for ($i = $RangeStart; $i -le $RangeEnd; $i++) {
        $PaddedNum = "{0:D3}" -f $i
        $ExistingFile = Get-ChildItem -Path $TargetDir -Filter "${PaddedNum}_*.maxion" -ErrorAction SilentlyContinue
        if (-not $ExistingFile) {
            return $i
        }
    }

    return $null
}

# Function to generate new filename
function Get-NewFilename {
    param(
        [string]$FileName,
        [hashtable]$Category
    )

    $BaseName = $FileName -replace "\.maxion$", ""
    $OldName = $BaseName -replace "^test_?", ""

    # Determine version number for archive files
    if ($Category.Name -eq "Archive Tests") {
        if ($OldName -match "^archive(\d*)$") {
            $Version = if ($Matches[1]) { "v$($Matches[1])" } else { "v1" }
            return "${Category.Prefix}_${Version}.maxion"
        }
        if ($OldName -match "^archive_(on|off)$") {
            $State = if ($Matches[1] -eq "on") { "enabled" } else { "disabled" }
            return "${Category.Prefix}_feature_${State}.maxion"
        }
    }

    if ($Category.Name -eq "Feature Enabled") {
        if ($OldName -eq "archive_on") {
            return "archive_feature_enabled.maxion"
        }
        return "${Category.Prefix}.maxion"
    }

    if ($Category.Name -eq "Feature Disabled") {
        if ($OldName -eq "archive_off") {
            return "archive_feature_disabled.maxion"
        }
        return "${Category.Prefix}.maxion"
    }

    if ($Category.Name -eq "Milestone") {
        return "${Category.Prefix}.maxion"
    }

    return "${Category.Prefix}.maxion"
}

# Process files
$Moves = @()

foreach ($File in $MaxionFiles) {
    $Category = Get-FileCategory -FileName $File.Name

    if ($Category) {
        $NewBaseName = Get-NewFilename -FileName $File.Name -Category $Category

        if ($Category.ContainsKey("Number")) {
            $Number = $Category.Number
        } else {
            $Number = Get-NextNumber -RangeStart $Category.RangeStart -RangeEnd $Category.RangeEnd -TargetDir $TargetDir
        }

        if ($Number) {
            $PaddedNumber = "{0:D3}" -f $Number
            $NewFileName = "${PaddedNumber}_${NewBaseName}"
            $NewFilePath = Join-Path $TargetDir $NewFileName

            $Moves += @{
                Source = $File.FullName
                Destination = $NewFilePath
                Category = $Category.Name
                OriginalName = $File.Name
                NewName = $NewFileName
            }
        } else {
            Write-Host "${Yellow}Warning: Could not find available number for $($File.Name)${NC}" -ForegroundColor Yellow
        }
    } else {
        Write-Host "${Yellow}Skipping unmatched file: $($File.Name)${NC}" -ForegroundColor Yellow
    }
}

# Display proposed moves
if ($Moves.Count -gt 0) {
    Write-Host "${Cyan}=== Proposed File Organization ===${NC}"
    Write-Host ""

    foreach ($Move in $Moves) {
        $SourceShort = Split-Path $Move.Source -Leaf
        $DestShort = Split-Path $Move.Destination -Leaf
        Write-Host "${Gray}$($Move.Category)${NC}"
        Write-Host "  ${White}$SourceShort${NC} → ${Green}$DestShort${NC}"
    }

    Write-Host ""

    if ($Interactive -and -not $DryRun) {
        $Response = Read-Host "Proceed with these moves? (y/n)"
        if ($Response -ne "y" -and $Response -ne "Y") {
            Write-Host "${Yellow}Cancelled${NC}"
            exit 0
        }
    }

    # Execute moves
    Write-Host "${Cyan}=== Organizing Files ===${NC}"
    Write-Host ""

    foreach ($Move in $Moves) {
        if ($DryRun) {
            Write-Host "[DRY RUN] Would move: $($Move.OriginalName) -> $($Move.NewName)"
        } else {
            try {
                Move-Item -Path $Move.Source -Destination $Move.Destination -Force:$Force
                Write-Host "${Green}[OK] Moved${NC}: ${White}$($Move.OriginalName)${NC} -> ${Green}$($Move.NewName)${NC}"
            }
            catch {
                Write-Host "${Red}[ERROR] Failed${NC}: $($Move.OriginalName) - $($_.Exception.Message)"
            }
        }
    }

    Write-Host ""
    Write-Host "${Green}[OK] Organization complete!${NC}"
} else {
    Write-Host "${Yellow}No files to organize${NC}"
}

Write-Host ""
Write-Host "${Cyan}Target directory:${NC} $TargetDir"
Write-Host ""

exit 0
