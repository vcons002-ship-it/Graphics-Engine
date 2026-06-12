<#
First Light — update after engine or game changes.

Pulls the latest code on the current branch, refreshes the Rust toolchain,
regenerates procedural Blender assets if generator scripts exist, and
rebuilds the release binary.

Usage, from a PowerShell prompt in the repo root:
    powershell -ExecutionPolicy Bypass -File .\firstlight_update.ps1
#>
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

function Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ($env:Path -notlike "*$cargoBin*") { $env:Path = "$cargoBin;$env:Path" }

Step "Pulling latest code"
$branch = git rev-parse --abbrev-ref HEAD
git pull --ff-only origin $branch
if ($LASTEXITCODE -ne 0) {
    throw "git pull failed — commit/stash local changes or resolve conflicts, then re-run."
}

Step "Updating Rust toolchain"
rustup update stable-msvc

# Regenerate .glb assets when generator scripts exist and Blender is installed.
$blenderScripts = Get-ChildItem -Path (Join-Path $PSScriptRoot 'tools\blender') `
    -Filter '*.py' -ErrorAction SilentlyContinue
if ($blenderScripts -and (Get-Command blender -ErrorAction SilentlyContinue)) {
    Step "Regenerating Blender assets"
    foreach ($script in $blenderScripts) {
        blender --background --python $script.FullName
        if ($LASTEXITCODE -ne 0) { throw "Asset generation failed: $($script.Name)" }
    }
}

Step "Rebuilding First Light (release)"
cargo build --release -p first_light
if ($LASTEXITCODE -ne 0) { throw "Build failed." }

Step "Up to date. Start the game with: .\firstlight_start.ps1"
