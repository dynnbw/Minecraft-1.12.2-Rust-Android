@echo off
setlocal EnableExtensions DisableDelayedExpansion

cd /d "%~dp0"
if errorlevel 1 (
    echo [ERROR] Cannot open the repository directory.
    pause
    exit /b 11
)

title Minecraft 1.12.2 Rust - Release Build and Run

echo ============================================================
echo   Minecraft 1.12.2 Rust - Release Build and Run
echo ============================================================
echo.

where cargo.exe >nul 2>nul
if errorlevel 1 (
    echo [ERROR] Cargo was not found.
    echo Install Rust with rustup and reopen this terminal.
    echo.
    pause
    exit /b 10
)

if not exist "%CD%\runtime\assets\minecraft\lang\en_us.lang" (
    echo [ERROR] runtime\assets is missing or incomplete.
    echo Run Import-Assets-OneClick.cmd before building the client.
    echo.
    pause
    exit /b 12
)

echo [1/2] Building optimized client...
cargo build --release --bin mc112-client
if errorlevel 1 goto :failed

echo.
echo [2/2] Launching client...
"%CD%\target\release\mc112-client.exe" run %*
if errorlevel 1 goto :failed

exit /b 0

:failed
echo.
echo [FAILED] Build or launch failed with exit code %ERRORLEVEL%.
pause
exit /b 1
