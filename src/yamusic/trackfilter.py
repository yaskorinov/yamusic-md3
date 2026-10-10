"""Поиск внутри списка треков: в QML — TrackFilter { source: модель; query: "текст" }.

Отдаёт те же роли, что TrackListModel, и годится для Player.playFrom: в очередь идёт найденное.
Запрос делится на слова, каждое должно встретиться в названии, исполнителях или альбоме
(без учёта регистра, «ё» = «е»). Пустой запрос (active = false) — все треки источника как есть.
Страница всегда показывает именно TrackFilter: ListView с reuseItems при подмене самой модели
достаёт из запаса строки, привязанные к прежней модели, и показывает в них чужие треки.
"""

from __future__ import annotations

from PySide6.QtCore import Property, QObject, Signal
from PySide6.QtQml import QmlElement

from .models import TRACK_KEYS, DictListModel

QML_IMPORT_NAME = "YaMusic.Core"
QML_IMPORT_MAJOR_VERSION = 1


def normalize(text: str) -> str:
    return text.casefold().replace("ё", "е")


def haystack(item: dict) -> str:
    return normalize(" ".join(str(item.get(k) or "") for k in ("title", "version", "artists", "album")))


@QmlElement
class TrackFilter(DictListModel):
    sourceChanged = Signal()
    queryChanged = Signal()

    def __init__(self, parent: QObject | None = None):
        super().__init__(TRACK_KEYS, parent)
        self._source: DictListModel | None = None
        self._query = ""
        self._words: list[str] = []
        self._hay: list[str] | None = None      # строки поиска по трекам источника, пока он не менялся

    @property
    def context(self) -> dict[str, str]:
        return dict(getattr(self._source, "context", {}) or {})

    def _get_source(self) -> QObject | None:
        return self._source

    def _set_source(self, source: QObject | None) -> None:
        if source is self._source:
            return
        if self._source is not None:
            for signal in (self._source.modelReset, self._source.rowsInserted, self._source.rowsRemoved):
                signal.disconnect(self._source_changed)
            self._source.destroyed.disconnect(self._source_gone)
        self._source = source if isinstance(source, DictListModel) else None
        if self._source is not None:
            for signal in (self._source.modelReset, self._source.rowsInserted, self._source.rowsRemoved):
                signal.connect(self._source_changed)
            self._source.destroyed.connect(self._source_gone)
        self._hay = None
        self.sourceChanged.emit()
        self._refresh(reset=True)

    source = Property(QObject, _get_source, _set_source, notify=sourceChanged)

    def _get_query(self) -> str:
        return self._query

    def _set_query(self, query: str) -> None:
        if query == self._query:
            return
        self._query = query
        self._words = normalize(query).split()
        self.queryChanged.emit()
        self._refresh(reset=True)

    query = Property(str, _get_query, _set_query, notify=queryChanged)

    @Property(bool, notify=queryChanged)
    def active(self) -> bool:
        return bool(self._words)

    def _source_gone(self, *_args) -> None:
        self._source = None
        self._hay = None
        self._refresh(reset=True)

    def _source_changed(self, *_args) -> None:
        self._hay = None
        self._refresh(reset=False)

    def _refresh(self, reset: bool) -> None:
        items = self._source.items() if self._source is not None else []
        if self._words:
            if self._hay is None:
                self._hay = [haystack(i) for i in items]
            found = [i for i, hay in zip(items, self._hay) if all(w in hay for w in self._words)]
        else:
            found = list(items)
        old = self._items
        if reset:
            self.reset(found)
            return
        # Источник изменился. Обычно это один кусок — догрузка, новый лайк в начале, снятый лайк:
        # повторяем то же изменение, чтобы список не терял прокрутку.
        head = 0
        while head < min(len(found), len(old)) and found[head] is old[head]:
            head += 1
        tail = 0
        while tail < min(len(found), len(old)) - head and found[-1 - tail] is old[-1 - tail]:
            tail += 1
        if head + tail == len(old) and len(found) > len(old):
            self.insert(head, found[head:len(found) - tail])
        elif head + tail == len(found) and len(old) > len(found):
            self.remove(head, len(old) - len(found))
        elif len(found) != len(old) or head != len(old):
            self.reset(found)
