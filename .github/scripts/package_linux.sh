#!/usr/bin/env bash
# Упаковка Linux-сборки: AppDir → AppImage (linuxdeploy + linuxdeploy-plugin-qt) и tar.gz того же AppDir.
#
#   QMAKE=~/Qt/6.12.0/gcc_64/bin/qmake6 .github/scripts/package_linux.sh 0.1.0
#
# Раскладка: usr/bin/yamusic, usr/bin/qml (QML приложения — движок ищет <exe>/qml), usr/bin/assets,
# Qt и его QML-модули — usr/lib, usr/plugins, usr/qml (qt.conf пишет плагин). libmpv кладётся в usr/lib:
# у exe RUNPATH=$ORIGIN/../lib, и dlopen("libmpv.so.2") находит её там раньше системной.
# Результат — в target/dist.
set -euo pipefail

VERSION="${1:?версия}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
EXE="${YAMUSIC_EXE:-$ROOT/target/release/yamusic}"
OUT="$ROOT/target/dist"
APPDIR="$OUT/AppDir"
TOOLS="${LINUXDEPLOY_DIR:-$OUT/tools}"
ARCH="$(uname -m)"

[ -x "$EXE" ] || { echo "нет $EXE — сначала cargo build --release -p yamusic" >&2; exit 1; }
: "${QMAKE:?нужен QMAKE (путь к qmake6 из Qt 6.12)}"
QTDIR="$("$QMAKE" -query QT_INSTALL_PREFIX)"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
    "$APPDIR/usr/share/icons/hicolor/scalable/apps" "$TOOLS"

install -m755 "$EXE" "$APPDIR/usr/bin/yamusic"
cp -r "$ROOT/src/yamusic/qml" "$APPDIR/usr/bin/qml"
find "$APPDIR/usr/bin/qml" -name __pycache__ -prune -exec rm -rf {} +
mkdir -p "$APPDIR/usr/bin/assets"
cp -r "$ROOT/src/yamusic/assets/fonts" "$APPDIR/usr/bin/assets/fonts"
cp "$ROOT/src/yamusic/assets/yamusic.svg" "$APPDIR/usr/bin/assets/yamusic.svg"
cp "$ROOT/src/yamusic/assets/yamusic.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/yamusic.svg"

cat > "$APPDIR/usr/share/applications/yamusic.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=YaMusic
GenericName=Музыкальный плеер
Comment=Неофициальный клиент Яндекс Музыки
Exec=yamusic %U
Icon=yamusic
Terminal=false
Categories=AudioVideo;Audio;Player;
StartupWMClass=yamusic
DESKTOP

fetch_tool() {
    local name="$1" url="$2"
    if [ ! -x "$TOOLS/$name" ]; then
        curl -fsSL --retry 4 -o "$TOOLS/$name" "$url"
        chmod +x "$TOOLS/$name"
    fi
}
fetch_tool "linuxdeploy-$ARCH.AppImage" \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$ARCH.AppImage"
fetch_tool "linuxdeploy-plugin-qt-$ARCH.AppImage" \
    "https://github.com/linuxdeploy/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-$ARCH.AppImage"
export PATH="$TOOLS:$PATH"
# без FUSE на раннерах CI
export APPIMAGE_EXTRACT_AND_RUN=1

# libmpv из системы (libmpv-dev / libmpv2) — вместе с зависимостями (ffmpeg и т. д.)
# awk дочитывает до конца: ранний exit оборвал бы трубу, и с pipefail скрипт упал бы
LIBMPV="${YAMUSIC_LIBMPV:-$(ldconfig -p | awk '/libmpv\.so\.2 / && !p {p=$NF} END{print p}')}"
[ -n "$LIBMPV" ] && [ -e "$LIBMPV" ] || { echo "libmpv.so.2 не найдена (apt install libmpv-dev)" >&2; exit 1; }

export QMAKE
export QML_SOURCES_PATHS="$ROOT/src/yamusic/qml"
export LD_LIBRARY_PATH="$QTDIR/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
# Wayland рядом с xcb, если есть в этой сборке Qt; offscreen — для --screenshot (смоук-тест CI)
wayland=()
for p in "$QTDIR"/plugins/platforms/libqwayland*.so "$QTDIR"/plugins/platforms/libqoffscreen.so; do
    [ -e "$p" ] && wayland+=("$(basename "$p")")
done
if [ ${#wayland[@]} -gt 0 ]; then
    EXTRA_PLATFORM_PLUGINS="$(IFS=';'; echo "${wayland[*]}")"
    export EXTRA_PLATFORM_PLUGINS
fi

# plugin-qt их не кладёт: без graphics-integration на Wayland не создаётся GL (аварийный выход),
# без iconengines не грузятся SVG-иконки окна и трея; зависимости дозаберёт linuxdeploy
mkdir -p "$APPDIR/usr/plugins"
for d in iconengines wayland-graphics-integration-client wayland-decoration-client wayland-shell-integration; do
    [ -d "$QTDIR/plugins/$d" ] && cp -r "$QTDIR/plugins/$d" "$APPDIR/usr/plugins/"
done

"linuxdeploy-$ARCH.AppImage" --appdir "$APPDIR" \
    --executable "$APPDIR/usr/bin/yamusic" \
    --desktop-file "$APPDIR/usr/share/applications/yamusic.desktop" \
    --icon-file "$APPDIR/usr/share/icons/hicolor/scalable/apps/yamusic.svg" \
    --library "$LIBMPV" \
    --plugin qt

TARBALL="$OUT/yamusic-$VERSION-linux-$ARCH.tar.gz"
tar -C "$OUT" --transform "s|^AppDir|yamusic-$VERSION|" -czf "$TARBALL" AppDir
echo "tar.gz: $TARBALL"

APPIMAGE="$OUT/YaMusic-$VERSION-$ARCH.AppImage"
rm -f "$APPIMAGE"
(cd "$OUT" && LDAI_OUTPUT="$APPIMAGE" OUTPUT="$APPIMAGE" \
    "linuxdeploy-$ARCH.AppImage" --appdir "$APPDIR" --output appimage)
[ -f "$APPIMAGE" ] || { echo "AppImage не собрался" >&2; exit 1; }
echo "AppImage: $APPIMAGE"
