@echo off
rem ============================================================
rem  First Light - start the game (double-click to run)
rem
rem  Runs the release build; cargo rebuilds only if sources
rem  changed. Optional arguments (from a terminal):
rem      firstlight_start.bat dev   - fast-iteration dev build
rem
rem  In game: click to grab the cursor, WASD + mouse to move,
rem  Space jump, Shift sprint, left-click throw, Esc pause menu
rem  (controls, settings, restart, exit), F2 screenshot, F3 FPS
rem  counter, F4 vsync toggle.
rem ============================================================
setlocal
cd /d "%~dp0"
title First Light
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

where /q cargo
if errorlevel 1 (
    echo [ERROR] cargo not found - run firstlight_install.bat first.
    echo.
    pause
    exit /b 1
)

if /i "%~1"=="dev" (
    cargo run -p first_light --features dev
) else (
    cargo run --release -p first_light
)

if errorlevel 1 (
    echo.
    echo [ERROR] The game failed to start or crashed - see output above.
    pause
    exit /b 1
)
exit /b 0
