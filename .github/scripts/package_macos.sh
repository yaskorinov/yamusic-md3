#!/usr/bin/env bash
# Упаковка macOS (best effort): YaMusic.app + .dmg через macdeployqt.
#
#   QMAKE=~/Qt/6.12.0/macos/bin/qmake6 .github/scripts/package_macos.sh 0.1.0
#
# Contents/MacOS: yamusic, qml/, assets/, libmpv.2.dylib (движок ищет <exe>/qml, плеер — libmpv рядом с exe).
# Зависимости libmpv (ffmpeg и т. д. из Homebrew) кладёт dylibbundler в Contents/Frameworks.
set -euo pipefail

VERSION="${1:?версия}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
EXE="${YAMUSIC_EXE:-$ROOT/target/release/yamusic}"
OUT="$ROOT/target/dist"
APP="$OUT/YaMusic.app"
: "${QMAKE:?нужен QMAKE (путь к qmake6 из Qt 6.12)}"
QTBIN="$("$QMAKE" -query QT_INSTALL_BINS)"

[ -x "$EXE" ] || { echo "нет $EXE" >&2; exit 1; }
rm -rf "$APP" "$OUT"/YaMusic*.dmg
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"

install -m755 "$EXE" "$APP/Contents/MacOS/yamusic"
cp -R "$ROOT/src/yamusic/qml" "$APP/Contents/MacOS/qml"
find "$APP/Contents/MacOS/qml" -name __pycache__ -prune -exec rm -rf {} +
mkdir -p "$APP/Contents/MacOS/assets"
cp -R "$ROOT/src/yamusic/assets/fonts" "$APP/Contents/MacOS/assets/fonts"
cp "$ROOT/src/yamusic/assets/yamusic.svg" "$APP/Contents/MacOS/assets/yamusic.svg"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>YaMusic</string>
  <key>CFBundleDisplayName</key><string>YaMusic</string>
  <key>CFBundleIdentifier</key><string>io.github.yaskorinov.yamusic</string>
  <key>CFBundleExecutable</key><string>yamusic</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict>
</plist>
PLIST

# libmpv из Homebrew рядом с exe, её зависимости — в Frameworks
LIBMPV="${YAMUSIC_LIBMPV:-$(brew --prefix mpv)/lib/libmpv.2.dylib}"
cp -L "$LIBMPV" "$APP/Contents/MacOS/libmpv.2.dylib"
chmod u+w "$APP/Contents/MacOS/libmpv.2.dylib"
install_name_tool -id "@executable_path/libmpv.2.dylib" "$APP/Contents/MacOS/libmpv.2.dylib"
dylibbundler -of -cd -b -x "$APP/Contents/MacOS/libmpv.2.dylib" \
    -d "$APP/Contents/Frameworks" -p "@executable_path/../Frameworks/"

"$QTBIN/macdeployqt" "$APP" -qmldir="$ROOT/src/yamusic/qml" -always-overwrite
# dylibbundler и macdeployqt правят бинарники — подпись ad hoc, иначе arm64 не запустит
codesign --force --deep --sign - "$APP"

DMG="$OUT/YaMusic-$VERSION-macos-$(uname -m).dmg"
hdiutil create -volname YaMusic -srcfolder "$APP" -ov -format UDZO "$DMG"
echo "dmg: $DMG"
