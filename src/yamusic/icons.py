"""Оптическое центрирование иконок Material Symbols.

Часть глифов нарисована не по центру своей ячейки (favorite выше центра на ~8 % кегля и т. п.),
а FontMetrics.tightBoundingRect для лигатур вариативного шрифта возвращает неверный контур.
Поэтому глиф один раз рисуется в буфер, его фактические границы меряются по пикселям,
и в QML отдаются поправка и размер контура в пикселях. Результат кэшируется по (имя, кегль).
Размер контура нужен кнопкам и чипам: отступы MD3 отмеряются от видимой иконки, а не от её квадрата.

Шрифт урезан до используемых иконок (tools/subset_fonts.py), поэтому иконка рисуется символом
по коду из assets/fonts/icons.json, а не лигатурой имени — glyph(name).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

from PySide6.QtCore import QObject, QRectF, Qt, Slot
from PySide6.QtGui import QFont, QFontMetricsF, QImage, QPainter
from PySide6.QtQml import QmlElement, QmlSingleton

QML_IMPORT_NAME = "YaMusic.Core"
QML_IMPORT_MAJOR_VERSION = 1

FAMILY = "Material Symbols Rounded"
CODEPOINTS: dict[str, int] = json.loads((Path(__file__).parent / "assets" / "fonts" / "icons.json").read_text())


@QmlElement
@QmlSingleton
class IconMetrics(QObject):
    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._cache: dict[tuple[str, int], QRectF] = {}

    @Slot(str, result=str)
    def glyph(self, name: str) -> str:
        code = CODEPOINTS.get(name)
        if code is None:
            if name:
                print(f"иконки «{name}» нет в урезанном шрифте — запустите tools/subset_fonts.py", file=sys.stderr)
            return ""
        return chr(code)

    @Slot(str, float, result=QRectF)
    def ink(self, name: str, size: float) -> QRectF:
        """x, y — смещение центра контура глифа от центра его текстового блока (advance × height);
        width, height — размер контура. Всё в пикселях, в том же кегле и opsz, что и отрисовка
        (у мелких кеглей свой вариант глифа и хинтинг). В QML: glyph.x -= x, glyph.y -= y."""
        key = (name, round(size))
        cached = self._cache.get(key)
        if cached is not None:
            return cached
        result = self._measure(name, key[1]) if name and key[1] > 0 else QRectF()
        self._cache[key] = result
        return result

    def _measure(self, name: str, size: int) -> QRectF:
        font = QFont(FAMILY)
        font.setPixelSize(size)
        font.setVariableAxis(QFont.Tag("FILL"), 1)
        font.setVariableAxis(QFont.Tag("opsz"), max(20, min(48, size)))
        text = self.glyph(name)
        if not text:
            return QRectF()
        metrics = QFontMetricsF(font)
        advance = metrics.horizontalAdvance(text)
        height = metrics.height()
        w, h = int(advance) + 8, int(height) + 8
        image = QImage(w, h, QImage.Format.Format_Alpha8)
        image.fill(0)
        painter = QPainter(image)
        painter.setFont(font)
        painter.setPen(Qt.GlobalColor.black)
        painter.drawText(QRectF(4, 4, advance, height), Qt.AlignmentFlag.AlignLeft | Qt.AlignmentFlag.AlignTop, text)
        painter.end()

        data = bytes(image.constBits())
        stride = image.bytesPerLine()
        top = bottom = left = right = None
        for y in range(h):
            row = data[y * stride:y * stride + w]
            if not any(row):
                continue
            if max(row) <= 96:
                continue
            first = next(i for i, v in enumerate(row) if v > 96)
            last = w - 1 - next(i for i, v in enumerate(reversed(row)) if v > 96)
            top = y if top is None else top
            bottom = y
            left = first if left is None else min(left, first)
            right = last if right is None else max(right, last)
        if top is None:
            return QRectF()
        cx = (left + right + 1) / 2 - (4 + advance / 2)
        cy = (top + bottom + 1) / 2 - (4 + height / 2)
        return QRectF(cx, cy, right - left + 1, bottom - top + 1)
