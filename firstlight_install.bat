@echo off
rem ============================================================
rem  First Light - one-time install (double-click to run)
rem
rem  Checks each prerequisite first and only installs what is
rem  missing: VS Build Tools (C++ workload), Rust (MSVC), and
rem  optionally Blender. Then builds the game in release mode.
rem  Safe to re-run at any time.
rem ============================================================
setlocal
cd /d "%~dp0"
title First Light - Install
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

echo === First Light installer ===
echo.

rem ---- winget (needed only if something has to be installed) ----
where /q winget
if errorlevel 1 (
    echo [ERROR] winget not found. Install "App Installer" from the
    echo         Microsoft Store, then run this file again.
    goto :fail
)

rem ---- Visual Studio Build Tools: C++ workload ----
set "HAVE_MSVC="
if exist "%VSWHERE%" (
    for /f "usebackq delims=" %%i in (`"%VSWHERE%" -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "HAVE_MSVC=%%i"
)
if defined HAVE_MSVC (
    echo [OK] MSVC C++ build tools already installed.
    goto :rust
)
echo [..] Installing Visual Studio Build Tools C++ workload - large download...
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact --accept-source-agreements --accept-package-agreements --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
if errorlevel 1 (
    echo [ERROR] Build Tools install failed.
    goto :fail
)

:rust
rem ---- Rust toolchain (rustup, stable MSVC) ----
where /q rustup
if errorlevel 1 (
    echo [..] Installing rustup...
    winget install --id Rustlang.Rustup --exact --accept-source-agreements --accept-package-agreements
    if errorlevel 1 (
        echo [ERROR] rustup install failed.
        goto :fail
    )
) else (
    echo [OK] rustup already installed.
)
rustup toolchain list 2>nul | findstr /c:"stable-" >nul
if errorlevel 1 (
    echo [..] Installing stable MSVC toolchain...
    rustup toolchain install stable-msvc
    if errorlevel 1 goto :fail
)
rustup default 2>nul | findstr /c:"stable-" >nul
if errorlevel 1 rustup default stable-msvc

rem ---- Blender (optional - only needed to regenerate assets) ----
where /q blender
if not errorlevel 1 goto :build
if exist "%ProgramFiles%\Blender Foundation" (
    echo [OK] Blender already installed.
    goto :build
)
choice /c YN /t 15 /d N /m "Install Blender for the asset pipeline (default No in 15s)"
if errorlevel 2 goto :build
winget install --id BlenderFoundation.Blender --exact --accept-source-agreements --accept-package-agreements

:build
rem ---- Build the game ----
echo.
if exist "target\release\first_light.exe" (
    echo [..] Rebuilding game if sources changed...
) else (
    echo [..] Building First Light - the first build takes 15-20 minutes...
)
cargo build --release -p first_light
if errorlevel 1 (
    echo [ERROR] Build failed. If Build Tools were just installed, reboot
    echo         and run this file again.
    goto :fail
)

echo.
echo === Done. Double-click firstlight_start.bat to play. ===
pause
exit /b 0

:fail
echo.
pause
exit /b 1
