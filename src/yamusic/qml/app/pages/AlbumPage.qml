import QtQuick
import Md3
import YaMusic.Core
import ".."

TrackListPage {
    id: page

    property string albumId
    readonly property QtObject album: Catalog.album(albumId)
    readonly property var info: album ? album.info : ({})

    title: info.title ? info.title + (info.version ? " (" + info.version + ")" : "") : ""
    overline: (info.kind ?? "Альбом").toUpperCase()
    heroImage: info.cover ?? ""
    heroIcon: "album"
    heroArtistRefs: info.artistRefs ?? []
    subtitle: {
        if (!info.title)
            return ""
        const parts = []
        if (info.year)
            parts.push(info.year)
        const n = album.tracks.count
        parts.push(n + " " + plural(n))
        return parts.join("  ·  ")
    }
    model: album ? album.tracks : null
    emptyIcon: "album"
    emptyTitle: album && album.errorText !== "" ? "Альбом не загрузился" : "В альбоме нет треков"
    emptyText: album ? album.errorText : ""

    function plural(n) {
        const m10 = n % 10, m100 = n % 100
        if (m10 === 1 && m100 !== 11) return "трек"
        if (m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14)) return "трека"
        return "треков"
    }
}
