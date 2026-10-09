"""Частота кадров на системах с мониторами разной частоты.

Qt Quick (6.12) берёт «период vsync» для драйвера анимаций из QGuiApplication.primaryScreen().
На Wayland основным экраном Qt считает первый монитор, о котором сообщил композитор, — например,
60-герцовый, даже если окно открыто на 144-герцовом. Тогда Qt видит, что кадры приходят «слишком
часто», решает, что vsync сломан, и до конца жизни окна гоняет анимации системным таймером 60 Гц.

Лечение — сделать основным самый быстрый монитор до создания первого окна. Публичного API нет,
поэтому вызываем экспортируемую QWindowSystemInterface::handlePrimaryScreenChanged (её используют
платформенные плагины). Затрагивает только наш процесс. Отключить: YAMUSIC_KEEP_PRIMARY_SCREEN=1.
Если окно окажется на медленном мониторе, Qt сам переключит драйвер на настенное время (TimerMode).
"""

from __future__ import annotations

import ctypes
import os
from pathlib import Path

import shiboken6
from PySide6 import QtGui
from PySide6.QtGui import QGuiApplication

_HANDLE = "_ZNK7QScreen6handleEv"
_SET_PRIMARY = "_ZN22QWindowSystemInterface26handlePrimaryScreenChangedEP15QPlatformScreen"


def prefer_fastest_screen(app: QGuiApplication) -> str:
    """Сделать основным экраном Qt монитор с максимальной частотой. Возвращает строку для лога."""
    if os.environ.get("YAMUSIC_KEEP_PRIMARY_SCREEN"):
        return "primary screen: оставлен системный (YAMUSIC_KEEP_PRIMARY_SCREEN)"
    screens = app.screens()
    if len(screens) < 2:
        return "primary screen: один монитор"
    primary = app.primaryScreen()
    fastest = max(screens, key=lambda s: s.refreshRate())
    if fastest is primary or fastest.refreshRate() <= primary.refreshRate() + 0.5:
        return f"primary screen: {primary.name()} уже самый быстрый"

    lib_path = Path(QtGui.__file__).parent / "Qt" / "lib" / "libQt6Gui.so.6"
    try:
        lib = ctypes.CDLL(str(lib_path))
        handle = getattr(lib, _HANDLE)
        handle.restype = ctypes.c_void_p
        handle.argtypes = [ctypes.c_void_p]
        set_primary = getattr(lib, _SET_PRIMARY)
        set_primary.restype = None
        set_primary.argtypes = [ctypes.c_void_p]
    except (OSError, AttributeError) as e:
        return f"primary screen: не удалось подключиться к Qt ({e})"

    platform_screen = handle(shiboken6.getCppPointer(fastest)[0])
    if not platform_screen:
        return "primary screen: у экрана нет платформенного объекта"
    set_primary(platform_screen)
    now = app.primaryScreen()
    return f"primary screen: {primary.name()} ({primary.refreshRate():.0f} Гц) → {now.name()} ({now.refreshRate():.0f} Гц)"
