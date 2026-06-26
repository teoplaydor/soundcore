<#
.SYNOPSIS
    Build a Release SoundCore.msi.

.DESCRIPTION
    Runs the full Release pipeline:
      cargo build --release          → soundcore-core.exe, soundcore_apo_core.lib
      cmake --build Release          → SoundCoreApo.dll, SoundCoreVirtualCamera.dll
      dotnet publish (UI)            → self-contained SoundCore.UI.exe
      wix build installer/SoundCore.wxs

    Requires the .NET-based WiX tool to be installed:
        dotnet tool install --global wix

.PARAMETER SignToolPath
    Optional. Full path to signtool.exe; when provided, all DLL/EXE artifacts
    and the final MSI are Authenticode-signed using $CertSubject.

.PARAMETER CertSubject
    Subject Name of the code-signing cert to use (passed to signtool /n).
#>

[CmdletBinding()]
param(
    [string]$SignToolPath = $null,
    [string]$CertSubject = $null
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path "$PSScriptRoot\..").Path
$env:Path = "$env:USERPROFILE\.cargo\bin;C:\Program Files\CMake\bin;$env:Path"

function Section($label) {
    Write-Host ""
    Write-Host "==================================================" -ForegroundColor Cyan
    Write-Host "  $label" -ForegroundColor Cyan
    Write-Host "==================================================" -ForegroundColor Cyan
}

# Run the layered build in Release.
Section "Release build pipeline"
& "$PSScriptRoot\build.ps1" -Configuration Release
if ($LASTEXITCODE -ne 0) { throw "build.ps1 failed" }

# Publish the UI as a self-contained x64 binary so the MSI doesn't have to
# carry the .NET 8 runtime as a separate prerequisite.
Section "dotnet publish (UI)"
$uiProj = Join-Path $RepoRoot 'ui\SoundCore.UI\SoundCore.UI.csproj'
dotnet publish $uiProj -c Release -p:Platform=x64 -r win-x64 --self-contained true `
    -p:PublishSingleFile=false `
    -p:WindowsAppSDKSelfContained=true `
    --nologo
if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed" }

if ($SignToolPath -and $CertSubject) {
    Section "Signing native binaries"
    $bins = @(
        "$RepoRoot\target\release\soundcore-core.exe",
        "$RepoRoot\native\build\x64\apo\Release\SoundCoreApo.dll",
        "$RepoRoot\native\build\x64\virtual-camera\Release\SoundCoreVirtualCamera.dll",
        "$RepoRoot\ui\SoundCore.UI\bin\x64\Release\net8.0-windows10.0.19041.0\win-x64\publish\SoundCore.UI.exe"
    )
    foreach ($bin in $bins) {
        & $SignToolPath sign /n $CertSubject /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $bin
        if ($LASTEXITCODE -ne 0) { throw "signtool failed for $bin" }
    }
}

Section "WiX build"
$wixBin = Get-Command wix -ErrorAction SilentlyContinue
if (-not $wixBin) {
    throw "WiX is not installed. Run: dotnet tool install --global wix"
}
$installerOut = Join-Path $RepoRoot 'build\SoundCore.msi'
New-Item -ItemType Directory -Force -Path (Split-Path $installerOut) | Out-Null

wix build (Join-Path $RepoRoot 'installer\SoundCore.wxs') `
    -arch x64 `
    -d "SoundCoreRoot=$RepoRoot" `
    -o $installerOut
if ($LASTEXITCODE -ne 0) { throw "wix build failed" }

if ($SignToolPath -and $CertSubject) {
    & $SignToolPath sign /n $CertSubject /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $installerOut
    if ($LASTEXITCODE -ne 0) { throw "signtool failed for MSI" }
}

Section "Done"
Write-Host "Installer: $installerOut"
