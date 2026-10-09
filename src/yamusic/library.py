"""Библиотека пользователя: плейлисты, «Мне нравится», треки плейлистов. Доступна в QML как Library."""

from __future__ import annotations

from PySide6.QtCore import Property, QObject, Signal, Slot
from PySide6.QtQml import QQmlEngine

from .aio import AsyncRunner
from .auth import Auth
from .models import PlaylistListModel, TrackListModel, playlist_to_dict, track_to_dict

CHUNK = 200  # треки догружаются порциями: первые строки видны сразу, даже если лайков тысячи


class Library(QObject):
    errorChanged = Signal()

    def __init__(self, runner: AsyncRunner, auth: Auth, parent: QObject | None = None):
        super().__init__(parent)
        self._runner = runner
        self._auth = auth
        self._playlists = PlaylistListModel(self)
        self._liked = TrackListModel(self)
        self._liked_ids: set[str] = set()
        self._playlist_models: dict[str, TrackListModel] = {}
        self._error = ""
        self._generation = 0  # смена аккаунта отменяет ответы старых запросов
        auth.signedIn.connect(self.refresh)
        auth.signedOut.connect(self._clear)

    # --- свойства --------------------------------------------------------

    @Property(QObject, constant=True)
    def playlists(self) -> PlaylistListModel:
        return self._playlists

    @Property(QObject, constant=True)
    def liked(self) -> TrackListModel:
        return self._liked

    @Property(str, notify=errorChanged)
    def errorText(self) -> str:
        return self._error

    # --- загрузка --------------------------------------------------------

    @Slot()
    def refresh(self) -> None:
        client = self._auth.client
        if client is None:
            return
        self._generation += 1
        gen = self._generation
        self._set_error("")

        self._playlists.set_loading(True)

        async def load_playlists():
            return [playlist_to_dict(p) for p in await client.users_playlists_list()]

        self._runner.submit(load_playlists(), lambda items: self._if_current(gen, self._playlists_loaded, items), self._failed)

        self._liked.set_loading(True)
        self._liked.reset([])

        async def load_liked() -> None:
            likes = await client.users_likes_tracks()
            ids = likes.tracks_ids if likes else []
            self._runner.call_in_gui(lambda: self._if_current(gen, self._set_liked_ids, ids))
            for start in range(0, len(ids), CHUNK):
                tracks = await client.tracks(ids[start:start + CHUNK])
                chunk = [track_to_dict(t) for t in tracks]
                self._runner.call_in_gui(lambda c=chunk: self._if_current(gen, self._liked.append, c))

        self._runner.submit(load_liked(), lambda _: self._if_current(gen, self._liked.set_loading, False), self._failed)

    @Slot(int, int, result=QObject)
    def playlistTracks(self, uid: int, kind: int) -> TrackListModel:
        """Модель треков плейлиста; заполняется асинхронно (loading → false)."""
        key = f"{uid}:{kind}"
        model = self._playlist_models.get(key)
        if model is None:
            model = TrackListModel(self)
            # объект, возвращённый из слота, QML иначе считает своим и может удалить сборщиком мусора
            QQmlEngine.setObjectOwnership(model, QQmlEngine.ObjectOwnership.CppOwnership)
            self._playlist_models[key] = model
        client = self._auth.client
        if client is None:
            return model
        gen = self._generation
        model.set_loading(True)

        async def load():
            playlist = await client.users_playlists(kind, uid)
            shorts = playlist.tracks if playlist and playlist.tracks else []
            if not shorts and playlist and playlist.track_count:
                shorts = await playlist.fetch_tracks_async()
            tracks = [s.track for s in shorts if s.track is not None]
            missing = [s.track_id for s in shorts if s.track is None]
            for start in range(0, len(missing), CHUNK):
                tracks += await client.tracks(missing[start:start + CHUNK])
            return [track_to_dict(t) for t in tracks]

        def done(items):
            if gen == self._generation:
                model.reset(items)
                model.set_loading(False)

        def failed(error):
            model.set_loading(False)
            self._failed(error)

        self._runner.submit(load(), done, failed)
        return model

    @Slot(str, result=bool)
    def isLiked(self, track_id: str) -> bool:
        return track_id.split(":")[0] in self._liked_ids

    # --- служебное -------------------------------------------------------

    def _if_current(self, gen: int, fn, *args) -> None:
        if gen == self._generation:
            fn(*args)

    def _playlists_loaded(self, items) -> None:
        self._playlists.reset(items)
        self._playlists.set_loading(False)

    def _set_liked_ids(self, ids) -> None:
        self._liked_ids = {i.split(":")[0] for i in ids}

    def _failed(self, error: BaseException) -> None:
        self._playlists.set_loading(False)
        self._liked.set_loading(False)
        self._set_error(f"Не удалось загрузить библиотеку: {error}")

    def _clear(self) -> None:
        self._generation += 1
        self._playlists.reset([])
        self._liked.reset([])
        self._liked_ids = set()
        for model in self._playlist_models.values():
            model.reset([])

    def _set_error(self, text: str) -> None:
        if text != self._error:
            self._error = text
            self.errorChanged.emit()
