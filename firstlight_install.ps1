<#
First Light — one-time setup for a fresh Windows machine.

Installs the toolchain (Visual Studio Build Tools C++ workload, rustup with
the MSVC toolchain), then builds the game in release mode. Idempotent — safe
to re-run if anything fails partway.

Usage, from a PowerShell prompt in the repo root:
    powershell -ExecutionPolicy Bypass -File .\firstlight_install.ps1
Options:
    -WithBlender   also install Blender (only needed to regenerate assets)
#>
param(
    [switch]$WithBlender
)

$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

function Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }

if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    throw "winget not found. Install 'App Installer' from the Microsoft Store, then re-run."
}

Step "Visual Studio Build Tools (C++ workload) — large download on first install"
winget install --id Microsoft.VisualStudio.2022.BuildTools --exact `
    --accept-source-agreements --accept-package-agreements `
    --override "--quiet --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
# 0x8A15002B = "no applicable update found", i.e. already installed.
if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne -1978335189) {
    Write-Warning "winget exited with code $LASTEXITCODE — fine if Build Tools are already installed."
}

Step "Rust toolchain (stable MSVC)"
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    winget install --id Rustlang.Rustup --exact `
        --accept-source-agreements --accept-package-agreements
}
# winget doesn't refresh this session's PATH; add cargo's bin dir directly.
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ($env:Path -notlike "*$cargoBin*") { $env:Path = "$cargoBin;$env:Path" }
rustup toolchain install stable-msvc
rustup default stable-msvc

if ($WithBlender) {
    Step "Blender (procedural asset pipeline)"
    winget install --id BlenderFoundation.Blender --exact `
        --accept-source-agreements --accept-package-agreements
}

Step "Building First Light (release) — the first build takes 15-20 minutes"
cargo build --release -p first_light
if ($LASTEXITCODE -ne 0) {
    throw "Build failed. Most common cause: incomplete C++ workload — reboot if Build Tools were just installed, then re-run this script."
}

Step "Done. Start the game with: .\firstlight_start.ps1"
