# Benchmark Analysis Script for Maxion Protector
# Analyzes benchmark results and generates comprehensive reports
#
# Usage: pwsh scripts\windows\analyze_benchmarks.ps1 [options]
#
# Options:
#   --metrics-dir <path>    Directory containing benchmark JSON files (default: target\benchmarks)
#   --output-dir <path>     Directory for output reports (default: target\benchmarks)
#   --format <format>       Output format: json, markdown, or both (default: both)
#   --verbose               Enable verbose output

param(
    [string]$MetricsDir = "target\benchmarks",
    [string]$OutputDir = "target\benchmarks",
    [ValidateSet("json", "markdown", "both")]
    [string]$Format = "both",
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent (Split-Path -Parent $ScriptDir)

# Performance targets from plan 003_benchmark.md
$PerformanceTargets = @{
    "game_startup" = @{
        native = 2000
        max_overhead_pct = 2.5
    }
    "texture_load" = @{
        native = 15
        max_overhead_pct = 6.7
    }
    "audio_stream" = @{
        native = 0.5
        max_overhead_pct = 10.0
    }
    "mesh_load" = @{
        native = 5
        max_overhead_pct = 4.0
    }
    "small_assets" = @{
        native = 8
        max_overhead_pct = 12.5
    }
}

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
    Write-ColorOutput "  $Message" "Gray"
}

# Check if directory exists
function EnsureDirectory {
    param([string]$Path)
    if (-not (Test-Path $Path)) {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
        Write-Success "Created directory: $Path"
    }
}

# Parse JSON metrics file
function Read-Metrics {
    param([string]$FilePath)

    if (-not (Test-Path $FilePath)) {
        Write-Warning "Metrics file not found: $FilePath"
        return $null
    }

    try {
        $json = Get-Content $FilePath -Raw | ConvertFrom-Json
        Write-Success "Loaded metrics: $FilePath"
        return $json
    }
    catch {
        Write-Error "Failed to parse metrics file: $FilePath"
        Write-Error $_.Exception.Message
        return $null
    }
}

# Calculate statistics for timing data
function Get-TimingStats {
    param([object]$Timings)

    if (-not $Timings) {
        return $null
    }

    $values = $Timings.PSObject.Properties.Value | Where-Object { $_ -is [double] -or $_ -is [int] }

    if ($values.Count -eq 0) {
        return $null
    }

    $stats = @{
        count = $values.Count
        min = [math]::Min($values)
        max = [math]::Max($values)
        avg = ($values | Measure-Object -Average).Average
        sum = ($values | Measure-Object -Sum).Sum
    }

    # Calculate standard deviation
    if ($values.Count -gt 1) {
        $variance = ($values | ForEach-Object { [math]::Pow($_ - $stats.avg, 2) } | Measure-Object -Average).Average
        $stats.stddev = [math]::Sqrt($variance)
    } else {
        $stats.stddev = 0
    }

    # Calculate percentiles
    $sortedValues = $values | Sort-Object
    $p50Index = [math]::Floor($sortedValues.Count * 0.5)
    $p90Index = [math]::Floor($sortedValues.Count * 0.9)
    $p95Index = [math]::Floor($sortedValues.Count * 0.95)
    $p99Index = [math]::Floor($sortedValues.Count * 0.99)

    $stats.p50 = $sortedValues[$p50Index]
    $stats.p90 = $sortedValues[[math]::Min($p90Index, $sortedValues.Count - 1)]
    $stats.p95 = $sortedValues[[math]::Min($p95Index, $sortedValues.Count - 1)]
    $stats.p99 = $sortedValues[[math]::Min($p99Index, $sortedValues.Count - 1)]

    return $stats
}

# Calculate overhead percentage
function Get-Overhead {
    param(
        [double]$Unpacked,
        [double]$Packed
    )

    if ($Unpacked -eq 0) {
        return $null
    }

    return (($Packed - $Unpacked) / $Unpacked) * 100
}

# Validate against performance targets
function Test-PerformanceTarget {
    param(
        [string]$Operation,
        [double]$OverheadPct
    )

    if (-not $PerformanceTargets.ContainsKey($Operation)) {
        return @{
            passed = $true
            target = "N/A"
            actual = "{0:N2}%" -f $OverheadPct
            message = "No target defined for '$Operation'"
        }
    }

    $target = $PerformanceTargets[$Operation]

    $result = @{
        operation = $Operation
        target = "{0:N1}%" -f $target.max_overhead_pct
        actual = "{0:N2}%" -f $OverheadPct
        passed = $OverheadPct -le $target.max_overhead_pct
        native_ms = $target.native
    }

    if ($result.passed) {
        $result.message = "PASSED: Within target of {0:N1}%" -f $target.max_overhead_pct
    } else {
        $result.message = "FAILED: Exceeds target of {0:N1}%" -f $target.max_overhead_pct
    }

    return $result
}

# Generate markdown report
function Generate-MarkdownReport {
    param(
        [hashtable]$Analysis,
        [string]$OutputPath
    )

    $report = @"
# Maxion Protector Benchmark Analysis Report

**Generated:** $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
**Platform:** Windows
**Analysis Version:** 1.0

## Executive Summary

"@

    if ($Analysis.summary.total_scenarios -gt 0) {
        $report += @"
- **Scenarios Analyzed:** $($Analysis.summary.total_scenarios)
- **Operations Tested:** $($Analysis.summary.total_operations)
- **Targets Met:** $($Analysis.summary.targets_met) / $($Analysis.summary.targets_total)
- **Average Latency Overhead:** {0:N2}%`n`n
"@ -f $Analysis.summary.avg_overhead_pct
    } else {
        $report += @"
No benchmark data available for analysis.`n`n
"@
    }

    # File size comparison
    $report += @"
## File Size Comparison

| Metric | Size |
|--------|------|
| **Unpacked Executable** | {0:N0} bytes |`n
"@ -f $Analysis.files.unpacked_size

    if ($Analysis.files.packed_size -gt 0) {
        $overhead = $Analysis.files.packed_size - $Analysis.files.unpacked_size
        $overhead_pct = ($overhead / $Analysis.files.unpacked_size) * 100

        $report += @"
| **Packed Executable** | {0:N0} bytes |
| **Protection Overhead** | {1:N0} bytes ({2:N1}%) |`n
"@ -f $Analysis.files.packed_size, $overhead, $overhead_pct
    }

    if ($Analysis.files.assets_size -gt 0) {
        $report += @"
| **Assets Directory** | {0:N0} bytes |`n
"@ -f $Analysis.files.assets_size
    }

    # Performance comparison
    $report += @"

## Performance Comparison

"@

    if ($Analysis.comparisons.Count -gt 0) {
        $report += @"

### Latency Metrics

| Operation | Unpacked (ms) | Packed (ms) | Overhead | Target | Status |
|-----------|---------------|-------------|----------|--------|--------|
"@

        foreach ($comp in $Analysis.comparisons.Values) {
            $statusIcon = if ($comp.target_result.passed) { "✅" } else { "❌" }
            $report += "| {0} | {1:N3} | {2:N3} | {3:N2}% | {4} | {5} |`n" -f `
                $comp.operation,
                $comp.unpacked_avg,
                $comp.packed_avg,
                $comp.overhead_pct,
                $comp.target_result.target,
                $statusIcon
        }

        # Target validation
        $report += @"

### Target Validation

| Operation | Target | Actual | Status |
|-----------|--------|--------|--------|
"@

        foreach ($comp in $Analysis.comparisons.Values) {
            $statusIcon = if ($comp.target_result.passed) { "✅ PASSED" } else { "❌ FAILED" }
            $report += "| {0} | {1} | {2} | {3} |`n" -f `
                $comp.operation,
                $comp.target_result.target,
                $comp.target_result.actual,
                $statusIcon
        }
    } else {
        $report += "No performance data available for comparison.`n`n"
    }

    # Detailed timing statistics
    $report += @"

## Detailed Statistics

### Unpacked Executable

"@

    if ($Analysis.unpacked.timings.Count -gt 0) {
        $report += @"

| Operation | Count | Min | Max | Avg | StdDev | P50 | P90 | P95 | P99 |
|-----------|-------|-----|-----|-----|--------|-----|-----|-----|-----|
"@

        foreach ($timing in $Analysis.unpacked.timings.Values) {
            $report += "| {0} | {1} | {2:N3} | {3:N3} | {4:N3} | {5:N3} | {6:N3} | {7:N3} | {8:N3} | {9:N3} |`n" -f `
                $timing.operation,
                $timing.stats.count,
                $timing.stats.min,
                $timing.stats.max,
                $timing.stats.avg,
                $timing.stats.stddev,
                $timing.stats.p50,
                $timing.stats.p90,
                $timing.stats.p95,
                $timing.stats.p99
        }
    } else {
        $report += "No timing data available.`n`n"
    }

    $report += @"

### Packed Executable

"@

    if ($Analysis.packed.timings.Count -gt 0) {
        $report += @"

| Operation | Count | Min | Max | Avg | StdDev | P50 | P90 | P95 | P99 |
|-----------|-------|-----|-----|-----|--------|-----|-----|-----|-----|
"@

        foreach ($timing in $Analysis.packed.timings.Values) {
            $report += "| {0} | {1} | {2:N3} | {3:N3} | {4:N3} | {5:N3} | {6:N3} | {7:N3} | {8:N3} | {9:N3} |`n" -f `
                $timing.operation,
                $timing.stats.count,
                $timing.stats.min,
                $timing.stats.max,
                $timing.stats.avg,
                $timing.stats.stddev,
                $timing.stats.p50,
                $timing.stats.p90,
                $timing.stats.p95,
                $timing.stats.p99
        }
    } else {
        $report += "No timing data available.`n`n"
    }

    # Counters
    $report += @"

## Operation Counters

### Unpacked Executable

"@

    if ($Analysis.unpacked.counters.Count -gt 0) {
        $report += @"

| Counter | Value |
|---------|-------|
"@

        foreach ($counter in $Analysis.unpacked.counters.GetEnumerator()) {
            $report += "| {0} | {1:N0} |`n" -f $counter.Key, $counter.Value
        }
    } else {
        $report += "No counter data available.`n`n"
    }

    $report += @"

### Packed Executable

"@

    if ($Analysis.packed.counters.Count -gt 0) {
        $report += @"

| Counter | Value |
|---------|-------|
"@

        foreach ($counter in $Analysis.packed.counters.GetEnumerator()) {
            $report += "| {0} | {1:N0} |`n" -f $counter.Key, $counter.Value
        }
    } else {
        $report += "No counter data available.`n`n"
    }

    # Conclusion
    $report += @"

## Conclusion

"@

    if ($Analysis.summary.targets_met -eq $Analysis.summary.targets_total -and $Analysis.summary.targets_total -gt 0) {
        $report += @"
✅ **All Performance Targets Met**

The Maxion Protector performs within acceptable limits for all benchmarked operations.
The protection overhead is minimal and within the defined performance targets.

### Recommendations

1. **Deploy with Confidence**: The protection system is production-ready
2. **Monitor in Production**: Track real-world performance post-deployment
3. **Consider Further Optimization**: For use cases with extreme latency sensitivity
"@
    } elseif ($Analysis.summary.targets_total -gt 0) {
        $failed = $Analysis.summary.targets_total - $Analysis.summary.targets_met
        $report += @"
⚠️ **$failed Performance Target(s) Not Met**

The Maxion Protector exceeds acceptable overhead limits for some operations.
Review the failed operations below and consider optimization strategies.

### Failed Operations

"@

        foreach ($comp in $Analysis.comparisons.Values) {
            if (-not $comp.target_result.passed) {
                $report += @"

- **{0}**: {1}`n
  - Target: {2}
  - Actual: {3}
  - Overhead: {4:N2}%`n
  - Native baseline: {5:N1}ms
  - Unpacked average: {6:N3}ms
  - Packed average: {7:N3}ms`n
"@ -f `
                    $comp.operation,
                    $comp.target_result.message,
                    $comp.target_result.target,
                    $comp.target_result.actual,
                    $comp.overhead_pct,
                    $comp.target_result.native_ms,
                    $comp.unpacked_avg,
                    $comp.packed_avg
            }
        }

        $report += @"

### Recommendations

1. **Review Failed Operations**: Analyze why specific operations exceed targets
2. **Optimization Opportunities**:
   - Adjust compression level for large assets
   - Tune chunk size for streaming operations
   - Consider asset pre-loading strategies
3. **Acceptable Use Cases**: The protection is still suitable for many applications
4. **Alternative Approaches**: For latency-critical operations, consider selective protection
"@
    } else {
        $report += @"
ℹ️ **No Performance Data Available**

Complete benchmark execution on Windows to generate performance metrics.

### Next Steps

1. Run benchmarks: `.\scripts\windows\run_all_benchmarks.ps1`
2. Generate metrics on Windows platform
3. Re-run analysis to compare results
"@
    }

    $report += @"

## Files Analyzed

- Unpacked metrics: `unpacked_metrics.json`
- Packed metrics: `packed_metrics.json`
- Report generated: `$(Get-Date -Format "yyyy-MM-dd_HHmmss")`

---

*Report generated by Maxion Protector Benchmark Analyzer*
"@

    # Write report to file
    $report | Out-File -FilePath $OutputPath -Encoding UTF8
    Write-Success "Markdown report generated: $OutputPath"
}

# Generate JSON report
function Generate-JsonReport {
    param(
        [hashtable]$Analysis,
        [string]$OutputPath
    )

    $jsonReport = $Analysis | ConvertTo-Json -Depth 10
    $jsonReport | Out-File -FilePath $OutputPath -Encoding UTF8
    Write-Success "JSON report generated: $OutputPath"
}

# Main execution
function Main {
    Write-ColorOutput "Maxion Protector Benchmark Analyzer" "Cyan"
    Write-Host ""

    # Ensure output directory exists
    EnsureDirectory $OutputDir

    # Resolve absolute paths
    $MetricsDir = Resolve-Path $MetricsDir -ErrorAction Stop
    $OutputDir = Resolve-Path $OutputDir -ErrorAction Stop

    Write-Info "Metrics directory: $MetricsDir"
    Write-Info "Output directory: $OutputDir"

    # Find metrics files
    $unpackedMetrics = Join-Path $MetricsDir "unpacked_metrics.json"
    $packedMetrics = Join-Path $MetricsDir "packed_metrics.json"

    # Read metrics
    Write-Section "Loading Metrics"

    $unpackedData = Read-Metrics $unpackedMetrics
    $packedData = Read-Metrics $packedMetrics

    if (-not $unpackedData -and -not $packedData) {
        Write-Error "No benchmark metrics found"
        Write-Info "Please run benchmarks first: .\scripts\windows\run_all_benchmarks.ps1"
        exit 1
    }

    # Initialize analysis structure
    $analysis = @{
        metadata = @{
            timestamp = Get-Date -Format "o"
            platform = "Windows"
            version = "1.0"
        }
        files = @{
            unpacked_size = 0
            packed_size = 0
            assets_size = 0
        }
        unpacked = @{
            timings = @{}
            counters = @{}
        }
        packed = @{
            timings = @{}
            counters = @{}
        }
        comparisons = @{}
        summary = @{
            total_scenarios = 0
            total_operations = 0
            targets_met = 0
            targets_total = 0
            avg_overhead_pct = 0
        }
    }

    # Extract file sizes if available
    if ($unpackedData -and $unpackedData.PSObject.Properties["file_size"]) {
        $analysis.files.unpacked_size = $unpackedData.file_size
    }

    if ($packedData -and $packedData.PSObject.Properties["file_size"]) {
        $analysis.files.packed_size = $packedData.file_size
    }

    if ($unpackedData -and $unpackedData.PSObject.Properties["assets_size"]) {
        $analysis.files.assets_size = $unpackedData.assets_size
    }

    # Process unpacked timings
    if ($unpackedData -and $unpackedData.PSObject.Properties["summary"]) {
        if ($unpackedData.summary.PSObject.Properties["timings"]) {
            foreach ($timing in $unpackedData.summary.timings.PSObject.Properties) {
                $stats = Get-TimingStats $timing.Value
                if ($stats) {
                    $analysis.unpacked.timings[$timing.Name] = @{
                        operation = $timing.Name
                        stats = $stats
                    }
                }
            }
        }

        if ($unpackedData.summary.PSObject.Properties["counters"]) {
            foreach ($counter in $unpackedData.summary.counters.PSObject.Properties) {
                $analysis.unpacked.counters[$counter.Name] = $counter.Value
            }
        }
    }

    # Process packed timings
    if ($packedData -and $packedData.PSObject.Properties["summary"]) {
        if ($packedData.summary.PSObject.Properties["timings"]) {
            foreach ($timing in $packedData.summary.timings.PSObject.Properties) {
                $stats = Get-TimingStats $timing.Value
                if ($stats) {
                    $analysis.packed.timings[$timing.Name] = @{
                        operation = $timing.Name
                        stats = $stats
                    }
                }
            }
        }

        if ($packedData.summary.PSObject.Properties["counters"]) {
            foreach ($counter in $packedData.summary.counters.PSObject.Properties) {
                $analysis.packed.counters[$counter.Name] = $counter.Value
            }
        }
    }

    # Compare performance
    Write-Section "Comparing Performance"

    foreach ($opName in $analysis.unpacked.timings.Keys) {
        if ($analysis.packed.timings.ContainsKey($opName)) {
            $unpacked = $analysis.unpacked.timings[$opName]
            $packed = $analysis.packed.timings[$opName]

            $overhead = Get-Overhead $unpacked.stats.avg $packed.stats.avg
            $targetResult = Test-PerformanceTarget $opName $overhead

            $analysis.comparisons[$opName] = @{
                operation = $opName
                unpacked_avg = $unpacked.stats.avg
                packed_avg = $packed.stats.avg
                overhead_pct = $overhead
                target_result = $targetResult
                unpacked_stats = $unpacked.stats
                packed_stats = $packed.stats
            }

            if ($targetResult.passed) {
                Write-Success "{0}: {1:N3}ms -> {2:N3}ms ({3:N2}% overhead)" -f `
                    $opName, $unpacked.stats.avg, $packed.stats.avg, $overhead
            } else {
                Write-Warning "{0}: {1:N3}ms -> {2:N3}ms ({3:N2}% overhead) {4}" -f `
                    $opName, $unpacked.stats.avg, $packed.stats.avg, $overhead, $targetResult.message
            }
        }
    }

    # Calculate summary statistics
    $analysis.summary.total_operations = $analysis.comparisons.Count
    $analysis.summary.total_scenarios = if ($analysis.summary.total_operations -gt 0) { 4 } else { 0 }  # Assume 4 scenarios if any data

    $overheadValues = @()
    foreach ($comp in $analysis.comparisons.Values) {
        if ($comp.overhead_pct -ne $null) {
            $overheadValues += $comp.overhead_pct
        }
        if ($comp.target_result -ne $null) {
            $analysis.summary.targets_total++
            if ($comp.target_result.passed) {
                $analysis.summary.targets_met++
            }
        }
    }

    if ($overheadValues.Count -gt 0) {
        $analysis.summary.avg_overhead_pct = ($overheadValues | Measure-Object -Average).Average
    }

    # Generate reports
    Write-Section "Generating Reports"

    $timestamp = Get-Date -Format "yyyy-MM-dd_HHmmss"

    if ($Format -eq "markdown" -or $Format -eq "both") {
        $mdPath = Join-Path $OutputDir "benchmark_analysis_$timestamp.md"
        Generate-MarkdownReport $analysis $mdPath
    }

    if ($Format -eq "json" -or $Format -eq "both") {
        $jsonPath = Join-Path $OutputDir "benchmark_analysis_$timestamp.json"
        Generate-JsonReport $analysis $jsonPath
    }

    # Print summary
    Write-Section "Summary"

    Write-Host "Operations tested: $($analysis.summary.total_operations)"
    Write-Host "Targets met: $($analysis.summary.targets_met) / $($analysis.summary.targets_total)"

    if ($analysis.summary.avg_overhead_pct -gt 0) {
        Write-Host "Average overhead: {0:N2}%" -f $analysis.summary.avg_overhead_pct
    }

    if ($analysis.files.unpacked_size -gt 0) {
        Write-Host ""
        Write-Host "File sizes:"
        Write-Info "Unpacked: {0:N0} bytes" -f $analysis.files.unpacked_size
        if ($analysis.files.packed_size -gt 0) {
            $overhead = $analysis.files.packed_size - $analysis.files.unpacked_size
            $pct = ($overhead / $analysis.files.unpacked_size) * 100
            Write-Info "Packed: {0:N0} bytes (+{1:N0}, +{2:N1}%)" -f `
                $analysis.files.packed_size, $overhead, $pct
        }
    }

    Write-Host ""
    Write-Success "Analysis complete!"
    Write-Host ""

    # Exit with appropriate code
    if ($analysis.summary.targets_total -gt 0 -and $analysis.summary.targets_met -lt $analysis.summary.targets_total) {
        Write-Warning "Some performance targets not met"
        exit 1
    } elseif ($analysis.summary.targets_total -eq 0) {
        Write-Warning "No performance data to validate"
        exit 2
    }

    exit 0
}

# Run main function
Main
