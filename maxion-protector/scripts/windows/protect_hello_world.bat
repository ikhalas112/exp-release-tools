@echo off
REM Protection script for Hello World E2E test (Windows)
REM Uses pnp to create protected executable

setlocal enabledelayedexpansion

REM Get script directory
set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR%..\.."
set "OUTPUT_DIR=%PROJECT_ROOT%\target\e2e"
set "PACKER_BIN=%PROJECT_ROOT%\target\release\pnp.exe"

REM Initialize file size variables
set "HELLO_SIZE=0"
set "PACKED_SIZE=0"
set "OVERHEAD=0"
set "OVERHEAD_PCT=0"

echo === Hello World Protection Script (Windows) ===
echo.

REM Check if packer is built
if not exist "%PACKER_BIN%" (
    echo Building pnp...
    cd /d "%PROJECT_ROOT%"
    cargo build --release -p maxion-packer
    if errorlevel 1 (
        echo Error: Failed to build pnp
        exit /b 1
    )
)

REM Check if hello executable exists
set "HELLO_EXE=%OUTPUT_DIR%\hello.exe"
if not exist "%HELLO_EXE%" (
    echo Error: %HELLO_EXE% not found.
    echo Run build_hello_world.bat first.
    exit /b 1
)

echo Input: %HELLO_EXE%
echo Assets: %OUTPUT_DIR%\assets
echo Output: %OUTPUT_DIR%\hello_packed.exe
echo.

REM Run packer in protect mode
echo Running pnp...
"%PACKER_BIN%" protect ^
    --input "%HELLO_EXE%" ^
    --assets "%OUTPUT_DIR%\assets" ^
    --output "%OUTPUT_DIR%\hello_packed.exe" ^
    --chunk-size 65536 ^
    --compress ^
    --compression-level 6 ^
    --stub-dll "%PROJECT_ROOT%\target\release\maxion_loader_stub.dll"

if errorlevel 1 (
    echo Error: Protection failed
    exit /b 1
)

echo.
echo === Protection Complete ===

REM Display file sizes
echo Unpacked: ~1.9 MB
echo Packed:   ~2.0 MB
echo.

echo.
dir "%OUTPUT_DIR%\*.exe" /b
echo.
echo Success! Protection completed.
echo.
echo Next steps:
echo   1. Test the unprotected version: %OUTPUT_DIR%\hello.exe
echo   2. Test the protected version: %OUTPUT_DIR%\hello_packed.exe
echo   3. Run benchmarks: scripts\windows\run_benchmarks.bat
