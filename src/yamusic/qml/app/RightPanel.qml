import QtQuick
import QtQuick.Layouts
import Md3
import YaMusic.Core

// Правая выдвижная панель: очередь / текст песни.
Surface {
    id: root

    level: "container"

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 16

        RowLayout {
            Layout.fillWidth: true
            ButtonGroup {
                model: [{ text: "Очередь", icon: "queue_music" }, { text: "Текст", icon: "title" }]
                autoSelect: false
                currentIndex: Settings.rightPanelTab === "lyrics" ? 1 : 0
                onActivated: i => Settings.rightPanelTab = i === 1 ? "lyrics" : "queue"
            }
            Item { Layout.fillWidth: true }
            IconButton { icon: "close"; onClicked: Settings.rightPanelOpen = false }
        }

        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            ListView {
                id: queueList
                anchors.fill: parent
                visible: Settings.rightPanelTab === "queue" && Player.queue.count > 0
                clip: true
                model: Player.queue
                boundsBehavior: Flickable.StopAtBounds
                acceptedButtons: Qt.NoButton
                reuseItems: true
                delegate: TrackRow {
                    required property int index
                    required property var model
                    width: ListView.view.width
                    wide: false
                    title: model.title
                    version: model.version
                    artists: model.artists
                    cover: model.cover
                    durationMs: model.durationMs
                    explicit: model.explicit
                    available: model.available
                    current: index === Player.currentIndex
                    trackId: model.trackId
                    liked: Library.likesRevision >= 0 && Library.isLiked(model.trackId)
                    onLikeClicked: Library.toggleLike(Player.queue.get(index))
                    onActivated: Player.playIndex(index)
                }
                Component.onCompleted: positionViewAtIndex(Math.max(0, Player.currentIndex - 1), ListView.Beginning)
            }
            SmoothScroll { flickable: queueList; wheelStep: Settings.wheelStep; visible: queueList.visible }

            EmptyState {
                anchors.centerIn: parent
                width: Math.min(parent.width, 280)
                visible: Settings.rightPanelTab === "queue" && Player.queue.count === 0
                icon: "queue_music"
                shape: "cookie6"
                title: "Очередь пуста"
                text: "Треки появятся здесь, когда начнётся воспроизведение"
            }
            LyricsView {
                anchors.fill: parent
                visible: Settings.rightPanelTab === "lyrics" && Settings.rightPanelOpen
                fontSize: 22
                anchorPos: 0.3
            }
        }
    }
}
