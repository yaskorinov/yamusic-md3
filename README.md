# YaMusic

Неофициальный клиент Яндекс Музыки для Linux в стиле **Material 3 Expressive**: QML + PySide6, звук через libmpv,
тема приложения перекрашивается под обложку играющего трека.

Умеет: «Моя волна» с настройками, «Мне нравится», плейлисты, поиск, страницы альбомов и исполнителей,
синхронный текст по словам, плавный переход между треками, lossless (FLAC) при подписке.

## Установка

Одной командой в терминале (Linux, x86_64 или aarch64):

```bash
curl -fsSL https://raw.githubusercontent.com/yaskorinov/yamusic-md3/main/install.sh | bash
```

Скрипт поставит недостающие системные пакеты (libmpv, git — спросит разрешение и пароль sudo), скачает
программу в `~/.local/share/yamusic-md3`, соберёт окружение (Python и библиотеки скачает
[uv](https://docs.astral.sh/uv/), системный Python не трогается) и добавит команду `yamusic` и значок
YaMusic в меню приложений. Первая установка — около 350 МБ.

- **Вход:** при первом запуске откройте «Аккаунт» — программа покажет код, его нужно ввести на
  [ya.ru/device](https://ya.ru/device) под своим аккаунтом Яндекса. Пароль программе не нужен.
- **Обновление:** запустите ту же команду ещё раз.
- **Удаление:** `bash ~/.local/share/yamusic-md3/install.sh --uninstall` (настройки и вход остаются).

## Запуск из исходников

```bash
sudo pacman -S mpv uv
uv run yamusic            # приложение
uv run yamusic --gallery  # галерея компонентов MD3
```

Устройство проекта — в [docs/SPEC.md](docs/SPEC.md).

## Шрифты

- [Google Sans](https://fonts.google.com/specimen/Google+Sans) — SIL Open Font License 1.1
- [Material Symbols Rounded](https://github.com/google/material-design-icons) — Apache License 2.0

Клиент использует неофициальный API Яндекс Музыки и не связан с Яндексом.

## Android

В каталоге `android/` — отдельный нативный клиент (Kotlin, Jetpack Compose, Media3) с тем же дизайном:
волна, коллекция, поиск, тексты песен, фон из обложки, плавный переход, скачивание для игры без сети.
Нужны JDK 21 и Android SDK; телефон — Android 12 и новее.

```bash
cd android && ./gradlew :app:assembleDebug
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

Вход — так же, по коду на ya.ru/device. Устройство описано в `docs/SPEC.md`, раздел «Android».
