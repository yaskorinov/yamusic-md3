"""QAbstractListModel для QML: треки и плейлисты. Данные — простые dict, собранные из объектов yandex_music."""

from __future__ import annotations

from PySide6.QtCore import Property, QAbstractListModel, QByteArray, QModelIndex, QObject, Qt, Signal, Slot

COVER_SIZE = "400x400"


def cover_url(uri: str | None, size: str = COVER_SIZE) -> str:
    """'avatars.yandex.net/get-music-content/…/%%' → https-URL нужного размера."""
    if not uri:
        return ""
    return "https://" + uri.replace("%%", size)


def track_to_dict(track) -> dict[str, object]:
    album = track.albums[0] if track.albums else None
    return {
        "trackId": str(track.id),
        "albumId": str(album.id) if album else "",
        "title": track.title or "",
        "version": getattr(track, "version", None) or "",
        "artists": ", ".join(a.name for a in track.artists if a.name),
        "album": (album.title or "") if album else "",
        "cover": cover_url(track.cover_uri or (album.cover_uri if album else None)),
        "durationMs": track.duration_ms or 0,
        "explicit": track.content_warning == "explicit",
        "available": track.available is not False,
    }


def playlist_to_dict(playlist) -> dict[str, object]:
    uri = None
    cover = playlist.cover
    if cover is not None:
        if cover.uri:
            uri = cover.uri
        elif cover.items_uri:
            uri = cover.items_uri[0]
    uri = uri or playlist.og_image
    return {
        "uid": playlist.uid or (playlist.owner.uid if playlist.owner else 0),
        "kind": playlist.kind or 0,
        "title": playlist.title or "",
        "trackCount": playlist.track_count or 0,
        "cover": cover_url(uri, "200x200"),
        "owner": (playlist.owner.name or playlist.owner.login or "") if playlist.owner else "",
    }


class DictListModel(QAbstractListModel):
    """Список dict с фиксированным набором ключей → роли QML (по имени ключа)."""

    countChanged = Signal()
    loadingChanged = Signal()

    def __init__(self, keys: list[str], parent: QObject | None = None):
        super().__init__(parent)
        self._keys = keys
        self._roles = {Qt.ItemDataRole.UserRole + i: QByteArray(k.encode()) for i, k in enumerate(keys)}
        self._items: list[dict[str, object]] = []
        self._loading = False

    def roleNames(self) -> dict[int, QByteArray]:
        return self._roles

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:
        return 0 if parent.isValid() else len(self._items)

    def data(self, index: QModelIndex, role: int = Qt.ItemDataRole.DisplayRole) -> object:
        if not index.isValid() or not (0 <= index.row() < len(self._items)):
            return None
        key = self._keys[role - Qt.ItemDataRole.UserRole] if role >= Qt.ItemDataRole.UserRole else None
        return self._items[index.row()].get(key) if key else None

    @Property(int, notify=countChanged)
    def count(self) -> int:
        return len(self._items)

    # Здесь, а не в TrackListModel: Property в подклассе с notify на сигнал базового класса
    # даёт битый метаобъект PySide (segfault в QMetaProperty::notifySignalIndex).
    @Property(int, notify=countChanged)
    def totalDurationMs(self) -> int:
        return sum(int(i.get("durationMs") or 0) for i in self._items)

    def _get_loading(self) -> bool:
        return self._loading

    def set_loading(self, value: bool) -> None:
        if value != self._loading:
            self._loading = value
            self.loadingChanged.emit()

    loading = Property(bool, _get_loading, notify=loadingChanged)

    @Slot(int, result="QVariantMap")
    def get(self, row: int) -> dict[str, object]:
        return self._items[row] if 0 <= row < len(self._items) else {}

    def items(self) -> list[dict[str, object]]:
        return self._items

    def reset(self, items: list[dict[str, object]]) -> None:
        self.beginResetModel()
        self._items = list(items)
        self.endResetModel()
        self.countChanged.emit()

    def append(self, items: list[dict[str, object]]) -> None:
        if not items:
            return
        first = len(self._items)
        self.beginInsertRows(QModelIndex(), first, first + len(items) - 1)
        self._items.extend(items)
        self.endInsertRows()
        self.countChanged.emit()


TRACK_KEYS = ["trackId", "albumId", "title", "version", "artists", "album", "cover", "durationMs", "explicit", "available"]
PLAYLIST_KEYS = ["uid", "kind", "title", "trackCount", "cover", "owner"]


class TrackListModel(DictListModel):
    def __init__(self, parent: QObject | None = None):
        super().__init__(TRACK_KEYS, parent)


class PlaylistListModel(DictListModel):
    def __init__(self, parent: QObject | None = None):
        super().__init__(PLAYLIST_KEYS, parent)
