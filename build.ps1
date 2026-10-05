[CmdletBinding()]
param(
    [switch]$SkipBuildToolsInstall,
    [switch]$DebugBuild
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$TargetTriple = 'x86_64-pc-windows-msvc'
$OutputDirectory = Join-Path $ProjectRoot 'outputs'

function Write-Step([string]$Message) {
    Write-Host "`n==> $Message" -ForegroundColor Cyan
}

function Test-MsvcBuildTools {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        $installPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
        if ($LASTEXITCODE -eq 0 -and $installPath) { return $true }
    }
    return [bool](Get-Command cl.exe -ErrorAction SilentlyContinue)
}

function Enable-VsDevEnvironment {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { throw 'vswhere.exe wurde nicht gefunden; die Visual Studio Build Tools sind unvollständig.' }
    $installPath = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null
    if ($LASTEXITCODE -ne 0 -or -not $installPath) { throw 'Die C++ Build Tools wurden nicht gefunden.' }
    $devCmd = Join-Path $installPath 'Common7\Tools\VsDevCmd.bat'
    if (-not (Test-Path $devCmd)) { throw "VsDevCmd.bat fehlt: $devCmd" }

    $environmentLines = & $env:ComSpec /d /s /c ('call "' + $devCmd + '" -no_logo -arch=x64 -host_arch=x64 >nul && set')
    if ($LASTEXITCODE -ne 0) { throw 'Die Visual Studio Entwicklerumgebung konnte nicht initialisiert werden.' }
    foreach ($line in $environmentLines) {
        $match = [regex]::Match($line, '^([^=]+)=(.*)$')
        if ($match.Success) {
            Set-Item -Path ('Env:' + $match.Groups[1].Value) -Value $match.Groups[2].Value
        }
    }
}

function Ensure-Rust {
    if (-not (Get-Command rustup.exe -ErrorAction SilentlyContinue)) {
        Write-Step 'Rust-Installer herunterladen und Rust installieren'
        $installer = Join-Path $env:TEMP 'eco-view-rustup-init.exe'
        Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile $installer
        try {
            & $installer -y --default-host $TargetTriple --default-toolchain stable --profile minimal
            if ($LASTEXITCODE -ne 0) { throw "Rustup-Installation endete mit Code $LASTEXITCODE." }
        } finally {
            Remove-Item -LiteralPath $installer -Force -ErrorAction SilentlyContinue
        }
        $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    }

    if (Get-Command rustup.exe -ErrorAction SilentlyContinue) {
        $activeToolchain = (& rustup.exe show active-toolchain 2>$null | Select-Object -First 1) -split '\s+' | Select-Object -First 1
        $rustHost = (& rustc.exe -vV 2>$null | Select-String '^host:').ToString().Split(':', 2)[1].Trim()
        $installedTargets = & rustup.exe target list --installed 2>$null
        if ($activeToolchain -eq "stable-$TargetTriple" -and $rustHost -eq $TargetTriple -and $installedTargets -contains $TargetTriple) { return }

        & rustup.exe toolchain install stable-$TargetTriple --profile minimal
        if ($LASTEXITCODE -ne 0) { throw 'Die Rust-MSVC-Toolchain konnte nicht installiert werden.' }
        & rustup.exe target add $TargetTriple --toolchain stable-$TargetTriple
        if ($LASTEXITCODE -ne 0) { throw 'Das Windows-MSVC-Ziel konnte nicht installiert werden.' }
    }
}

function Ensure-BuildTools {
    if (Test-MsvcBuildTools) { return }
    if ($SkipBuildToolsInstall) {
        throw 'Microsoft C++ Build Tools fehlen. Installiere Visual Studio Build Tools 2022 mit der Workload "Desktopentwicklung mit C++" und starte das Skript erneut.'
    }
    if (-not (Get-Command winget.exe -ErrorAction SilentlyContinue)) {
        throw 'Microsoft C++ Build Tools fehlen und winget ist nicht verfügbar. Installiere Visual Studio Build Tools 2022 mit der Workload "Desktopentwicklung mit C++".'
    }

    Write-Step 'Microsoft C++ Build Tools und Windows SDK installieren (Rust MSVC braucht den Linker)'
    & winget.exe install --id Microsoft.VisualStudio.2022.BuildTools --exact --accept-package-agreements --accept-source-agreements --override '--wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    if ($LASTEXITCODE -ne 0 -and -not (Test-MsvcBuildTools)) {
        throw 'Die Installation der Visual Studio Build Tools ist fehlgeschlagen. Starte PowerShell als Administrator und führe build.ps1 erneut aus.'
    }
    if (-not (Test-MsvcBuildTools)) {
        throw 'Die Build Tools wurden noch nicht erkannt. Schließe offene Installer ab und führe build.ps1 erneut aus.'
    }
}

Push-Location $ProjectRoot
try {
    Write-Step 'Voraussetzungen prüfen'
    Ensure-BuildTools
    Ensure-Rust
    Enable-VsDevEnvironment

    if (-not (Get-Command cargo.exe -ErrorAction SilentlyContinue)) {
        $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    }
    if (-not (Get-Command cargo.exe -ErrorAction SilentlyContinue)) { throw 'Cargo wurde nicht gefunden. Öffne ein neues Terminal und starte build.ps1 erneut.' }

    Write-Step 'eco view für Windows x64 bauen'
    $buildArgs = @()
    if (Get-Command rustup.exe -ErrorAction SilentlyContinue) { $buildArgs += '+stable-' + $TargetTriple }
    $buildArgs += @('build', '--target', $TargetTriple)
    if (-not $DebugBuild) { $buildArgs += '--release' }
    & cargo.exe @buildArgs
    if ($LASTEXITCODE -ne 0) { throw "Cargo-Build fehlgeschlagen (Code $LASTEXITCODE)." }

    $profile = if ($DebugBuild) { 'debug' } else { 'release' }
    $builtExe = Join-Path $ProjectRoot "target\$TargetTriple\$profile\eco-view.exe"
    if (-not (Test-Path $builtExe)) { throw "Build erfolgreich gemeldet, aber $builtExe wurde nicht gefunden." }

    New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
    $deliverable = Join-Path $OutputDirectory 'eco-view.exe'
    try {
        Copy-Item -LiteralPath $builtExe -Destination $deliverable -Force -ErrorAction Stop
    } catch {
        # Windows cannot replace an executable while it is open. Keep the build usable by
        # writing a uniquely named deliverable instead of terminating after compilation.
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $deliverable = Join-Path $OutputDirectory "eco-view-$stamp.exe"
        Copy-Item -LiteralPath $builtExe -Destination $deliverable -Force -ErrorAction Stop
        Write-Warning 'eco-view.exe ist gesperrt; die neue Version wurde unter einem zeitgestempelten Namen gespeichert.'
    }
    Write-Host "`nFertig: $deliverable" -ForegroundColor Green
} finally {
    Pop-Location
}
