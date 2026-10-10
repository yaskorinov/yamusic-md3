"""Синхронный текст играющего трека. В QML — Lyrics (lines, status, source, kind).

Поиск — lyrics_sources.fetch в рабочем потоке; Яндекс спрашивается через asyncio-цикл клиента.
Результат кэшируется в ~/.cache/yamusic/YaMusic/lyrics (ненайденный — на 6 часов).
"""

from __future__ import annotations

import asyncio
import hashlib
import json
import logging
import os
import time
from pathlib import Path

import aiohttp
from PySide6.QtCore import Property, QObject, QTimer, Signal

from . import lyrics_sources as sources
from .aio import AsyncRunner
from .auth import Auth
from .images import cache_dir
from .player import Player

log = logging.getLogger(__name__)


class Lyrics(QObject):
    changed = Signal()

    def __init__(self, runner: AsyncRunner, auth: Auth, player: Player, parent: QObject | None = None):
        super().__init__(parent)
        self._runner = runner
        self._auth = auth
        self._player = player
        self._lines: list[dict] = []
        self._status = "idle"            # idle | loading | ok | notfound | error
        self._source = ""
        self._kind = ""
        self._track_id = ""
        self._gen = 0
        sources.CACHE_DIR = cache_dir("lyrics")
        os.makedirs(sources.CACHE_DIR, exist_ok=True)
        # Пролистывание треков подряд не должно запускать поиск на каждый
        self._debounce = QTimer(self, singleShot=True, interval=350, timeout=self._load)
        player.trackChanged.connect(self._debounce.start)

    @Property("QVariantList", notify=changed)
    def lines(self) -> list[dict]:
        return self._lines

    @Property(str, notify=changed)
    def trackId(self) -> str:
        """Трек, к которому относятся lines/status (после смены трека отстаёт на время поиска)."""
        return self._track_id

    @Property(str, notify=changed)
    def status(self) -> str:
        return self._status

    @Property(str, notify=changed)
    def source(self) -> str:
        return self._source

    @Property(str, notify=changed)
    def kind(self) -> str:
        """word — время каждого слова, line — только строк (слова интерполированы)."""
        return self._kind

    def _set(self, status: str, result: dict | None = None) -> None:
        self._status = status
        self._lines = (result or {}).get("lines", [])
        self._source = (result or {}).get("source", "")
        self._kind = (result or {}).get("kind", "")
        self.changed.emit()

    def _load(self) -> None:
        track = self._player.track
        track_id = str(track.get("trackId", "")) if track else ""
        if track_id == self._track_id and self._status in ("ok", "loading", "notfound"):
            return
        self._track_id = track_id
        self._gen += 1
        gen = self._gen
        if not track_id:
            self._set("idle")
            return
        self._set("loading")
        client = self._auth.client
        loop = self._runner.loop
        title = str(track.get("title", ""))
        if track.get("version"):
            title += f" ({track['version']})"
        artist = str(track.get("artists", ""))
        album = str(track.get("album", ""))
        duration = int(track.get("durationMs") or 0) / 1000

        def yandex_lrc() -> str | None:
            if client is None or not loop.is_running():   # приложение закрывается
                return None
            return asyncio.run_coroutine_threadsafe(_yandex_lrc(client, track_id), loop).result(timeout=15)

        def done(result: dict) -> None:
            if gen != self._gen:
                return
            if result.get("ok"):
                self._set("ok", result)
            else:
                self._set("error" if result.get("details") else "notfound")
            log.info("текст %s: %s %s", track_id, result.get("source", "—"), result.get("kind", ""))

        def failed(error: BaseException) -> None:
            if gen == self._gen:
                log.warning("текст не загрузился: %s", error)
                self._set("error")

        self._runner.submit(asyncio.to_thread(_fetch_cached, track_id, title, artist, album, duration, yandex_lrc),
                            done, failed)


def _cache_path(track_id: str) -> Path:
    key = hashlib.sha1(f"{sources.CACHE_VERSION}|{track_id}".encode()).hexdigest()
    return Path(sources.CACHE_DIR) / (key + ".json")


def _fetch_cached(track_id, title, artist, album, duration, yandex_lrc) -> dict:
    path = _cache_path(track_id)
    try:
        cached = json.loads(path.read_text())
        if cached.get("ok") or time.time() - path.stat().st_mtime < sources.NOT_FOUND_TTL:
            return cached
    except (OSError, ValueError):
        pass
    result = sources.fetch(title, artist, album, duration, yandex_lrc=yandex_lrc)
    if result.get("ok"):
        result["lines"] = sources.add_gaps(result["lines"], duration)
    # Источник с ошибкой (503, таймаут) мог знать текст — такое «не найдено» не кэшируем
    if result.get("ok") or not result.get("details"):
        tmp = path.with_suffix(".part")
        tmp.write_text(json.dumps(result, ensure_ascii=False))
        tmp.replace(path)
    return result


async def _yandex_lrc(client, track_id: str) -> str | None:
    from yandex_music.exceptions import NotFoundError

    try:
        info = await client.tracks_lyrics(track_id, format_="LRC")
    except NotFoundError:
        return None
    if info is None or not info.download_url:
        return None
    async with aiohttp.ClientSession() as http, http.get(info.download_url) as response:
        response.raise_for_status()
        return await response.text()
