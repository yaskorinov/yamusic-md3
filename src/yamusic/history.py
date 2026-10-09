"""Учёт прослушиваний (/play-audio): по нему Яндекс строит историю, рекомендации и «Мою волну».

Как официальные клиенты: отметка в начале трека (0 с) и в конце — сколько реально проиграно и где остановились.
"""

from __future__ import annotations

import logging
import os
import uuid

from PySide6.QtCore import QObject

from .aio import AsyncRunner
from .auth import Auth
from .player import Player

log = logging.getLogger(__name__)

DEFAULT_FROM = "desktop_win-own_tracks-track-default"
# Тесты на настоящем аккаунте: YAMUSIC_NO_REPORT=1 — не писать в историю и не слать обратную связь волне
NO_REPORT = bool(os.environ.get("YAMUSIC_NO_REPORT"))


class PlayReporter(QObject):
    def __init__(self, runner: AsyncRunner, auth: Auth, player: Player, parent: QObject | None = None):
        super().__init__(parent)
        self._runner = runner
        self._auth = auth
        self._play_id = ""
        player.trackStarted.connect(self._started)
        player.trackEnded.connect(self._ended)

    def _started(self, event: dict) -> None:
        self._play_id = str(uuid.uuid4())
        self._send(event, 0, 0)

    def _ended(self, event: dict) -> None:
        if event["played"] >= 1:
            self._send(event, event["played"], event["end"])

    def _send(self, event: dict, played: float, end: float) -> None:
        client = self._auth.client
        track, context = event["track"], event["context"]
        if client is None or not track.get("trackId"):
            return
        if NO_REPORT:
            log.warning("play-audio %s: %.1f с, конец %.1f с", track["trackId"], played, end)
            return
        coro = client.play_audio(
            track_id=track["trackId"],
            from_=context.get("from") or DEFAULT_FROM,
            album_id=track.get("albumId") or 0,
            playlist_id=context.get("playlistId"),
            play_id=self._play_id,
            track_length_seconds=round(int(track.get("durationMs") or 0) / 1000),
            total_played_seconds=played,
            end_position_seconds=end,
        )
        self._runner.submit(coro, None, lambda e: log.warning("учёт прослушиваний: %s", e))
