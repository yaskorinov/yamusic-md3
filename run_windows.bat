@echo off
setlocal

:: Проверяем uv
where uv >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo uv не найден. Поставьте: https://docs.astral.sh/uv/getting-started/installation/
    pause
    exit /b 1
)

:: Проверяем libmpv-2.dll (или mpv-2.dll)
set MPV_FOUND=0
if exist "%~dp0libmpv-2.dll" set MPV_FOUND=1
if exist "%~dp0mpv-2.dll"    set MPV_FOUND=1
if %MPV_FOUND% == 0 (
    where mpv >nul 2>&1
    if %ERRORLEVEL% == 0 set MPV_FOUND=1
)
if %MPV_FOUND% == 0 (
    echo.
    echo libmpv-2.dll не найдена. Нужно установить mpv одним из способов:
    echo   winget install mpv
    echo   scoop install mpv
    echo   или запустите build_windows.ps1 (он скачает DLL автоматически^)
    echo.
    pause
    exit /b 1
)
:: Добавляем каталог проекта в PATH, чтобы python-mpv нашёл DLL
set PATH=%~dp0;%PATH%

:: Запуск
cd /d "%~dp0"
uv run --python 3.14 yamusic %*
