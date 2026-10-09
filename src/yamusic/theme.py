"""Цветовая схема MD3 из seed-цвета или обложки (materialyoucolor, spec 2025)."""

from __future__ import annotations

from PySide6.QtCore import (
    Property,
    QObject,
    QRunnable,
    QThreadPool,
    QUrl,
    Qt,
    Signal,
    Slot,
)
from PySide6.QtGui import QColor, QImage
from PySide6.QtQml import QmlElement, QmlSingleton

from materialyoucolor.dynamiccolor.color_spec import COLOR_NAMES
from materialyoucolor.dynamiccolor.material_dynamic_colors import MaterialDynamicColors
from materialyoucolor.hct import Hct
from materialyoucolor.quantize import QuantizeCelebi
from materialyoucolor.scheme.scheme_content import SchemeContent
from materialyoucolor.scheme.scheme_expressive import SchemeExpressive
from materialyoucolor.scheme.scheme_fidelity import SchemeFidelity
from materialyoucolor.scheme.scheme_monochrome import SchemeMonochrome
from materialyoucolor.scheme.scheme_neutral import SchemeNeutral
from materialyoucolor.scheme.scheme_tonal_spot import SchemeTonalSpot
from materialyoucolor.scheme.scheme_vibrant import SchemeVibrant
from materialyoucolor.score.score import Score

QML_IMPORT_NAME = "YaMusic.Core"
QML_IMPORT_MAJOR_VERSION = 1

SCHEMES = {
    "content": SchemeContent,
    "tonalSpot": SchemeTonalSpot,
    "vibrant": SchemeVibrant,
    "expressive": SchemeExpressive,
    "fidelity": SchemeFidelity,
    "neutral": SchemeNeutral,
    "monochrome": SchemeMonochrome,
}

DEFAULT_SEED = 0xFF6750A4
_DYNAMIC = MaterialDynamicColors()
_ROLES = [n for n in COLOR_NAMES if getattr(_DYNAMIC, n, None) is not None]


def seed_from_image(image: QImage) -> int:
    """Доминантный «фирменный» цвет картинки по алгоритму Material (Celebi + Score)."""
    small = image.scaled(
        112, 112, Qt.AspectRatioMode.IgnoreAspectRatio, Qt.TransformationMode.SmoothTransformation
    ).convertToFormat(QImage.Format.Format_RGB888)
    data = bytes(small.constBits())
    stride, width = small.bytesPerLine(), small.width()
    pixels = [
        [data[row + x], data[row + x + 1], data[row + x + 2]]
        for row in range(0, stride * small.height(), stride)
        for x in range(0, width * 3, 3)
    ]
    ranked = Score.score(QuantizeCelebi(pixels, 128))
    return ranked[0] if ranked else DEFAULT_SEED


def build_scheme(seed: int, dark: bool, variant: str, contrast: float) -> dict[str, int]:
    """ARGB всех ролей. ~40 мс чистого Python — вызывать только из пула потоков."""
    scheme_cls = SCHEMES.get(variant, SchemeContent)
    scheme = scheme_cls(Hct.from_int(seed), dark, contrast)
    return {name: scheme.get_argb(getattr(_DYNAMIC, name)) for name in _ROLES}


class _JobSignals(QObject):
    done = Signal(int, object, object)  # request id, seed argb (не влезает в signed int), {роль: argb}


class _SchemeJob(QRunnable):
    """Фоновая задача: (опционально) seed из картинки + сборка схемы. В GUI-потоке — только QColor."""

    def __init__(self, request_id: int, signals: _JobSignals, seed: int, dark: bool, variant: str,
                 contrast: float, image_path: str | None = None):
        super().__init__()
        self.request_id = request_id
        self.signals = signals
        self.seed = seed
        self.dark = dark
        self.variant = variant
        self.contrast = contrast
        self.image_path = image_path

    def run(self) -> None:
        seed = self.seed
        if self.image_path is not None:
            image = QImage(self.image_path)
            if image.isNull():
                return
            seed = seed_from_image(image)
        colors = build_scheme(seed, self.dark, self.variant, self.contrast)
        self.signals.done.emit(self.request_id, seed, colors)


@QmlElement
@QmlSingleton
class ThemeEngine(QObject):
    """Источник истины для цветовых ролей. Анимация смены цветов — в QML (Md3/Theme.qml)."""

    schemeChanged = Signal()
    seedChanged = Signal()
    darkChanged = Signal()
    variantChanged = Signal()

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._seed = DEFAULT_SEED
        self._dark = True
        self._variant = "content"
        self._contrast = 0.0
        self._request = 0
        self._pending_image: str | None = None
        self._cover: str | None = None      # обложка текущего трека (путь), даже если акцент не из неё
        self._settings = None
        self._hints = None
        self._signals = _JobSignals()
        self._signals.done.connect(self._on_scheme_ready)
        # Стартовая схема синхронно: QML должен получить цвета сразу при создании.
        self._colors = self._to_qcolors(build_scheme(self._seed, self._dark, self._variant, self._contrast))

    @staticmethod
    def _to_qcolors(argb: dict[str, int]) -> dict[str, QColor]:
        return {name: QColor.fromRgba(value) for name, value in argb.items()}

    def _rebuild(self, image_path: str | None = None) -> None:
        """Любое изменение (seed, обложка, тёмная тема, вариант) пересчитывается в пуле потоков;
        более новый запрос отменяет результат предыдущего (но не теряет ещё не разобранную обложку)."""
        if image_path is None:
            image_path = self._pending_image
        self._pending_image = image_path
        self._request += 1
        QThreadPool.globalInstance().start(_SchemeJob(
            self._request, self._signals, self._seed, self._dark, self._variant, self._contrast, image_path))

    def _on_scheme_ready(self, request_id: int, seed: int, colors: dict[str, int]) -> None:
        if request_id != self._request:
            return
        self._pending_image = None
        seed &= 0xFFFFFFFF
        if seed != self._seed:
            self._seed = seed
            self.seedChanged.emit()
        self._colors = self._to_qcolors(colors)
        self.schemeChanged.emit()

    # --- properties -------------------------------------------------------

    @Property("QVariantMap", notify=schemeChanged)
    def colors(self) -> dict[str, QColor]:
        return self._colors

    @Property(QColor, notify=seedChanged)
    def seed(self) -> QColor:
        return QColor.fromRgba(self._seed)

    def _get_dark(self) -> bool:
        return self._dark

    def _set_dark(self, value: bool) -> None:
        if value != self._dark:
            self._dark = value
            self.darkChanged.emit()
            self._rebuild()

    dark = Property(bool, _get_dark, _set_dark, notify=darkChanged)

    def _get_variant(self) -> str:
        return self._variant

    def _set_variant(self, value: str) -> None:
        if value != self._variant and value in SCHEMES:
            self._variant = value
            self.variantChanged.emit()
            self._rebuild()

    variant = Property(str, _get_variant, _set_variant, notify=variantChanged)

    @Property("QStringList", constant=True)
    def variants(self) -> list[str]:
        return list(SCHEMES)

    # --- настройки ----------------------------------------------------------

    def bind_settings(self, settings: QObject, style_hints) -> None:
        """Тема следует настройкам: themeMode, schemeVariant, accentFromCover, customSeed.
        Стартовая схема строится синхронно, чтобы окно не «перекрашивалось» при запуске."""
        self._settings = settings
        self._hints = style_hints
        for signal in (settings.themeModeChanged, settings.schemeVariantChanged,
                       settings.accentFromCoverChanged, settings.customSeedChanged):
            signal.connect(self._apply_settings)
        style_hints.colorSchemeChanged.connect(self._apply_settings)

        self._dark, self._variant, self._seed = self._settings_state()
        self._colors = self._to_qcolors(build_scheme(self._seed, self._dark, self._variant, self._contrast))
        self.darkChanged.emit()
        self.variantChanged.emit()
        self.seedChanged.emit()
        self.schemeChanged.emit()

    def _settings_state(self) -> tuple[bool, str, int]:
        s = self._settings
        mode = s.themeMode
        if mode == "system":
            dark = self._hints.colorScheme() != Qt.ColorScheme.Light
        else:
            dark = mode != "light"
        variant = s.schemeVariant if s.schemeVariant in SCHEMES else "content"
        seed = QColor(s.customSeed)
        return dark, variant, (seed.rgba() if seed.isValid() else DEFAULT_SEED) & 0xFFFFFFFF

    def _apply_settings(self, *_args) -> None:
        dark, variant, seed = self._settings_state()
        if dark != self._dark:
            self._dark = dark
            self.darkChanged.emit()
        if variant != self._variant:
            self._variant = variant
            self.variantChanged.emit()
        if self._settings.accentFromCover and self._cover:
            self._rebuild(self._cover)
        else:
            self._pending_image = None
            self._seed = seed
            self.seedChanged.emit()
            self._rebuild()

    @Slot(str)
    def setCover(self, source: str) -> None:
        """Обложка текущего трека ('' — ничего не играет). Акцент берётся из неё, если так настроено."""
        url = QUrl(source)
        self._cover = (url.toLocalFile() if url.isLocalFile() else source) or None
        if self._settings is None or self._settings.accentFromCover:
            if self._cover:
                self._rebuild(self._cover)
            elif self._settings is not None:
                self._apply_settings()

    # --- slots ------------------------------------------------------------

    @Slot(QColor)
    def setSeed(self, color: QColor) -> None:
        self._pending_image = None
        self._seed = color.rgba() & 0xFFFFFFFF
        self.seedChanged.emit()
        self._rebuild()

    @Slot(str)
    def setSeedFromImage(self, source: str) -> None:
        """Принимает путь или file:// URL."""
        url = QUrl(source)
        self._rebuild(url.toLocalFile() if url.isLocalFile() else source)
