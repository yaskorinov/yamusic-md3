# YaMusic

[![Build](https://github.com/yaskorinov/yamusic-md3/actions/workflows/build.yml/badge.svg?branch=main)](https://github.com/yaskorinov/yamusic-md3/actions/workflows/build.yml)
[![Release](https://github.com/yaskorinov/yamusic-md3/actions/workflows/release.yml/badge.svg)](https://github.com/yaskorinov/yamusic-md3/actions/workflows/release.yml)
[![Latest release](https://img.shields.io/github/v/release/yaskorinov/yamusic-md3?include_prereleases&label=%D0%B2%D1%8B%D0%BF%D1%83%D1%81%D0%BA)](https://github.com/yaskorinov/yamusic-md3/releases)

Неофициальный клиент Яндекс Музыки в стиле **Material 3 Expressive** для **Linux**, **Windows** и **Android**:
интерфейс на QML (Qt 6.12), ядро на Rust ([cxx-qt](https://github.com/KDAB/cxx-qt)), звук через libmpv,
тема приложения перекрашивается под обложку играющего трека.

Умеет: «Моя волна» с настройками, «Мне нравится», плейлисты, поиск, страницы альбомов и исполнителей,
синхронный текст по словам, плавный переход между треками, lossless (FLAC) при подписке.

| Платформа | Что скачать | Примечание |
|---|---|---|
| Windows 10/11 x64 | `YaMusic-<версия>-Setup.exe` | инсталлер без прав администратора; есть и портативный `.zip` |
| Linux x86_64 | `YaMusic-<версия>-x86_64.AppImage` | или сборка из исходников одной командой (ниже) |
| Android 13+ | `YaMusic-<версия>-android.apk` | отдельный нативный клиент, см. [Android](#android) |
| macOS | `YaMusic-<версия>-macos-*.dmg` | экспериментально, сборка «как получится» |

Всё лежит на странице [Releases](https://github.com/yaskorinov/yamusic-md3/releases).

**Вход:** при первом запуске откройте «Аккаунт» — программа покажет код, его нужно ввести на
[ya.ru/device](https://ya.ru/device) под своим аккаунтом Яндекса. Пароль программе не нужен.

## Установка

### Windows

Скачайте `YaMusic-<версия>-Setup.exe` из [Releases](https://github.com/yaskorinov/yamusic-md3/releases) и запустите.
Программа ставится в `%LocalAppData%\Programs\YaMusic` без UAC, добавляет значок в меню «Пуск»
(по желанию — на рабочий стол и в автозапуск) и появляется в «Программы и компоненты». Новая версия
ставится поверх старой, в том числе поверх прежней Python-сборки; настройки и вход сохраняются.

Инсталлер не подписан, поэтому SmartScreen может предупредить: «Подробнее» → «Выполнить в любом случае».
Без установки — `YaMusic-<версия>-windows-x64-portable.zip`: распакуйте и запустите `yamusic.exe`.

### Linux

**AppImage** (всё нужное внутри, включая Qt и libmpv):

```bash
chmod +x YaMusic-*-x86_64.AppImage
./YaMusic-*-x86_64.AppImage
```

**Сборка из исходников одной командой** (x86_64 или aarch64):

```bash
curl -fsSL https://raw.githubusercontent.com/yaskorinov/yamusic-md3/main/install.sh | bash
```

Скрипт поставит недостающие системные пакеты (компилятор C++, libmpv, git — спросит разрешение и пароль sudo),
скачает программу в `~/.local/share/yamusic-md3`, поставит Rust через [rustup](https://rustup.rs) (в `~/.cargo`,
без sudo), возьмёт системный Qt, если он 6.12 или новее, а иначе скачает Qt 6.12 в ту же папку, соберёт
программу и добавит команду `yamusic` и значок в меню приложений. Первая сборка — 5–15 минут и около 3 ГБ
на время сборки (каталог сборки потом удаляется).

- **Обновление:** запустите ту же команду ещё раз.
- **Удаление:** `bash ~/.local/share/yamusic-md3/install.sh --uninstall` (настройки и вход остаются).

## Сборка из исходников

Нужны:

- **Rust** 1.98 или новее — [rustup](https://rustup.rs) (на Windows — toolchain `stable-x86_64-pc-windows-msvc`);
- **Qt 6.12** с модулями qtdeclarative, qtsvg, qtshadertools, qtimageformats (и qtwayland на Linux) —
  через Qt Online Installer или скриптом `.github/scripts/install_qt.py`, который качает архивы прямо
  с download.qt.io (им же пользуется CI);
- **libmpv** — подгружается во время работы (`libmpv.so.2` / `libmpv-2.dll` / `libmpv.2.dylib`), для сборки не нужна.

Сборщик cxx-qt находит Qt по переменной `QMAKE` (путь к `qmake6`). Готовый `yamusic` сам находит
QML и ресурсы: `$YAMUSIC_QML_DIR`, иначе `qml/` рядом с exe, иначе `src/yamusic/qml` в репозитории —
поэтому `cargo run` работает прямо из исходников.

### Linux

```bash
# Debian/Ubuntu; для Arch: sudo pacman -S base-devel mpv 7zip (и qt6-declarative и др., если Qt 6.12 из системы)
sudo apt install build-essential pkg-config python3 7zip libmpv-dev libgl-dev libegl-dev \
    libxkbcommon-dev libfontconfig1-dev libxcb-cursor0 libxkbcommon-x11-0

python3 .github/scripts/install_qt.py --out ~/Qt/6.12.0/gcc_64
export QMAKE=~/Qt/6.12.0/gcc_64/bin/qmake6 LD_LIBRARY_PATH=~/Qt/6.12.0/gcc_64/lib

cargo run --release -p yamusic              # приложение
cargo run --release -p yamusic -- --gallery # галерея компонентов MD3
```

AppImage и `tar.gz`: `bash .github/scripts/package_linux.sh 0.1.0` → `target/dist/`.

### Windows

Нужны [Build Tools for Visual Studio 2022](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
(«Разработка классических приложений на C++»), Rust (msvc), Qt 6.12 `msvc2022_64`, PowerShell 7 и, для
инсталлера, [Inno Setup 6](https://jrsoftware.org/isinfo.php) (`winget install JRSoftware.InnoSetup`).

```powershell
python .github\scripts\install_qt.py --out C:\Qt\6.12.0\msvc2022_64   # если Qt ещё нет
.\build_windows.ps1 -QtDir C:\Qt\6.12.0\msvc2022_64
```

Скрипт соберёт `cargo build --release`, разложит в `target\dist\yamusic\` exe, `qml\`, `assets\`, Qt
(windeployqt), рантайм MSVC и `libmpv-2.dll` (возьмёт из корня репозитория или скачает свежую из
[shinchiro/mpv-winbuild-cmake](https://github.com/shinchiro/mpv-winbuild-cmake/releases)) и соберёт
`target\dist\YaMusic-<версия>-Setup.exe`. Ключи: `-SkipInstaller`, `-Portable` (ещё и `.zip`),
`-SkipBuild`, `-CargoProfile debug`, `-MpvDll путь`, `-Version 1.2.3`.

Для разработки без упаковки:

```powershell
$env:QMAKE = "C:\Qt\6.12.0\msvc2022_64\bin\qmake6.exe"
$env:PATH = "C:\Qt\6.12.0\msvc2022_64\bin;$env:PATH"   # Qt DLL
cargo run -p yamusic                                     # libmpv-2.dll — из корня репозитория
```

### macOS (экспериментально)

```bash
brew install mpv dylibbundler sevenzip
python3 .github/scripts/install_qt.py --out ~/Qt/6.12.0/macos
export QMAKE=~/Qt/6.12.0/macos/bin/qmake6
cargo build --release -p yamusic
bash .github/scripts/package_macos.sh 0.1.0   # target/dist/YaMusic.app и .dmg
```

### Полезное

- Ключи: `--gallery`, `--offline`, `--new-instance`, `--light`, `--size 1280x800`, `--screenshot shot.png`.
- Переменные: `YAMUSIC_QML_DIR` (свой каталог QML), `YAMUSIC_LIBMPV` (путь к libmpv), `YAMUSIC_AO`
  (аудиовыход mpv), `RUST_LOG` (журнал, например `RUST_LOG=yamusic=debug`).
- Устройство проекта — [docs/SPEC.md](docs/SPEC.md), контракт Rust-ядра — [docs/RUST_CONTRACT.md](docs/RUST_CONTRACT.md).
- Прежняя версия ядра на Python (PySide6) пока лежит в `src/yamusic/*.py` и запускается `uv run yamusic`.

## CI и выпуски

- [`build.yml`](.github/workflows/build.yml) — на каждый push и PR в `main`: Linux (AppImage, tar.gz), Windows
  (инсталлер, portable), macOS (best effort, падение не роняет сборку), Android (APK). Готовые файлы — в
  артефактах запуска.
- [`release.yml`](.github/workflows/release.yml) — на тег `v*` (например `git tag v0.2.0 && git push --tags`):
  те же сборки и публикация в GitHub Release с `SHA256SUMS.txt`. Тег с дефисом (`v0.2.0-rc1`) — пред-выпуск.
- Подписанный выпускной APK собирается, если в секретах репозитория есть `ANDROID_KEYSTORE_BASE64`
  (`base64 -w0 release.jks`), `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`;
  без них публикуется отладочный APK.

## Android

В каталоге `android/` — отдельный нативный клиент (Kotlin, Jetpack Compose, Media3) с тем же дизайном:
волна, коллекция, поиск, тексты песен, фон из обложки, плавный переход, скачивание для игры без сети.
Нужны JDK 21 и Android SDK; телефон — Android 13 и новее.

```bash
cd android && ./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

Вход — так же, по коду на ya.ru/device. Устройство описано в `docs/SPEC.md`, раздел «Android».

## Шрифты

- [Google Sans](https://fonts.google.com/specimen/Google+Sans) — SIL Open Font License 1.1
- [Material Symbols Rounded](https://github.com/google/material-design-icons) — Apache License 2.0

Клиент использует неофициальный API Яндекс Музыки и не связан с Яндексом.
