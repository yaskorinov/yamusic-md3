# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec для YaMusic Windows."""

import sys
from pathlib import Path

src = Path("src/yamusic")

a = Analysis(
    ["src/yamusic/__main__.py"],
    pathex=[str(Path("src"))],
    binaries=[
        # libmpv ищет python-mpv; кладём рядом с exe
        (r"C:\Users\Fatyzzz\yamusic\yamusic-md3\libmpv-2.dll", "."),
    ],
    datas=[
        (str(src / "qml"), "yamusic/qml"),
        (str(src / "assets"), "yamusic/assets"),
    ],
    hiddenimports=[
        "keyring.backends.Windows",
        "keyring.backends.fail",
        "materialyoucolor",
        "materialyoucolor.hct",
        "materialyoucolor.dynamiccolor",
        "materialyoucolor.scheme",
        "PySide6.QtDBus",      # импорт условный, но Qt-плагины могут ссылаться
        "PySide6.QtSvg",
        "PySide6.QtSvgWidgets",
        "yandex_music",
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        "dbus_fast",
        "secretstorage",
        "tkinter",
        "unittest",
        "test",
    ],
    noarchive=False,
)

pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="yamusic",
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=False,    # без консольного окна
    disable_windowed_traceback=False,
    icon="yamusic.ico",
)

coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=False,
    upx_exclude=[],
    name="yamusic",
)
