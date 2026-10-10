#Requires -Version 7.0
<#
.SYNOPSIS
    Сборка YaMusic (Rust + Qt 6.12) для Windows: exe, портативная папка и инсталлер Inno Setup.

.DESCRIPTION
    1. Находит Qt 6.12 MSVC (qmake6.exe, windeployqt.exe) и выставляет QMAKE/PATH для cxx-qt.
    2. Находит или скачивает libmpv-2.dll (shinchiro/mpv-winbuild-cmake, mpv-dev-x86_64).
    3. cargo build --release -p yamusic (или -CargoProfile debug).
    4. Собирает target\dist\yamusic\: yamusic.exe, qml\ (src\yamusic\qml), assets\ (шрифты, значок),
       Qt через windeployqt --qmldir, рантайм MSVC, libmpv-2.dll.
    5. Инсталлер через Inno Setup → target\dist\YaMusic-<версия>-Setup.exe.

.PARAMETER QtDir
    Префикс Qt (каталог с bin\qmake6.exe). По умолчанию: $env:QTDIR, каталог $env:QMAKE,
    C:\Qt\6.12.0\msvc2022_64, ~\Qt\6.12.0\msvc2022_64, ~\Qt\6.12.0.

.PARAMETER MpvDll
    Путь к libmpv-2.dll (или mpv-2.dll). По умолчанию — из корня проекта, иначе скачивается.

.PARAMETER Version
    Версия в инсталлере. По умолчанию — [workspace.package] version из Cargo.toml.

.PARAMETER OutDir
    Куда класть результат. По умолчанию target\dist.

.PARAMETER CargoProfile
    Профиль cargo: release (по умолчанию) или debug.

.PARAMETER SkipBuild
    Не запускать cargo (взять готовый target\<профиль>\yamusic.exe).

.PARAMETER SkipInstaller
    Без шага Inno Setup.

.PARAMETER Portable
    Дополнительно упаковать папку в YaMusic-<версия>-windows-x64-portable.zip.
#>
param(
    [string]$QtDir,
    [string]$MpvDll,
    [string]$Version,
    [string]$OutDir,
    [ValidateSet("release", "debug")]
    [string]$CargoProfile = "release",
    [switch]$SkipBuild,
    [switch]$SkipInstaller,
    [switch]$Portable
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$ProjectRoot = $PSScriptRoot
if (-not $OutDir) { $OutDir = Join-Path $ProjectRoot "target\dist" }
$OutDir = [IO.Path]::GetFullPath($OutDir)
$Stage = Join-Path $OutDir "yamusic"

function Write-Step { param([string]$msg) Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok   { param([string]$msg) Write-Host "    OK: $msg" -ForegroundColor Green }
function Write-Warn { param([string]$msg) Write-Host "    WARN: $msg" -ForegroundColor Yellow }
function Abort      { param([string]$msg) Write-Host "`nОШИБКА: $msg" -ForegroundColor Red; exit 1 }

function Find-SevenZip {
    $cmd = Get-Command 7z -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    foreach ($p in @("C:\Program Files\7-Zip\7z.exe", "C:\Program Files (x86)\7-Zip\7z.exe")) {
        if (Test-Path $p) { return $p }
    }
    return $null
}

# ─── Версия ───────────────────────────────────────────────────────────────────
if (-not $Version) {
    $cargoToml = Get-Content (Join-Path $ProjectRoot "Cargo.toml") -Raw
    if ($cargoToml -match '(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"') {
        $Version = $Matches[1]
    } else {
        $Version = "0.0.0"
    }
}
$Version = $Version.TrimStart("v")
Write-Host "YaMusic $Version → $OutDir" -ForegroundColor White

# ─── 1. Qt ────────────────────────────────────────────────────────────────────
Write-Step "Поиск Qt 6.12 (MSVC)"
$candidates = @()
if ($QtDir) { $candidates += $QtDir }
if ($env:QTDIR) { $candidates += $env:QTDIR }
if ($env:QMAKE) { $candidates += (Split-Path (Split-Path $env:QMAKE)) }
$candidates += @(
    "C:\Qt\6.12.0\msvc2022_64",
    (Join-Path $HOME "Qt\6.12.0\msvc2022_64"),
    (Join-Path $HOME "Qt\6.12.0")
)
$QtDir = $null
foreach ($c in $candidates) {
    if ($c -and (Test-Path (Join-Path $c "bin\qmake6.exe"))) { $QtDir = (Resolve-Path $c).Path; break }
}
if (-not $QtDir) {
    Abort ("Qt 6.12 MSVC не найден. Укажите -QtDir или поставьте его:`n" +
        "    python .github\scripts\install_qt.py --out C:\Qt\6.12.0\msvc2022_64")
}
$Qmake = Join-Path $QtDir "bin\qmake6.exe"
$WinDeployQt = Join-Path $QtDir "bin\windeployqt.exe"
if (-not (Test-Path $WinDeployQt)) { $WinDeployQt = Join-Path $QtDir "bin\windeployqt6.exe" }
if (-not (Test-Path $WinDeployQt)) { Abort "windeployqt не найден в $QtDir\bin" }
$env:QMAKE = $Qmake
$env:QTDIR = $QtDir
$env:PATH = "$QtDir\bin;$env:PATH"
Write-Ok "$QtDir ($(& $Qmake -query QT_VERSION))"

# ─── 2. libmpv-2.dll ──────────────────────────────────────────────────────────
Write-Step "Поиск libmpv-2.dll"
if ($MpvDll) {
    if (-not (Test-Path $MpvDll)) { Abort "Нет файла $MpvDll" }
    $MpvDll = (Resolve-Path $MpvDll).Path
} else {
    foreach ($name in @("libmpv-2.dll", "mpv-2.dll")) {
        $candidate = Join-Path $ProjectRoot $name
        if (Test-Path $candidate) { $MpvDll = $candidate; break }
    }
}
if (-not $MpvDll) {
    Write-Warn "не найдена, скачиваем mpv-dev-x86_64 (shinchiro/mpv-winbuild-cmake)…"
    $headers = @{ "User-Agent" = "yamusic-build" }
    if ($env:GITHUB_TOKEN) { $headers["Authorization"] = "Bearer $env:GITHUB_TOKEN" }
    $release = Invoke-RestMethod -Headers $headers `
        -Uri "https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/latest"
    # обычная сборка, не -v3 (та требует AVX2)
    $asset = $release.assets | Where-Object { $_.name -match '^mpv-dev-x86_64-\d{8}-git-.*\.7z$' } |
        Select-Object -First 1
    if (-not $asset) { Abort "В последнем релизе нет архива mpv-dev-x86_64." }
    $sevenZip = Find-SevenZip
    if (-not $sevenZip) { Abort "Нужен 7-Zip (winget install 7zip.7zip)." }
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("mpv-dev-" + [guid]::NewGuid())
    New-Item -ItemType Directory -Force $tmp | Out-Null
    try {
        $archive = Join-Path $tmp $asset.name
        Write-Host "    $($asset.name)" -ForegroundColor Gray
        Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $archive
        & $sevenZip e $archive "-o$tmp" "libmpv-2.dll" -r -y | Out-Null
        if ($LASTEXITCODE -ne 0) { Abort "7-Zip не распаковал $($asset.name)" }
        $dll = Join-Path $tmp "libmpv-2.dll"
        if (-not (Test-Path $dll)) { Abort "libmpv-2.dll нет в архиве $($asset.name)" }
        $MpvDll = Join-Path $ProjectRoot "libmpv-2.dll"
        Copy-Item $dll $MpvDll -Force
    } finally {
        Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
}
Write-Ok $MpvDll

# ─── 3. cargo build ───────────────────────────────────────────────────────────
$Exe = Join-Path $ProjectRoot "target\$CargoProfile\yamusic.exe"
if (-not $SkipBuild) {
    Write-Step "cargo build ($CargoProfile) -p yamusic"
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Abort "cargo не найден. Поставьте Rust: https://rustup.rs (toolchain stable-x86_64-pc-windows-msvc)"
    }
    Push-Location $ProjectRoot
    try {
        if ($CargoProfile -eq "release") { cargo build --release -p yamusic } else { cargo build -p yamusic }
        if ($LASTEXITCODE -ne 0) { Abort "cargo build завершился с ошибкой" }
    } finally {
        Pop-Location
    }
}
if (-not (Test-Path $Exe)) { Abort "Нет $Exe" }
Write-Ok $Exe

# ─── 4. Папка дистрибутива ────────────────────────────────────────────────────
Write-Step "Сборка $Stage"
if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
New-Item -ItemType Directory -Force $Stage | Out-Null
Copy-Item $Exe $Stage

# QML и ресурсы — рядом с exe: движок ищет <exe>\qml, assets — соседний каталог
$srcQml = Join-Path $ProjectRoot "src\yamusic\qml"
$srcAssets = Join-Path $ProjectRoot "src\yamusic\assets"
Copy-Item $srcQml (Join-Path $Stage "qml") -Recurse
Get-ChildItem (Join-Path $Stage "qml") -Recurse -Directory -Filter "__pycache__" |
    Remove-Item -Recurse -Force
$assets = Join-Path $Stage "assets"
New-Item -ItemType Directory -Force $assets | Out-Null
Copy-Item (Join-Path $srcAssets "fonts") (Join-Path $assets "fonts") -Recurse   # fonts-src (исходники шрифтов) не нужен
Copy-Item (Join-Path $srcAssets "yamusic.svg") $assets

$deployArgs = @("--qmldir", $srcQml, "--no-opengl-sw", "--no-compiler-runtime")
if ($CargoProfile -eq "release") { $deployArgs = @("--release") + $deployArgs }
& $WinDeployQt @deployArgs (Join-Path $Stage "yamusic.exe")
if ($LASTEXITCODE -ne 0) { Abort "windeployqt завершился с ошибкой $LASTEXITCODE" }
Write-Ok "Qt разложен windeployqt"

# Рантайм MSVC (vcruntime140*.dll, msvcp140*.dll) кладём рядом с exe — без vc_redist у пользователя
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$crtDir = $null
if (Test-Path $vswhere) {
    $vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath | Select-Object -First 1
    if ($vs) {
        $crtDir = Get-ChildItem (Join-Path $vs "VC\Redist\MSVC") -Directory -ErrorAction SilentlyContinue |
            Sort-Object { try { [version]$_.Name } catch { [version]"0.0" } } -Descending |
            ForEach-Object { Get-ChildItem (Join-Path $_.FullName "x64") -Directory -Filter "Microsoft.VC*.CRT" -ErrorAction SilentlyContinue } |
            Select-Object -First 1
    }
}
if ($crtDir) {
    Copy-Item (Join-Path $crtDir.FullName "*.dll") $Stage
    Write-Ok "рантайм MSVC: $($crtDir.FullName)"
} else {
    Write-Warn "рантайм MSVC не найден — на чистой системе понадобится Visual C++ Redistributable"
}

Copy-Item $MpvDll (Join-Path $Stage "libmpv-2.dll")
Write-Ok "libmpv-2.dll"

$sizeMB = [math]::Round(((Get-ChildItem $Stage -Recurse -File | Measure-Object Length -Sum).Sum) / 1MB, 0)
Write-Ok "папка готова: $Stage ($sizeMB МБ)"

if ($Portable) {
    $zip = Join-Path $OutDir "YaMusic-$Version-windows-x64-portable.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path (Join-Path $Stage "*") -DestinationPath $zip -CompressionLevel Optimal
    Write-Ok "портативный архив: $zip"
}

if ($SkipInstaller) {
    Write-Host "`nГотово: $Stage\yamusic.exe (Inno Setup пропущен)" -ForegroundColor Green
    exit 0
}

# ─── 5. Inno Setup ────────────────────────────────────────────────────────────
Write-Step "Поиск Inno Setup"
$iscc = $null
$isccCmd = Get-Command ISCC -ErrorAction SilentlyContinue
foreach ($candidate in @(
    $(if ($isccCmd) { $isccCmd.Source }),
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
)) {
    if ($candidate -and (Test-Path $candidate)) { $iscc = $candidate; break }
}
if (-not $iscc) {
    Abort "Inno Setup 6 не найден. Установите: winget install JRSoftware.InnoSetup (или запустите с -SkipInstaller)"
}
Write-Ok $iscc

Write-Step "Сборка инсталлера"
# VersionInfoVersion — только числа: 0.2.0-rc1 → 0.2.0
$numVersion = ($Version -split '[-+]')[0]
& $iscc "/Qp" "/DAppVersion=$Version" "/DAppNumVersion=$numVersion" "/DDistDir=$Stage" "/O$OutDir" (Join-Path $ProjectRoot "yamusic.iss")
if ($LASTEXITCODE -ne 0) { Abort "Inno Setup завершился с ошибкой $LASTEXITCODE" }

$setupExe = Join-Path $OutDir "YaMusic-$Version-Setup.exe"
if (-not (Test-Path $setupExe)) { Abort "Нет $setupExe после ISCC" }
$sizeMB = [math]::Round((Get-Item $setupExe).Length / 1MB, 0)
Write-Host "`nГотово:" -ForegroundColor Green
Write-Host "  Инсталлер:  $setupExe ($sizeMB МБ)" -ForegroundColor White
Write-Host "  Portable:   $Stage\yamusic.exe" -ForegroundColor White
