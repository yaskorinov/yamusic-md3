import QtQuick
import Md3
import YaMusic.Core

// Меню трека (ПКМ или «⋯»): волна, повтор, очередь, плейлисты, ссылки, переходы.
// Один экземпляр на окно: show(track, point); действия разбирает владелец через triggered(id, track, item).
Item {
    id: root

    property var track: ({})
    readonly property bool opened: menu.opened

    signal triggered(string id, var track, var item)

    readonly property bool _isCurrent: Player.hasTrack && (track.trackId ?? "") !== "" && Player.trackId === track.trackId
    readonly property var _refs: (track.artistRefs ?? []).filter(r => r && r.id)

    function show(t, point) {
        track = t ?? ({})
        if ((track.trackId ?? "") === "")
            return
        menu.open(_items(), point)
    }
    function close() { menu.close() }

    function _playlistItems() {
        const out = [{ id: "playlist:new", text: "Новый плейлист…", icon: "add" }]
        const model = Library.playlists
        const uid = Auth.account ? Auth.account.uid : undefined
        const own = []
        for (let i = 0; model && i < model.count; i++) {
            const p = model.get(i)
            if (uid !== undefined && uid !== null && String(p.uid) !== String(uid))
                continue
            own.push({ id: "playlist:" + p.kind, text: p.title, icon: "queue_music" })
        }
        if (own.length > 0)
            out.push({ separator: true })
        return out.concat(own)
    }

    function _shareItems() {
        const out = [{ id: "share:track", text: "Скопировать ссылку на трек", icon: "content_copy" }]
        if (_refs.length === 1)
            out.push({ id: "share:artist:" + _refs[0].id, text: "Скопировать ссылку на исполнителя", icon: "person" })
        else
            for (const r of _refs)
                out.push({ id: "share:artist:" + r.id, text: "Ссылка на " + r.name, icon: "person" })
        return out
    }

    function _items() {
        const id = track.trackId
        const hasAlbum = (track.albumId ?? "") !== ""
        const pinned = Library.isWavePinned("track:" + id)
        const items = [
            { id: "pin", text: pinned ? "Открепить Мою волну" : "Закрепить Мою волну", icon: "push_pin" },
            { id: "wave", text: "Моя волна по треку", icon: "radio" },
            { separator: true }
        ]
        if (_isCurrent)
            items.push({ id: "repeat", text: "Повторять", icon: Player.repeat === "one" ? "repeat_one" : "repeat",
                         checked: Player.repeat !== "off" })
        items.push(
            { id: "queue", text: "Открыть очередь", icon: "queue_music" },
            { id: "sound", text: "Настройки звука", icon: "tune" },
            { separator: true },
            { id: "playlist", text: "Добавить в плейлист", icon: "playlist_add", submenu: _playlistItems() },
            { id: "share", text: "Поделиться", icon: "share", submenu: _shareItems() },
            { id: "goto:track", text: "Перейти к треку", icon: "music_note", enabled: hasAlbum },
            { id: "goto:album", text: "Перейти к альбому", icon: "album", enabled: hasAlbum },
            { id: "goto:artist", text: "Перейти к исполнителю", icon: "person", enabled: _refs.length > 0 }
        )
        return items
    }

    anchors.fill: parent

    Menu {
        id: menu
        anchors.fill: parent
        onTriggered: (id, item) => root.triggered(id, root.track, item)
    }
}
