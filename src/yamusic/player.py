"""Плеер: очередь, mpv (libmpv), прямые ссылки Яндекс Музыки. Доступен в QML как Player.

Без пауз между треками: в плейлисте mpv всегда «текущий + следующий». Ссылка на следующий трек
запрашивается заранее, mpv переходит на него сам (gapless), а мы по смене playlist-pos двигаем очередь.
Колбэки mpv приходят из его потока — всё, что трогает Qt, уходит в GUI-поток через runner.call_in_gui.

У очереди есть контекст (откуда она: «Мне нравится», плейлист, волна) — для учёта прослушиваний,
и у волны — «подкормка» (feeder): когда впереди остаётся мало треков, плеер просит догрузить ещё.
"""

from __future__ import annotations

import asyncio
import hashlib
import locale
import os
import random
import time
import traceback
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import aiohttp
import mpv
from PySide6.QtCore import Property, QObject, QTimer, Signal, Slot

from .aio import AsyncRunner
from .auth import Auth
from .images import cache_dir
from .models import TrackListModel

QUALITY = {"lossless": "lossless", "high": "hq", "low": "nq"}
CODEC_LABEL = {"flac": "FLAC", "flac-mp4": "FLAC", "aac-mp4": "AAC", "aac": "AAC",
               "he-aac-mp4": "HE-AAC", "he-aac": "HE-AAC", "mp3": "MP3"}
POSITION_EMIT_INTERVAL = 0.2  # с: QML сглаживает прогресс между обновлениями
PREFETCH_DELAY_MS = 2500
FEED_AHEAD = 3   # волна: сколько треков держать впереди текущего


def mpv_volume(ui: float) -> float:
    """Ползунок 0..1 → громкость mpv. Программная громкость mpv кубическая (амплитуда = (vol/100)³),
    и линейный перенос глушил звук уже ниже ~13 % ползунка. Берём амплитуду = ui²: тихо, но слышно."""
    return 100 * max(0.0, min(1.0, ui)) ** (2 / 3)


class Player(QObject):
    trackChanged = Signal()
    stateChanged = Signal()
    positionChanged = Signal()
    volumeChanged = Signal()
    modeChanged = Signal()
    queueChanged = Signal()
    coverFileChanged = Signal()
    errorChanged = Signal()
    # Для учёта прослушиваний и обратной связи волны (только Python):
    # {"track", "context"} и {"track", "context", "played", "end", "natural"} (секунды)
    trackStarted = Signal(object)
    trackEnded = Signal(object)

    def __init__(self, runner: AsyncRunner, auth: Auth, parent: QObject | None = None):
        super().__init__(parent)
        self._runner = runner
        self._auth = auth
        self._settings: QObject | None = None

        self._queue = TrackListModel(self)
        self._order: list[int] = []        # порядок воспроизведения (индексы очереди), с учётом shuffle
        self._pos = -1                      # позиция в _order
        self._track: dict[str, object] = {}
        self._playing = False
        self._buffering = False
        self._position_ms = 0
        self._duration_ms = 0
        self._last_emit = 0.0
        self._shuffle = False
        self._repeat = "off"                # off | all | one
        self._codec = ""
        self._cover_file = ""
        self._error = ""
        self._load_token = 0                # отменяет устаревшие ответы resolve
        self._prefetched_for = -1           # индекс очереди, чья ссылка уже стоит в mpv вторым номером
        self._http: aiohttp.ClientSession | None = None
        self._context: dict[str, str] = {}  # {"type": wave|liked|playlist|…, "from": …, "playlistId": …}
        self._feeder = None                 # callable() — догрузить треки (волна)
        self._played = 0.0                  # сколько секунд текущего трека реально проиграно
        self._last_time: float | None = None
        self._track_live = False            # trackStarted отправлен, trackEnded — ещё нет
        # Состояние mpv — из наблюдателей; синхронные запросы к libmpv из GUI-потока блокируются,
        # пока mpv открывает сетевой поток, и подвешивают анимации (видно на смене темы).
        self._paused = False
        self._idle_active = True
        self._volume = 0.7
        self._cmd_pool = ThreadPoolExecutor(max_workers=1, thread_name_prefix="mpv-cmd")

        # libmpv отказывается работать с не-C LC_NUMERIC, а QGuiApplication выставил локаль из системы
        locale.setlocale(locale.LC_NUMERIC, "C")
        self._mpv = mpv.MPV(
            vid="no", ytdl=False, audio_display="no", input_default_bindings=False, input_vo_keyboard=False,
            gapless_audio="weak", prefetch_playlist="yes", cache="yes", demuxer_max_bytes="64MiB",
            keep_open="no", idle="yes", audio_client_name="yamusic",
        )
        if os.environ.get("YAMUSIC_AO"):  # тесты: YAMUSIC_AO=null — не играть в колонки
            self._mpv.ao = os.environ["YAMUSIC_AO"]
        self._mpv.volume = mpv_volume(self._volume)
        self._mpv.observe_property("pause", lambda _n, v: self._gui(self._on_pause, v))
        self._mpv.observe_property("idle-active", lambda _n, v: self._gui(self._on_idle, v))
        self._mpv.observe_property("paused-for-cache", lambda _n, v: self._gui(self._on_buffering, v))
        self._mpv.observe_property("time-pos", lambda _n, v: self._gui(self._on_time, v))
        self._mpv.observe_property("duration", lambda _n, v: self._gui(self._on_duration, v))
        self._mpv.observe_property("playlist-pos", lambda _n, v: self._gui(self._on_playlist_pos, v))
        self._mpv.event_callback("end-file")(lambda e: self._gui(self._on_end_file, e))

    def bind_settings(self, settings: QObject) -> None:
        """Настройки (громкость, качество) — синглтон QML, он появляется после создания движка."""
        self._settings = settings
        self._volume = max(0.0, min(1.0, float(settings.volume)))
        self._cmd(lambda v=mpv_volume(self._volume): setattr(self._mpv, "volume", v))
        self.volumeChanged.emit()

    def _gui(self, fn, *args) -> None:
        self._runner.call_in_gui(lambda: fn(*args))

    def _cmd(self, fn) -> None:
        """Команда libmpv в отдельном потоке (по порядку). GUI-поток никогда не ждёт mpv."""
        def run() -> None:
            try:
                fn()
            except Exception:
                traceback.print_exc()
        self._cmd_pool.submit(run)

    def shutdown(self) -> None:
        self._cmd_pool.shutdown(wait=True, cancel_futures=True)
        try:
            self._mpv.terminate()
        except Exception:
            pass
        if self._http is not None and not self._http.closed:
            try:
                asyncio.run_coroutine_threadsafe(self._http.close(), self._runner.loop).result(timeout=1)
            except Exception:
                pass

    # --- свойства --------------------------------------------------------

    @Property(bool, notify=trackChanged)
    def hasTrack(self) -> bool:
        return bool(self._track)

    @Property(str, notify=trackChanged)
    def trackId(self) -> str:
        return str(self._track.get("trackId", ""))

    @Property(str, notify=trackChanged)
    def title(self) -> str:
        return str(self._track.get("title", ""))

    @Property(str, notify=trackChanged)
    def artist(self) -> str:
        return str(self._track.get("artists", ""))

    @Property(str, notify=trackChanged)
    def album(self) -> str:
        return str(self._track.get("album", ""))

    @Property(str, notify=trackChanged)
    def cover(self) -> str:
        return str(self._track.get("cover", ""))

    @Property("QVariantMap", notify=trackChanged)
    def track(self) -> dict[str, object]:
        return self._track

    @Property(str, notify=coverFileChanged)
    def coverFile(self) -> str:
        """Локальная копия обложки (для темы и MPRIS)."""
        return self._cover_file

    @Property(str, notify=trackChanged)
    def codec(self) -> str:
        return self._codec

    @Property(bool, notify=stateChanged)
    def playing(self) -> bool:
        return self._playing

    @Property(bool, notify=stateChanged)
    def buffering(self) -> bool:
        return self._buffering

    @Property(int, notify=positionChanged)
    def positionMs(self) -> int:
        return self._position_ms

    @Property(int, notify=positionChanged)
    def durationMs(self) -> int:
        return self._duration_ms

    @Property(float, notify=positionChanged)
    def position(self) -> float:
        return self._position_ms / self._duration_ms if self._duration_ms > 0 else 0.0

    def _get_volume(self) -> float:
        return self._volume

    def _set_volume(self, value: float) -> None:
        value = max(0.0, min(1.0, float(value)))
        self._volume = value
        self._cmd(lambda v=mpv_volume(value): setattr(self._mpv, "volume", v))
        if self._settings is not None:
            self._settings.volume = value
        self.volumeChanged.emit()

    volume = Property(float, _get_volume, _set_volume, notify=volumeChanged)

    def _get_shuffle(self) -> bool:
        return self._shuffle

    def _set_shuffle(self, value: bool) -> None:
        if value == self._shuffle or self.source == "wave":
            return
        self._shuffle = value
        current = self._order[self._pos] if 0 <= self._pos < len(self._order) else -1
        self._order = self._make_order(current)
        self._pos = self._order.index(current) if current >= 0 else -1
        self.modeChanged.emit()
        self._prefetch_next(force=True)

    shuffle = Property(bool, _get_shuffle, _set_shuffle, notify=modeChanged)

    def _get_repeat(self) -> str:
        return self._repeat

    def _set_repeat(self, value: str) -> None:
        if value not in ("off", "all", "one") or value == self._repeat:
            return
        self._repeat = value
        self._cmd(lambda: setattr(self._mpv, "loop_file", "inf" if value == "one" else "no"))
        self.modeChanged.emit()
        self._prefetch_next(force=True)

    repeat = Property(str, _get_repeat, _set_repeat, notify=modeChanged)

    @Property(QObject, constant=True)
    def queue(self) -> TrackListModel:
        return self._queue

    @Property(int, notify=queueChanged)
    def currentIndex(self) -> int:
        return self._order[self._pos] if 0 <= self._pos < len(self._order) else -1

    @Property(str, notify=errorChanged)
    def errorText(self) -> str:
        return self._error

    @Property(str, notify=queueChanged)
    def source(self) -> str:
        """Тип очереди: wave | liked | playlist | '' — волна, например, без перемешивания."""
        return self._context.get("type", "")

    # --- управление ------------------------------------------------------

    @Slot(QObject, int)
    def playFrom(self, model: QObject, row: int) -> None:
        """Поставить в очередь всю модель (TrackListModel) и начать с row."""
        items = [dict(i) for i in model.items()] if hasattr(model, "items") else []
        self.play_items(items, row, dict(getattr(model, "context", {}) or {}))

    def play_items(self, items: list[dict], row: int, context: dict[str, str], feeder=None) -> None:
        if not items or not 0 <= row < len(items):
            return
        self._context = context
        self._feeder = feeder
        self._queue.reset(items)
        self._order = self._make_order(row)
        self._pos = self._order.index(row)
        self.queueChanged.emit()
        self._start_current()

    def append_tracks(self, items: list[dict]) -> None:
        """Догрузка в конец очереди (волна). Если очередь уже встала на последнем треке — играем дальше."""
        if not items:
            return
        stalled = bool(self._track) and self._idle_active and self._pos == len(self._order) - 1
        first = self._queue.count
        self._queue.append(items)
        self._order.extend(range(first, first + len(items)))
        self.queueChanged.emit()
        if stalled:
            self.next()
        else:
            self._prefetch_next()

    def replace_upcoming(self, items: list[dict]) -> None:
        """Заменить всё после текущего трека (волна с новыми настройками). Без перемешивания: _order — 0..n-1."""
        current = self.currentIndex
        if current < 0:
            return
        self._queue.remove(current + 1, self._queue.count - current - 1)
        self._order = list(range(self._queue.count))
        self._prefetched_for = -1
        self._drop_queued()
        self.append_tracks(items)

    def owns_feeder(self, feeder) -> bool:
        return self._feeder is feeder

    @property
    def played_seconds(self) -> float:
        return self._played

    @Slot(QObject)
    def shuffleFrom(self, model: QObject) -> None:
        """«Перемешать»: включить shuffle и начать со случайного трека модели."""
        count = len(model.items()) if hasattr(model, "items") else 0
        if count == 0:
            return
        if not self._shuffle:
            self._shuffle = True
            self.modeChanged.emit()
        self.playFrom(model, random.randrange(count))

    @Slot(int)
    def playIndex(self, index: int) -> None:
        if 0 <= index < self._queue.count and index in self._order:
            self._pos = self._order.index(index)
            self.queueChanged.emit()
            self._start_current()

    @Slot()
    def togglePlay(self) -> None:
        if not self._track:
            return
        if self._idle_active:              # трек закончился и очередь встала — играть заново
            self._start_current()
        else:
            self._cmd(lambda: self._mpv.cycle("pause"))

    @Slot()
    def play(self) -> None:
        if self._track:
            self._cmd(lambda: setattr(self._mpv, "pause", False))

    @Slot()
    def pause(self) -> None:
        self._cmd(lambda: setattr(self._mpv, "pause", True))

    @Slot()
    def stop(self) -> None:
        self._cmd(lambda: self._mpv.command("stop"))

    @Slot()
    def next(self) -> None:
        nxt = self._next_pos(manual=True)
        if nxt is not None:
            self._pos = nxt
            self.queueChanged.emit()
            self._start_current()

    @Slot()
    def previous(self) -> None:
        # Как в большинстве плееров: после 3 с — в начало трека, иначе — предыдущий
        if self._position_ms > 3000 or self._pos <= 0 and self._repeat != "all":
            self.seekMs(0)
            return
        self._pos = (self._pos - 1) % len(self._order)
        self.queueChanged.emit()
        self._start_current()

    @Slot(float)
    def seek(self, fraction: float) -> None:
        if self._duration_ms > 0:
            self.seekMs(int(fraction * self._duration_ms))

    @Slot(int)
    def seekMs(self, ms: int) -> None:
        if self._track:
            self._cmd(lambda: self._mpv.seek(max(0, ms) / 1000, reference="absolute"))
            self._position_ms = max(0, ms)
            self.positionChanged.emit()

    @Slot()
    def cycleRepeat(self) -> None:
        # У волны нет «конца» — повтор всей очереди не имеет смысла
        if self.source == "wave":
            self.repeat = "off" if self._repeat != "off" else "one"
        else:
            self.repeat = {"off": "all", "all": "one", "one": "off"}[self._repeat]

    # --- внутреннее: очередь ----------------------------------------------

    def _make_order(self, first: int) -> list[int]:
        n = self._queue.count
        order = list(range(n))
        if self._shuffle and n > 1 and self.source != "wave":
            rest = [i for i in order if i != first]
            random.shuffle(rest)
            order = ([first] if first >= 0 else []) + rest
        return order

    def _next_pos(self, manual: bool) -> int | None:
        if not self._order:
            return None
        if self._repeat == "one" and not manual:
            return self._pos
        if self._pos + 1 < len(self._order):
            return self._pos + 1
        if self.source == "wave":
            return None                    # ждём догрузки
        return 0 if self._repeat == "all" or manual else None

    def _start_current(self) -> None:
        index = self.currentIndex
        if index < 0:
            return
        track = self._queue.get(index)
        self._set_track(track)
        self._load_token += 1
        token = self._load_token
        self._prefetched_for = -1

        def ready(result: tuple[str, str]) -> None:
            if token != self._load_token:
                return
            url, codec = result
            self._codec = codec
            self.trackChanged.emit()

            def load() -> None:
                self._mpv.loadfile(url, "replace")
                self._mpv.pause = False
            self._cmd(load)
            # Следующий трек — через пару секунд: запрос и разбор ответа (чистый Python, держит GIL)
            # пришлись бы на анимацию смены темы и подвесили бы кадры.
            QTimer.singleShot(PREFETCH_DELAY_MS, lambda t=token: t == self._load_token and self._prefetch_next())

        self._runner.submit(self._resolve(str(track["trackId"])), ready, self._failed)
        self._runner.submit(self._fetch_cover(str(track.get("cover", ""))), self._set_cover_file)

    def _prefetch_next(self, force: bool = False) -> None:
        """Поставить в mpv вторым номером следующий трек — для перехода без паузы."""
        nxt = self._next_pos(manual=False)
        if nxt is None or self._repeat == "one" or not self._track:
            self._drop_queued()
            return
        index = self._order[nxt]
        if index == self._prefetched_for and not force:
            return
        token = self._load_token
        track = self._queue.get(index)

        def ready(result: tuple[str, str]) -> None:
            if token != self._load_token:
                return
            self._prefetched_for = index

            def append() -> None:
                self._drop_queued_sync()
                self._mpv.playlist_append(result[0])
            self._cmd(append)

        self._runner.submit(self._resolve(str(track["trackId"])), ready, lambda e: None)

    def _drop_queued(self) -> None:
        self._prefetched_for = -1
        self._cmd(self._drop_queued_sync)

    def _drop_queued_sync(self) -> None:
        """Убрать из плейлиста mpv всё после текущего (поток mpv-cmd)."""
        try:
            while int(self._mpv.playlist_count or 0) > 1 + int(self._mpv.playlist_pos or 0):
                self._mpv.playlist_remove(int(self._mpv.playlist_count) - 1)
        except (SystemError, TypeError):
            pass

    async def _resolve(self, track_id: str) -> tuple[str, str]:
        client = self._auth.client
        if client is None:
            raise RuntimeError("Войдите в аккаунт, чтобы слушать музыку")
        quality = QUALITY.get(str(getattr(self._settings, "quality", "lossless")), "lossless")
        try:
            info = (await client.tracks_file_info(track_id, quality=quality, transport="raw")).download_info
        except Exception:
            if quality == "hq":
                raise
            # Lossless/низкое качество недоступно или API изменился — откат на AAC/MP3
            info = (await client.tracks_file_info(track_id, quality="hq", transport="raw")).download_info
        label = CODEC_LABEL.get(info.codec, info.codec.upper())
        if info.bitrate:
            label += f" {info.bitrate}"
        return (info.urls[0] if info.urls else info.url), label

    async def _fetch_cover(self, url: str) -> str:
        if not url:
            return ""
        folder = Path(cache_dir("covers"))
        path = folder / (hashlib.sha1(url.encode()).hexdigest() + ".jpg")
        if path.exists():
            return str(path)
        folder.mkdir(parents=True, exist_ok=True)
        if self._http is None or self._http.closed:
            self._http = aiohttp.ClientSession()
        async with self._http.get(url) as response:
            response.raise_for_status()
            data = await response.read()
        tmp = path.with_suffix(".part")
        await asyncio.to_thread(tmp.write_bytes, data)
        tmp.replace(path)
        return str(path)

    # --- внутреннее: события mpv (уже в GUI-потоке) -------------------------

    def _set_track(self, track: dict[str, object]) -> None:
        self._end_current()
        self._track = dict(track)
        self._duration_ms = int(track.get("durationMs") or 0)
        self._position_ms = 0
        self._played = 0.0
        self._last_time = None
        self._set_error("")
        self.trackChanged.emit()
        self.positionChanged.emit()
        self._track_live = True
        self.trackStarted.emit({"track": self._track, "context": self._context})
        self._feed()

    def _end_current(self) -> None:
        """Сообщить о конце текущего трека (смена трека или очередь кончилась)."""
        if not self._track_live:
            return
        self._track_live = False
        end = self._position_ms / 1000
        duration = self._duration_ms / 1000
        self.trackEnded.emit({
            "track": self._track, "context": self._context, "played": round(self._played, 1),
            "end": round(end, 1), "natural": duration > 0 and end >= duration - 3,
        })

    def _feed(self) -> None:
        if self._feeder is not None and len(self._order) - 1 - self._pos < FEED_AHEAD:
            self._feeder()

    def _set_cover_file(self, path: str) -> None:
        if path != self._cover_file:
            self._cover_file = path
            self.coverFileChanged.emit()

    def _on_pause(self, paused) -> None:
        self._paused = bool(paused)
        self._update_playing()

    def _on_idle(self, idle) -> None:
        self._idle_active = bool(idle)
        if self._idle_active and self._track_live and self._played > 0:
            self._end_current()            # очередь доиграла до конца
        self._update_playing()

    def _on_buffering(self, value) -> None:
        if bool(value) != self._buffering:
            self._buffering = bool(value)
            self.stateChanged.emit()

    def _update_playing(self) -> None:
        playing = bool(self._track) and not self._paused and not self._idle_active
        if playing != self._playing:
            self._playing = playing
            self.stateChanged.emit()

    def _on_time(self, seconds) -> None:
        if seconds is None:
            return
        last, self._last_time = self._last_time, seconds
        if last is not None and 0 < seconds - last < 1.0:   # перемотка не считается прослушиванием
            self._played += seconds - last
        now = time.monotonic()
        if now - self._last_emit < POSITION_EMIT_INTERVAL:
            return
        self._last_emit = now
        self._position_ms = int(seconds * 1000)
        self.positionChanged.emit()

    def _on_duration(self, seconds) -> None:
        if seconds:
            self._duration_ms = int(seconds * 1000)
            self.positionChanged.emit()

    def _on_playlist_pos(self, pos) -> None:
        # mpv сам перешёл на заранее поставленный трек (gapless) → догоняем очередь
        if pos is None or pos <= 0 or self._prefetched_for < 0:
            return
        nxt = self._next_pos(manual=False)
        if nxt is None:
            return
        self._pos = nxt
        self._prefetched_for = -1
        self._cmd(lambda: self._mpv.playlist_remove(0))
        self.queueChanged.emit()
        self._set_track(self._queue.get(self.currentIndex))
        self._runner.submit(self._fetch_cover(self.cover), self._set_cover_file)
        self._prefetch_next()

    def _on_end_file(self, event) -> None:
        self._update_playing()
        reason = getattr(getattr(event, "data", None), "reason", None)
        if reason is not None and int(reason) == 4:   # MPV_END_FILE_REASON_ERROR
            self._set_error("Не удалось воспроизвести трек — пропускаю")
            if self._next_pos(manual=False) is not None:
                self.next()

    def _failed(self, error: BaseException) -> None:
        self._set_error(f"Не удалось получить трек: {error}")
        self._update_playing()

    def _set_error(self, text: str) -> None:
        if text != self._error:
            self._error = text
            self.errorChanged.emit()
