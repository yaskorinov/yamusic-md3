import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core
import ".."

TrackListPage {
    id: page

    property int uid: 0
    property int kind: 0
    property string playlistTitle
    property string cover

    title: playlistTitle
    overline: "ПЛЕЙЛИСТ"
    heroImage: cover
    subtitle: model && !(model.loading && model.count === 0) ? model.count + " " + plural(model.count) : ""
    model: Library.playlistTracks(uid, kind)
    emptyIcon: "queue_music"
    emptyTitle: "В плейлисте нет треков"

    function plural(n) {
        const m10 = n % 10, m100 = n % 100
        if (m10 === 1 && m100 !== 11) return "трек"
        if (m10 >= 2 && m10 <= 4 && (m100 < 12 || m100 > 14)) return "трека"
        return "треков"
    }
}
