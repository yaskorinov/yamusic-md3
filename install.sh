#!/usr/bin/env bash
# Установка YaMusic (неофициальный клиент Яндекс Музыки для Linux) одной командой:
#
#   curl -fsSL https://raw.githubusercontent.com/yaskorinov/yamusic-md3/main/install.sh | bash
#
# Что делает: ставит системные пакеты (libmpv, git, uv — спросит разрешение и пароль sudo),
# скачивает программу в ~/.local/share/yamusic-md3, собирает окружение (Python и библиотеки
# скачает uv, системный Python не трогает), кладёт команду ~/.local/bin/yamusic и значок в меню.
# Повторный запуск обновляет программу. Удаление: install.sh --uninstall (настройки и вход остаются).
set -euo pipefail
PATH_ORIG="$PATH"

REPO="${YAMUSIC_REPO:-https://github.com/yaskorinov/yamusic-md3.git}"
DIR="${YAMUSIC_DIR:-$HOME/.local/share/yamusic-md3}"
BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"

say()  { printf '\033[1;33m==>\033[0m %s\n' "$*"; }
fail() { printf '\033[1;31mОшибка:\033[0m %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

if [ "${1:-}" = "--uninstall" ]; then
    rm -rf "$DIR"
    rm -f "$BIN/yamusic" "$APPS/yamusic.desktop" "$ICONS/yamusic.svg"
    say "YaMusic удалён. Настройки (~/.config/yamusic) и кэш (~/.cache/yamusic) оставлены."
    exit 0
fi

[ "$(uname -s)" = "Linux" ] || fail "YaMusic работает только на Linux."
case "$(uname -m)" in
    x86_64 | aarch64) ;;
    *) fail "Архитектура $(uname -m) не поддерживается: для неё нет готовой сборки Qt." ;;
esac
[ "$(id -u)" != "0" ] || fail "Запускайте от обычного пользователя, без sudo: пароль спросится сам, когда понадобится."

# --- Системные пакеты -------------------------------------------------------

# без grep -q: он закрывает трубу раньше времени, и с pipefail вся проверка считается упавшей
has_libmpv() {
    ldconfig -p 2>/dev/null | grep 'libmpv\.so' >/dev/null && return 0
    ls /usr/lib/libmpv.so* /usr/lib64/libmpv.so* /usr/lib/*/libmpv.so* >/dev/null 2>&1
}

need=()
has_libmpv || need+=(libmpv)
have git || need+=(git)
have curl || need+=(curl)

if [ ${#need[@]} -gt 0 ]; then
    if have pacman; then
        cmd="sudo pacman -S --needed --noconfirm mpv git curl uv"
    elif have apt-get; then
        # libmpv2 — в новых выпусках Debian/Ubuntu, libmpv1 — в старых; остальное нужно Qt
        mpvlib=libmpv2
        apt-cache show libmpv2 >/dev/null 2>&1 || mpvlib=libmpv1
        cmd="sudo apt-get install -y $mpvlib git curl libegl1 libxkbcommon0 libxcb-cursor0 libfontconfig1 libdbus-1-3"
    elif have dnf; then
        cmd="sudo dnf install -y mpv-libs git curl"
    elif have zypper; then
        cmd="sudo zypper install -y libmpv2 git curl"
    else
        fail "Не хватает: ${need[*]}. Установите их пакетным менеджером своего дистрибутива (libmpv обычно в пакете mpv) и запустите скрипт снова."
    fi
    say "Не хватает системных пакетов: ${need[*]}"
    echo "    Будет выполнено: $cmd"
    if [ "${YAMUSIC_YES:-}" != "1" ]; then
        printf '    Продолжить? [Y/n] '
        read -r answer </dev/tty || answer=n
        case "$answer" in "" | y | Y | д | Д) ;; *) fail "Отменено." ;; esac
    fi
    $cmd
    has_libmpv || fail "libmpv так и не появилась. Установите пакет mpv (или libmpv) вручную."
fi

# uv — менеджер окружений Python: сам скачает нужный Python и библиотеки
export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
if ! have uv; then
    say "Ставлю uv (в ~/.local/bin, без sudo)"
    curl -LsSf https://astral.sh/uv/install.sh | sh
    have uv || fail "uv не установился. Поставьте его вручную: https://docs.astral.sh/uv/"
fi

# --- Программа --------------------------------------------------------------

if [ -d "$DIR/.git" ]; then
    say "Обновляю YaMusic в $DIR"
    git -C "$DIR" pull --ff-only --quiet || fail "Не удалось обновить $DIR (там есть свои правки?). Удалите папку и запустите скрипт снова."
else
    say "Скачиваю YaMusic в $DIR"
    mkdir -p "$(dirname "$DIR")"
    git clone --depth 1 --quiet "$REPO" "$DIR"
fi

say "Собираю окружение (первый раз скачается около 300 МБ, это несколько минут)"
(cd "$DIR" && uv sync --no-dev --frozen --quiet)
[ -x "$DIR/.venv/bin/yamusic" ] || fail "Окружение не собралось: нет $DIR/.venv/bin/yamusic"

# --- Запуск: команда и значок в меню ----------------------------------------

mkdir -p "$BIN" "$APPS" "$ICONS"
cat > "$BIN/yamusic" <<LAUNCHER
#!/bin/sh
exec "$DIR/.venv/bin/yamusic" "\$@"
LAUNCHER
chmod +x "$BIN/yamusic"
cp "$DIR/packaging/yamusic.svg" "$ICONS/yamusic.svg"
cat > "$APPS/yamusic.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=YaMusic
GenericName=Музыкальный плеер
Comment=Неофициальный клиент Яндекс Музыки
Exec=$BIN/yamusic
Icon=yamusic
Terminal=false
Categories=AudioVideo;Audio;Player;
StartupWMClass=yamusic
DESKTOP
have update-desktop-database && update-desktop-database "$APPS" >/dev/null 2>&1 || true

say "Готово. Запуск: значок YaMusic в меню приложений или команда: yamusic"
case ":$PATH_ORIG:" in
    *":$BIN:"*) ;;
    *) echo "    Если команда yamusic не находится — $BIN нет в PATH: запускайте $BIN/yamusic или из меню." ;;
esac
echo "    При первом запуске откройте «Аккаунт» и войдите в свой Яндекс: программа покажет код для ya.ru/device."
