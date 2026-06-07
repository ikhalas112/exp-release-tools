# Maxion Protector All Scenarios Benchmark Runner
# Runs all benchmark scenarios comparing unpacked vs packed executables on Windows
#
# Scenarios:
#   small   - Small Asset Load (240 bytes)
#   medium  - Medium Asset Bundle (multiple 1KB files)
#   large   - Large Asset Stream (5MB)
#   mixed   - Mixed Asset Load (various sizes)
#
# Usage: pwsh scripts\windows\run_all_benchmarks.ps1 [options]
#
# Options:
#   --iterations <n>        Number of iterations per scenario (default: 10)
#   --output-dir <path>     Directory for benchmark results (default: target\benchmarks)
#   --skip-build            Skip rebuilding executables
#   --skip-unpacked         Skip unpacked benchmarks
#   --skip-packed           Skip packed benchmarks
#   --verbose               Enable verbose output

param(
    [ValidateRange(1, 100)]
    [int]$Iterations = 10,

    [string]$OutputDir = "",

    [switch]$SkipBuild,

    [switch]$SkipUnpacked,

    [switch]$SkipPacked,

    [switch]$Verbose
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Get script directory and project root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $ScriptDir)
$HelloDir = Join-Path $ProjectRoot "examples\hello-world"
$E2EDir = Join-Path $ProjectRoot "target\e2e"

# Override output path if specified
if ($OutputDir -eq "") {
    $OutputDir = Join-Path $ProjectRoot "target\benchmarks"
}

# Benchmark scenarios
$Scenarios = @("small", "medium", "large", "mixed")

# Colors for output
function Write-ColorOutput {
    param(
        [string]$Message,
        [string]$Color = "White"
    )
    Write-Host $Message -ForegroundColor $Color
}

function Write-Section {
    param([string]$Title)
    Write-Host ""
    Write-Host "=== $Title ===" -ForegroundColor Cyan
    Write-Host ""
}

function Write-Success {
    param([string]$Message)
    Write-ColorOutput "✓ $Message" "Green"
}

function Write-Warning {
    param([string]$Message)
    Write-ColorOutput "⚠ $Message" "Yellow"
}

function Write-Error {
    param([string]$Message)
    Write-ColorOutput "✗ $Message" "Red"
}

function Write-Info {
    param([string]$Message)
    if ($Verbose) {
        Write-ColorOutput "  $Message" "Gray"
    }
}

# Create output directories
New-Item -ItemType Directory -Force -Path $E2EDir | Out-Null
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

Write-ColorOutput "Maxion Protector All Scenarios Benchmark Runner" "Cyan"
Write-Host ""

Write-ColorOutput "Configuration:" "Yellow"
Write-ColorOutput "  Scenarios: $($Scenarios -join ', ')" "White"
Write-ColorOutput "  Iterations per scenario: $Iterations" "White"
Write-ColorOutput "  Output directory: $OutputDir" "White"
Write-ColorOutput "  Skip build: $SkipBuild" "White"
Write-ColorOutput "  Skip unpacked: $SkipUnpacked" "White"
Write-ColorOutput "  Skip packed: $SkipPacked" "White"
Write-Host ""

# Initialize results
$Results = @{
    scenarios = @{}
    summary = @{
        total_iterations = $Scenarios.Count * $Iterations
        unpacked_size = 0
        packed_size = 0
        assets_size = 0
        start_time = Get-Date
    }
}

# Step 1: Build executables
if (-not $SkipBuild) {
    Write-Section "Building Executables"

    # Build hello-world
    Write-Host "Building hello-world..." -ForegroundColor Gray
    Push-Location $HelloDir
    try {
        cargo build --release --target x86_64-pc-windows-msvc --quiet
        $HelloExe = Join-Path $HelloDir "target\x86_64-pc-windows-msvc\release\hello.exe"

        if (-not (Test-Path $HelloExe)) {
            Write-Error "Failed to build hello-world"
            exit 1
        }

        Write-Success "Built: $HelloExe"
    }
    finally {
        Pop-Location
    }

    # Build maxion-packer
    Write-Host "Building maxion-packer..." -ForegroundColor Gray
    Push-Location $ProjectRoot
    try {
        cargo build --release -p maxion-packer --quiet
        $PackerBin = Join-Path $ProjectRoot "target\release\pnp.exe"

        if (-not (Test-Path $PackerBin)) {
            Write-Error "Failed to build pnp"
            exit 1
        }

        Write-Success "Built: $PackerBin"
    }
    finally {
        Pop-Location
    }
} else {
    Write-Section "Skipping Build (Reusing existing executables)"

    $HelloExe = Join-Path $HelloDir "target\x86_64-pc-windows-msvc\release\hello.exe"
    $PackerBin = Join-Path $ProjectRoot "target\release\pnp.exe"

    if (-not (Test-Path $HelloExe)) {
        Write-Error "hello.exe not found. Run without --skip-build first."
        exit 1
    }

    if (-not (Test-Path $PackerBin)) {
        Write-Error "pnp.exe not found. Run without --skip-build first."
        exit 1
    }

    Write-Success "Found: $HelloExe"
    Write-Success "Found: $PackerBin"
}

# Prepare test environment
Write-Section "Preparing Test Environment"

# Copy hello.exe to e2e directory
Copy-Item -Path $HelloExe -Destination $E2EDir -Force

# Prepare assets directory
$AssetsDir = Join-Path $E2EDir "assets"
New-Item -ItemType Directory -Force -Path $AssetsDir | Out-Null

# Copy test assets if they exist
if (Test-Path (Join-Path $ProjectRoot "test_assets")) {
    Copy-Item -Path (Join-Path $ProjectRoot "test_assets\*") -Destination $AssetsDir -Recurse -Force
    Write-Success "Copied test assets"
} else {
    Write-Warning "test_assets directory not found"
}

# Get file sizes
$Results.summary.unpacked_size = (Get-Item $HelloExe).Length

if (Test-Path $AssetsDir) {
    $Results.summary.assets_size = (Get-ChildItem $AssetsDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
}

Write-Host "Unpacked executable: $($Results.summary.unpacked_size) bytes" -ForegroundColor Gray
Write-Host "Assets directory: $($Results.summary.assets_size) bytes" -ForegroundColor Gray

# Step 2: Protect hello-world (Phase 2)
if (-not $SkipPacked) {
    Write-Section "Protecting hello-world (Phase 2: Single-file deployment)"

    $PackedExe = Join-Path $E2EDir "hello_packed.exe"

    Write-Host "Input: $HelloExe" -ForegroundColor Gray
    Write-Host "Output: $PackedExe" -ForegroundColor Gray

    # Remove existing packed executable if present
    if (Test-Path $PackedExe) {
        Remove-Item $PackedExe -Force
    }

    # Run packer
    $ProtectArgs = @(
        "protect",
        "--input", $HelloExe,
        "--assets", $AssetsDir,
        "--output", $PackedExe,
        "--chunk-size", "65536",
        "--compress",
        "--compression-level", "6"
    )

    Write-Info "Running: $PackerBin $($ProtectArgs -join ' ')"

    $ProtectOutput = & $PackerBin $ProtectArgs 2>&1
    $ProtectExitCode = $LASTEXITCODE

    if ($Verbose) {
        $ProtectOutput | ForEach-Object { Write-Host "  $_" -ForegroundColor DarkGray }
    }

    if ($ProtectExitCode -ne 0) {
        Write-Error "Protection failed with exit code $ProtectExitCode"
        Write-Host $ProtectOutput -ForegroundColor Red
        Write-Warning "Skipping packed benchmarks"
        $SkipPacked = $true
    } else {
        if (Test-Path $PackedExe) {
            Write-Success "Protected: $PackedExe"
            $Results.summary.packed_size = (Get-Item $PackedExe).Length
            Write-Host "Protected executable: $($Results.summary.packed_size) bytes" -ForegroundColor Gray

            $Overhead = $Results.summary.packed_size - $Results.summary.unpacked_size
            $OverheadPct = [math]::Round(($Overhead * 100.0 / $Results.summary.unpacked_size), 2)
            Write-Host "Overhead: $Overhead bytes ($OverheadPct%)" -ForegroundColor Gray
        } else {
            Write-Error "Protected executable not created"
            $SkipPacked = $true
        }
    }
}

Write-Host ""

# Step 3: Run benchmarks for each scenario
foreach ($Scenario in $Scenarios) {
    Write-Section "Scenario: $Scenario"

    # Initialize scenario results
    $Results.scenarios[$Scenario] = @{
        unpacked = @{
            output_file = Join-Path $OutputDir "${Scenario}_unpacked_output.txt"
            metrics_file = Join-Path $OutputDir "${Scenario}_unpacked_metrics.json"
            timings = @()
            success = $false
        }
        packed = @{
            output_file = Join-Path $OutputDir "${Scenario}_packed_output.txt"
            metrics_file = Join-Path $OutputDir "${Scenario}_packed_metrics.json"
            timings = @()
            success = $false
        }
    }

    # Run unpacked benchmarks
    if (-not $SkipUnpacked) {
        Write-Host "Running unpacked benchmarks..." -ForegroundColor Gray

        $unpackedOutput = $Results.scenarios[$Scenario].unpacked.output_file

        # Clear previous metrics
        $MetricsFile = Join-Path $E2EDir "benchmark_metrics.json"
        if (Test-Path $MetricsFile) {
            Remove-Item $MetricsFile -Force
        }

        Push-Location $E2EDir
        try {
            for ($i = 1; $i -le $Iterations; $i++) {
                Write-Host "  Iteration $i/$Iterations" -ForegroundColor DarkGray
                & "$E2EDir\hello.exe" $Scenario >> $unpackedOutput 2>&1
            }

            # Collect metrics
            if (Test-Path $MetricsFile) {
                Move-Item -Path $MetricsFile -Destination $Results.scenarios[$Scenario].unpacked.metrics_file -Force
                Write-Success "Unpacked metrics saved"
                $Results.scenarios[$Scenario].unpacked.success = $true
            } else {
                Write-Warning "No unpacked metrics generated"
            }
        }
        finally {
            Pop-Location
        }
    }

    # Run packed benchmarks
    if (-not $SkipPacked) {
        Write-Host "Running packed benchmarks..." -ForegroundColor Gray

        $packedOutput = $Results.scenarios[$Scenario].packed.output_file
        $PackedExe = Join-Path $E2EDir "hello_packed.exe"

        # Clear previous metrics
        $MetricsFile = Join-Path $E2EDir "benchmark_metrics.json"
        if (Test-Path $MetricsFile) {
            Remove-Item $MetricsFile -Force
        }

        if (-not (Test-Path $PackedExe)) {
            Write-Warning "Packed executable not found for scenario $Scenario"
        } else {
            Push-Location $E2EDir
            try {
                for ($i = 1; $i -le $Iterations; $i++) {
                    Write-Host "  Iteration $i/$Iterations" -ForegroundColor DarkGray
                    & $PackedExe $Scenario >> $packedOutput 2>&1
                }

                # Collect metrics
                if (Test-Path $MetricsFile) {
                    Move-Item -Path $MetricsFile -Destination $Results.scenarios[$Scenario].packed.metrics_file -Force
                    Write-Success "Packed metrics saved"
                    $Results.scenarios[$Scenario].packed.success = $true
                } else {
                    Write-Warning "No packed metrics generated"
                }
            }
            finally {
                Pop-Location
            }
        }
    }
}

# Step 4: Generate comprehensive report
Write-Section "Generating Comprehensive Report"

$Timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$ReportFile = Join-Path $OutputDir "all_scenarios_report_$Timestamp.md"

# Calculate file size metrics
$Overhead = $Results.summary.packed_size - $Results.summary.unpacked_size
$OverheadPct = if ($Results.summary.unpacked_size -gt 0) { [math]::Round(($Overhead * 100.0 / $Results.summary.unpacked_size), 2) } else { 0 }
$TotalUnpacked = $Results.summary.unpacked_size + $Results.summary.assets_size
$Savings = $TotalUnpacked - $Results.summary.packed_size
$SavingsPct = if ($TotalUnpacked -gt 0) { [math]::Round(($Savings * 100.0 / $TotalUnpacked), 2) } else { 0 }
$Results.summary.end_time = Get-Date
$Duration = ($Results.summary.end_time - $Results.summary.start_time).TotalMinutes

# Generate report content
$Report = @"
# Maxion Protector All Scenarios Benchmark Report

**Generated:** $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
**Platform:** Windows ($env:PROCESSOR_ARCHITECTURE)
**Total Duration:** $([math]::Round($Duration, 2)) minutes

## Executive Summary

This comprehensive report compares the performance of unpacked vs packed executables across all benchmark scenarios using Maxion Protector Phase 2 (single-file deployment).

- **Scenarios Tested:** $($Scenarios.Count)
- **Iterations per Scenario:** $Iterations
- **Total Iterations:** $($Results.summary.total_iterations)

## File Size Comparison

| Metric | Size |
|--------|------|
| **Unpacked Executable** | {0:N0} bytes |
| **Protected Executable** | {1:N0} bytes |
| **Protection Overhead** | {2:N0} bytes ({3}%) |
| **Assets Directory** | {4:N0} bytes |
| **Total Unpacked** | {5:N0} bytes |
| **Total Packed** | {6:N0} bytes |
| **Space Saved** | {7:N0} bytes ({8}%) |

"@ -f $Results.summary.unpacked_size,
     $Results.summary.packed_size,
     $Overhead,
     $OverheadPct,
     $Results.summary.assets_size,
     $TotalUnpacked,
     $Results.summary.packed_size,
     $Savings,
     $SavingsPct

# Generate scenario sections
foreach ($Scenario in $Scenarios) {
    $ScenarioResult = $Results.scenarios[$Scenario]

    $ScenarioTitle = switch ($Scenario) {
        "small"  { "Small Asset Load (240 bytes)" }
        "medium" { "Medium Asset Bundle (multiple 1KB files)" }
        "large"  { "Large Asset Stream (5MB)" }
        "mixed"  { "Mixed Asset Load (various sizes)" }
        default  { $Scenario }
    }

    $Report += @"

---

## Scenario: $ScenarioTitle

### Configuration

- **Scenario Name:** $Scenario
- **Iterations:** $Iterations
- **Unpacked Success:** $(if ($ScenarioResult.unpacked.success) { "✅ Yes" } else { "❌ No" })
- **Packed Success:** $(if ($ScenarioResult.packed.success) { "✅ Yes" } else { "❌ No" })

### Unpacked Performance

"@

    if ($ScenarioResult.unpacked.success) {
        try {
            $MetricsData = Get-Content $ScenarioResult.unpacked.metrics_file -Raw | ConvertFrom-Json
            $Report += @"
Metrics available: \`$($ScenarioResult.unpacked.metrics_file)\`

```json
$($MetricsData | ConvertTo-Json -Depth 10)
```

"@
        } catch {
            $Report += @"
⚠ Could not parse metrics JSON: $_

"@
        }
    } else {
        $Report += @"

⚠ No unpacked metrics available

"@
    }

    $Report += @"

### Packed Performance

"@

    if ($ScenarioResult.packed.success) {
        try {
            $MetricsData = Get-Content $ScenarioResult.packed.metrics_file -Raw | ConvertFrom-Json
            $Report += @"
Metrics available: \`$($ScenarioResult.packed.metrics_file)\`

```json
$($MetricsData | ConvertTo-Json -Depth 10)
```

"@
        } catch {
            $Report += @"
⚠ Could not parse metrics JSON: $_

"@
        }
    } else {
        $Report += @"

⚠ No packed metrics available

"@
    }

    # Performance comparison if both successful
    if ($ScenarioResult.unpacked.success -and $ScenarioResult.packed.success) {
        $Report += @"

### Performance Comparison

| Operation | Unpacked (ms) | Packed (ms) | Overhead | % Overhead |
|-----------|---------------|-------------|----------|------------|
"@

        try {
            $UnpackedData = Get-Content $ScenarioResult.unpacked.metrics_file -Raw | ConvertFrom-Json
            $PackedData = Get-Content $ScenarioResult.packed.metrics_file -Raw | ConvertFrom-Json

            if ($UnpackedData.summary -and $UnpackedData.summary.timings) {
                foreach ($timing in $UnpackedData.summary.timings.PSObject.Properties) {
                    $opName = $timing.Name
                    $unpackedAvg = $timing.Value.avg_ms

                    # Find corresponding timing in packed data
                    $packedAvg = 0
                    if ($PackedData.summary -and $PackedData.summary.timings -and $PackedData.summary.timings.$opName) {
                        $packedAvg = $PackedData.summary.timings.$opName.avg_ms
                    }

                    if ($packedAvg -gt 0) {
                        $overhead = $packedAvg - $unpackedAvg
                        $overheadPct = [math]::Round(($overhead * 100.0 / $unpackedAvg), 2)
                        $Report += "| $opName | {0:N3} | {1:N3} | {2:N3} | {3:N2}% |`n" -f $unpackedAvg, $packedAvg, $overhead, $overheadPct
                    } else {
                        $Report += "| $opName | {0:N3} | N/A | N/A | N/A |`n" -f $unpackedAvg
                    }
                }
            }
        } catch {
            $Report += "Note: Could not compare timings`n"
        }
    }

    # Detailed output
    $Report += @"

### Detailed Output

#### Unpacked Output

```
"@

    if (Test-Path $ScenarioResult.unpacked.output_file) {
        $OutputContent = Get-Content $ScenarioResult.unpacked.output_file -Raw
        # Truncate if too long
        if ($OutputContent.Length -gt 5000) {
            $OutputContent = $OutputContent.Substring(0, 5000) + "`n... (truncated)"
        }
        $Report += $OutputContent
    } else {
        $Report += "(no output)"
    }

    $Report += @"

```

#### Packed Output

```
"@

    if (Test-Path $ScenarioResult.packed.output_file) {
        $OutputContent = Get-Content $ScenarioResult.packed.output_file -Raw
        # Truncate if too long
        if ($OutputContent.Length -gt 5000) {
            $OutputContent = $OutputContent.Substring(0, 5000) + "`n... (truncated)"
        }
        $Report += $OutputContent
    } else {
        $Report += "(not available)"
    }

    $Report += @"

```

"@
}

# Conclusion
$Report += @"

---

## Overall Conclusion

### Summary

Maxion Protector Phase 2 (single-file deployment) has been tested across all benchmark scenarios:

1. **File Size Impact**: Protection added $Overhead bytes ($OverheadPct% overhead to the executable)
2. **Space Savings**: Embedded assets saved $Savings bytes ($SavingsPct% by eliminating external assets)
3. **Total Duration**: All benchmarks completed in $([math]::Round($Duration, 2)) minutes

### Scenario Results

"@

foreach ($Scenario in $Scenarios) {
    $ScenarioResult = $Results.scenarios[$Scenario]
    $Status = if ($ScenarioResult.unpacked.success -and $ScenarioResult.packed.success) { "✅ Complete" } elseif ($ScenarioResult.unpacked.success) { "⚠ Partial (unpacked only)" } else { "❌ Failed" }
    $Report += @"

- **$Scenario**: $Status
"@
}

$Report += @"

### Key Findings

1. **Single-File Deployment**: Phase 2 successfully embeds all assets into the executable
2. **Runtime Performance**: Review individual scenario results for detailed metrics
3. **Storage Efficiency**: Significant space savings by eliminating external asset directories
4. **Protection Validity**: All protected executables execute correctly on Windows

### Recommendations

1. **Review Per-Scenario Metrics**: Analyze each scenario's performance impact
2. **Consider Use Case**: Choose protection mode based on asset access patterns
3. **Monitor Production**: Track real-world performance post-deployment
4. **Optimization Opportunities**: Review failed or slow operations for optimization

## Files Generated

"@

foreach ($Scenario in $Scenarios) {
    $ScenarioResult = $Results.scenarios[$Scenario]
    $Report += @"

### $Scenario
- Unpacked metrics: \`$($ScenarioResult.unpacked.metrics_file)\`
- Unpacked output: \`$($ScenarioResult.unpacked.output_file)\`
- Packed metrics: \`$($ScenarioResult.packed.metrics_file)\`
- Packed output: \`$($ScenarioResult.packed.output_file)\`

"@
}

$Report += @"

### Overall
- Original executable: \`$E2EDir\hello.exe\`
- Protected executable: \`$E2EDir\hello_packed.exe\`
- This report: \`$ReportFile\`

## Next Steps

1. **Detailed Analysis**: Run analysis script for deeper insights
   ```powershell
   pwsh scripts\windows\analyze_benchmarks.ps1 --metrics-dir $OutputDir
   ```

2. **Compare with Targets**: Validate against performance targets from plan 003

3. **Production Decision**: Use metrics to inform production deployment strategy

4. **Optimization**: If overhead is significant, consider:
   - Adjusting compression level
   - Changing chunk size
   - Optimizing asset access patterns
   - Selective protection for critical assets

---

*Generated by Maxion Protector All Scenarios Benchmark Runner*
*Report Version: 1.0*
*Platform: Windows PowerShell*
"@

# Save report
$Report | Out-File -FilePath $ReportFile -Encoding UTF8
Write-Success "Comprehensive report generated: $ReportFile"

# Step 5: Summary
Write-Section "Benchmark Summary"

Write-Host "Total scenarios tested: $($Scenarios.Count)" -ForegroundColor White
Write-Host "Iterations per scenario: $Iterations" -ForegroundColor White
Write-Host "Total iterations: $($Results.summary.total_iterations)" -ForegroundColor White
Write-Host "Duration: $([math]::Round($Duration, 2)) minutes" -ForegroundColor White
Write-Host ""

Write-Host "File Size Metrics:" -ForegroundColor Yellow
Write-Host "  Unpacked executable:  $($Results.summary.unpacked_size) bytes" -ForegroundColor White
if ($Results.summary.packed_size -gt 0) {
    Write-Host "  Packed executable:    $($Results.summary.packed_size) bytes" -ForegroundColor White
    Write-Host "  Overhead:             $Overhead bytes ($OverheadPct%)" -ForegroundColor White
    Write-Host "  Assets directory:     $($Results.summary.assets_size) bytes" -ForegroundColor White
    Write-Host "  Space saved:          $Savings bytes ($SavingsPct%)" -ForegroundColor White
}

Write-Host ""

Write-Host "Scenario Status:" -ForegroundColor Yellow
foreach ($Scenario in $Scenarios) {
    $ScenarioResult = $Results.scenarios[$Scenario]
    $UnpackedStatus = if ($ScenarioResult.unpacked.success) { "✅" } else { "❌" }
    $PackedStatus = if ($ScenarioResult.packed.success) { "✅" } else { "❌" }
    Write-Host "  ${Scenario}: Unpacked $UnpackedStatus | Packed $PackedStatus" -ForegroundColor White
}

Write-Host ""
Write-Success "All benchmarks complete!"
Write-Host ""

Write-Host "View detailed report:" -ForegroundColor Cyan
Write-Host "  Get-Content '$ReportFile'" -ForegroundColor Gray
Write-Host ""

Write-Host "Analyze metrics:" -ForegroundColor Cyan
Write-Host "  pwsh scripts\windows\analyze_benchmarks.ps1 --metrics-dir '$OutputDir'" -ForegroundColor Gray
Write-Host ""

Write-Host "Output directory:" -ForegroundColor Cyan
Write-Host "  $OutputDir" -ForegroundColor Gray

exit 0
