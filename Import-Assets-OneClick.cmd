@echo off
setlocal EnableExtensions DisableDelayedExpansion

cd /d "%~dp0"
if errorlevel 1 (
    echo [ERROR] Cannot open the project directory.
    pause
    exit /b 11
)

title Minecraft 1.12.2 Asset Importer
echo ============================================================
echo   Minecraft 1.12.2 + OptiFine Asset Importer
echo ============================================================
echo.

set "PYTHON_EXE="
set "PYTHON_PREFIX="

where py.exe >nul 2>nul
if not errorlevel 1 (
    set "PYTHON_EXE=py.exe"
    set "PYTHON_PREFIX=-3"
    goto :python_found
)

where python.exe >nul 2>nul
if not errorlevel 1 (
    set "PYTHON_EXE=python.exe"
    goto :python_found
)

where python3.exe >nul 2>nul
if not errorlevel 1 (
    set "PYTHON_EXE=python3.exe"
    goto :python_found
)

echo [ERROR] Python 3 was not found.
echo Install Python 3 and enable "Add Python to PATH".
echo.
pause
exit /b 10

:python_found
set "PYTHONUTF8=1"
set "PYTHONIOENCODING=utf-8:replace"
chcp 65001 >nul 2>nul

if not exist "%CD%\tools\one_click_import_assets.py" (
    echo [ERROR] Missing tools\one_click_import_assets.py
    echo Re-extract the complete project archive.
    echo.
    pause
    exit /b 12
)

"%PYTHON_EXE%" %PYTHON_PREFIX% -X utf8 "%CD%\tools\one_click_import_assets.py" --project-root "%CD%" %*
set "EXIT_CODE=%ERRORLEVEL%"

echo.
if "%EXIT_CODE%"=="0" (
    echo [DONE] Assets were imported into runtime\assets.
) else (
    echo [FAILED] Importer exit code: %EXIT_CODE%
)
echo.
pause
exit /b %EXIT_CODE%
