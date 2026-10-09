"""Настройки приложения (QSettings → ~/.config/yamusic/yamusic.conf), доступны в QML как Settings."""

from __future__ import annotations

from PySide6.QtCore import Property, QObject, QSettings, Signal
from PySide6.QtQml import QmlElement, QmlSingleton

QML_IMPORT_NAME = "YaMusic.Core"
QML_IMPORT_MAJOR_VERSION = 1

# имя → (тип, значение по умолчанию). Каждое становится Q_PROPERTY с сигналом <имя>Changed.
SCHEMA: dict[str, tuple[type, object]] = {
    # Внешний вид
    "themeMode": (str, "dark"),            # system | light | dark
    "schemeVariant": (str, "content"),     # см. theme.SCHEMES
    "accentFromCover": (bool, True),       # перекрашивать приложение под обложку
    "customSeed": (str, "#FFCC00"),        # seed, когда обложки нет или accentFromCover выключен
    "windowButtons": (bool, False),        # кнопки окна для не-тайлинговых DE
    # Раскладка
    "rightPanelOpen": (bool, False),
    "rightPanelTab": (str, "queue"),       # queue | lyrics
    "sidebarCollapsed": (bool, False),
    "windowWidth": (int, 1280),
    "windowHeight": (int, 800),
    # Воспроизведение
    "quality": (str, "lossless"),          # lossless | high | low
    "volume": (float, 0.7),
    # Поведение
    "wheelStep": (int, 150),
}


class _SettingsBase(QObject):
    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._store = QSettings()

    def _read(self, name: str):
        typ, default = SCHEMA[name]
        value = self._store.value(name, default)
        if typ is bool and isinstance(value, str):  # INI-формат хранит bool строками
            return value.lower() == "true"
        try:
            return typ(value)
        except (TypeError, ValueError):
            return default

    def _write(self, name: str, value) -> None:
        if self._read(name) == value:
            return
        self._store.setValue(name, value)
        getattr(self, f"{name}Changed").emit()


def _build_class() -> type:
    attrs: dict[str, object] = {"__doc__": "Персистентные настройки; свойства генерируются из SCHEMA."}
    for name, (typ, _default) in SCHEMA.items():
        signal = Signal()
        attrs[f"{name}Changed"] = signal
        attrs[name] = Property(
            typ,
            lambda self, n=name: self._read(n),
            lambda self, value, n=name: self._write(n, value),
            notify=signal,
        )
    return type("Settings", (_SettingsBase,), attrs)


Settings = QmlElement(QmlSingleton(_build_class()))
