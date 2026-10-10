#Requires -Version 5.1
<#
.SYNOPSIS
    Сборка YaMusic для Windows: устанавливает зависимости и собирает standalone-пакет через PyInstaller.

.DESCRIPTION
    Что делает скрипт:
    1. Проверяет наличие uv (менеджер Python-окружений).
    2. Находит или скачивает libmpv / mpv-2.dll (нужна для python-mpv).
    3. Создаёт виртуальное окружение с Python 3.14 и ставит все зависимости.
    4. Запускает PyInstaller → dist\yamusic\ (папка с exe и всем нужным).

.PARAMETER SkipMpv
    Пропустить поиск/скачивание mpv-2.dll (если DLL уже лежит в корне проекта).

.PARAMETER DevOnly
    Только установить зависимости, без сборки PyInstaller (для запуска через `uv run yamusic`).
#>
param(
    [switch]$SkipMpv,
    [switch]$DevOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$ProjectRoot = $PSScriptRoot

function Write-Step { param([string]$msg) Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok   { param([string]$msg) Write-Host "    OK: $msg" -ForegroundColor Green }
function Write-Warn { param([string]$msg) Write-Host "    WARN: $msg" -ForegroundColor Yellow }
function Abort      { param([string]$msg) Write-Host "`nОШИБКА: $msg" -ForegroundColor Red; exit 1 }

# ─── 1. Проверяем uv ──────────────────────────────────────────────────────────
Write-Step "Проверка uv"
if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
    Write-Host "uv не найден. Устанавливаем..." -ForegroundColor Yellow
    Invoke-RestMethod https://astral.sh/uv/install.ps1 | Invoke-Expression
    $env:PATH = "$env:USERPROFILE\.local\bin;$env:PATH"
    if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
        Abort "uv не удалось установить. Поставьте вручную: https://docs.astral.sh/uv/getting-started/installation/"
    }
}
Write-Ok (uv --version)

# ─── 2. Ищем / скачиваем mpv-2.dll ───────────────────────────────────────────
$MpvDll = $null

if (-not $SkipMpv) {
    Write-Step "Поиск mpv-2.dll"

    # 2a. Уже есть в корне проекта? (libmpv-2.dll или mpv-2.dll)
    $localDll = $null
    foreach ($name in @("libmpv-2.dll", "mpv-2.dll")) {
        $candidate = Join-Path $ProjectRoot $name
        if (Test-Path $candidate) { $localDll = $candidate; break }
    }
    if ($localDll) {
        $MpvDll = $localDll
        Write-Ok "найдена локально: $MpvDll"
    }

    # 2b. В PATH (winget/scoop/chocolatey install mpv)?
    if (-not $MpvDll) {
        $mpvExe = (Get-Command mpv -ErrorAction SilentlyContinue)?.Source
        if ($mpvExe) {
            $mpvDir = Split-Path $mpvExe
            foreach ($name in @("libmpv-2.dll", "mpv-2.dll")) {
                $candidate = Join-Path $mpvDir $name
                if (Test-Path $candidate) { $MpvDll = $candidate; break }
            }
            if ($MpvDll) { Write-Ok "найдена рядом с mpv.exe: $MpvDll" }
        }
    }

    # 2c. Скачиваем mpv-dev с GitHub (shinchiro/mpv-winbuild-cmake) — там есть libmpv-2.dll
    if (-not $MpvDll) {
        Write-Warn "libmpv-2.dll не найдена, скачиваем mpv-dev с GitHub..."

        $apiUrl  = "https://api.github.com/repos/shinchiro/mpv-winbuild-cmake/releases/latest"
        try {
            $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ "User-Agent" = "YaMusic-build" }
        } catch {
            Abort "Не удалось получить релиз mpv с GitHub: $_`nСкачайте libmpv-2.dll вручную и положите в корень проекта."
        }

        # mpv-dev-x86_64-*.7z содержит libmpv-2.dll
        $asset = $release.assets | Where-Object { $_.name -match "^mpv-dev-x86_64-\d+.*\.7z$" -and $_.name -notmatch "v3" } | Select-Object -First 1
        if (-not $asset) {
            Abort "Не нашли mpv-dev-x86_64 архив в релизе. Скачайте libmpv-2.dll вручную."
        }

        $archivePath = Join-Path $env:TEMP "mpv_dev.7z"
        Write-Host "    Скачиваем $($asset.name) (~30 МБ)..." -ForegroundColor Gray
        Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $archivePath -UseBasicParsing

        $sevenZip = (Get-Command 7z -ErrorAction SilentlyContinue)?.Source
        if (-not $sevenZip) { $sevenZip = "C:\Program Files\7-Zip\7z.exe" }
        if (-not (Test-Path $sevenZip)) {
            try { winget install -e --id 7zip.7zip --silent --accept-package-agreements --accept-source-agreements 2>$null } catch {}
            $sevenZip = "C:\Program Files\7-Zip\7z.exe"
        }
        if (-not (Test-Path $sevenZip)) {
            Abort "Нужен 7-Zip. Установите: winget install 7zip.7zip"
        }

        $extractDir = Join-Path $env:TEMP "mpv_dev_extracted"
        New-Item -ItemType Directory -Force $extractDir | Out-Null
        & $sevenZip x $archivePath "-o$extractDir" "libmpv-2.dll" -y 2>&1 | Out-Null

        $dll = Get-ChildItem -Recurse $extractDir -Filter "libmpv-2.dll" | Select-Object -First 1
        if (-not $dll) { Abort "libmpv-2.dll не найдена в скачанном архиве." }
        $destDll = Join-Path $ProjectRoot "libmpv-2.dll"
        Copy-Item $dll.FullName -Destination $destDll
        $MpvDll = $destDll
        Write-Ok "libmpv-2.dll скачана: $MpvDll"

        Remove-Item $archivePath -Force
        Remove-Item $extractDir -Recurse -Force
    }
} else {
    Write-Warn "Поиск libmpv-2.dll пропущен (--SkipMpv). Убедитесь, что DLL в корне проекта или в PATH."
    foreach ($name in @("libmpv-2.dll", "mpv-2.dll")) {
        $candidate = Join-Path $ProjectRoot $name
        if (Test-Path $candidate) { $MpvDll = $candidate; break }
    }
    if (-not $MpvDll) { $MpvDll = Join-Path $ProjectRoot "libmpv-2.dll" }
}

# ─── 3. Устанавливаем зависимости (uv sync) ───────────────────────────────────
Write-Step "Установка Python 3.14 и зависимостей через uv"
Push-Location $ProjectRoot
try {
    uv sync --python 3.14
    if ($LASTEXITCODE -ne 0) { Abort "uv sync завершился с ошибкой" }
    Write-Ok "зависимости установлены"
} finally {
    Pop-Location
}

if ($DevOnly) {
    Write-Host "`n✓ Готово (dev-режим). Запуск:" -ForegroundColor Green
    Write-Host "    uv run yamusic" -ForegroundColor White
    exit 0
}

# ─── 4. Ставим PyInstaller в venv ─────────────────────────────────────────────
Write-Step "Установка PyInstaller"
Push-Location $ProjectRoot
try {
    uv pip install pyinstaller --python 3.14
    if ($LASTEXITCODE -ne 0) { Abort "Не удалось установить PyInstaller" }
    Write-Ok "PyInstaller установлен"
} finally {
    Pop-Location
}

# ─── 5. Конвертируем иконку SVG → ICO (если есть Pillow/Wand, иначе — без иконки) ──
$IcoPath = Join-Path $ProjectRoot "yamusic.ico"
if (-not (Test-Path $IcoPath)) {
    Write-Step "Подготовка иконки"
    $svgPath = Join-Path $ProjectRoot "src\yamusic\assets\yamusic.svg"
    try {
        uv run --python 3.14 python -c @"
from PySide6.QtGui import QGuiApplication, QIcon, QPixmap
import sys, os
app = QGuiApplication(sys.argv)
pm = QPixmap(r'$svgPath')
if not pm.isNull():
    pm.scaled(256, 256).save(r'$IcoPath'.replace('.ico', '.png'))
"@
        # Если есть Pillow — конвертируем PNG → ICO
        uv run --python 3.14 python -c @"
try:
    from PIL import Image
    img = Image.open(r'$($IcoPath -replace '\.ico$', '.png')')
    img.save(r'$IcoPath', sizes=[(16,16),(32,32),(48,48),(128,128),(256,256)])
    print('ico ok')
except ImportError:
    pass
"@ 2>$null
    } catch {}
    if (-not (Test-Path $IcoPath)) {
        Write-Warn "Иконка .ico не создана — сборка продолжится без неё."
        $IcoPath = $null
    }
}

# ─── 6. Патчим spec-файл: путь к mpv-2.dll ────────────────────────────────────
Write-Step "Подготовка yamusic.spec"
$specPath = Join-Path $ProjectRoot "yamusic.spec"
$specContent = Get-Content $specPath -Raw

# Заменяем заглушку пути DLL на реальный путь
if ($MpvDll -and (Test-Path $MpvDll)) {
    $escaped = $MpvDll -replace '\\', '\\\\'
    $dllName = Split-Path $MpvDll -Leaf
    $specContent = $specContent -replace '"libmpv-2\.dll"', "`"$escaped`""
    Write-Ok "путь к $dllName прописан в spec"
} else {
    Write-Warn "libmpv-2.dll не найдена — exe потребует наличие DLL в PATH или рядом с собой."
    $specContent = $specContent -replace '\("libmpv-2\.dll", "\."[^)]*\),', ""
}

# Иконка
if ($IcoPath -and (Test-Path $IcoPath)) {
    $escapedIco = $IcoPath -replace '\\', '\\\\'
    $specContent = $specContent -replace 'icon=str\(.*?\),', "icon=r'$escapedIco',"
}

Set-Content $specPath $specContent -Encoding UTF8
Write-Ok "spec обновлён"

# ─── 7. Запускаем PyInstaller ─────────────────────────────────────────────────
Write-Step "Сборка PyInstaller (dist\yamusic\)"
Push-Location $ProjectRoot
try {
    uv run --python 3.14 pyinstaller yamusic.spec --clean --noconfirm
    if ($LASTEXITCODE -ne 0) { Abort "PyInstaller завершился с ошибкой" }
} finally {
    Pop-Location
}

$distDir = Join-Path $ProjectRoot "dist\yamusic"
if (-not (Test-Path (Join-Path $distDir "yamusic.exe"))) {
    Abort "yamusic.exe не найден после сборки."
}

# ─── 8. Копируем libmpv-2.dll в dist (на случай, если PyInstaller не подхватил) ──
if ($MpvDll -and (Test-Path $MpvDll)) {
    $dllName = Split-Path $MpvDll -Leaf
    $dllDest = Join-Path $distDir $dllName
    if (-not (Test-Path $dllDest)) {
        Copy-Item $MpvDll $dllDest
        Write-Ok "$dllName скопирована в dist"
    }
}

Write-Host "`n✓ Готово! Результат: $distDir" -ForegroundColor Green
Write-Host "  Запускайте: $distDir\yamusic.exe" -ForegroundColor White
Write-Host "  Папку dist\yamusic\ можно заархивировать и распространять." -ForegroundColor Gray
