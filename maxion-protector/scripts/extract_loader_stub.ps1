# Maxion Loader Stub Binary Extractor
#
# This script extracts the raw binary of the loader stub from the compiled DLL
# and saves it to a file that can be embedded in the injector.
#
# Usage: pwsh scripts/extract_loader_stub.ps1

$ErrorActionPreference = "Stop"

# Configuration
$LoaderDll = "target\release\maxion_loader_stub.dll"
$OutputBin = "target\release\loader_stub.bin"
$TempObj = "target\release\loader_stub.obj"

Write-Host "Maxion Loader Stub Binary Extractor" -ForegroundColor Cyan
Write-Host ""

# Step 1: Check if DLL exists
if (-not (Test-Path $LoaderDll)) {
    Write-Error "Loader DLL not found: $LoaderDll"
    Write-Host "Please build the loader stub first:" -ForegroundColor Yellow
    Write-Host "  cargo build --release -p maxion-loader-stub"
    exit 1
}

Write-Host "Step 1: Found loader DLL" -ForegroundColor Green
Write-Host "  Path: $LoaderDll"
$dllSize = (Get-Item $LoaderDll).Length
Write-Host "  Size: $dllSize bytes"
Write-Host ""

# Step 2: Try to use dumpbin (from MSVC) to extract raw binary
$dumpbin = Get-Command dumpbin -ErrorAction SilentlyContinue

if ($dumpbin) {
    Write-Host "Step 2: Using dumpbin to extract binary" -ForegroundColor Green

    try {
        # Use dumpbin to extract raw data from DLL
        & dumpbin /RAWDATA:$LoaderDll 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) {
            & dumpbin /RAWDATA:$LoaderDll | Out-File -FilePath "temp_raw.txt" -Encoding ASCII

            # Parse the output and extract hex bytes
            $bytes = @()
            $content = Get-Content "temp_raw.txt"
            $inRawData = $false

            foreach ($line in $content) {
                if ($line -match "RAW DATA") {
                    $inRawData = $true
                    continue
                }

                if ($inRawData -and $line -match "^([0-9A-F]{8})\s+(.+)$") {
                    $hexData = $matches[2] -split "\s+"
                    foreach ($hex in $hexData) {
                        if ($hex -match "^[0-9A-F]{2}$") {
                            $bytes += [byte]::Parse($hex, "HexNumber")
                        }
                    }
                }

                if ($inRawData -and $line -eq "") {
                    break
                }
            }

            # Clean up temp file
            Remove-Item "temp_raw.txt" -ErrorAction SilentlyContinue

            if ($bytes.Count -gt 0) {
                Write-Host "  Extracted $($bytes.Count) bytes" -ForegroundColor Green
                [System.IO.File]::WriteAllBytes($OutputBin, $bytes)
                Write-Host "  Saved to: $OutputBin"
                Write-Host ""

                # Verify output
                $outputSize = (Get-Item $OutputBin).Length
                Write-Host "Step 3: Verification" -ForegroundColor Green
                Write-Host "  Output size: $outputSize bytes"

                if ($outputSize -gt 0 -and $outputSize -lt 10000) {
                    Write-Host "  ✓ Binary size looks reasonable" -ForegroundColor Green
                    exit 0
                } else {
                    Write-Host "  ⚠ Warning: Binary size seems unusual" -ForegroundColor Yellow
                }
            }
        }
    } catch {
        Write-Host "  dumpbin extraction failed: $_" -ForegroundColor Yellow
    }
}

# Step 4: Fallback - Read DLL as binary and try to find .text section
Write-Host "Step 4: Fallback - Parsing DLL structure" -ForegroundColor Green

try {
    # Read the entire DLL as bytes
    $dllBytes = [System.IO.File]::ReadAllBytes($LoaderDll)

    # Parse PE header (DOS header at offset 0)
    if ($dllBytes[0] -ne 0x4D -or $dllBytes[1] -ne 0x5A) {
        Write-Host "  ✗ Invalid DOS signature" -ForegroundColor Red
        exit 1
    }

    # Get PE header offset (at offset 0x3C)
    $peOffset = [BitConverter]::ToInt32($dllBytes, 0x3C)
    Write-Host "  PE header offset: 0x$($peOffset.ToString('X'))"

    # Check PE signature
    if ($dllBytes[$peOffset] -ne 0x50 -or $dllBytes[$peOffset+1] -ne 0x45) {
        Write-Host "  ✗ Invalid PE signature" -ForegroundColor Red
        exit 1
    }

    # Get number of sections (at PE header offset + 6)
    $numberOfSections = $dllBytes[$peOffset + 6] + ($dllBytes[$peOffset + 7] -shl 8)
    Write-Host "  Number of sections: $numberOfSections"

    # Get section headers start offset
    # Optional header starts at PE offset + 24, size is at PE offset + 20
    $optionalHeaderSize = [BitConverter]::ToInt16($dllBytes, $peOffset + 20)
    $sectionHeaderOffset = $peOffset + 24 + $optionalHeaderSize
    Write-Host "  Section headers at: 0x$($sectionHeaderOffset.ToString('X'))"

    # Look for .text section
    $sectionHeaderSize = 40
    $textOffset = 0
    $textSize = 0

    for ($i = 0; $i -lt $numberOfSections; $i++) {
        $sectionOffset = $sectionHeaderOffset + ($i * $sectionHeaderSize)

        # Read section name (8 bytes, padded with nulls)
        $sectionNameBytes = $dllBytes[($sectionOffset)..($sectionOffset + 7)]
        $sectionName = [System.Text.Encoding]::ASCII.GetString($sectionNameBytes).TrimEnd([char]0)

        if ($sectionName -eq ".text") {
            # Virtual size (at offset + 8)
            $virtualSize = [BitConverter]::ToUInt32($dllBytes, $sectionOffset + 8)

            # Pointer to raw data (at offset + 20)
            $textOffset = [BitConverter]::ToUInt32($dllBytes, $sectionOffset + 20)

            # Size of raw data (at offset + 16)
            $textSize = [BitConverter]::ToUInt32($dllBytes, $sectionOffset + 16)

            Write-Host "  Found .text section:"
            Write-Host "    Offset: 0x$($textOffset.ToString('X')) ($textOffset)"
            Write-Host "    Size: 0x$($textSize.ToString('X')) ($textSize)"
            break
        }
    }

    if ($textOffset -eq 0) {
        Write-Host "  ✗ .text section not found" -ForegroundColor Red
        exit 1
    }

    # Extract .text section
    $textBytes = $dllBytes[$textOffset..($textOffset + $textSize - 1)]

    # Remove padding (zero bytes at the end)
    $actualLength = $textSize
    for ($i = $textSize - 1; $i -ge 0; $i--) {
        if ($textBytes[$i] -ne 0) {
            $actualLength = $i + 1
            break
        }
    }

    if ($actualLength -lt $textSize) {
        Write-Host "  Trailing padding: $($textSize - $actualLength) bytes"
        $textBytes = $textBytes[0..$actualLength]
    }

    # Write to output file
    [System.IO.File]::WriteAllBytes($OutputBin, $textBytes)

    Write-Host ""
    Write-Host "Step 5: Extraction complete" -ForegroundColor Green
    Write-Host "  Output: $OutputBin"
    Write-Host "  Size: $($textBytes.Count) bytes"

    # Display first 32 bytes as hex
    Write-Host ""
    Write-Host "First 32 bytes:"
    $hex = ($textBytes[0..31] | ForEach-Object { $_.ToString("X2") }) -join " "
    Write-Host "  $hex"

    # Check for loader_entry signature (should start with standard function prologue)
    # x64: 55 (push rbp) or 48 89 ... (mov ...)
    # x86: 55 (push ebp) or 8B ... (mov ...)
    if ($textBytes[0] -eq 0x55 -or $textBytes[0] -eq 0x48 -or $textBytes[0] -eq 0x8B) {
        Write-Host "  ✓ Looks like valid code" -ForegroundColor Green
    } else {
        Write-Host "  ⚠ Unusual start byte: 0x$($textBytes[0].ToString('X2'))" -ForegroundColor Yellow
    }

    Write-Host ""
    Write-Host "✓ Extraction successful!" -ForegroundColor Green
    Write-Host "You can now use the extracted binary in the injector."

    exit 0

} catch {
    Write-Host "  ✗ Error parsing DLL: $_" -ForegroundColor Red
    exit 1
}
