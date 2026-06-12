@echo off
rem ============================================================
rem  First Light - one-time install (double-click to run)
rem
rem  Self-bootstrapping: if the game code is not next to this
rem  script, it installs Git, downloads the game into
rem  %USERPROFILE%\Games\Graphics-Engine, and continues there.
rem  Checks every prerequisite first and only installs what is
rem  missing: Git, VS Build Tools (C++ workload), Rust (MSVC),
rem  optionally Blender. Then builds the game and puts a
rem  "First Light" shortcut on the Desktop. Safe to re-run.
rem ============================================================
setlocal
cd /d "%~dp0"
title First Light - Install
set "REPO_URL=https://github.com/vcons002-ship-it/graphics-engine.git"
set "BRANCH=main"
set "GAME_DIR=%USERPROFILE%\Games\Graphics-Engine"
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

echo === First Light installer ===
echo.

rem ---- winget is required for any install step ----
where /q winget
if errorlevel 1 (
    echo [ERROR] winget not found. Install "App Installer" from the
    echo         Microsoft Store, then run this file again.
    goto :fail
)

rem ---- Locate or download the game code ----
if exist "Cargo.toml" (
    echo %~dp0 | findstr /i "OneDrive" >nul
    if not errorlevel 1 (
        echo [WARN] This folder is inside OneDrive. Builds create huge
        echo        temporary files that OneDrive will try to sync.
        echo        Consider moving the game folder somewhere like
        echo        %GAME_DIR%
    )
    set "GAME_DIR=%~dp0"
    goto :have_repo
)
echo [..] Game code not found next to this script - downloading it.

where /q git
if errorlevel 1 (
    echo [..] Installing Git...
    winget install --id Git.Git --exact --accept-source-agreements --accept-package-agreements
    set "PATH=%ProgramFiles%\Git\cmd;%PATH%"
)
where /q git
if errorlevel 1 (
    echo [ERROR] Git was installed but is not available yet. Close this
    echo         window and double-click the installer again.
    goto :fail
)

if exist "%GAME_DIR%\Cargo.toml" (
    echo [OK] Found existing game folder: %GAME_DIR%
    goto :have_repo
)
echo [..] Downloading the game to %GAME_DIR% ...
echo      A browser window may open asking you to sign in to GitHub.
git clone --branch %BRANCH% %REPO_URL% "%GAME_DIR%"
if errorlevel 1 (
    echo [ERROR] Download failed. Check your internet connection and
    echo         GitHub sign-in, then run this file again.
    goto :fail
)

:have_repo
cd /d "%GAME_DIR%"

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
choice /c YN /t 15 /d N /m "Install Blender for the asset pipeline - default No in 15s"
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

rem ---- Desktop shortcut to the start script ----
powershell -NoProfile -Command "$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut([Environment]::GetFolderPath('Desktop') + '\First Light.lnk'); $s.TargetPath = '%GAME_DIR%\firstlight_start.bat'; $s.WorkingDirectory = '%GAME_DIR%'; $s.Save()" >nul 2>&1
if errorlevel 1 (
    echo [WARN] Could not create a Desktop shortcut.
) else (
    echo [OK] Desktop shortcut created: First Light
)

echo.
echo === Done. ===
echo Game folder: %GAME_DIR%
echo Play: double-click "First Light" on the Desktop, or
echo       firstlight_start.bat in the game folder.
echo Update later with firstlight_update.bat in the game folder.
pause
exit /b 0

:fail
echo.
pause
exit /b 1
