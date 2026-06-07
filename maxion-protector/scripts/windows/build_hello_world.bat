@echo off
REM Build script for Hello World E2E test (Windows)
REM Builds the hello-world application and prepares assets

setlocal enabledelayedexpansion

REM Get script directory
set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR%..\.."
set "EXAMPLES_DIR=%PROJECT_ROOT%\examples"
set "HELLO_DIR=%EXAMPLES_DIR%\hello-world"
set "ASSETS_DIR=%EXAMPLES_DIR%\assets"
set "OUTPUT_DIR=%PROJECT_ROOT%\target\e2e"

echo === Hello World E2E Build Script (Windows) ===
echo Project root: %PROJECT_ROOT%
echo.

REM Create output directory
if not exist "%OUTPUT_DIR%" mkdir "%OUTPUT_DIR%"

REM Build hello-world application
echo Building hello-world...
cd /d "%HELLO_DIR%"

cargo build --release
if errorlevel 1 (
    echo Error: Failed to build hello-world
    exit /b 1
)

REM Copy executable
copy /Y "%HELLO_DIR%\target\release\hello.exe" "%OUTPUT_DIR%\hello.exe" >nul
if errorlevel 1 (
    echo Error: Failed to copy hello.exe
    exit /b 1
)

echo.

REM Copy assets
echo Copying assets...
if not exist "%OUTPUT_DIR%\assets" mkdir "%OUTPUT_DIR%\assets"
if exist "%ASSETS_DIR%\*" (
    copy /Y "%ASSETS_DIR%\*" "%OUTPUT_DIR%\assets\" >nul 2>&1
)

REM Display results
echo.
echo === Build Complete ===
echo Output directory: %OUTPUT_DIR%
dir "%OUTPUT_DIR%\" /b

echo.
echo Success! Build completed.
echo.
echo Next steps:
echo   1. Protect the application: scripts\windows\protect_hello_world.bat
echo   2. Run benchmarks: scripts\windows\run_benchmarks.bat
