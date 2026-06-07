# Maxion Protector Simple Benchmark Runner
# Runs simple_bench example and saves results with auto-numbering

param(
    [Parameter(Mandatory=$false)]
    [string]$OutputDir = "benchmark_results",

    [Parameter(Mandatory=$false)]
    [string]$OutputPrefix = "benchmark",

    [Parameter(Mandatory=$false)]
    [switch]$Release = $true
)

# Set error action preference
$ErrorActionPreference = "Stop"

# Get script directory and project root
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

# Set full output path
$OutputPath = Join-Path $ProjectRoot $OutputDir

Write-Host "=== Maxion Protector Simple Benchmark Runner ===" -ForegroundColor Cyan
Write-Host ""

# Create output directory if it doesn't exist
if (-not (Test-Path $OutputPath)) {
    New-Item -ItemType Directory -Force -Path $OutputPath | Out-Null
    Write-Host "Created directory: $OutputPath" -ForegroundColor Gray
    Write-Host ""
}

# Find next available number
$NextNumber = 1
do {
    $PaddedNumber = "{0:D3}" -f $NextNumber
    $ExistingFile = Join-Path $OutputPath "${PaddedNumber}_${OutputPrefix}.txt"
    if (-not (Test-Path $ExistingFile)) {
        break
    }
    $NextNumber++
} while ($true)

$OutputFile = Join-Path $OutputPath "${PaddedNumber}_${OutputPrefix}.txt"

Write-Host "Configuration:" -ForegroundColor Yellow
Write-Host "  Output directory: $OutputPath"
Write-Host "  Output file:      $OutputFile"
Write-Host "  Release build:     $Release"
Write-Host ""

# Build command arguments
$BuildArgs = @("run", "-p", "maxion-core", "--example", "simple_bench")
if ($Release) {
    $BuildArgs += "--release"
}

Write-Host "=== Running Benchmark ===" -ForegroundColor Cyan
Write-Host "Command: cargo $($BuildArgs -join ' ')" -ForegroundColor Gray
Write-Host ""

# Run benchmark and capture output
Push-Location $ProjectRoot
try {
    # Capture both stdout and stderr, suppress error action for cargo warnings
    $Output = @()
    $Process = Start-Process -FilePath "cargo" -ArgumentList $BuildArgs -RedirectStandardOutput (Join-Path $env:TEMP "bench_out.txt") -RedirectStandardError (Join-Path $env:TEMP "bench_err.txt") -NoNewWindow -Wait -PassThru
    $ExitCode = $Process.ExitCode

    # Read captured output
    $StdOut = Get-Content (Join-Path $env:TEMP "bench_out.txt") -ErrorAction SilentlyContinue
    $StdErr = Get-Content (Join-Path $env:TEMP "bench_err.txt") -ErrorAction SilentlyContinue
    $Output = @($StdOut) + @($StdErr)

    # Cleanup temp files
    Remove-Item (Join-Path $env:TEMP "bench_out.txt") -ErrorAction SilentlyContinue
    Remove-Item (Join-Path $env:TEMP "bench_err.txt") -ErrorAction SilentlyContinue

    # Save output to file
    $Output | Out-File -FilePath $OutputFile -Encoding UTF8

    if ($ExitCode -eq 0) {
        Write-Host "✓ Benchmark completed successfully" -ForegroundColor Green
        Write-Host "✓ Results saved to: $OutputFile" -ForegroundColor Green

        # Show quick summary from output
        $Output | Select-String -Pattern "PASS|FAIL|SLOW|✅|⚠️|Total Throughput|Encryption|Compression|Write|Read" | ForEach-Object {
            Write-Host "  $_" -ForegroundColor White
        }
    } else {
        Write-Host "✗ Benchmark failed with exit code: $ExitCode" -ForegroundColor Red
        Write-Host "  Output saved to: $OutputFile" -ForegroundColor Yellow
        Write-Host ""
        Write-Host "Last 20 lines of output:" -ForegroundColor Yellow
        $Output | Select-Object -Last 20 | ForEach-Object {
            Write-Host "  $_" -ForegroundColor Gray
        }
        exit $ExitCode
    }
}
finally {
    Pop-Location
}

Write-Host ""
Write-Host "=== Complete ===" -ForegroundColor Cyan
Write-Host "Run: Get-Content '$OutputFile' -Tail 50" -ForegroundColor Gray
Write-Host ""

exit 0
