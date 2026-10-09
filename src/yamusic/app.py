"""Запуск Qt-приложения и загрузка QML."""

from __future__ import annotations

import argparse
import os
import sys
import tempfile
from pathlib import Path

from PySide6.QtCore import QSettings, QTimer, QUrl
from PySide6.QtGui import QFontDatabase, QGuiApplication
from PySide6.QtQml import QQmlApplicationEngine, qmlRegisterSingletonInstance
from PySide6.QtQuick import QQuickWindow

from . import icons, settings, theme  # noqa: F401  регистрируют YaMusic.Core
from .aio import AsyncRunner
from .auth import Auth
from .images import CachingNamFactory
from .library import Library

PACKAGE_DIR = Path(__file__).resolve().parent
QML_DIR = PACKAGE_DIR / "qml"
FONTS_DIR = PACKAGE_DIR / "assets" / "fonts"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(prog="yamusic")
    parser.add_argument("--gallery", action="store_true", help="галерея компонентов MD3")
    parser.add_argument("--qml", metavar="FILE", help="загрузить произвольный QML (стенды для отладки)")
    parser.add_argument("--screenshot", metavar="PNG", help="сохранить снимок окна и выйти")
    parser.add_argument("--delay", type=int, default=1500, help="задержка перед снимком, мс")
    parser.add_argument("--seed-image", metavar="PATH", help="взять цвет темы из картинки")
    parser.add_argument("--light", action="store_true", help="светлая тема")
    parser.add_argument("--offline", action="store_true", help="не восстанавливать сессию из keyring")
    parser.add_argument("--debug-wheel", action="store_true", help="печатать сырые события колеса мыши")
    parser.add_argument("--debug-fps", action="store_true", help="счётчик кадров в углу окна")
    parser.add_argument("--size", help="размер окна, ШxВ (по умолчанию — из настроек)")
    parser.add_argument("--config-dir", metavar="DIR",
                        help="каталог настроек (для --screenshot по умолчанию временный, чтобы не трогать настоящие)")
    parser.add_argument("--set", action="append", default=[], metavar="KEY=VALUE",
                        help="начальное свойство корневого окна (число или строка)")
    return parser.parse_known_args(argv)[0]


def _parse_value(value: str) -> object:
    if value in ("true", "false"):
        return value == "true"
    for cast in (int, float):
        try:
            return cast(value)
        except ValueError:
            pass
    return value


def load_fonts() -> None:
    for font in FONTS_DIR.glob("*.ttf"):
        if QFontDatabase.addApplicationFont(str(font)) < 0:
            print(f"не удалось загрузить шрифт {font.name}", file=sys.stderr)


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv if argv is None else argv
    args = parse_args(argv[1:])
    # Простой драйвер анимаций тикает таймером ~60 Гц вместо vsync: на 144 Гц всё дёргается.
    os.environ.pop("QSG_USE_SIMPLE_ANIMATION_DRIVER", None)
    if args.debug_wheel:  # QT_LOGGING_RULES пользователя глушит console.log
        os.environ["QT_LOGGING_RULES"] = os.environ.get("QT_LOGGING_RULES", "") + ";qml.debug=true"
    if args.screenshot:
        # Снимок без окна на экране (перекрывает QT_QPA_PLATFORM=wayland из окружения);
        # OpenGL-RHI нужен для Shape/MultiEffect — software-бэкенд их не рисует.
        os.environ["QT_QPA_PLATFORM"] = "offscreen"
        os.environ["QT_QUICK_BACKEND"] = "rhi"
        os.environ.setdefault("QSG_RHI_BACKEND", "opengl")

    config_dir = args.config_dir or (tempfile.mkdtemp(prefix="yamusic-shot-") if args.screenshot else None)
    if config_dir:
        QSettings.setPath(QSettings.Format.NativeFormat, QSettings.Scope.UserScope, config_dir)

    QGuiApplication.setApplicationName("YaMusic")
    QGuiApplication.setOrganizationName("yamusic")
    QGuiApplication.setDesktopFileName("yamusic")
    app = QGuiApplication(argv)
    load_fonts()

    # Сервисы: API живёт в asyncio-потоке, в QML — синглтоны YaMusic.Core.Auth / .Library
    runner = AsyncRunner()
    auth = Auth(runner)
    library = Library(runner, auth)
    qmlRegisterSingletonInstance(Auth, "YaMusic.Core", 1, 0, "Auth", auth)
    qmlRegisterSingletonInstance(Library, "YaMusic.Core", 1, 0, "Library", library)

    engine = QQmlApplicationEngine()
    nam_factory = CachingNamFactory()  # ссылка должна жить столько же, сколько движок
    engine.setNetworkAccessManagerFactory(nam_factory)
    engine.addImportPath(str(QML_DIR))
    engine.rootContext().setContextProperty("yamusicDebugWheel", args.debug_wheel)
    engine.rootContext().setContextProperty("yamusicDebugFps", args.debug_fps)
    app_settings = engine.singletonInstance("YaMusic.Core", "Settings")
    theme_engine = engine.singletonInstance("YaMusic.Core", "ThemeEngine")
    theme_engine.bind_settings(app_settings, app.styleHints())
    if args.light:  # разовое переопределение для разработки, в настройки не пишется
        theme_engine.dark = False
    if args.seed_image:
        theme_engine.setSeedFromImage(str(Path(args.seed_image).resolve()))

    if args.qml:
        entry = Path(args.qml).resolve()
    else:
        entry = QML_DIR / ("gallery/Gallery.qml" if args.gallery else "Main.qml")

    initial: dict[str, object] = {}
    if args.size:
        width, height = (int(v) for v in args.size.lower().split("x"))
        initial.update(width=width, height=height)
    for item in args.set:
        key, _, value = item.partition("=")
        initial[key] = _parse_value(value)
    engine.setInitialProperties(initial)
    engine.load(QUrl.fromLocalFile(str(entry)))
    if not engine.rootObjects():
        return 1
    if not args.offline:
        auth.start()

    if args.screenshot:
        window: QQuickWindow = engine.rootObjects()[0]

        def grab() -> None:
            window.grabWindow().save(args.screenshot)
            app.quit()

        QTimer.singleShot(args.delay, grab)

    code = app.exec()
    runner.stop()
    return code
