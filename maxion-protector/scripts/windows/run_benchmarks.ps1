# Maxion Protector Windows Benchmark Runner
# Runs comprehensive performance benchmarks comparing unpacked vs packed executables on Windows

param(
    [Parameter(Mandatory=$false)]
    [ValidateSet("small", "medium", "large", "mixed", "all")]
    [string]$Scenario = "all",

    [Parameter(Mandatory=$false)]
    [ValidateRange(1, 100)]
    [int]$Iterations = 10,

    [Parameter(Mandatory=$false)]
    [string]$OutputPath = ""
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Get script directory and project root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $ScriptDir)
$HelloDir = Join-Path $ProjectRoot "examples\hello-world"
$OutputDir = Join-Path $ProjectRoot "target\e2e"
$BenchmarkDir = Join-Path $ProjectRoot "target\benchmarks"

# Override output path if specified
if ($OutputPath -ne "") {
    $BenchmarkDir = $OutputPath
}

Write-Host "=== Maxion Protector Windows Benchmark Runner ===" -ForegroundColor Cyan
Write-Host ""

Write-Host "Configuration:" -ForegroundColor Yellow
Write-Host "  Scenario: $Scenario"
Write-Host "  Iterations: $Iterations"
Write-Host "  Output: $BenchmarkDir"
Write-Host ""

# Create output directories
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
New-Item -ItemType Directory -Force -Path $BenchmarkDir | Out-Null

# Step 1: Build hello-world application
Write-Host "=== Step 1: Building hello-world application ===" -ForegroundColor Cyan
Write-Host "Target: x86_64-pc-windows-msvc" -ForegroundColor Gray

Push-Location $HelloDir
try {
    cargo build --release --target x86_64-pc-windows-msvc
    $HelloExe = Join-Path $HelloDir "target\x86_64-pc-windows-msvc\release\hello.exe"

    if (-not (Test-Path $HelloExe)) {
        Write-Error "Failed to build hello-world"
        exit 1
    }

    Write-Host "[OK] Built: $HelloExe" -ForegroundColor Green
} finally {
    Pop-Location
}
Write-Host ""

# Step 2: Run unpacked benchmarks
Write-Host "=== Step 2: Running unpacked benchmarks ===" -ForegroundColor Cyan
$UnpackedMetrics = Join-Path $BenchmarkDir "unpacked_metrics.json"
$UnpackedOutput = Join-Path $BenchmarkDir "unpacked_output.txt"

Push-Location $OutputDir
try {
    Write-Host "Running unpacked benchmark..." -ForegroundColor Gray

    for ($i = 1; $i -le $Iterations; $i++) {
        Write-Host "  Iteration $i/$Iterations"
        & $HelloExe $Scenario >> $UnpackedOutput 2>&1
    }

    # Check if metrics were generated
    $GeneratedMetrics = Join-Path $OutputDir "benchmark_metrics.json"
    if (Test-Path $GeneratedMetrics) {
        Move-Item -Path $GeneratedMetrics -Destination $UnpackedMetrics -Force
        Write-Host "[OK] Metrics saved to: $UnpackedMetrics" -ForegroundColor Green
    } else {
        Write-Host "[WARNING] Warning: No metrics generated" -ForegroundColor Yellow
    }
} finally {
    Pop-Location
}
Write-Host ""

# Step 3: Build maxion-packer
Write-Host "=== Step 3: Building maxion-packer ===" -ForegroundColor Cyan
Push-Location $ProjectRoot
try {
    cargo build --release -p maxion-packer
    $PackerBin = Join-Path $ProjectRoot "target\release\pnp.exe"

    if (-not (Test-Path $PackerBin)) {
        Write-Error "Failed to build maxion-packer"
        exit 1
    }

    Write-Host "[OK] Built: $PackerBin" -ForegroundColor Green
} finally {
    Pop-Location
}
Write-Host ""

# Step 4: Protect hello-world application
Write-Host "=== Step 4: Protecting hello-world application ===" -ForegroundColor Cyan
$PackedExe = Join-Path $OutputDir "hello_packed.exe"

Write-Host "Input: $HelloExe" -ForegroundColor Gray
Write-Host "Output: $PackedExe" -ForegroundColor Gray

# Run packer
try {
    $ProtectArgs = @(
        "protect",
        "--input", $HelloExe,
        "--assets", (Join-Path $OutputDir "assets"),
        "--output", $PackedExe,
        "--chunk-size", "65536",
        "--compress",
        "--compression-level", "6",
        "--stub-dll", (Join-Path $ProjectRoot "target\release\maxion_loader_stub.dll")
    )

    $ProtectResult = & $PackerBin $ProtectArgs 2>&1
    $ProtectExitCode = $LASTEXITCODE

    if ($ProtectExitCode -ne 0) {
        Write-Host "[WARNING] Warning: Protection failed" -ForegroundColor Yellow
        Write-Host $ProtectResult -ForegroundColor Gray
        Write-Host "[WARNING] Skipping packed benchmarks" -ForegroundColor Yellow
        $PackedAvailable = $false
    } else {
        if (Test-Path $PackedExe) {
            Write-Host "[OK] Protected: $PackedExe" -ForegroundColor Green
            $PackedAvailable = $true
        } else {
            Write-Host "[WARNING] Protected executable not created" -ForegroundColor Yellow
            $PackedAvailable = $false
        }
    }
} catch {
    Write-Host "[WARNING] Error during protection: $_" -ForegroundColor Yellow
    $PackedAvailable = $false
}
Write-Host ""

# Step 5: Run packed benchmarks (if available)
if ($PackedAvailable) {
    Write-Host "=== Step 5: Running packed benchmarks ===" -ForegroundColor Cyan
    $PackedMetrics = Join-Path $BenchmarkDir "packed_metrics.json"
    $PackedOutput = Join-Path $BenchmarkDir "packed_output.txt"

    Push-Location $OutputDir
    try {
        for ($i = 1; $i -le $Iterations; $i++) {
            Write-Host "  Iteration $i/$Iterations"
            & $PackedExe $Scenario >> $PackedOutput 2>&1
        }

        # Check if metrics were generated
        $GeneratedMetrics = Join-Path $OutputDir "benchmark_metrics.json"
        if (Test-Path $GeneratedMetrics) {
            Move-Item -Path $GeneratedMetrics -Destination $PackedMetrics -Force
            Write-Host "[OK] Metrics saved to: $PackedMetrics" -ForegroundColor Green
        } else {
            Write-Host "[WARNING] Warning: No metrics generated" -ForegroundColor Yellow
        }
    }
    finally {
        Pop-Location
    }
} else {
    Write-Host "=== Step 5: Packed benchmarks skipped ===" -ForegroundColor Yellow
    Write-Host "Packed executable not available"
    $PackedMetrics = ""
}
Write-Host ""

# Step 6: Generate comparison report
Write-Host "=== Step 6: Generating comparison report ===" -ForegroundColor Cyan
$Timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$ReportFile = Join-Path $BenchmarkDir "benchmark_report_$Timestamp.md"

# Get file sizes
$UnpackedSize = 0
$PackedSize = 0
$AssetsSize = 0

if (Test-Path $HelloExe) {
    $UnpackedSize = (Get-Item $HelloExe).Length
}

if ($PackedAvailable -and (Test-Path $PackedExe)) {
    $PackedSize = (Get-Item $PackedExe).Length
}

$AssetsDir = Join-Path $OutputDir "assets"
if (Test-Path $AssetsDir) {
    $AssetsSize = (Get-ChildItem $AssetsDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
}

# Calculate overhead and savings
$Overhead = $PackedSize - $UnpackedSize
$OverheadPct = if ($UnpackedSize -gt 0) { [math]::Round(($Overhead * 100.0 / $UnpackedSize), 2) } else { 0 }

$TotalUnpacked = $UnpackedSize + $AssetsSize
$Savings = $TotalUnpacked - $PackedSize
$SavingsPct = if ($TotalUnpacked -gt 0) { [math]::Round(($Savings * 100.0 / $TotalUnpacked), 2) } else { 0 }

# Generate report content
$Report = @"
# Maxion Protector Benchmark Report

**Date:** $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
**Scenario:** $Scenario
**Iterations:** $Iterations
**Platform:** Windows ($env:PROCESSOR_ARCHITECTURE)

## Executive Summary

This report compares the performance of unpacked vs packed executables using the Maxion Protector system.

## Environment

- **Scenario:** $Scenario
- **Iterations per test:** $Iterations
- **Output directory:** $BenchmarkDir

## File Size Comparison

- **Unpacked executable:** $UnpackedSize bytes
- **Packed executable:** $PackedSize bytes
- **Overhead:** $Overhead bytes ($OverheadPct%)
- **Assets directory:** $AssetsSize bytes
- **Total unpacked:** $TotalUnpacked bytes
- **Total packed:** $PackedSize bytes
- **Space saved:** $Savings bytes ($SavingsPct%)

## Performance Metrics

### Unpacked Performance

"@

if (Test-Path $UnpackedMetrics) {
    $Report += "Unpacked metrics available at: `$UnpackedMetrics`n`n"

    # Parse JSON if available
    try {
        $MetricsData = Get-Content $UnpackedMetrics -Raw | ConvertFrom-Json
        $Report += "```json`n"
        $Report += ($MetricsData | ConvertTo-Json -Depth 10)
        $Report += "`n```n`n"
    } catch {
        $Report += "Note: Could not parse metrics JSON`n"
    }
} else {
    $Report += "No metrics available`n"
}

$Report += @"

### Packed Performance

"@

if ($PackedAvailable -and $PackedMetrics -ne "" -and (Test-Path $PackedMetrics)) {
    $Report += "Packed metrics available at: `$PackedMetrics`n`n"

    # Parse JSON if available
    try {
        $MetricsData = Get-Content $PackedMetrics -Raw | ConvertFrom-Json
        $Report += "```json`n"
        $Report += ($MetricsData | ConvertTo-Json -Depth 10)
        $Report += "`n```n`n"
    } catch {
        $Report += "Note: Could not parse metrics JSON`n"
    }
} else {
    $Report += "No metrics available`n"
}

# Performance comparison
if ((Test-Path $UnpackedMetrics) -and $PackedAvailable -and $PackedMetrics -ne "" -and (Test-Path $PackedMetrics)) {
    $Report += @"

### Performance Comparison

| Operation | Unpacked (ms) | Packed (ms) | Overhead | % Overhead |
|-----------|---------------|-------------|----------|------------|
"@

    try {
        $UnpackedData = Get-Content $UnpackedMetrics -Raw | ConvertFrom-Json
        $PackedData = Get-Content $PackedMetrics -Raw | ConvertFrom-Json

        if ($UnpackedData.summary -and $UnpackedData.summary.timings) {
            foreach ($timing in $UnpackedData.summary.timings.PSObject.Properties) {
                $opName = $timing.Name
                $unpackedAvg = $timing.Value.avg_ms

                # Find corresponding timing in packed data
                $packedAvg = 0
                if ($PackedData.summary -and $PackedData.summary.timings -and $PackedData.summary.timings.$opName) {
                    $packedAvg = $PackedData.summary.timings.$opName.avg_ms
                }

                $overhead = $packedAvg - $unpackedAvg
                $overheadPct = if ($unpackedAvg -gt 0) { [math]::Round(($overhead * 100.0 / $unpackedAvg), 2) } else { 0 }

                $Report += "| $opName | $unpackedAvg | $packedAvg | $overhead | $overheadPct% |`n"
            }
        }
    } catch {
        $Report += "Note: Could not generate comparison table`n"
    }
} else {
    $Report += "`nComparison table not available (missing metrics)`n"
}

$Report += @"

## Conclusion

This benchmark shows the performance impact of using Maxion Protector for asset encryption and compression.

Key findings:
- **File Size:** Packed executable size is $PackedSize bytes ($OverheadPct% overhead over unpacked)
- **Space Savings:** Assets compressed and embedded, saving $Savings bytes ($SavingsPct%)
- **Performance:** See comparison table above for detailed metrics

---

Report generated: $Timestamp
"@

# Save report
$Report | Out-File -FilePath $ReportFile -Encoding UTF8
Write-Host "[OK] Report saved to: $ReportFile" -ForegroundColor Green
Write-Host ""

Write-Host "=== Benchmark Complete ===" -ForegroundColor Cyan
Write-Host ""
Write-Host "Summary:"
Write-Host "  Unpacked executable: $UnpackedSize bytes"
Write-Host "  Packed executable:   $PackedSize bytes"
if ($PackedAvailable) {
    Write-Host "  Overhead:             $Overhead bytes ($OverheadPct%)"
}
Write-Host "  Space saved:          $Savings bytes ($SavingsPct%)"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Review the benchmark report: $ReportFile"
Write-Host "  2. Compare performance metrics in the report"
Write-Host "  3. Adjust compression level if needed"
Write-Host ""
Write-Host "Success! Benchmark completed."
