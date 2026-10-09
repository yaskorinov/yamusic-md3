"""«Моя волна»: сессия rotor, настройки (занятие, характер, настроение, язык), обратная связь. В QML — Wave.

Сессия создаётся по сидам: `user:onyourwave` (или сид занятия, например `activity:workout`) + сиды настроек
(`settingDiversity:discover`, …). Треки приходят партиями; плеер просит следующую, когда впереди остаётся мало.
Смена настроек во время игры пересоздаёт сессию и заменяет всё после текущего трека.
"""

from __future__ import annotations

import logging

from PySide6.QtCore import Property, QObject, Signal, Slot
from yandex_music import SessionEvent
from yandex_music._client_base import utc_now_iso

from .aio import AsyncRunner
from .auth import Auth
from .history import NO_REPORT
from .library import Library, full_id
from .models import track_to_dict
from .player import Player

log = logging.getLogger(__name__)

WAVE_SEED = "user:onyourwave"
WAVE_FROM = "desktop_win-radio-user-onyourwave-default"
RECENT = 60  # сколько последних треков передавать в queue, чтобы волна их не повторяла

# Если /rotor/wave/settings недоступен. Сиды — как в ответе API.
FALLBACK_GROUPS = [
    {"key": "context", "title": "Занятие", "items": [
        {"label": "Танцую", "seed": "genre:dance"},
        {"label": "Просыпаюсь", "seed": "activity:wake-up"},
        {"label": "В дороге", "seed": "activity:road-trip"},
        {"label": "Работаю", "seed": "activity:work-background"},
        {"label": "Тренируюсь", "seed": "activity:workout"},
        {"label": "Отдыхаю", "seed": "mood:relaxed"},
        {"label": "Засыпаю", "seed": "activity:fall-asleep"},
    ]},
    {"key": "diversity", "title": "Характер", "items": [
        {"label": "Любимое", "seed": "settingDiversity:favorite"},
        {"label": "Незнакомое", "seed": "settingDiversity:discover"},
        {"label": "Популярное", "seed": "settingDiversity:popular"},
    ]},
    {"key": "moodEnergy", "title": "Настроение", "items": [
        {"label": "Бодрое", "seed": "settingMoodEnergy:active"},
        {"label": "Весёлое", "seed": "settingMoodEnergy:fun"},
        {"label": "Спокойное", "seed": "settingMoodEnergy:calm"},
        {"label": "Грустное", "seed": "settingMoodEnergy:sad"},
    ]},
    {"key": "language", "title": "Язык", "items": [
        {"label": "Русский", "seed": "settingLanguage:russian"},
        {"label": "Иностранный", "seed": "settingLanguage:not-russian"},
        {"label": "Без слов", "seed": "settingLanguage:without-words"},
    ]},
]


def _groups_from_api(ws) -> list[dict]:
    groups: list[dict] = []
    for block in ws.blocks or []:
        if block.type != "contexts":
            continue
        items = []
        for item in block.items or []:
            station = getattr(item, "station", None) or item
            if station.id and station.name:
                items.append({"label": station.name, "seed": f"{station.id.type}:{station.id.tag}"})
        if items:
            groups.append({"key": "context", "title": "Занятие", "items": items})
    restrictions = ws.setting_restrictions
    for key, attr, title in (("diversity", "diversity", "Характер"), ("moodEnergy", "mood_energy", "Настроение"),
                             ("language", "language", "Язык")):
        enum = getattr(restrictions, attr, None) if restrictions else None
        values = getattr(enum, "possible_values", None) or []
        items = [{"label": v.name, "seed": v.serialized_seed} for v in values if v.serialized_seed and not v.unspecified]
        if items:
            groups.append({"key": key, "title": title, "items": items})
    keys = [g["key"] for g in groups]
    # Чего API не прислал — из запасного списка, в том же порядке
    return [next(g for g in groups if g["key"] == f["key"]) if f["key"] in keys else f for f in FALLBACK_GROUPS]


class Wave(QObject):
    groupsChanged = Signal()
    selectionChanged = Signal()
    stateChanged = Signal()
    errorChanged = Signal()

    def __init__(self, runner: AsyncRunner, auth: Auth, player: Player, library: Library,
                 parent: QObject | None = None):
        super().__init__(parent)
        self._runner = runner
        self._auth = auth
        self._player = player
        self._settings: QObject | None = None
        self._groups: list[dict] = FALLBACK_GROUPS
        self._selection: dict[str, str] = {}      # key группы → сид
        self._loading = False
        self._error = ""
        self._session_id = ""
        self._batch_id = ""
        self._recent: list[str] = []               # id:album полученных треков
        self._fetching = False
        self._gen = 0
        self._active = False
        self._feeder = self._more                  # один объект: плеер сравнивает по identity

        auth.signedIn.connect(self._load_settings)
        player.queueChanged.connect(self._update_active)
        player.trackStarted.connect(self._on_track_started)
        player.trackEnded.connect(self._on_track_ended)
        library.feedback.connect(self._on_like_feedback)

    def bind_settings(self, settings: QObject) -> None:
        self._settings = settings
        seeds = [s for s in str(settings.waveSeeds).split(",") if s]
        self._selection = {g["key"]: s for s in seeds for g in self._groups if any(i["seed"] == s for i in g["items"])}
        self.selectionChanged.emit()

    # --- свойства --------------------------------------------------------

    @Property("QVariantList", notify=groupsChanged)
    def groups(self) -> list[dict]:
        return self._groups

    @Property("QVariantMap", notify=selectionChanged)
    def selection(self) -> dict[str, str]:
        return self._selection

    @Property(bool, notify=stateChanged)
    def active(self) -> bool:
        """Плеер сейчас играет волну."""
        return self._active

    @Property(bool, notify=stateChanged)
    def loading(self) -> bool:
        return self._loading

    @Property(str, notify=errorChanged)
    def errorText(self) -> str:
        return self._error

    # --- управление ------------------------------------------------------

    @Slot()
    def play(self) -> None:
        """Большая кнопка: запустить волну или поставить/снять паузу, если она уже играет."""
        if self._active:
            self._player.togglePlay()
        else:
            self._start(replace=False)

    @Slot(str, str)
    def select(self, key: str, seed: str) -> None:
        """Выбрать вариант настройки; повторный выбор того же — снять («любое»)."""
        selection = dict(self._selection)
        if selection.get(key) == seed:
            selection.pop(key)
        else:
            selection[key] = seed
        self._selection = selection
        self.selectionChanged.emit()
        if self._settings is not None:
            self._settings.waveSeeds = ",".join(selection.values())
        if self._active:
            self._start(replace=True)

    # --- сессия ----------------------------------------------------------

    def _seeds(self) -> list[str]:
        context = self._selection.get("context")
        return [context or WAVE_SEED] + [s for k, s in self._selection.items() if k != "context"]

    def _start(self, replace: bool) -> None:
        client = self._auth.client
        if client is None:
            self._set_error("Войдите в аккаунт, чтобы слушать волну")
            return
        self._gen += 1
        gen = self._gen
        self._set_loading(True)
        self._set_error("")
        current = self._player.track if replace else {}
        queue = [full_id(current)] if current else None

        async def create():
            return await client.rotor_session_new(self._seeds(), queue=queue, include_tracks_in_response=True)

        def done(session) -> None:
            if gen != self._gen:
                return
            self._set_loading(False)
            if session is None or not session.sequence:
                self._set_error("Волна не прислала треков — попробуйте другие настройки")
                return
            self._session_id = session.radio_session_id
            self._batch_id = session.batch_id or ""
            self._fetching = False
            items = self._take(session.sequence)
            if replace and self._active:
                self._player.replace_upcoming(items)
            else:
                self._player.play_items(items, 0, {"type": "wave", "from": WAVE_FROM}, feeder=self._feeder)
            self._send(client.rotor_session_feedback_radio_started(self._session_id, self._batch_id, WAVE_FROM))

        def failed(error: BaseException) -> None:
            if gen == self._gen:
                self._set_loading(False)
                self._set_error(f"Не удалось запустить волну: {error}")

        self._runner.submit(create(), done, failed)

    def _more(self) -> None:
        """Плееру не хватает треков впереди — следующая партия."""
        client = self._auth.client
        if client is None or self._fetching or not self._session_id:
            return
        self._fetching = True
        gen = self._gen
        session_id = self._session_id

        def done(batch) -> None:
            if gen != self._gen:
                return
            self._fetching = False
            if batch is None or batch.unknown_session or not batch.sequence:
                # Сессия протухла (долгая пауза) — новая с теми же настройками, в конец очереди
                self._session_id = ""
                self._renew()
                return
            self._batch_id = batch.batch_id or self._batch_id
            if self._player.owns_feeder(self._feeder):
                self._player.append_tracks(self._take(batch.sequence))

        def failed(error: BaseException) -> None:
            if gen == self._gen:
                self._fetching = False
                log.warning("волна: следующая партия не загрузилась: %s", error)

        self._runner.submit(client.rotor_session_tracks(session_id, queue=self._recent[-RECENT:]), done, failed)

    def _renew(self) -> None:
        client = self._auth.client
        if client is None:
            return
        gen = self._gen

        def done(session) -> None:
            if gen != self._gen or session is None:
                return
            self._session_id = session.radio_session_id
            self._batch_id = session.batch_id or ""
            if self._player.owns_feeder(self._feeder):
                self._player.append_tracks(self._take(session.sequence or []))

        self._runner.submit(client.rotor_session_new(self._seeds(), queue=self._recent[-RECENT:],
                                                     include_tracks_in_response=True), done,
                            lambda e: log.warning("волна: новая сессия не создалась: %s", e))

    def _take(self, sequence) -> list[dict]:
        items = [track_to_dict(s.track) for s in sequence if s.track is not None]
        self._recent += [full_id(i) for i in items]
        del self._recent[:-RECENT]
        return items

    # --- обратная связь --------------------------------------------------

    def _on_track_started(self, event: dict) -> None:
        client = self._auth.client
        if client and self._session_id and event["context"].get("type") == "wave":
            self._send(client.rotor_session_feedback_track_started(
                self._session_id, full_id(event["track"]), self._batch_id))

    def _on_track_ended(self, event: dict) -> None:
        client = self._auth.client
        if not (client and self._session_id and event["context"].get("type") == "wave"):
            return
        track_id = full_id(event["track"])
        if event["natural"]:
            self._send(client.rotor_session_feedback_track_finished(
                self._session_id, track_id, event["played"], self._batch_id))
        else:
            self._send(client.rotor_session_feedback_skip(self._session_id, track_id, event["played"], self._batch_id))

    def _on_like_feedback(self, kind: str, track: dict) -> None:
        """Лайк/дизлайк во время волны — подсказка рекомендациям (коллекцию меняет Library)."""
        client = self._auth.client
        if not (client and self._session_id and self._active):
            return
        played = self._player.played_seconds if str(track.get("trackId")) == self._player.trackId else 0
        event = SessionEvent(kind, utc_now_iso(), track_id=full_id(track),
                             total_played_seconds=round(played, 1) if kind == "dislike" else None)
        self._send(client.rotor_session_feedback(self._session_id, event, self._batch_id))

    def _send(self, coro) -> None:
        if NO_REPORT:
            log.warning("волна: обратная связь не отправлена (YAMUSIC_NO_REPORT)")
            coro.close()
            return
        self._runner.submit(coro, None, lambda e: log.warning("волна: обратная связь не отправилась: %s", e))

    # --- служебное -------------------------------------------------------

    def _load_settings(self) -> None:
        client = self._auth.client
        if client is None:
            return

        def done(ws) -> None:
            if ws is not None:
                self._groups = _groups_from_api(ws)
                self.groupsChanged.emit()

        self._runner.submit(client.rotor_wave_settings(WAVE_SEED), done,
                            lambda e: log.warning("волна: настройки не загрузились: %s", e))

    def _update_active(self) -> None:
        active = self._player.source == "wave"
        if active != self._active:
            self._active = active
            if not active:
                self._gen += 1            # ответы старой сессии больше не нужны
                self._fetching = False
            self.stateChanged.emit()

    def _set_loading(self, value: bool) -> None:
        if value != self._loading:
            self._loading = value
            self.stateChanged.emit()

    def _set_error(self, text: str) -> None:
        if text != self._error:
            self._error = text
            self.errorChanged.emit()
