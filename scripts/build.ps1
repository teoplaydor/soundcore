<#
.SYNOPSIS
    SoundCore single-exe build.

.DESCRIPTION
    Builds the C++ DLLs (APO, Virtual Camera, JUCE host) in Release
    config, then `cargo build`s the single SoundCore.exe that embeds the
    Release DLLs as `include_bytes!`. End artefact: ONE .exe with no
    runtime dependencies beyond Windows itself.

.PARAMETER Configuration
    Debug | Release. Defaults to Debug.

.PARAMETER SkipNative
    Skip the C++/CMake stage (assume artefacts are already up to date).
#>

[CmdletBinding()]
param(
    [ValidateSet('Debug','Release')]
    [string]$Configuration = 'Debug',
    [switch]$SkipNative
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path "$PSScriptRoot\..").Path
$RustTargetDir = Join-Path $RepoRoot 'target'
$NativeDir = Join-Path $RepoRoot 'native'
$NativeBuildDir = Join-Path $NativeDir 'build/x64'

$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\CMake\bin;$env:Path"

function Section($label) {
    Write-Host ""
    Write-Host "==================================================" -ForegroundColor Cyan
    Write-Host "  $label" -ForegroundColor Cyan
    Write-Host "==================================================" -ForegroundColor Cyan
}

# ----- 1. Native C++ (always Release — Rust always links the release CRT) ---
if (-not $SkipNative) {
    Section "CMake configure + build (Release)"
    if (-not (Test-Path $NativeBuildDir)) {
        New-Item -ItemType Directory -Force -Path $NativeBuildDir | Out-Null
    }
    $generator = if (Get-Command ninja -ErrorAction SilentlyContinue) {
        'Ninja Multi-Config'
    } else {
        'Visual Studio 17 2022'
    }
    $cmakeArgs = @(
        '-S', $NativeDir,
        '-B', $NativeBuildDir,
        '-G', $generator,
        "-DSOUNDCORE_RUST_TARGET_DIR=$RustTargetDir"
    )
    if ($generator -like 'Visual Studio*') { $cmakeArgs += @('-A', 'x64') }
    cmake @cmakeArgs
    if ($LASTEXITCODE -ne 0) { throw "cmake configure failed" }

    # Build only the two targets that get embedded. JUCE host static
    # lib isn't needed until APO chain wiring lands.
    cmake --build $NativeBuildDir --config Release --target SoundCoreApo --parallel
    if ($LASTEXITCODE -ne 0) { throw "build SoundCoreApo failed" }
    cmake --build $NativeBuildDir --config Release --target SoundCoreVirtualCamera --parallel
    if ($LASTEXITCODE -ne 0) { throw "build SoundCoreVirtualCamera failed" }
}

# ----- 2. Single SoundCore.exe -----
Section "Cargo build ($Configuration) — single SoundCore.exe"
Push-Location $RepoRoot
try {
    $cargoArgs = @('build', '-p', 'soundcore-core-service')
    if ($Configuration -eq 'Release') { $cargoArgs += '--release' }
    # Force a re-run of build.rs so freshly-built Release DLLs are picked up.
    cargo clean -p soundcore-core-service | Out-Null
    cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally { Pop-Location }

$profileDir = if ($Configuration -eq 'Release') { 'release' } else { 'debug' }
$exePath = Join-Path $RustTargetDir "$profileDir\SoundCore.exe"
$sizeMb = if (Test-Path $exePath) { [math]::Round((Get-Item $exePath).Length / 1MB, 1) } else { 0 }

Section "Done"
Write-Host "Single binary: $exePath" -ForegroundColor Green
Write-Host "Size: $sizeMb MB"
Write-Host ""
Write-Host "Run:  $exePath"
Write-Host '      (will prompt for UAC, then auto-extracts and registers embedded DLLs)'
