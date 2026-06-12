@echo off
rem ============================================================
rem  First Light - update (double-click to run)
rem
rem  Pulls the latest code on the current branch, updates the
rem  Rust toolchain if needed, regenerates Blender assets when
rem  generator scripts exist, and rebuilds the game.
rem ============================================================
setlocal
cd /d "%~dp0"
title First Light - Update
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

echo === First Light updater ===
echo.

where /q git
if errorlevel 1 (
    echo [ERROR] git not found on PATH.
    goto :fail
)
where /q cargo
if errorlevel 1 (
    echo [ERROR] cargo not found - run firstlight_install.bat first.
    goto :fail
)

rem ---- Pull latest code on the current branch ----
for /f "delims=" %%b in ('git rev-parse --abbrev-ref HEAD') do set "BRANCH=%%b"
echo [..] Pulling latest code on branch %BRANCH%...
git pull --ff-only origin %BRANCH%
if errorlevel 1 (
    echo [ERROR] git pull failed - commit/stash local changes first.
    goto :fail
)

rem ---- Update Rust toolchain (engine may require newer stable) ----
echo [..] Checking Rust toolchain...
rustup update stable-msvc

rem ---- Regenerate procedural assets if scripts and Blender exist ----
if not exist "tools\blender\*.py" goto :build
where /q blender
if errorlevel 1 (
    echo [..] Blender not on PATH - skipping asset regeneration.
    goto :build
)
echo [..] Regenerating Blender assets...
for %%s in (tools\blender\*.py) do (
    blender --background --python "%%s"
    if errorlevel 1 (
        echo [ERROR] Asset generation failed: %%s
        goto :fail
    )
)

:build
rem ---- Rebuild ----
echo [..] Rebuilding First Light...
cargo build --release -p first_light
if errorlevel 1 (
    echo [ERROR] Build failed.
    goto :fail
)

echo.
echo === Up to date. Double-click firstlight_start.bat to play. ===
pause
exit /b 0

:fail
echo.
pause
exit /b 1
