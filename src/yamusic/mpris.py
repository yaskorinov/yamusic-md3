"""MPRIS2 (org.mpris.MediaPlayer2.yamusic): медиаклавиши, playerctl, waybar, виджеты DE.

D-Bus живёт в asyncio-потоке (dbus-fast). Состояние плеера копируется сюда из GUI-потока
(snapshot), команды D-Bus отправляются в GUI-поток через runner.call_in_gui.
"""

# Без `from __future__ import annotations`: dbus-fast читает D-Bus-сигнатуры из аннотаций ("s", "a{sv}"…).
from dbus_fast import BusType, PropertyAccess, Variant
from dbus_fast.aio import MessageBus
from dbus_fast.service import ServiceInterface, dbus_property, method, signal

from .aio import AsyncRunner
from .player import Player

BUS_NAME = "org.mpris.MediaPlayer2.yamusic"
OBJECT_PATH = "/org/mpris/MediaPlayer2"
NO_TRACK = "/org/mpris/MediaPlayer2/TrackList/NoTrack"


class _Root(ServiceInterface):
    def __init__(self, bridge: "Mpris"):
        super().__init__("org.mpris.MediaPlayer2")
        self._bridge = bridge

    @method(name="Raise")
    def raise_(self) -> None:
        self._bridge.gui(lambda: self._bridge.raise_window())

    @method(name="Quit")
    def quit(self) -> None:
        self._bridge.gui(lambda: self._bridge.quit_app())

    @dbus_property(access=PropertyAccess.READ)
    def CanQuit(self) -> "b":
        return True

    @dbus_property(access=PropertyAccess.READ)
    def CanRaise(self) -> "b":
        return True

    @dbus_property(access=PropertyAccess.READ)
    def HasTrackList(self) -> "b":
        return False

    @dbus_property(access=PropertyAccess.READ)
    def Identity(self) -> "s":
        return "YaMusic"

    @dbus_property(access=PropertyAccess.READ)
    def DesktopEntry(self) -> "s":
        return "yamusic"

    @dbus_property(access=PropertyAccess.READ)
    def SupportedUriSchemes(self) -> "as":
        return []

    @dbus_property(access=PropertyAccess.READ)
    def SupportedMimeTypes(self) -> "as":
        return []


class _PlayerIface(ServiceInterface):
    def __init__(self, bridge: "Mpris"):
        super().__init__("org.mpris.MediaPlayer2.Player")
        self._b = bridge

    # --- методы -----------------------------------------------------------
    @method()
    def Next(self) -> None:
        self._b.gui(self._b.player.next)

    @method()
    def Previous(self) -> None:
        self._b.gui(self._b.player.previous)

    @method()
    def Pause(self) -> None:
        self._b.gui(self._b.player.pause)

    @method()
    def PlayPause(self) -> None:
        self._b.gui(self._b.player.togglePlay)

    @method()
    def Stop(self) -> None:
        self._b.gui(self._b.player.pause)

    @method()
    def Play(self) -> None:
        self._b.gui(self._b.player.play)

    @method()
    def Seek(self, offset: "x") -> None:
        target = self._b.state["position_us"] + offset
        self._b.gui(lambda: self._b.player.seekMs(int(target / 1000)))

    @method()
    def SetPosition(self, track_id: "o", position: "x") -> None:
        if track_id == self._b.track_path():
            self._b.gui(lambda: self._b.player.seekMs(int(position / 1000)))

    @method()
    def OpenUri(self, uri: "s") -> None:
        pass

    @signal()
    def Seeked(self) -> "x":
        return self._b.state["position_us"]

    # --- свойства ---------------------------------------------------------
    @dbus_property(access=PropertyAccess.READ)
    def PlaybackStatus(self) -> "s":
        s = self._b.state
        return "Playing" if s["playing"] else ("Paused" if s["has_track"] else "Stopped")

    @dbus_property()
    def LoopStatus(self) -> "s":
        return {"off": "None", "all": "Playlist", "one": "Track"}[self._b.state["repeat"]]

    @LoopStatus.setter
    def LoopStatus(self, value: "s") -> None:
        mode = {"None": "off", "Playlist": "all", "Track": "one"}.get(value, "off")
        self._b.gui(lambda: setattr(self._b.player, "repeat", mode))

    @dbus_property()
    def Shuffle(self) -> "b":
        return self._b.state["shuffle"]

    @Shuffle.setter
    def Shuffle(self, value: "b") -> None:
        self._b.gui(lambda: setattr(self._b.player, "shuffle", bool(value)))

    @dbus_property()
    def Volume(self) -> "d":
        return self._b.state["volume"]

    @Volume.setter
    def Volume(self, value: "d") -> None:
        self._b.gui(lambda: setattr(self._b.player, "volume", max(0.0, min(1.0, value))))

    @dbus_property()
    def Rate(self) -> "d":
        return 1.0

    @Rate.setter
    def Rate(self, value: "d") -> None:
        pass

    @dbus_property(access=PropertyAccess.READ)
    def Metadata(self) -> "a{sv}":
        s = self._b.state
        if not s["has_track"]:
            return {"mpris:trackid": Variant("o", NO_TRACK)}
        meta = {
            "mpris:trackid": Variant("o", self._b.track_path()),
            "mpris:length": Variant("x", s["duration_us"]),
            "xesam:title": Variant("s", s["title"]),
            "xesam:artist": Variant("as", [a.strip() for a in s["artist"].split(",") if a.strip()]),
            "xesam:album": Variant("s", s["album"]),
            "xesam:url": Variant("s", f"https://music.yandex.ru/track/{s['track_id']}"),
        }
        if s["art"]:
            meta["mpris:artUrl"] = Variant("s", s["art"])
        return meta

    @dbus_property(access=PropertyAccess.READ)
    def Position(self) -> "x":
        return self._b.state["position_us"]

    @dbus_property(access=PropertyAccess.READ)
    def MinimumRate(self) -> "d":
        return 1.0

    @dbus_property(access=PropertyAccess.READ)
    def MaximumRate(self) -> "d":
        return 1.0

    @dbus_property(access=PropertyAccess.READ)
    def CanGoNext(self) -> "b":
        return self._b.state["has_track"]

    @dbus_property(access=PropertyAccess.READ)
    def CanGoPrevious(self) -> "b":
        return self._b.state["has_track"]

    @dbus_property(access=PropertyAccess.READ)
    def CanPlay(self) -> "b":
        return self._b.state["has_track"]

    @dbus_property(access=PropertyAccess.READ)
    def CanPause(self) -> "b":
        return self._b.state["has_track"]

    @dbus_property(access=PropertyAccess.READ)
    def CanSeek(self) -> "b":
        return self._b.state["has_track"]

    @dbus_property(access=PropertyAccess.READ)
    def CanControl(self) -> "b":
        return True


class Mpris:
    def __init__(self, runner: AsyncRunner, player: Player, raise_window, quit_app):
        self._runner = runner
        self.player = player
        self.raise_window = raise_window
        self.quit_app = quit_app
        self.state = self._snapshot()
        self._root = _Root(self)
        self._iface = _PlayerIface(self)
        self._bus: MessageBus | None = None

        player.trackChanged.connect(self._sync)
        player.stateChanged.connect(self._sync)
        player.modeChanged.connect(self._sync)
        player.volumeChanged.connect(self._sync)
        player.coverFileChanged.connect(self._sync)
        player.positionChanged.connect(self._sync_position)

        runner.submit(self._connect(), on_error=lambda e: print(f"yamusic: MPRIS недоступен: {e}"))

    def gui(self, fn) -> None:
        self._runner.call_in_gui(fn)

    def track_path(self) -> str:
        tid = "".join(c if c.isalnum() else "_" for c in str(self.state["track_id"]))
        return f"/org/yamusic/track/{tid}" if tid else NO_TRACK

    async def _connect(self) -> None:
        self._bus = await MessageBus(bus_type=BusType.SESSION).connect()
        self._bus.export(OBJECT_PATH, self._root)
        self._bus.export(OBJECT_PATH, self._iface)
        await self._bus.request_name(BUS_NAME)

    def _snapshot(self) -> dict[str, object]:
        p = self.player
        return {
            "has_track": p.hasTrack, "playing": p.playing, "track_id": p.trackId,
            "title": p.title, "artist": p.artist, "album": p.album,
            "art": f"file://{p.coverFile}" if p.coverFile else p.cover,
            "duration_us": p.durationMs * 1000, "position_us": p.positionMs * 1000,
            "shuffle": p.shuffle, "repeat": p.repeat, "volume": p.volume,
        }

    def _sync(self, *_args) -> None:
        old, new = self.state, self._snapshot()
        self.state = new
        changed: dict[str, object] = {}
        if (old["playing"], old["has_track"]) != (new["playing"], new["has_track"]):
            changed["PlaybackStatus"] = None
        if any(old[k] != new[k] for k in ("track_id", "title", "artist", "album", "art", "duration_us")):
            changed["Metadata"] = None
            changed.update(CanGoNext=None, CanGoPrevious=None, CanPlay=None, CanPause=None, CanSeek=None)
        if old["shuffle"] != new["shuffle"]:
            changed["Shuffle"] = None
        if old["repeat"] != new["repeat"]:
            changed["LoopStatus"] = None
        if abs(old["volume"] - new["volume"]) > 1e-3:
            changed["Volume"] = None
        if changed and self._bus is not None:
            iface = self._iface
            values = {name: getattr(iface, name) for name in changed}
            self._runner.loop.call_soon_threadsafe(iface.emit_properties_changed, values)

    def _sync_position(self) -> None:
        old = self.state["position_us"]
        self.state["position_us"] = self.player.positionMs * 1000
        self.state["duration_us"] = self.player.durationMs * 1000
        # Seeked — только при скачке (перемотка), а не при обычном ходе воспроизведения
        if abs(self.state["position_us"] - old) > 2_000_000 and self._bus is not None:
            self._runner.loop.call_soon_threadsafe(self._iface.Seeked)

    async def close(self) -> None:
        if self._bus is not None:
            self._bus.disconnect()
