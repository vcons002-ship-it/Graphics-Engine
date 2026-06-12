<#
First Light — start the game.

Default is the release build (full performance); cargo rebuilds only when
sources changed, so starting an up-to-date build is quick.

Usage, from a PowerShell prompt in the repo root:
    powershell -ExecutionPolicy Bypass -File .\firstlight_start.ps1
Options:
    -Dev   fast-iteration dev build (dynamic linking + dev tooling)
    -Fps   release build with the F3 FPS overlay available

In-game: click to grab the cursor, WASD + mouse to move, Space jump,
Shift sprint, left-click throw, Esc release cursor, F2 screenshot,
F4 vsync toggle.
#>
param(
    [switch]$Dev,
    [switch]$Fps
)
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if ($env:Path -notlike "*$cargoBin*") { $env:Path = "$cargoBin;$env:Path" }

if ($Dev) {
    cargo run -p first_light --features dev
}
elseif ($Fps) {
    cargo run --release -p first_light --features dev_tools
}
else {
    cargo run --release -p first_light
}
