#!/usr/bin/env bash
# Установка YaMusic (неофициальный клиент Яндекс Музыки) на Linux одной командой — сборка из исходников:
#
#   curl -fsSL https://raw.githubusercontent.com/yaskorinov/yamusic-md3/main/install.sh | bash
#
# Что делает: ставит системные пакеты (компилятор C++, libmpv, git, заголовки OpenGL/xkb — спросит
# разрешение и пароль sudo), скачивает программу в ~/.local/share/yamusic-md3, ставит Rust (rustup,
# в ~/.cargo, без sudo) и Qt 6.12 (если в системе Qt старее — скачивает в ту же папку), собирает
# cargo build --release и кладёт команду ~/.local/bin/yamusic и значок в меню.
# Повторный запуск обновляет программу. Удаление: install.sh --uninstall (настройки и вход остаются).
#
# Переменные: YAMUSIC_DIR — куда ставить, YAMUSIC_QMAKE — свой qmake6, YAMUSIC_YES=1 — без вопросов,
# YAMUSIC_KEEP_BUILD=1 — не удалять каталог сборки target/ (быстрее обновления, но +2–3 ГБ).
set -euo pipefail
PATH_ORIG="$PATH"

REPO="${YAMUSIC_REPO:-https://github.com/yaskorinov/yamusic-md3.git}"
DIR="${YAMUSIC_DIR:-$HOME/.local/share/yamusic-md3}"
BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/scalable/apps"
QT_VERSION="6.12.0"
RUST_MIN="1.98"

say()  { printf '\033[1;33m==>\033[0m %s\n' "$*"; }
fail() { printf '\033[1;31mОшибка:\033[0m %s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }
# версия a >= b (через sort -V)
ver_ge() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -n1)" = "$2" ]; }

if [ "${1:-}" = "--uninstall" ]; then
    rm -rf "$DIR"
    rm -f "$BIN/yamusic" "$APPS/yamusic.desktop" "$ICONS/yamusic.svg"
    say "YaMusic удалён. Настройки (~/.config/yamusic), кэш (~/.cache/yamusic) и Rust (~/.cargo) оставлены."
    exit 0
fi

[ "$(uname -s)" = "Linux" ] || fail "Этот скрипт — для Linux. Windows: инсталлер на странице Releases."
case "$(uname -m)" in
    x86_64) QT_HOST=linux; QT_ARCH=gcc_64 ;;
    aarch64) QT_HOST=linux_arm64; QT_ARCH=gcc_arm64 ;;
    *) fail "Архитектура $(uname -m) не поддерживается: для неё нет готовой сборки Qt." ;;
esac
[ "$(id -u)" != "0" ] || fail "Запускайте от обычного пользователя, без sudo: пароль спросится сам, когда понадобится."

confirm() {
    [ "${YAMUSIC_YES:-}" = "1" ] && return 0
    printf '    Продолжить? [Y/n] '
    local answer
    read -r answer </dev/tty || answer=n
    case "$answer" in "" | y | Y | д | Д) ;; *) fail "Отменено." ;; esac
}

# --- Системные пакеты -------------------------------------------------------

# без grep -q: он закрывает трубу раньше времени, и с pipefail вся проверка считается упавшей
has_libmpv() {
    ldconfig -p 2>/dev/null | grep -E 'libmpv\.so(\.2)? ' >/dev/null && return 0
    ls /usr/lib/libmpv.so* /usr/lib64/libmpv.so* /usr/lib/*/libmpv.so* >/dev/null 2>&1
}
has_header() { [ -e "/usr/include/$1" ]; }

need=()
has_libmpv || need+=(libmpv)
have git || need+=(git)
have curl || need+=(curl)
have c++ || need+=(c++)
have python3 || need+=(python3)
have 7z || have 7zz || need+=(7zip)
has_header GL/gl.h || need+=(opengl-dev)
has_header xkbcommon/xkbcommon.h || need+=(xkbcommon-dev)

if [ ${#need[@]} -gt 0 ]; then
    if have pacman; then
        cmd="sudo pacman -S --needed --noconfirm base-devel mpv git curl python 7zip mesa libxkbcommon"
    elif have apt-get; then
        # libmpv2 — в новых выпусках Debian/Ubuntu; в старых хватит libmpv-dev (там есть libmpv.so)
        mpvlib=libmpv2
        apt-cache show libmpv2 >/dev/null 2>&1 || mpvlib=libmpv-dev
        sz=7zip
        apt-cache show 7zip >/dev/null 2>&1 || sz=p7zip-full
        cmd="sudo apt-get install -y build-essential pkg-config git curl python3 $sz $mpvlib \
libgl-dev libegl-dev libxkbcommon-dev libfontconfig1-dev libdbus-1-dev \
libegl1 libxkbcommon-x11-0 libxcb-cursor0 libxcb-icccm4 libxcb-keysyms1 libxcb-shape0 libfontconfig1 libdbus-1-3"
    elif have dnf; then
        cmd="sudo dnf install -y gcc-c++ make pkgconf git curl python3 7zip mpv-libs \
mesa-libGL-devel mesa-libEGL-devel libxkbcommon-devel libxkbcommon-x11 fontconfig-devel xcb-util-cursor"
    elif have zypper; then
        cmd="sudo zypper install -y gcc-c++ make pkg-config git curl python3 7zip libmpv2 \
Mesa-libGL-devel Mesa-libEGL-devel libxkbcommon-devel libxkbcommon-x11-0 fontconfig-devel libxcb-cursor0"
    else
        fail "Не хватает: ${need[*]}. Установите их пакетным менеджером своего дистрибутива (libmpv обычно в пакете mpv) и запустите скрипт снова."
    fi
    say "Не хватает системных пакетов: ${need[*]}"
    echo "    Будет выполнено: $cmd"
    confirm
    $cmd
    has_libmpv || fail "libmpv так и не появилась. Установите пакет mpv (или libmpv) вручную."
fi

# --- Исходники --------------------------------------------------------------

export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

if [ -d "$DIR/.git" ]; then
    say "Обновляю YaMusic в $DIR"
    git -C "$DIR" pull --ff-only --quiet || fail "Не удалось обновить $DIR (там есть свои правки?). Удалите папку и запустите скрипт снова."
else
    say "Скачиваю YaMusic в $DIR"
    mkdir -p "$(dirname "$DIR")"
    git clone --depth 1 --quiet "$REPO" "$DIR"
fi

# --- Qt 6.12 ----------------------------------------------------------------

qt_ok() { [ -x "$1" ] && ver_ge "$("$1" -query QT_VERSION 2>/dev/null || echo 0)" "${QT_VERSION%.*}"; }
# системный Qt годится, только если стоят и QML-модули (qt6-declarative и т. п.)
qt_full() {
    local qml
    qml="$("$1" -query QT_INSTALL_QML 2>/dev/null)" || return 1
    [ -e "$qml/QtQuick/qmldir" ] && [ -e "$qml/QtQuick/Effects/qmldir" ] && [ -e "$qml/Qt/labs/platform/qmldir" ]
}

QMAKE="${YAMUSIC_QMAKE:-}"
if [ -z "$QMAKE" ]; then
    for q in "$(command -v qmake6 || true)" /usr/lib/qt6/bin/qmake6 /usr/lib64/qt6/bin/qmake6 /usr/lib/qt6/bin/qmake; do
        if [ -n "$q" ] && qt_ok "$q" && qt_full "$q"; then QMAKE="$q"; break; fi
    done
fi
QT_PRIVATE=""
if [ -z "$QMAKE" ]; then
    QT_PRIVATE="$DIR/qt/$QT_VERSION/$QT_ARCH"
    say "Qt $QT_VERSION в системе нет — скачиваю в $QT_PRIVATE (около 250 МБ)"
    QMAKE="$(python3 "$DIR/.github/scripts/install_qt.py" --host "$QT_HOST" --version "$QT_VERSION" \
        --out "$QT_PRIVATE" | tail -n1)"
fi
qt_ok "$QMAKE" || fail "Qt ${QT_VERSION%.*}+ не найден ($QMAKE). Укажите свой: YAMUSIC_QMAKE=/путь/к/qmake6"
say "Qt $("$QMAKE" -query QT_VERSION): $QMAKE"

# --- Rust -------------------------------------------------------------------

rust_ok() { have cargo && ver_ge "$(rustc --version | awk '{print $2}')" "$RUST_MIN"; }
if ! rust_ok; then
    if have rustup; then
        say "Обновляю Rust (rustup update stable)"
        rustup update stable --no-self-update
        rustup default stable >/dev/null
    else
        say "Ставлю Rust через rustup (в ~/.cargo, без sudo)"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path
    fi
    # shellcheck disable=SC1091
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
    rust_ok || fail "Нужен Rust $RUST_MIN или новее (сейчас: $(rustc --version 2>/dev/null || echo нет))."
fi
say "$(rustc --version)"

# --- Сборка -----------------------------------------------------------------

say "Собираю YaMusic (первый раз — 5–15 минут)"
(cd "$DIR" && QMAKE="$QMAKE" cargo build --release -p yamusic --quiet) || fail "Сборка не удалась (вывод выше)."
mkdir -p "$DIR/bin"
install -m755 "$DIR/target/release/yamusic" "$DIR/bin/yamusic"
[ "${YAMUSIC_KEEP_BUILD:-}" = "1" ] || rm -rf "$DIR/target"
# окружение прежней Python-версии больше не нужно
rm -rf "$DIR/.venv"

# --- Запуск: команда и значок в меню ----------------------------------------
# QML и ресурсы exe находит сам: $DIR/src/yamusic/qml — вверх от $DIR/bin

mkdir -p "$BIN" "$APPS" "$ICONS"
{
    echo '#!/bin/sh'
    [ -n "$QT_PRIVATE" ] && echo "export LD_LIBRARY_PATH=\"$QT_PRIVATE/lib\${LD_LIBRARY_PATH:+:\$LD_LIBRARY_PATH}\""
    echo "exec \"$DIR/bin/yamusic\" \"\$@\""
} > "$BIN/yamusic"
chmod +x "$BIN/yamusic"
cp "$DIR/src/yamusic/assets/yamusic.svg" "$ICONS/yamusic.svg"
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
