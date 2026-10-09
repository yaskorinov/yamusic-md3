"""Каталог: поиск, страницы альбомов и исполнителей, переходы к ним. В QML — Catalog.

Переходы идут через сигнал openRequested(page, props): его слушает главное окно (закрывает
полноэкранный плеер и открывает страницу), поэтому ссылку на исполнителя или альбом можно
нажать откуда угодно — из строки трека, мини-плеера, очереди.
"""

from __future__ import annotations

from PySide6.QtCore import Property, QObject, Signal, Slot
from PySide6.QtQml import QQmlEngine

from .aio import AsyncRunner
from .auth import Auth
from .models import (AlbumListModel, ArtistListModel, PlaylistListModel, TrackListModel, album_to_dict,
                     artist_to_dict, playlist_to_dict, track_to_dict)

SEARCH_TYPES = {"all": "all", "tracks": "track", "albums": "album", "artists": "artist", "playlists": "playlist"}


def _keep(obj: QObject) -> QObject:
    # объект, возвращённый из слота, QML иначе считает своим и может удалить сборщиком мусора
    QQmlEngine.setObjectOwnership(obj, QQmlEngine.ObjectOwnership.CppOwnership)
    return obj


class AlbumData(QObject):
    changed = Signal()

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._info: dict = {}
        self._loading = True
        self._error = ""
        self._tracks = _keep(TrackListModel(self))
        self._tracks.set_loading(True)

    @Property("QVariantMap", notify=changed)
    def info(self) -> dict:
        return self._info

    @Property(bool, notify=changed)
    def loading(self) -> bool:
        return self._loading

    @Property(str, notify=changed)
    def errorText(self) -> str:
        return self._error

    @Property(QObject, constant=True)
    def tracks(self) -> TrackListModel:
        return self._tracks


class ArtistData(QObject):
    changed = Signal()

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._info: dict = {}
        self._loading = True
        self._error = ""
        self._popular = _keep(TrackListModel(self))
        self._albums = _keep(AlbumListModel(self))
        self._also = _keep(AlbumListModel(self))
        self._similar = _keep(ArtistListModel(self))

    @Property("QVariantMap", notify=changed)
    def info(self) -> dict:
        return self._info

    @Property(bool, notify=changed)
    def loading(self) -> bool:
        return self._loading

    @Property(str, notify=changed)
    def errorText(self) -> str:
        return self._error

    @Property(QObject, constant=True)
    def popular(self) -> TrackListModel:
        return self._popular

    @Property(QObject, constant=True)
    def albums(self) -> AlbumListModel:
        """Свои альбомы, синглы и EP — от новых к старым."""
        return self._albums

    @Property(QObject, constant=True)
    def alsoAlbums(self) -> AlbumListModel:
        """Сборники и чужие релизы с участием исполнителя."""
        return self._also

    @Property(QObject, constant=True)
    def similar(self) -> ArtistListModel:
        return self._similar


class Catalog(QObject):
    openRequested = Signal(str, "QVariantMap")   # страница, параметры
    chooseArtist = Signal("QVariantList")        # у трека несколько исполнителей — пусть выберет
    searchChanged = Signal()

    def __init__(self, runner: AsyncRunner, auth: Auth, parent: QObject | None = None):
        super().__init__(parent)
        self._runner = runner
        self._auth = auth
        self._albums: dict[str, AlbumData] = {}
        self._artists: dict[str, ArtistData] = {}
        # поиск
        self._query = ""
        self._type = "all"
        self._state = "idle"             # idle | loading | ok | empty | error
        self._best: dict = {}
        self._gen = 0
        self._s_tracks = TrackListModel(self)
        self._s_tracks.context = {"type": "search", "from": "desktop_win-search-track-default"}
        self._s_albums = AlbumListModel(self)
        self._s_artists = ArtistListModel(self)
        self._s_playlists = PlaylistListModel(self)
        self._totals: dict[str, int] = {}
        auth.signedOut.connect(self._clear)

    # --- переходы --------------------------------------------------------

    @Slot(str)
    def openArtist(self, artist_id: str) -> None:
        if artist_id:
            self.openRequested.emit("artist", {"artistId": str(artist_id)})

    @Slot(str)
    def openAlbum(self, album_id: str) -> None:
        if album_id:
            self.openRequested.emit("album", {"albumId": str(album_id)})

    @Slot("QVariantList")
    def openArtists(self, refs: list) -> None:
        refs = [r for r in (refs or []) if r and r.get("id")]
        if len(refs) == 1:
            self.openArtist(refs[0]["id"])
        elif refs:
            self.chooseArtist.emit(refs)

    @Slot("QVariantMap")
    def openPlaylist(self, playlist: dict) -> None:
        self.openRequested.emit("playlist", {
            "uid": int(playlist.get("uid") or 0), "kind": int(playlist.get("kind") or 0),
            "playlistTitle": playlist.get("title", ""), "cover": playlist.get("cover", ""),
        })

    # --- альбом и исполнитель --------------------------------------------

    @Slot(str, result=QObject)
    def album(self, album_id: str) -> AlbumData:
        data = self._albums.get(album_id)
        if data is not None and not data._error:
            return data
        data = _keep(AlbumData(self))
        self._albums[album_id] = data
        client = self._auth.client
        if client is None:
            data._loading = False
            data._error = "Войдите в аккаунт"
            return data

        async def load():
            album = await client.albums_with_tracks(album_id)
            tracks = [t for volume in (album.volumes or []) for t in volume]
            return album, tracks

        def done(result) -> None:
            album, tracks = result
            info = album_to_dict(album)
            labels = [label if isinstance(label, str) else label.name for label in (album.labels or [])]
            info["label"] = ", ".join(x for x in labels if x)
            info["genre"] = album.genre or ""
            info["durationMs"] = sum(t.duration_ms or 0 for t in tracks)
            data._info = info
            data._tracks.context = {"type": "album", "from": "desktop_win-album-track-default"}
            data._tracks.reset([track_to_dict(t) for t in tracks])
            data._tracks.set_loading(False)
            data._loading = False
            data.changed.emit()

        self._runner.submit(load(), done, lambda e: self._failed(data, e))
        return data

    @Slot(str, result=QObject)
    def artist(self, artist_id: str) -> ArtistData:
        data = self._artists.get(artist_id)
        if data is not None and not data._error:
            return data
        data = _keep(ArtistData(self))
        self._artists[artist_id] = data
        client = self._auth.client
        if client is None:
            data._loading = False
            data._error = "Войдите в аккаунт"
            return data

        async def load():
            brief = await client.artists_brief_info(artist_id)
            try:
                direct = await client.artists_direct_albums(artist_id, page_size=100)
                albums = direct.albums if direct and direct.albums else None
            except Exception:
                albums = None
            return brief, albums

        def done(result) -> None:
            brief, albums = result
            info = artist_to_dict(brief.artist)
            stats = brief.stats
            info["listeners"] = stats.last_month_listeners if stats and stats.last_month_listeners else 0
            info["likes"] = brief.artist.likes_count or 0
            data._info = info
            data._popular.context = {"type": "artist", "from": "desktop_win-artist-track-default"}
            data._popular.reset([track_to_dict(t) for t in brief.popular_tracks or []])
            own = albums if albums is not None else brief.albums or []
            own = sorted(own, key=lambda a: (a.year or 0, a.release_date or ""), reverse=True)
            data._albums.reset([album_to_dict(a) for a in own])
            data._also.reset([album_to_dict(a) for a in brief.also_albums or []])
            data._similar.reset([artist_to_dict(a) for a in brief.similar_artists or []])
            data._loading = False
            data.changed.emit()

        self._runner.submit(load(), done, lambda e: self._failed(data, e))
        return data

    def _failed(self, data: QObject, error: BaseException) -> None:
        if isinstance(data, AlbumData):
            data._tracks.set_loading(False)
        data._loading = False
        data._error = f"Не удалось загрузить: {error}"
        data.changed.emit()

    # --- поиск -----------------------------------------------------------

    @Property(str, notify=searchChanged)
    def searchState(self) -> str:
        return self._state

    @Property(str, notify=searchChanged)
    def searchQuery(self) -> str:
        return self._query

    @Property("QVariantMap", notify=searchChanged)
    def best(self) -> dict:
        """{type: track|album|artist|playlist, item: dict}"""
        return self._best

    @Property("QVariantMap", notify=searchChanged)
    def totals(self) -> dict:
        return self._totals

    @Property(QObject, constant=True)
    def searchTracks(self) -> TrackListModel:
        return self._s_tracks

    @Property(QObject, constant=True)
    def searchAlbums(self) -> AlbumListModel:
        return self._s_albums

    @Property(QObject, constant=True)
    def searchArtists(self) -> ArtistListModel:
        return self._s_artists

    @Property(QObject, constant=True)
    def searchPlaylists(self) -> PlaylistListModel:
        return self._s_playlists

    @Slot(str, str)
    def search(self, query: str, kind: str = "all") -> None:
        query = query.strip()
        kind = kind if kind in SEARCH_TYPES else "all"
        if query == self._query and kind == self._type and self._state in ("loading", "ok", "empty"):
            return
        self._query = query
        self._type = kind
        self._gen += 1
        gen = self._gen
        if not query:
            self._clear()
            return
        client = self._auth.client
        if client is None:
            self._state = "error"
            self.searchChanged.emit()
            return
        self._state = "loading"
        self.searchChanged.emit()

        async def run():
            return await client.search(query, type_=SEARCH_TYPES[kind], nocorrect=False)

        def done(result) -> None:
            if gen != self._gen:
                return
            self._apply(result)

        def failed(error: BaseException) -> None:
            if gen == self._gen:
                self._state = "error"
                self.searchChanged.emit()

        self._runner.submit(run(), done, failed)

    def _apply(self, result) -> None:
        def items(block):
            return block.results if block and block.results else []

        self._s_tracks.reset([track_to_dict(t) for t in items(result.tracks) if t is not None])
        self._s_albums.reset([album_to_dict(a) for a in items(result.albums)])
        self._s_artists.reset([artist_to_dict(a) for a in items(result.artists)])
        self._s_playlists.reset([playlist_to_dict(p) for p in items(result.playlists)])
        self._totals = {k: (getattr(result, k).total if getattr(result, k, None) else 0)
                        for k in ("tracks", "albums", "artists", "playlists")}
        self._best = {}
        best = result.best
        if best is not None and best.result is not None:
            convert = {"track": track_to_dict, "album": album_to_dict, "artist": artist_to_dict,
                       "playlist": playlist_to_dict}.get(best.type)
            if convert is not None:
                self._best = {"type": best.type, "item": convert(best.result)}
        empty = not any((self._s_tracks.count, self._s_albums.count, self._s_artists.count, self._s_playlists.count))
        self._state = "empty" if empty else "ok"
        self.searchChanged.emit()

    def _clear(self) -> None:
        self._state = "idle"
        self._best = {}
        self._totals = {}
        for model in (self._s_tracks, self._s_albums, self._s_artists, self._s_playlists):
            model.reset([])
        self.searchChanged.emit()
