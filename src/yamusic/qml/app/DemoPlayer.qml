import QtQuick

// Заглушка плеера до этапа 4: тот же интерфейс, что будет у настоящего (Python) плеера.
QtObject {
    property bool hasTrack: false
    property bool playing: false
    property string title: ""
    property string artist: ""
    property string cover: ""
    property real position: 0
    property real volume: 0.7

    function togglePlay() { if (hasTrack) playing = !playing }
    function seek(value) { position = value }
}
